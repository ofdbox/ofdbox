#![allow(dead_code)]

use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, UNIX_EPOCH};

use ofdbox_crypto::{DEFAULT_DIST_ID, sign_sm3};
use sm2::SecretKey;
use sm2::dsa::DerSignature;
use sm2::dsa::signature::{Keypair, Signer};
use x509_cert::Certificate;
use x509_cert::builder::{Builder, CertificateBuilder, CrlBuilder, profile::BuilderProfile};
use x509_cert::crl::{CertificateList, RevokedCert};
use x509_cert::der::asn1::{BitString, ObjectIdentifier};
use x509_cert::der::{Decode, Encode};
use x509_cert::ext::ToExtension;
use x509_cert::ext::pkix::crl::CrlNumber;
use x509_cert::ext::pkix::{BasicConstraints, KeyUsage, KeyUsages};
use x509_cert::name::Name;
use x509_cert::serial_number::SerialNumber;
use x509_cert::spki::{
    AlgorithmIdentifierOwned, DynSignatureAlgorithmIdentifier, SubjectPublicKeyInfoOwned,
};
use x509_cert::time::{Time, Validity};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

pub const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfictional-seal-image";
pub const OID_SM2_SM3: &[u8] = &[42, 0x81, 0x1c, 0xcf, 0x55, 0x01, 0x83, 0x75];
pub const OID_RSA: &[u8] = &[42, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];

const SM2_CURVE_OID: &str = "1.2.156.10197.1.301";
const SM2_SIGN_OID: &str = "1.2.156.10197.1.501";

pub fn tlv(tag: u8, value: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let length = value.len();
    if length < 0x80 {
        out.push(length as u8);
    } else {
        let bytes = (length as u32).to_be_bytes();
        let start = bytes.iter().position(|&byte| byte != 0).unwrap();
        out.push(0x80 | (bytes.len() - start) as u8);
        out.extend_from_slice(&bytes[start..]);
    }
    out.extend_from_slice(value);
    out
}

pub fn seq(items: &[Vec<u8>]) -> Vec<u8> {
    tlv(0x30, &items.concat())
}

pub fn integer(value: i64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let mut start = 0;
    while start + 1 < bytes.len()
        && ((bytes[start] == 0 && bytes[start + 1] & 0x80 == 0)
            || (bytes[start] == 0xff && bytes[start + 1] & 0x80 != 0))
    {
        start += 1;
    }
    tlv(0x02, &bytes[start..])
}

pub fn ia5(value: &str) -> Vec<u8> {
    tlv(0x16, value.as_bytes())
}

pub fn utf8(value: &str) -> Vec<u8> {
    tlv(0x0c, value.as_bytes())
}

pub fn octets(value: &[u8]) -> Vec<u8> {
    tlv(0x04, value)
}

pub fn oid(value: &[u8]) -> Vec<u8> {
    tlv(0x06, value)
}

pub fn bit_string(value: &[u8]) -> Vec<u8> {
    let mut content = vec![0];
    content.extend_from_slice(value);
    tlv(0x03, &content)
}

pub fn utc(value: &str) -> Vec<u8> {
    tlv(0x17, value.as_bytes())
}

pub fn generalized(value: &str) -> Vec<u8> {
    tlv(0x18, value.as_bytes())
}

pub fn header(version: i64, vendor: &str) -> Vec<u8> {
    seq(&[ia5("ES"), integer(version), ia5(vendor)])
}

pub fn picture() -> Vec<u8> {
    seq(&[ia5("PNG"), octets(PNG), integer(40), integer(40)])
}

pub fn gm_property_with_certs(certificates: &[Vec<u8>]) -> Vec<u8> {
    seq(&[
        integer(1),
        utf8("测试公章"),
        seq(&certificates
            .iter()
            .map(|certificate| octets(certificate))
            .collect::<Vec<_>>()),
        utc("230117071431Z"),
        utc("230109071043Z"),
        utc("280109071043Z"),
    ])
}

pub fn gm_property() -> Vec<u8> {
    gm_property_with_certs(&[b"fictional-certificate".to_vec()])
}

pub fn gb_property_with_certs(certificates: &[Vec<u8>]) -> Vec<u8> {
    seq(&[
        integer(1),
        utf8("测试公章"),
        integer(1),
        seq(&certificates
            .iter()
            .map(|certificate| octets(certificate))
            .collect::<Vec<_>>()),
        generalized("20211229175736Z"),
        generalized("20211229180422Z"),
        generalized("20241231180430Z"),
    ])
}

