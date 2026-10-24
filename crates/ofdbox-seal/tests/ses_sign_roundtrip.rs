use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use der::Encode;
use der::asn1::{BitString, ObjectIdentifier};
use ofdbox_core::{DEFAULT_SIGNATURE_METHOD, OfdPackage, SignatureVerifiers, SigningRequest};
use ofdbox_crypto::PasswordBackend;
use ofdbox_seal::{SesSignatureVerifier, Sm2SesSignatureProducer, parse_ses_signature};
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
    <ofd:DocInfo><ofd:DocID>stamp-test</ofd:DocID></ofd:DocInfo>
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
            subject: Name::from_str("CN=OFDBox Stamp Test").expect("name"),
        },
        SerialNumber::from(9u32),
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

/// 从语料里取一份真实的 38540 印章数据。
fn real_seal() -> Option<Vec<u8>> {
    let _ = dotenvy::dotenv();
    let dir = std::env::var_os("OFDBOX_TESTDATA_DIR").map(PathBuf::from)?;
    let path = dir
        .join("ofd-files")
        .join("增值税电子发票样例")
        .join("invoice.ofd");
    let bytes = std::fs::read(path).ok()?;
    let package = OfdPackage::from_bytes(bytes).ok()?;
    let mut package = package;
    let index = package.parse_signatures().ok()?;
    let entry = index.signatures().next()?;
    let signature = package.parse_signature(entry.base_loc()?).ok()?;
    let value_path = signature.signed_value()?.to_owned();
    let value = package.read_entry(&value_path).ok()?;
    let ses = parse_ses_signature(&value).ok()?;
    Some(ses.seal.to_vec())
}

#[test]
fn stamps_with_real_seal_and_verifies() {
    let Some(seal) = real_seal() else {
        return;
    };
    let (certificate, secret) = make_certificate();
    let producer = Sm2SesSignatureProducer::new(seal, certificate, secret.to_bytes().to_vec())
        .expect("producer");

    let mut document = OfdPackage::from_bytes(package()).expect("open");
    let request = SigningRequest::new(DEFAULT_SIGNATURE_METHOD, "20261024050000");
    let mut output = Vec::new();
    let report = document
        .sign_package(&request, &producer, &mut Cursor::new(&mut output))
        .expect("stamp");
    assert_eq!(report.signature_id, "s001");

    let mut signed = OfdPackage::from_bytes(output).expect("open stamped");
    let index = signed.parse_signatures().expect("signatures");
    let entry = index.signatures().next().expect("entry");
    assert_eq!(entry.signature_type(), Some("Seal"));
    let signature = signed
        .parse_signature(entry.base_loc().expect("BaseLoc"))
        .expect("signature");

    // 测试证书不在真实印章的签章者列表中，跳过该检查；其余照常验证。
    let verifiers = SignatureVerifiers::new()
        .with(Box::new(SesSignatureVerifier::new().without_signer_check()));
    signed
        .verify_signature(&signature, &verifiers)
        .expect("verify ses signature");

    let checks = signed
        .verify_references(&signature, &PasswordBackend::new(""))
        .expect("references");
    assert!(!checks.is_empty());
    for check in checks {
        assert_eq!(check.expected, check.actual, "{}", check.file_ref);
    }
}
