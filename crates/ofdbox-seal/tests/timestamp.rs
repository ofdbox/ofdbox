mod common;

use std::str::FromStr;

use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::{CmsVersion, ContentInfo};
use cms::signed_data::{
    CertificateSet, EncapsulatedContentInfo, SignedData, SignerIdentifier, SignerInfo, SignerInfos,
};
use der::Tag;
use der::asn1::{Any, AnyRef, ObjectIdentifier, OctetString, SetOfVec};
use der::{Decode, Encode};
use ofdbox_core::{DEFAULT_SIGNATURE_METHOD, SignedValueProducer, SigningContext};
use ofdbox_seal::{
    SealBuilder, SealPictureInput, SealSpec, SealStandard, SesVerifyOptions,
    Sm2SesSignatureProducer, TimeStampRequest, TimeStampVerifyOptions, attach_time_stamp,
    parse_ses_signature, parse_time_stamp_request, parse_time_stamp_response,
    parse_time_stamp_token, verify_ses_signature, verify_time_stamp_token,
};
use sm2::dsa::DerSignature;
use sm2::dsa::signature::{Keypair, Signer};
use sm3::{Digest, Sm3};
use x509_cert::Certificate;
use x509_cert::attr::{Attribute, Attributes};
use x509_cert::builder::{Builder, CertificateBuilder, profile::BuilderProfile};
use x509_cert::name::Name;
use x509_cert::serial_number::SerialNumber;
use x509_cert::spki::{
    AlgorithmIdentifierOwned, DynSignatureAlgorithmIdentifier, SubjectPublicKeyInfoOwned,
};
use x509_cert::time::Validity;

const SM2_CURVE_OID: &str = "1.2.156.10197.1.301";
const SM2_SIGN_OID: &str = "1.2.156.10197.1.501";
const SM3_OID: &str = "1.2.156.10197.1.401";
const ID_SIGNED_DATA: &str = "1.2.840.113549.1.7.2";
const ID_CT_TST_INFO: &str = "1.2.840.113549.1.9.16.1.4";
const ID_MESSAGE_DIGEST: &str = "1.2.840.113549.1.9.4";
const DIST_ID: &str = "1234567812345678";
const POLICY_OID: &str = "1.2.3.4";

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

fn tsa_certificate() -> (Vec<u8>, sm2::SecretKey) {
    let secret = sm2::SecretKey::from_slice(&[0x55u8; 32]).expect("secret");
    let signing = sm2::dsa::SigningKey::new(DIST_ID, &secret).expect("signing");
    let public = secret.public_key().to_sec1_bytes();
    let spki = SubjectPublicKeyInfoOwned {
        algorithm: AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM2_CURVE_OID),
            parameters: None,
        },
        subject_public_key: der::asn1::BitString::from_bytes(&public).expect("bit string"),
    };
    let builder = CertificateBuilder::new(
        TestProfile {
            subject: Name::from_str("CN=OFDBox Test TSA").expect("name"),
        },
        SerialNumber::from(9u32),
        Validity::from_now(std::time::Duration::from_secs(3600)).expect("validity"),
        spki,
    )
    .expect("builder");
    let certificate: Certificate = builder
        .build::<_, DerSignature>(&Sm2Signer { key: signing })
        .expect("certificate");
    (certificate.to_der().expect("der"), secret)
}

fn tlv(tag: Tag, value: &[u8]) -> Vec<u8> {
    Any::new(tag, value).expect("any").to_der().expect("der")
}

fn sequence(parts: &[Vec<u8>]) -> Vec<u8> {
    let mut content = Vec::new();
    for part in parts {
        content.extend_from_slice(part);
    }
    tlv(Tag::Sequence, &content)
}

fn oid(value: &str) -> Vec<u8> {
    ObjectIdentifier::new_unwrap(value)
        .to_der()
        .expect("oid der")
}

fn tst_info(imprint: &[u8], hash_oid: &str) -> Vec<u8> {
    let digest_algorithm = AlgorithmIdentifierOwned {
        oid: ObjectIdentifier::new_unwrap(hash_oid),
        parameters: None,
    }
    .to_der()
    .expect("digest algorithm");
    let message_imprint = sequence(&[digest_algorithm, tlv(Tag::OctetString, imprint)]);
    sequence(&[
        tlv(Tag::Integer, &[1]),
        oid(POLICY_OID),
        message_imprint,
        tlv(Tag::Integer, &[7]),
        tlv(Tag::GeneralizedTime, b"20261024050000Z"),
    ])
}

