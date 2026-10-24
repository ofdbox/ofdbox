mod common;

use ofdbox_core::{
    DEFAULT_SIGNATURE_METHOD, SignatureVerifier, SignedValueFormat, SignedValueProducer,
    SigningContext, detect_signed_value_format,
};
use ofdbox_seal::{
    GmT0031SignatureVerifier, GmT0031VerifyOptions, SealBuilder, SealPictureInput, SealSpec,
    SealStandard, Sm2GmT0031SignatureProducer, parse_gmt0031_signature, verify_gmt0031_signature,
};

const CONTENT: &[u8] = b"<ofd:Signature>0031 content</ofd:Signature>";

fn make_seal(certificate: &[u8], secret: &sm2::SecretKey) -> Vec<u8> {
    let spec = SealSpec::new(
        SealStandard::GmT0031,
        "SEAL-0031",
        "0031 测试印章",
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

fn make_signature() -> (Vec<u8>, Vec<u8>) {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let seal = make_seal(&certificate, &secret);
    let producer =
        Sm2GmT0031SignatureProducer::new(seal, certificate.clone(), secret.to_bytes().to_vec())
            .expect("producer");
    let value = producer
        .produce(&SigningContext {
            content: CONTENT,
            signed_entry: "/Doc_0/Signs/Sign_0/Signature.xml",
            signature_method: DEFAULT_SIGNATURE_METHOD,
            signed_at: "20261024050000",
            references: &[],
        })
        .expect("produce");
    (value, certificate)
}

#[test]
fn produces_and_verifies_0031_signature() {
    let (value, _) = make_signature();
    assert_eq!(
        detect_signed_value_format(&value),
        SignedValueFormat::GmT0031Signature
    );

    let parsed = parse_gmt0031_signature(&value).expect("parse");
    assert_eq!(parsed.property_info, "/Doc_0/Signs/Sign_0/Signature.xml");
    assert_eq!(parsed.time_info, "2026-10-24T05:00:00.000Z");
    assert_eq!(parsed.algorithm, "1.2.156.10197.1.501");
    assert!(!parsed.signature.is_empty());

    let verification = verify_gmt0031_signature(&value, CONTENT, &GmT0031VerifyOptions::default())
        .expect("verify");
    assert_eq!(verification.time_info, "2026-10-24T05:00:00.000Z");
}

#[test]
fn rejects_tampered_content() {
    let (value, _) = make_signature();
    assert!(
        verify_gmt0031_signature(&value, b"tampered", &GmT0031VerifyOptions::default()).is_err()
    );
}

#[test]
fn rejects_tampered_signature() {
    let (mut value, _) = make_signature();
    let last = value.len() - 1;
    value[last] ^= 1;
    assert!(parse_gmt0031_signature(&value).is_ok());
    assert!(verify_gmt0031_signature(&value, CONTENT, &GmT0031VerifyOptions::default()).is_err());
}

#[test]
fn verifier_is_registered_for_gmt0031() {
    let (value, _) = make_signature();
    let verifier = GmT0031SignatureVerifier::new();
    assert_eq!(verifier.format(), SignedValueFormat::GmT0031Signature);
    let _ = value;
}