pub fn gb_property_with_digests(digests: &[[u8; 32]]) -> Vec<u8> {
    let entries = digests
        .iter()
        .map(|digest| seq(&[tlv(0x13, b"SM3"), octets(digest)]))
        .collect::<Vec<_>>();
    seq(&[
        integer(1),
        utf8("测试公章"),
        integer(2),
        seq(&entries),
        generalized("20211229175736Z"),
        generalized("20211229180422Z"),
        generalized("20241231180430Z"),
    ])
}

pub fn gb_property() -> Vec<u8> {
    gb_property_with_certs(&[
        b"fictional-certificate".to_vec(),
        b"fictional-certificate-2".to_vec(),
    ])
}

pub fn seal_info(property: Vec<u8>, extensions: bool) -> Vec<u8> {
    let mut fields = vec![
        header(1, "OFDBOX"),
        ia5("seal-id-0001"),
        property,
        picture(),
    ];
    if extensions {
        fields.push(seq(&[seq(&[
            oid(&[42, 1]),
            tlv(0x01, &[0xff]),
            octets(b"extension"),
        ])]));
    }
    seq(&fields)
}

pub fn gm_seal() -> Vec<u8> {
    let signer = seq(&[
        octets(b"fictional-certificate"),
        oid(OID_SM2_SM3),
        bit_string(b"fictional-signature"),
    ]);
    seq(&[seal_info(gm_property(), false), signer])
}

pub fn gb_seal() -> Vec<u8> {
    seq(&[
        seal_info(gb_property(), false),
        octets(b"fictional-certificate"),
        oid(OID_SM2_SM3),
        bit_string(b"fictional-signature"),
    ])
}

pub fn zip_with_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .expect("start zip entry");
        writer.write_all(data).expect("write zip entry");
    }
    writer.finish().expect("finish zip").into_inner()
}

pub fn test_secret() -> SecretKey {
    SecretKey::from_slice(&[0x44u8; 32]).expect("secret")
}

pub const FIXED_NOT_BEFORE: u64 = 1_577_836_800;
pub const FIXED_NOT_AFTER: u64 = 2_051_222_400;
pub const ROOT_SERIAL: u32 = 1;
pub const LEAF_SERIAL: u32 = 2;

pub fn test_certificate(secret: &SecretKey) -> Vec<u8> {
    build_certificate(
        "CN=OFDBox Seal Test",
        "CN=OFDBox Seal Test",
        secret,
        secret,
        1,
        false,
        Validity::from_now(Duration::from_secs(3600)).expect("validity"),
    )
}

pub fn test_root_key() -> SecretKey {
    SecretKey::from_slice(&[0x11u8; 32]).expect("root key")
}

pub fn test_leaf_key() -> SecretKey {
    SecretKey::from_slice(&[0x22u8; 32]).expect("leaf key")
}

pub fn test_root_certificate() -> Vec<u8> {
    build_certificate(
        "CN=OFDBox Root",
        "CN=OFDBox Root",
        &test_root_key(),
        &test_root_key(),
        ROOT_SERIAL,
        true,
        fixed_validity(),
    )
}

pub fn test_leaf_certificate() -> Vec<u8> {
    build_certificate(
        "CN=OFDBox Seal",
        "CN=OFDBox Root",
        &test_leaf_key(),
        &test_root_key(),
        LEAF_SERIAL,
        false,
        fixed_validity(),
    )
}

pub fn fixed_validity() -> Validity {
    Validity::new(cert_time(FIXED_NOT_BEFORE), cert_time(FIXED_NOT_AFTER))
}

pub fn cert_time(unix_seconds: u64) -> Time {
    Time::try_from(UNIX_EPOCH + Duration::from_secs(unix_seconds)).expect("time")
}

pub fn test_crl(
    root_der: &[u8],
    root_key: &SecretKey,
    revoked_serials: &[u32],
    this_update: u64,
    next_update: Option<u64>,
) -> Vec<u8> {
    let root = Certificate::from_der(root_der).expect("root certificate");
    let mut builder = CrlBuilder::new_with_this_update(
        &root,
        CrlNumber::try_from(1u32).expect("crl number"),
        cert_time(this_update),
    )
    .expect("crl builder");
    builder = builder.with_next_update(next_update.map(cert_time));
    builder = builder.with_certificates(revoked_serials.iter().map(|serial| RevokedCert {
        serial_number: SerialNumber::from(*serial),
        revocation_date: cert_time(this_update),
        crl_entry_extensions: None,
    }));
    let signing = sm2::dsa::SigningKey::new(DEFAULT_DIST_ID, root_key).expect("signing key");
    let crl: CertificateList = builder
        .build::<_, DerSignature>(&Sm2Signer { key: signing })
        .expect("crl");
    crl.to_der().expect("crl der")
}