fn build_token(imprint: &[u8], hash_oid: &str, with_econtent: bool) -> (Vec<u8>, Vec<u8>) {
    let (certificate_der, secret) = tsa_certificate();
    let certificate = Certificate::from_der(&certificate_der).expect("certificate");
    let info = tst_info(imprint, hash_oid);

    let digest = Sm3::digest(&info);
    let attribute = Attribute {
        oid: ObjectIdentifier::new_unwrap(ID_MESSAGE_DIGEST),
        values: SetOfVec::try_from(vec![
            Any::new(Tag::OctetString, digest.as_slice()).expect("digest any"),
        ])
        .expect("attribute values"),
    };
    let attributes: Attributes = SetOfVec::try_from(vec![attribute]).expect("attributes");
    let signed_bytes = attributes.to_der().expect("attributes der");
    let signing = sm2::dsa::SigningKey::new(DIST_ID, &secret).expect("signing");
    let signature: sm2::dsa::Signature = signing.try_sign(&signed_bytes).expect("signature");

    let signer = SignerInfo {
        version: CmsVersion::V1,
        sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: certificate.tbs_certificate().issuer().clone(),
            serial_number: certificate.tbs_certificate().serial_number().clone(),
        }),
        digest_alg: AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM3_OID),
            parameters: None,
        },
        signed_attrs: Some(attributes),
        signature_algorithm: AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM2_SIGN_OID),
            parameters: None,
        },
        signature: OctetString::new(signature.to_der().to_vec()).expect("signature octets"),
        unsigned_attrs: None,
    };
    let signed_data = SignedData {
        version: CmsVersion::V1,
        digest_algorithms: SetOfVec::try_from(vec![AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM3_OID),
            parameters: None,
        }])
        .expect("digest algorithms"),
        encap_content_info: EncapsulatedContentInfo {
            econtent_type: ObjectIdentifier::new_unwrap(ID_CT_TST_INFO),
            econtent: with_econtent
                .then(|| Any::new(Tag::OctetString, info.as_slice()).expect("econtent")),
        },
        certificates: Some(
            CertificateSet::try_from(vec![CertificateChoices::Certificate(certificate)])
                .expect("certificates"),
        ),
        crls: None,
        signer_infos: SignerInfos::try_from(vec![signer]).expect("signer infos"),
    };
    let signed_data_der = signed_data.to_der().expect("signed data");
    let content_info = ContentInfo {
        content_type: ObjectIdentifier::new_unwrap(ID_SIGNED_DATA),
        content: Any::from(AnyRef::try_from(signed_data_der.as_slice()).expect("any ref")),
    };
    (
        content_info.to_der().expect("content info"),
        certificate_der,
    )
}

fn options() -> TimeStampVerifyOptions<'static> {
    TimeStampVerifyOptions {
        require_time_stamping_eku: false,
        ..Default::default()
    }
}

#[test]
fn parses_and_verifies_token() {
    let signed_value = b"signature-value";
    let imprint = Sm3::digest(signed_value);
    let (token, _) = build_token(imprint.as_slice(), SM3_OID, true);

    let parsed = parse_time_stamp_token(&token).expect("parse");
    assert_eq!(parsed.policy, POLICY_OID);
    assert_eq!(parsed.serial_number, 7);
    assert_eq!(parsed.gen_time, "20261024050000Z");
    assert_eq!(parsed.hash_algorithm, SM3_OID);

    let verification = verify_time_stamp_token(&token, signed_value, &options()).expect("verify");
    assert_eq!(verification.gen_time, "20261024050000Z");
    assert!(!verification.certificate_validated);
}

#[test]
fn validates_certificate_with_trust_anchor() {
    let signed_value = b"signature-value";
    let imprint = Sm3::digest(signed_value);
    let (token, certificate) = build_token(imprint.as_slice(), SM3_OID, true);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_secs();
    let mut policy = ofdbox_seal::CertificatePolicy::new()
        .with_trust_anchor(certificate)
        .at_time(now);
    // 测试证书没有 KeyUsage 扩展，关闭该检查以专注信任链验证。
    policy.require_digital_signature = false;

    let verification = verify_time_stamp_token(
        &token,
        signed_value,
        &TimeStampVerifyOptions {
            certificate: Some(&policy),
            ..options()
        },
    )
    .expect("verify");
    assert!(verification.certificate_validated);
}

#[test]
fn rejects_wrong_imprint() {
    let imprint = Sm3::digest(b"other-value");
    let (token, _) = build_token(imprint.as_slice(), SM3_OID, true);
    assert!(verify_time_stamp_token(&token, b"signature-value", &options()).is_err());
}

#[test]
fn rejects_unsupported_hash_algorithm() {
    let signed_value = b"signature-value";
    let imprint = Sm3::digest(signed_value);
    let (token, _) = build_token(imprint.as_slice(), "2.16.840.1.101.3.4.2.1", true);
    assert!(matches!(
        verify_time_stamp_token(&token, signed_value, &options()),
        Err(ofdbox_seal::SealError::UnsupportedAlgorithm(_))
    ));
}

#[test]
fn rejects_missing_econtent() {
    let imprint = Sm3::digest(b"signature-value");
    let (token, _) = build_token(imprint.as_slice(), SM3_OID, false);
    assert!(parse_time_stamp_token(&token).is_err());
}

