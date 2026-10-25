use std::io::{Cursor, Write};
use std::str::FromStr;
use std::time::Duration;

use der::asn1::{BitString, ObjectIdentifier};
use der::{Decode, Encode};
use ofdbox_core::{
    DEFAULT_SIGNATURE_METHOD, EntriesWriteOptions, OfdError, OfdPackage, PackageEdits, ReadOptions,
    ReferenceGraphOptions, SignatureVerifiers, SignedValueProducer, SigningContext, SigningRequest,
};
use ofdbox_crypto::{PasswordBackend, Sm2SignedDataProducer, Sm2SignedDataVerifier};
use sm2::SecretKey;
use sm2::dsa::DerSignature;
use sm2::dsa::signature::{Keypair, Signer};
use x509_cert::Certificate;
use x509_cert::builder::{Builder, CertificateBuilder, profile::BuilderProfile};
use x509_cert::name::Name;
use x509_cert::serial_number::SerialNumber;
use x509_cert::spki::{
    AlgorithmIdentifierOwned, DynSignatureAlgorithmIdentifier, SubjectPublicKeyInfoOwned,
};
use x509_cert::time::Validity;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const SM2_CURVE_OID: &str = "1.2.156.10197.1.301";
const SM2_SIGN_OID: &str = "1.2.156.10197.1.501";
const DIST_ID: &str = "1234567812345678";

const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>sign-test</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>2</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;

const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Content><ofd:Layer ID="1"/></ofd:Content>
</ofd:Page>"#;

struct TestProfile {
    subject: Name,
}

impl BuilderProfile for TestProfile {
    fn get_issuer(&self, _subject: &Name) -> Name {
        self.subject.clone()
    }

    fn get_subject(&self) -> Name {
        self.subject.clone()
    }

    fn build_extensions(
        &self,
        _spk: x509_cert::spki::SubjectPublicKeyInfoRef<'_>,
        _issuer_spk: x509_cert::spki::SubjectPublicKeyInfoRef<'_>,
        _tbs: &x509_cert::certificate::TbsCertificate,
    ) -> x509_cert::builder::Result<Vec<x509_cert::ext::Extension>> {
        Ok(Vec::new())
    }
}

struct Sm2Signer {
    key: sm2::dsa::SigningKey,
}

impl Keypair for Sm2Signer {
    type VerifyingKey = sm2::dsa::VerifyingKey;

    fn verifying_key(&self) -> Self::VerifyingKey {
        self.key.verifying_key().clone()
    }
}

impl DynSignatureAlgorithmIdentifier for Sm2Signer {
    fn signature_algorithm_identifier(&self) -> x509_cert::spki::Result<AlgorithmIdentifierOwned> {
        Ok(AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM2_SIGN_OID),
            parameters: None,
        })
    }
}

impl Signer<DerSignature> for Sm2Signer {
    fn try_sign(&self, msg: &[u8]) -> sm2::dsa::signature::Result<DerSignature> {
        let signature: sm2::dsa::Signature = self.key.try_sign(msg)?;
        Ok(signature.to_der())
    }
}

fn make_certificate() -> (Vec<u8>, SecretKey) {
    let secret = SecretKey::from_slice(&[0x66u8; 32]).expect("secret");
    let signing = sm2::dsa::SigningKey::new(DIST_ID, &secret).expect("signing");
    let public = secret.public_key().to_sec1_bytes();
    let spki = SubjectPublicKeyInfoOwned {
        algorithm: AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM2_CURVE_OID),
            parameters: None,
        },
        subject_public_key: BitString::from_bytes(&public).expect("bit string"),
    };
    let builder = CertificateBuilder::new(
        TestProfile {
            subject: Name::from_str("CN=OFDBox Signing Test").expect("name"),
        },
        SerialNumber::from(7u32),
        Validity::from_now(Duration::from_secs(3600)).expect("validity"),
        spki,
    )
    .expect("builder");
    let certificate: Certificate = builder
        .build::<_, DerSignature>(&Sm2Signer { key: signing })
        .expect("certificate");
    (certificate.to_der().expect("der"), secret)
}

