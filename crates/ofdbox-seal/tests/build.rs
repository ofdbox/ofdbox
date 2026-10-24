mod common;

use ofdbox_seal::{
    SealBuilder, SealFormat, SealPictureInput, SealSpec, SealStandard, parse_seal_for_display,
    signer_in_seal, verify_seal,
};

fn picture() -> SealPictureInput {
    SealPictureInput {
        media_type: "PNG".to_owned(),
        data: common::PNG.to_vec(),
        width: 40,
        height: 40,
    }
}

#[test]
fn builds_seal_in_both_standards() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);

    let spec = SealSpec::new(SealStandard::GbT38540, "SEAL-001", "示例公章", picture())
        .with_validity("20260101000000", "20260101000000", "20360101000000")
        .with_certificates(vec![certificate.clone()]);
    let seal = SealBuilder::new(spec, certificate.clone(), secret.to_bytes().to_vec())
        .build()
        .expect("build 38540 seal");
    let display = parse_seal_for_display(&seal).expect("display 38540");
    assert_eq!(display.format(), SealFormat::GbT38540);
    assert_eq!(display.header.version, 4);
    assert_eq!(display.property.name, "示例公章");
    assert_eq!(display.property.certificate_list_type, Some(1));
    assert_eq!(display.picture.media_type, "PNG");
    verify_seal(&seal, SealFormat::GbT38540).expect("verify 38540 seal");
    assert!(signer_in_seal(&seal, &certificate).expect("signer in 38540 seal"));

    let spec = SealSpec::new(SealStandard::GmT0031, "SEAL-002", "示例个人章", picture())
        .with_validity("20260101000000", "20260101000000", "20360101000000")
        .with_certificates(vec![certificate.clone()]);
    let seal = SealBuilder::new(spec, certificate.clone(), secret.to_bytes().to_vec())
        .build()
        .expect("build 0031 seal");
    let display = parse_seal_for_display(&seal).expect("display 0031");
    assert_eq!(display.format(), SealFormat::GmT0031);
    assert_eq!(display.property.certificate_list_type, None);
    verify_seal(&seal, SealFormat::GmT0031).expect("verify 0031 seal");
    assert!(signer_in_seal(&seal, &certificate).expect("signer in 0031 seal"));
}

#[test]
fn builds_certificate_digest_list() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let digest = ofdbox_crypto::sm3_digest(&certificate).to_vec();

    let spec = SealSpec::new(
        SealStandard::GbT38540,
        "SEAL-003",
        "摘要列表示例",
        picture(),
    )
    .with_validity("20260101000000", "20260101000000", "20360101000000")
    .with_certificate_digests(vec![("SM3".to_owned(), digest)]);
    let seal = SealBuilder::new(spec, certificate.clone(), secret.to_bytes().to_vec())
        .build()
        .expect("build digest seal");
    let display = parse_seal_for_display(&seal).expect("display digest seal");
    assert_eq!(display.property.certificate_list_type, Some(2));
    verify_seal(&seal, SealFormat::GbT38540).expect("verify digest seal");
    assert!(signer_in_seal(&seal, &certificate).expect("signer in digest seal"));
}

#[test]
fn rejects_incomplete_spec() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let spec = SealSpec::new(SealStandard::GbT38540, "SEAL-004", "缺时间", picture())
        .with_certificates(vec![certificate.clone()]);
    let error = SealBuilder::new(spec, certificate, secret.to_bytes().to_vec())
        .build()
        .expect_err("missing validity");
    assert!(matches!(error, ofdbox_seal::SealError::Invalid(_)));
}