pub fn build_certificate(
    subject: &str,
    issuer: &str,
    subject_key: &SecretKey,
    issuer_key: &SecretKey,
    serial: u32,
    is_ca: bool,
    validity: Validity,
) -> Vec<u8> {
    let signing = sm2::dsa::SigningKey::new(DEFAULT_DIST_ID, issuer_key).expect("signing key");
    let public = subject_key.public_key().to_sec1_bytes();
    let spki = SubjectPublicKeyInfoOwned {
        algorithm: AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM2_CURVE_OID),
            parameters: None,
        },
        subject_public_key: BitString::from_bytes(&public).expect("public key bit string"),
    };
    let profile = TestProfile {
        subject: Name::from_str(subject).expect("subject"),
        issuer: Name::from_str(issuer).expect("issuer"),
        is_ca,
    };
    let builder = CertificateBuilder::new(profile, SerialNumber::from(serial), validity, spki)
        .expect("certificate builder");
    let certificate: Certificate = builder
        .build::<_, DerSignature>(&Sm2Signer { key: signing })
        .expect("certificate");
    certificate.to_der().expect("certificate der")
}

pub fn signed_gm_seal(certificate: &[u8], secret: &[u8], dist_id: &str) -> Vec<u8> {
    signed_gm_seal_with_certs(
        certificate,
        &[b"fictional-certificate".to_vec()],
        secret,
        dist_id,
    )
}

pub fn signed_gm_seal_with_certs(
    certificate: &[u8],
    signer_certificates: &[Vec<u8>],
    secret: &[u8],
    dist_id: &str,
) -> Vec<u8> {
    let info = seal_info(gm_property_with_certs(signer_certificates), false);
    let certificate_value = octets(certificate);
    let algorithm = oid(OID_SM2_SM3);
    let tbs = seq(&[info.clone(), certificate_value.clone(), algorithm.clone()]);
    let signature = sign_sm3(secret, &tbs, dist_id).expect("seal signature");
    let signer = seq(&[certificate_value, algorithm, bit_string(&signature)]);
    seq(&[info, signer])
}

pub fn signed_gb_seal(certificate: &[u8], secret: &[u8], dist_id: &str) -> Vec<u8> {
    signed_gb_seal_with_property(gb_property(), certificate, secret, dist_id)
}

pub fn signed_gb_seal_with_certs(
    certificate: &[u8],
    signer_certificates: &[Vec<u8>],
    secret: &[u8],
    dist_id: &str,
) -> Vec<u8> {
    signed_gb_seal_with_property(
        gb_property_with_certs(signer_certificates),
        certificate,
        secret,
        dist_id,
    )
}

pub fn signed_gb_seal_with_property(
    property: Vec<u8>,
    certificate: &[u8],
    secret: &[u8],
    dist_id: &str,
) -> Vec<u8> {
    let info = seal_info(property, false);
    let signature = sign_sm3(secret, &info, dist_id).expect("seal signature");
    seq(&[
        info,
        octets(certificate),
        oid(OID_SM2_SM3),
        bit_string(&signature),
    ])
}

pub fn signed_gb_seal_with_rsa_oid(certificate: &[u8]) -> Vec<u8> {
    seq(&[
        seal_info(gb_property(), false),
        octets(certificate),
        oid(OID_RSA),
        bit_string(b"fictional-signature"),
    ])
}

struct TestProfile {
    subject: Name,
    issuer: Name,
    is_ca: bool,
}

impl BuilderProfile for TestProfile {
    fn get_issuer(&self, _subject: &Name) -> Name {
        self.issuer.clone()
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
        let mut extensions = Vec::new();
        let constraints = BasicConstraints {
            ca: self.is_ca,
            path_len_constraint: None,
        };
        extensions.push(constraints.to_extension(&self.subject, &extensions)?);
        let usage: x509_cert::der::flagset::FlagSet<KeyUsages> = if self.is_ca {
            KeyUsages::KeyCertSign | KeyUsages::CRLSign
        } else {
            KeyUsages::DigitalSignature.into()
        };
        extensions.push(KeyUsage(usage).to_extension(&self.subject, &extensions)?);
        Ok(extensions)
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

pub fn collect_seals(dir: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            collect_seals(&path, paths);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("esl"))
        {
            paths.push(path);
        }
    }
}
