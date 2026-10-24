mod common;

use ofdbox_core::{DEFAULT_SIGNATURE_METHOD, SignedValueProducer, SigningContext};
use ofdbox_seal::{
    SealBuilder, SealError, SealPictureInput, SealSpec, SealStandard, SesVerifyOptions,
    Sm2SesSignatureProducer, attach_time_stamp, parse_ses_signature, verify_ses_signature,
};
use sm2::SecretKey;

const CONTENT: &[u8] = b"<ofd:Signature>content</ofd:Signature>";

fn make_seal(certificates: &[Vec<u8>], maker_certificate: &[u8], maker: &SecretKey) -> Vec<u8> {
    let spec = SealSpec::new(
        SealStandard::GbT38540,
        "SEAL-TEST",
        "测试印章",
        SealPictureInput {
            media_type: "PNG".to_owned(),
            data: common::PNG.to_vec(),
            width: 40,
            height: 40,
        },
    )
    .with_validity("20260101000000", "20260101000000", "20360101000000")
    .with_certificates(certificates.to_vec());
    SealBuilder::new(spec, maker_certificate.to_vec(), maker.to_bytes().to_vec())
        .build()
        .expect("seal")
}

fn make_ses(content: &[u8], seal: Vec<u8>, certificate: &[u8], secret: &SecretKey) -> Vec<u8> {
    let producer =
        Sm2SesSignatureProducer::new(seal, certificate.to_vec(), secret.to_bytes().to_vec())
            .expect("producer");
    producer
        .produce(&SigningContext {
            content,
            signed_entry: "/Doc_0/Signs/Sign_0/Signature.xml",
            signature_method: DEFAULT_SIGNATURE_METHOD,
            signed_at: "20260101000000",
            references: &[],
        })
        .expect("ses")
}

#[test]
fn verifies_generated_ses_signature() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let seal = make_seal(std::slice::from_ref(&certificate), &certificate, &secret);
    let ses = make_ses(CONTENT, seal, &certificate, &secret);

    let parsed = parse_ses_signature(&ses).expect("parse");
    assert_eq!(parsed.version, 4);
    verify_ses_signature(&ses, CONTENT, &SesVerifyOptions::default()).expect("verify");
}

#[test]
fn rejects_tampered_content() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let seal = make_seal(std::slice::from_ref(&certificate), &certificate, &secret);
    let ses = make_ses(CONTENT, seal, &certificate, &secret);

    let mut tampered = CONTENT.to_vec();
    tampered[0] ^= 1;
    assert!(verify_ses_signature(&ses, &tampered, &SesVerifyOptions::default()).is_err());
}

#[test]
fn rejects_tampered_signature() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let seal = make_seal(std::slice::from_ref(&certificate), &certificate, &secret);
    let mut ses = make_ses(CONTENT, seal, &certificate, &secret);

    let last = ses.len() - 1;
    ses[last] ^= 1;
    assert!(verify_ses_signature(&ses, CONTENT, &SesVerifyOptions::default()).is_err());
}

#[test]
fn rejects_unknown_signature_algorithm() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let seal = make_seal(std::slice::from_ref(&certificate), &certificate, &secret);
    let mut ses = make_ses(CONTENT, seal, &certificate, &secret);

    // 1.2.156.10197.1.501 的 DER 内容，把最后一段改成 .500。
    let oid = [0x2A, 0x81, 0x1C, 0xCF, 0x55, 0x01, 0x83, 0x75];
    let position = ses
        .windows(oid.len())
        .rposition(|window| window == oid)
        .expect("algorithm oid");
    ses[position + oid.len() - 1] = 0x74;
    assert!(matches!(
        verify_ses_signature(&ses, CONTENT, &SesVerifyOptions::default()),
        Err(SealError::UnsupportedAlgorithm(_))
    ));
}

#[test]
fn rejects_truncated_ses_signature() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let seal = make_seal(std::slice::from_ref(&certificate), &certificate, &secret);
    let ses = make_ses(CONTENT, seal, &certificate, &secret);
    assert!(parse_ses_signature(&ses[..ses.len() / 2]).is_err());
}

#[test]
fn rejects_non_zero_trailing_bytes() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let seal = make_seal(std::slice::from_ref(&certificate), &certificate, &secret);
    let mut ses = make_ses(CONTENT, seal, &certificate, &secret);
    ses.push(1);
    assert!(parse_ses_signature(&ses).is_err());
}

#[test]
fn rejects_signer_not_in_seal_list() {
    let signer_secret = common::test_secret();
    let signer_certificate = common::test_certificate(&signer_secret);
    let other_secret = SecretKey::from_slice(&[0x77u8; 32]).expect("secret");
    let other_certificate = common::test_certificate(&other_secret);

    // 印章列表里放另一张证书，签章者不在其中。
    let seal = make_seal(&[other_certificate], &signer_certificate, &signer_secret);
    let ses = make_ses(CONTENT, seal, &signer_certificate, &signer_secret);
    assert!(matches!(
        verify_ses_signature(&ses, CONTENT, &SesVerifyOptions::default()),
        Err(SealError::SignerNotInSeal)
    ));
}

#[test]
fn seal_requires_certificate_list() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let spec = SealSpec::new(
        SealStandard::GbT38540,
        "SEAL-EMPTY",
        "空列表",
        SealPictureInput {
            media_type: "PNG".to_owned(),
            data: common::PNG.to_vec(),
            width: 40,
            height: 40,
        },
    )
    .with_validity("20260101000000", "20260101000000", "20360101000000");
    assert!(
        SealBuilder::new(spec, certificate, secret.to_bytes().to_vec())
            .build()
            .is_err()
    );
}

#[test]
fn gmt_0031_rejects_digest_list() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let spec = SealSpec::new(
        SealStandard::GmT0031,
        "SEAL-0031",
        "摘要列表",
        SealPictureInput {
            media_type: "PNG".to_owned(),
            data: common::PNG.to_vec(),
            width: 40,
            height: 40,
        },
    )
    .with_validity("20260101000000", "20260101000000", "20360101000000")
    .with_certificate_digests(vec![("SM3".to_owned(), vec![0u8; 32])]);
    assert!(
        SealBuilder::new(spec, certificate, secret.to_bytes().to_vec())
            .build()
            .is_err()
    );
}

#[test]
fn roundtrips_time_stamp_token() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let seal = make_seal(std::slice::from_ref(&certificate), &certificate, &secret);
    let token = vec![0x30, 0x03, 0x02, 0x01, 0x01];
    let ses =
        attach_time_stamp(&make_ses(CONTENT, seal, &certificate, &secret), &token).expect("attach");

    let parsed = parse_ses_signature(&ses).expect("parse");
    assert_eq!(parsed.time_stamp, Some(token.as_slice()));
    verify_ses_signature(&ses, CONTENT, &SesVerifyOptions::default()).expect("verify");
}

#[test]
fn rejects_time_stamp_with_wrong_tag() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let seal = make_seal(std::slice::from_ref(&certificate), &certificate, &secret);
    let token = vec![0x30, 0x03, 0x02, 0x01, 0x01];
    let mut ses =
        attach_time_stamp(&make_ses(CONTENT, seal, &certificate, &secret), &token).expect("attach");

    // 令牌前 5 个字节依次是 A0 len 03 len 00，把 [0] 标签改成非法标签。
    let position = ses
        .windows(token.len())
        .rposition(|window| window == token.as_slice())
        .expect("token");
    ses[position - 5] = 0x80;
    assert!(parse_ses_signature(&ses).is_err());
}