fn package() -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in [
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", CONTENT.as_bytes()),
    ] {
        writer
            .start_file(name, SimpleFileOptions::default())
            .expect("start");
        writer.write_all(data).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

fn sign(package: &[u8], certificate: &[u8], secret: &SecretKey) -> (Vec<u8>, String) {
    let mut document = OfdPackage::from_bytes(package.to_vec()).expect("open");
    let request = SigningRequest::new(DEFAULT_SIGNATURE_METHOD, "20261024040000");
    let producer = Sm2SignedDataProducer::new(certificate.to_vec(), secret.to_bytes().to_vec());
    let mut output = Vec::new();
    let report = document
        .sign_package(&request, &producer, &mut Cursor::new(&mut output))
        .expect("sign package");
    (output, report.signature_id)
}

#[test]
fn signs_and_verifies_package() {
    let (certificate, secret) = make_certificate();
    let (signed, id) = sign(&package(), &certificate, &secret);
    assert_eq!(id, "s001");

    let mut document = OfdPackage::from_bytes(signed).expect("open signed");
    let root = document.parse_root().expect("root");
    let body = root.doc_bodies().next().expect("doc body");
    assert_eq!(
        body.signatures(),
        Some("Doc_0/Signs/Signatures.xml"),
        "OFD.xml 应补上 Signatures 链接"
    );

    let index = document.parse_signatures().expect("signatures");
    let entry = index.signatures().next().expect("signature entry");
    assert_eq!(entry.id(), Some("s001"));
    assert_eq!(entry.signature_type(), Some("Sign"));

    let signature = document
        .parse_signature("Sign_0/Signature.xml")
        .expect("signature");
    let info = signature.signed_info().expect("signed info");
    assert_eq!(info.signature_method(), Some(SM2_SIGN_OID));
    let provider = info.provider().expect("provider");
    assert_eq!(provider.provider_name(), Some("ofdbox"));
    assert_eq!(
        signature.signed_value(),
        Some("/Doc_0/Signs/Sign_0/SignedValue.dat")
    );
    let references: Vec<_> = info
        .references()
        .expect("references")
        .references()
        .collect();
    assert_eq!(references.len(), 3);

    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    document
        .verify_signature(&signature, &verifiers)
        .expect("verify signed data");

    let checks = document
        .verify_references(&signature, &PasswordBackend::new(""))
        .expect("references");
    assert_eq!(checks.len(), 3);
    for check in checks {
        assert_eq!(check.expected, check.actual, "{}", check.file_ref);
    }

    // signedAttrs 应包含 contentType、signingTime、messageDigest。
    let value = document
        .read_entry("/Doc_0/Signs/Sign_0/SignedValue.dat")
        .expect("signed value");
    let info = cms::content_info::ContentInfo::from_der(&value).expect("content info");
    let signed_data: cms::signed_data::SignedData = info.content.decode_as().expect("signed data");
    let signer = signed_data.signer_infos.0.iter().next().expect("signer");
    let attributes = signer.signed_attrs.as_ref().expect("signed attrs");
    let oids: Vec<String> = attributes.iter().map(|attr| attr.oid.to_string()).collect();
    assert!(
        oids.contains(&"1.2.840.113549.1.9.5".to_owned()),
        "缺少 signingTime: {oids:?}"
    );
    assert!(oids.contains(&"1.2.840.113549.1.9.4".to_owned()));
    assert!(oids.contains(&"1.2.840.113549.1.9.3".to_owned()));
}

#[test]
fn appending_second_signature_keeps_first_valid() {
    let (certificate, secret) = make_certificate();
    let (first, first_id) = sign(&package(), &certificate, &secret);
    let (second, second_id) = sign(&first, &certificate, &secret);
    assert_eq!(first_id, "s001");
    assert_eq!(second_id, "s002");

    let mut document = OfdPackage::from_bytes(second).expect("open");
    let index = document.parse_signatures().expect("signatures");
    let entries: Vec<_> = index.signatures().collect();
    assert_eq!(entries.len(), 2);
    assert_eq!(index.max_sign_id(), Some("s002"));

    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    for loc in ["Sign_0/Signature.xml", "Sign_1/Signature.xml"] {
        let signature = document.parse_signature(loc).expect("signature");
        document
            .verify_signature(&signature, &verifiers)
            .unwrap_or_else(|error| panic!("{loc}: {error}"));
        let checks = document
            .verify_references(&signature, &PasswordBackend::new(""))
            .expect("references");
        for check in checks {
            assert_eq!(check.expected, check.actual, "{loc}: {}", check.file_ref);
        }
    }
}

#[test]
fn strict_reparse_accepts_generated_structure() {
    let (certificate, secret) = make_certificate();
    let (signed, _) = sign(&package(), &certificate, &secret);
    let mut strict = OfdPackage::from_bytes_with_options(
        signed,
        ReadOptions::STRICT,
        ofdbox_core::CacheOptions::default(),
    )
    .expect("strict open");
    strict.parse_root().expect("strict root");
    strict.parse_document().expect("strict document");
    strict.parse_signatures().expect("strict signatures");
    strict
        .parse_signature("Sign_0/Signature.xml")
        .expect("strict signature");
}

const ENTRIES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:DocEntries xmlns:ofd="http://www.ofdspec.org/2016" ID="1" CreatorName="OFDBox" Version="1.0">
  <ofd:FileList>
    <ofd:File ID="f1" FileLoc="/Doc_0/extra.txt"/>
  </ofd:FileList>
  <ofd:SignedValueLoc>/Integrity/SignedValue.dat</ofd:SignedValueLoc>
</ofd:DocEntries>"#;

#[test]
fn verifies_ofd_entries_signature() {
    let (certificate, secret) = make_certificate();
    let producer = Sm2SignedDataProducer::new(certificate, secret.to_bytes().to_vec());
    let context = SigningContext {
        content: ENTRIES.as_bytes(),
        signed_entry: "/OFDEntries.xml",
        signature_method: DEFAULT_SIGNATURE_METHOD,
        signed_at: "20261024050000",
        references: &[],
    };
    let value = producer.produce(&context).expect("signed value");

    let mut base = OfdPackage::from_bytes(package()).expect("open");
    let mut edits = PackageEdits::new();
    edits.add("Doc_0/extra.txt", b"extra".to_vec());
    edits.add("OFDEntries.xml", ENTRIES.as_bytes().to_vec());
    edits.add("Integrity/SignedValue.dat", value.clone());
    let mut output = Vec::new();
    base.write_with(&mut Cursor::new(&mut output), &edits, &Default::default())
        .expect("write");
    let mut opened = OfdPackage::from_bytes(output).expect("open entries");
    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    let report = opened
        .verify_ofd_entries(&verifiers)
        .expect("verify entries");
    assert_eq!(report.files, 1);
    assert_eq!(
        report.signed_value.as_deref(),
        Some("/Integrity/SignedValue.dat")
    );

    // 篡改 OFDEntries.xml 后签名应失败。
    let tampered = ENTRIES.replace("CreatorName=\"OFDBox\"", "CreatorName=\"Other\"");
    let mut edits = PackageEdits::new();
    edits.add("Doc_0/extra.txt", b"extra".to_vec());
    edits.add("OFDEntries.xml", tampered.into_bytes());
    edits.add("Integrity/SignedValue.dat", value);
    let mut output = Vec::new();
    OfdPackage::from_bytes(package())
        .expect("open")
        .write_with(&mut Cursor::new(&mut output), &edits, &Default::default())
        .expect("write tampered");
    let mut tampered = OfdPackage::from_bytes(output).expect("open tampered");
    assert!(tampered.verify_ofd_entries(&verifiers).is_err());
}

#[test]
fn rejects_missing_listed_file() {
    let (certificate, secret) = make_certificate();
    let producer = Sm2SignedDataProducer::new(certificate, secret.to_bytes().to_vec());
    let entries = ENTRIES.replace("/Doc_0/extra.txt", "/Doc_0/missing.txt");
    let context = SigningContext {
        content: entries.as_bytes(),
        signed_entry: "/OFDEntries.xml",
        signature_method: DEFAULT_SIGNATURE_METHOD,
        signed_at: "20261024050000",
        references: &[],
    };
    let value = producer.produce(&context).expect("signed value");

    let mut edits = PackageEdits::new();
    edits.add("OFDEntries.xml", entries.into_bytes());
    edits.add("Integrity/SignedValue.dat", value);
    let mut output = Vec::new();
    OfdPackage::from_bytes(package())
        .expect("open")
        .write_with(&mut Cursor::new(&mut output), &edits, &Default::default())
        .expect("write");
    let mut opened = OfdPackage::from_bytes(output).expect("open missing");
    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    assert!(opened.verify_ofd_entries(&verifiers).is_err());
}

#[test]
fn reports_unlisted_trailing_file() {
    let (certificate, secret) = make_certificate();
    let producer = Sm2SignedDataProducer::new(certificate, secret.to_bytes().to_vec());
    let context = SigningContext {
        content: ENTRIES.as_bytes(),
        signed_entry: "/OFDEntries.xml",
        signature_method: DEFAULT_SIGNATURE_METHOD,
        signed_at: "20261024050000",
        references: &[],
    };
    let value = producer.produce(&context).expect("signed value");

    let mut edits = PackageEdits::new();
    edits.add("Doc_0/extra.txt", b"extra".to_vec());
    edits.add("Doc_0/trick.txt", b"trick".to_vec());
    edits.add("OFDEntries.xml", ENTRIES.as_bytes().to_vec());
    edits.add("Integrity/SignedValue.dat", value);
    let mut output = Vec::new();
    OfdPackage::from_bytes(package())
        .expect("open")
        .write_with(&mut Cursor::new(&mut output), &edits, &Default::default())
        .expect("write");
    let mut opened = OfdPackage::from_bytes(output).expect("open entries");
    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    let report = opened
        .verify_ofd_entries(&verifiers)
        .expect("verify entries");
    assert_eq!(report.files, 1);
    assert_eq!(report.unlisted, vec!["/Doc_0/trick.txt".to_owned()]);
    assert!(!report.is_consistent());
}

#[test]
fn reports_listed_but_referenced_file() {
    let (certificate, secret) = make_certificate();
    let producer = Sm2SignedDataProducer::new(certificate, secret.to_bytes().to_vec());
    let entries = ENTRIES.replace("/Doc_0/extra.txt", "/OFD.xml");
    let context = SigningContext {
        content: entries.as_bytes(),
        signed_entry: "/OFDEntries.xml",
        signature_method: DEFAULT_SIGNATURE_METHOD,
        signed_at: "20261024050000",
        references: &[],
    };
    let value = producer.produce(&context).expect("signed value");

    let mut edits = PackageEdits::new();
    edits.add("OFDEntries.xml", entries.into_bytes());
    edits.add("Integrity/SignedValue.dat", value);
    let mut output = Vec::new();
    OfdPackage::from_bytes(package())
        .expect("open")
        .write_with(&mut Cursor::new(&mut output), &edits, &Default::default())
        .expect("write");
    let mut opened = OfdPackage::from_bytes(output).expect("open entries");
    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    let report = opened
        .verify_ofd_entries(&verifiers)
        .expect("verify entries");
    assert_eq!(report.listed_but_referenced, vec!["/OFD.xml".to_owned()]);
    assert!(!report.is_consistent());
}

#[test]
fn writes_ofd_entries_and_verifies() {
    let (certificate, secret) = make_certificate();
    let producer = Sm2SignedDataProducer::new(certificate, secret.to_bytes().to_vec());

    let mut base = OfdPackage::from_bytes(package()).expect("open");
    let mut edits = PackageEdits::new();
    edits.add("Doc_0/trick.txt", b"trick".to_vec());
    let mut seeded = Vec::new();
    base.write_with(&mut Cursor::new(&mut seeded), &edits, &Default::default())
        .expect("seed package");

    let mut package = OfdPackage::from_bytes(seeded).expect("open seeded");
    let options = EntriesWriteOptions {
        id: "entries".to_owned(),
        signed_at: "20261024050000".to_owned(),
        ..Default::default()
    };
    let mut output = Vec::new();
    let report = package
        .write_ofd_entries(&mut Cursor::new(&mut output), &producer, &options)
        .expect("write entries");
    assert_eq!(report.files, 1);
    assert_eq!(report.entries_entry, "/OFDEntries.xml");

    let mut written = OfdPackage::from_bytes(output).expect("open written");
    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    let check = written
        .verify_ofd_entries(&verifiers)
        .expect("verify entries");
    assert_eq!(check.files, 1);
    assert!(check.is_consistent());
    assert_eq!(
        check.signed_value.as_deref(),
        Some("/Integrity/SignedValue.dat")
    );
}

#[test]
fn follows_custom_tag_schema_loc() {
    let (certificate, secret) = make_certificate();
    let producer = Sm2SignedDataProducer::new(certificate, secret.to_bytes().to_vec());
    let context = SigningContext {
        content: ENTRIES.as_bytes(),
        signed_entry: "/OFDEntries.xml",
        signature_method: DEFAULT_SIGNATURE_METHOD,
        signed_at: "20261024050000",
        references: &[],
    };
    let value = producer.produce(&context).expect("signed value");

    let document = DOCUMENT.replace(
        "</ofd:Document>",
        "<ofd:CustomTags>CustomTags.xml</ofd:CustomTags></ofd:Document>",
    );
    let custom_tags = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:CustomTags xmlns:ofd="http://www.ofdspec.org/2016"><ofd:CustomTag ID="1"><ofd:SchemaLoc>schema.xsd</ofd:SchemaLoc></ofd:CustomTag></ofd:CustomTags>"#;
    let mut edits = PackageEdits::new();
    edits.replace("Doc_0/Document.xml", document.into_bytes());
    edits.add("Doc_0/CustomTags.xml", custom_tags.as_bytes().to_vec());
    edits.add("Doc_0/schema.xsd", b"schema".to_vec());
    edits.add("Doc_0/extra.txt", b"extra".to_vec());
    edits.add("OFDEntries.xml", ENTRIES.as_bytes().to_vec());
    edits.add("Integrity/SignedValue.dat", value);
    let mut output = Vec::new();
    OfdPackage::from_bytes(package())
        .expect("open")
        .write_with(&mut Cursor::new(&mut output), &edits, &Default::default())
        .expect("write");
    let mut opened = OfdPackage::from_bytes(output).expect("open entries");
    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    let report = opened
        .verify_ofd_entries(&verifiers)
        .expect("verify entries");
    assert!(report.unlisted.is_empty(), "{:?}", report.unlisted);
    assert!(report.is_consistent());
}

#[test]
fn reference_graph_matches_case_insensitively() {
    let (certificate, secret) = make_certificate();
    let producer = Sm2SignedDataProducer::new(certificate, secret.to_bytes().to_vec());
    let entries = ENTRIES.replace(
        r#"<ofd:File ID="f1" FileLoc="/Doc_0/extra.txt"/>"#,
        r#"<ofd:File ID="f1" FileLoc="/Doc_0/Document.xml"/>"#,
    );
    let context = SigningContext {
        content: entries.as_bytes(),
        signed_entry: "/OFDEntries.xml",
        signature_method: DEFAULT_SIGNATURE_METHOD,
        signed_at: "20261024050000",
        references: &[],
    };
    let value = producer.produce(&context).expect("signed value");

    let root = OFD_XML.replace("Doc_0/Document.xml", "doc_0/document.xml");
    let mut edits = PackageEdits::new();
    edits.replace("OFD.xml", root.into_bytes());
    edits.add("OFDEntries.xml", entries.into_bytes());
    edits.add("Integrity/SignedValue.dat", value);
    let mut output = Vec::new();
    OfdPackage::from_bytes(package())
        .expect("open")
        .write_with(&mut Cursor::new(&mut output), &edits, &Default::default())
        .expect("write");
    let mut opened = OfdPackage::from_bytes(output).expect("open entries");
    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    let report = opened
        .verify_ofd_entries(&verifiers)
        .expect("verify entries");
    assert!(report.unlisted.is_empty(), "{:?}", report.unlisted);
    assert!(
        report
            .listed_but_referenced
            .contains(&"/Doc_0/Document.xml".to_owned())
    );
}

#[test]
fn verify_rejects_duplicate_listed_files() {
    let (certificate, secret) = make_certificate();
    let producer = Sm2SignedDataProducer::new(certificate, secret.to_bytes().to_vec());
    let entries = ENTRIES.replace(
        r#"<ofd:File ID="f1" FileLoc="/Doc_0/extra.txt"/>"#,
        r#"<ofd:File ID="f1" FileLoc="/OFD.xml"/><ofd:File ID="f2" FileLoc="/OFD.xml"/>"#,
    );
    let context = SigningContext {
        content: entries.as_bytes(),
        signed_entry: "/OFDEntries.xml",
        signature_method: DEFAULT_SIGNATURE_METHOD,
        signed_at: "20261024050000",
        references: &[],
    };
    let value = producer.produce(&context).expect("signed value");

    let mut edits = PackageEdits::new();
    edits.add("OFDEntries.xml", entries.into_bytes());
    edits.add("Integrity/SignedValue.dat", value);
    let mut output = Vec::new();
    OfdPackage::from_bytes(package())
        .expect("open")
        .write_with(&mut Cursor::new(&mut output), &edits, &Default::default())
        .expect("write");
    let mut opened = OfdPackage::from_bytes(output).expect("open entries");
    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    assert!(matches!(
        opened.verify_ofd_entries(&verifiers),
        Err(OfdError::InvalidStructure(_))
    ));
}

#[test]
fn verify_without_signed_value_loc_skips_signature() {
    let entries = ENTRIES.replace(
        "<ofd:SignedValueLoc>/Integrity/SignedValue.dat</ofd:SignedValueLoc>",
        "",
    );
    let mut edits = PackageEdits::new();
    edits.add("Doc_0/extra.txt", b"extra".to_vec());
    edits.add("OFDEntries.xml", entries.into_bytes());
    let mut output = Vec::new();
    OfdPackage::from_bytes(package())
        .expect("open")
        .write_with(&mut Cursor::new(&mut output), &edits, &Default::default())
        .expect("write");
    let mut opened = OfdPackage::from_bytes(output).expect("open entries");
    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    let report = opened
        .verify_ofd_entries(&verifiers)
        .expect("verify entries");
    assert_eq!(report.signed_value, None);
}

#[test]
fn reference_graph_extension_recognizes_vendor_paths() {
    let (certificate, secret) = make_certificate();
    let producer = Sm2SignedDataProducer::new(certificate, secret.to_bytes().to_vec());
    let context = SigningContext {
        content: ENTRIES.as_bytes(),
        signed_entry: "/OFDEntries.xml",
        signature_method: DEFAULT_SIGNATURE_METHOD,
        signed_at: "20261024050000",
        references: &[],
    };
    let value = producer.produce(&context).expect("signed value");

    let document = DOCUMENT.replace(
        "</ofd:Document>",
        "<ofd:VendorPath>vendor.txt</ofd:VendorPath></ofd:Document>",
    );
    let mut edits = PackageEdits::new();
    edits.replace("Doc_0/Document.xml", document.into_bytes());
    edits.add("Doc_0/vendor.txt", b"vendor".to_vec());
    edits.add("Doc_0/extra.txt", b"extra".to_vec());
    edits.add("OFDEntries.xml", ENTRIES.as_bytes().to_vec());
    edits.add("Integrity/SignedValue.dat", value);
    let mut output = Vec::new();
    OfdPackage::from_bytes(package())
        .expect("open")
        .write_with(&mut Cursor::new(&mut output), &edits, &Default::default())
        .expect("write");
    let mut opened = OfdPackage::from_bytes(output).expect("open entries");
    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));

    let default = opened
        .verify_ofd_entries(&verifiers)
        .expect("default verify");
    assert!(
        default.unlisted.contains(&"/Doc_0/vendor.txt".to_owned()),
        "{:?}",
        default.unlisted
    );

    let options = ReferenceGraphOptions {
        extra_text_refs: vec!["VendorPath".to_owned()],
        ..Default::default()
    };
    let extended = opened
        .verify_ofd_entries_with(&verifiers, &options)
        .expect("extended verify");
    assert!(extended.unlisted.is_empty(), "{:?}", extended.unlisted);
}