#[test]
fn rejects_tampered_signature() {
    let signed_value = b"signature-value";
    let imprint = Sm3::digest(signed_value);
    let (mut token, _) = build_token(imprint.as_slice(), SM3_OID, true);
    let last = token.len() - 1;
    token[last] ^= 1;
    assert!(verify_time_stamp_token(&token, signed_value, &options()).is_err());
}

#[test]
fn requires_time_stamping_eku_by_default() {
    let signed_value = b"signature-value";
    let imprint = Sm3::digest(signed_value);
    let (token, _) = build_token(imprint.as_slice(), SM3_OID, true);
    assert!(matches!(
        verify_time_stamp_token(&token, signed_value, &TimeStampVerifyOptions::default()),
        Err(ofdbox_seal::SealError::InvalidCertificate)
    ));
}

const CONTENT: &[u8] = b"<ofd:Signature>content</ofd:Signature>";

fn make_seal(certificate: &[u8], secret: &sm2::SecretKey) -> Vec<u8> {
    let spec = SealSpec::new(
        SealStandard::GbT38540,
        "SEAL-TIMESTAMP",
        "时间戳测试印章",
        SealPictureInput {
            media_type: "PNG".to_owned(),
            data: common::PNG.to_vec(),
            width: 40,
            height: 40,
        },
    )
    .with_validity("20260101000000", "20260101000000", "20360101000000")
    .with_certificates(vec![certificate.to_vec()]);
    SealBuilder::new(spec, certificate.to_vec(), secret.to_bytes().to_vec())
        .build()
        .expect("seal")
}

#[test]
fn ses_verification_checks_attached_timestamp() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let seal = make_seal(&certificate, &secret);
    let producer =
        Sm2SesSignatureProducer::new(seal, certificate.clone(), secret.to_bytes().to_vec())
            .expect("producer");
    let ses = producer
        .produce(&SigningContext {
            content: CONTENT,
            signed_entry: "/Doc_0/Signs/Sign_0/Signature.xml",
            signature_method: DEFAULT_SIGNATURE_METHOD,
            signed_at: "20260101000000",
            references: &[],
        })
        .expect("ses");

    let parsed = parse_ses_signature(&ses).expect("parse");
    let imprint = Sm3::digest(parsed.signature);
    let (token, _) = build_token(imprint.as_slice(), SM3_OID, true);
    let with_timestamp = attach_time_stamp(&ses, &token).expect("attach");

    let verification = verify_ses_signature(
        &with_timestamp,
        CONTENT,
        &SesVerifyOptions {
            check_timestamp: true,
            ..Default::default()
        },
    )
    .expect("verify");
    let timestamp = verification.timestamp.expect("timestamp");
    assert_eq!(timestamp.gen_time, "20261024050000Z");
    assert!(!timestamp.certificate_validated);
}

#[test]
fn time_stamp_request_roundtrip() {
    let imprint = Sm3::digest(b"signed-value");
    let request = TimeStampRequest::new(SM3_OID, imprint.to_vec())
        .with_nonce(42)
        .with_policy(POLICY_OID)
        .with_cert_req(true);
    let der = request.to_der().expect("encode request");
    let parsed = parse_time_stamp_request(&der).expect("parse request");
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.hash_algorithm, SM3_OID);
    assert_eq!(parsed.hashed_message, imprint.to_vec());
    assert_eq!(parsed.nonce, Some(42));
    assert_eq!(parsed.policy.as_deref(), Some(POLICY_OID));
    assert!(parsed.cert_req);
}

#[test]
fn time_stamp_response_parses_token() {
    let imprint = Sm3::digest(b"signature-value");
    let (token, _) = build_token(imprint.as_slice(), SM3_OID, true);
    let status = sequence(&[tlv(Tag::Integer, &[0])]);
    let response = sequence(&[status, token.clone()]);
    let parsed = parse_time_stamp_response(&response).expect("parse response");
    assert_eq!(parsed.status, 0);
    assert_eq!(parsed.token.as_deref(), Some(token.as_slice()));
}

#[test]
fn rejects_timestamp_nonce_mismatch() {
    let signed_value = b"signature-value";
    let imprint = Sm3::digest(signed_value);
    let (token, _) = build_token(imprint.as_slice(), SM3_OID, true);
    let mismatch = TimeStampVerifyOptions {
        nonce: Some(7),
        ..options()
    };
    assert!(verify_time_stamp_token(&token, signed_value, &mismatch).is_err());
}

#[test]
fn rejects_timestamp_policy_mismatch() {
    let signed_value = b"signature-value";
    let imprint = Sm3::digest(signed_value);
    let (token, _) = build_token(imprint.as_slice(), SM3_OID, true);
    let mismatch = TimeStampVerifyOptions {
        policy: Some("1.2.3.5"),
        ..options()
    };
    assert!(verify_time_stamp_token(&token, signed_value, &mismatch).is_err());
}
