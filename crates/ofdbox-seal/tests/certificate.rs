use sm2::SecretKey;

use ofdbox_seal::{CertificatePolicy, SealError, validate_certificate, validate_certificate_at};

mod common;

use common::*;

const AFTER_START: u64 = FIXED_NOT_BEFORE + 86_400;

fn trusted_policy() -> CertificatePolicy {
    CertificatePolicy::new()
        .with_trust_anchor(test_root_certificate())
        .at_time(AFTER_START)
}

#[test]
fn validates_trusted_leaf_chain() {
    let leaf = test_leaf_certificate();
    let validation = validate_certificate(&leaf, &trusted_policy()).expect("trusted chain");
    assert_eq!(validation.chain_length, 2);
    assert!(!validation.revocation_checked);
}

#[test]
fn validates_intermediate_chain() {
    let intermediate_key = SecretKey::from_slice(&[0x33u8; 32]).expect("intermediate key");
    let intermediate = build_certificate(
        "CN=OFDBox Intermediate",
        "CN=OFDBox Root",
        &intermediate_key,
        &test_root_key(),
        5,
        true,
        fixed_validity(),
    );
    let leaf = build_certificate(
        "CN=OFDBox Seal",
        "CN=OFDBox Intermediate",
        &test_leaf_key(),
        &intermediate_key,
        LEAF_SERIAL,
        false,
        fixed_validity(),
    );
    let policy = CertificatePolicy::new()
        .with_trust_anchor(test_root_certificate())
        .with_intermediate(intermediate)
        .at_time(AFTER_START);
    let validation = validate_certificate(&leaf, &policy).expect("intermediate chain");
    assert_eq!(validation.chain_length, 3);

    let without_intermediate = CertificatePolicy::new()
        .with_trust_anchor(test_root_certificate())
        .at_time(AFTER_START);
    assert_eq!(
        validate_certificate(&leaf, &without_intermediate),
        Err(SealError::UntrustedCertificate)
    );
}

#[test]
fn rejects_untrusted_chain() {
    let leaf = test_leaf_certificate();
    let policy = CertificatePolicy::new().at_time(AFTER_START);
    assert_eq!(
        validate_certificate(&leaf, &policy),
        Err(SealError::UntrustedCertificate)
    );
}

#[test]
fn rejects_wrong_anchor() {
    let wrong_key = SecretKey::from_slice(&[0x55u8; 32]).expect("wrong key");
    let wrong_anchor = build_certificate(
        "CN=OFDBox Root",
        "CN=OFDBox Root",
        &wrong_key,
        &wrong_key,
        ROOT_SERIAL,
        true,
        fixed_validity(),
    );
    let policy = CertificatePolicy::new()
        .with_trust_anchor(wrong_anchor)
        .at_time(AFTER_START);
    assert_eq!(
        validate_certificate(&test_leaf_certificate(), &policy),
        Err(SealError::CertificateSignatureInvalid)
    );
}

#[test]
fn rejects_expired_and_not_yet_valid_certificates() {
    let leaf = test_leaf_certificate();
    let expired = CertificatePolicy::new()
        .with_trust_anchor(test_root_certificate())
        .at_time(FIXED_NOT_AFTER + 1);
    assert_eq!(
        validate_certificate(&leaf, &expired),
        Err(SealError::CertificateExpired)
    );

    let future = CertificatePolicy::new()
        .with_trust_anchor(test_root_certificate())
        .at_time(FIXED_NOT_BEFORE - 1);
    assert_eq!(
        validate_certificate(&leaf, &future),
        Err(SealError::CertificateNotYetValid)
    );
}

#[test]
fn rejects_ca_as_leaf() {
    let leaf = build_certificate(
        "CN=OFDBox Seal",
        "CN=OFDBox Root",
        &test_leaf_key(),
        &test_root_key(),
        LEAF_SERIAL,
        true,
        fixed_validity(),
    );
    assert_eq!(
        validate_certificate(&leaf, &trusted_policy()),
        Err(SealError::InvalidCertificateChain)
    );
}

#[test]
fn rejects_leaf_with_wrong_signature() {
    let wrong_key = SecretKey::from_slice(&[0x66u8; 32]).expect("wrong key");
    let leaf = build_certificate(
        "CN=OFDBox Seal",
        "CN=OFDBox Root",
        &test_leaf_key(),
        &wrong_key,
        LEAF_SERIAL,
        false,
        fixed_validity(),
    );
    assert_eq!(
        validate_certificate(&leaf, &trusted_policy()),
        Err(SealError::CertificateSignatureInvalid)
    );
}

#[test]
fn detects_revoked_certificate_with_crl() {
    let root = test_root_certificate();
    let leaf = test_leaf_certificate();
    let crl = test_crl(
        &root,
        &test_root_key(),
        &[LEAF_SERIAL],
        AFTER_START,
        Some(FIXED_NOT_AFTER),
    );
    let policy = CertificatePolicy::new()
        .with_trust_anchor(root)
        .with_crl(crl)
        .with_revocation_required()
        .at_time(AFTER_START);
    assert_eq!(
        validate_certificate(&leaf, &policy),
        Err(SealError::CertificateRevoked)
    );
}

#[test]
fn accepts_clean_crl() {
    let root = test_root_certificate();
    let leaf = test_leaf_certificate();
    let crl = test_crl(
        &root,
        &test_root_key(),
        &[],
        AFTER_START,
        Some(FIXED_NOT_AFTER),
    );
    let policy = CertificatePolicy::new()
        .with_trust_anchor(root)
        .with_crl(crl)
        .with_revocation_required()
        .at_time(AFTER_START);
    let validation = validate_certificate(&leaf, &policy).expect("clean crl");
    assert!(validation.revocation_checked);
}

#[test]
fn requires_revocation_data_when_configured() {
    let policy = trusted_policy().with_revocation_required();
    assert_eq!(
        validate_certificate(&test_leaf_certificate(), &policy),
        Err(SealError::RevocationUnknown)
    );
}

#[test]
fn ignores_crl_with_bad_signature() {
    let root = test_root_certificate();
    let wrong_key = SecretKey::from_slice(&[0x77u8; 32]).expect("wrong key");
    let crl = test_crl(
        &root,
        &wrong_key,
        &[LEAF_SERIAL],
        AFTER_START,
        Some(FIXED_NOT_AFTER),
    );
    let policy = CertificatePolicy::new()
        .with_trust_anchor(root)
        .with_crl(crl)
        .with_revocation_required()
        .at_time(AFTER_START);
    assert_eq!(
        validate_certificate(&test_leaf_certificate(), &policy),
        Err(SealError::RevocationUnknown)
    );
}

#[test]
fn explicit_time_overrides_policy_time() {
    let leaf = test_leaf_certificate();
    let policy = trusted_policy();
    let validation = validate_certificate_at(&leaf, &policy, Some(FIXED_NOT_BEFORE + 10))
        .expect("explicit time");
    assert_eq!(validation.chain_length, 2);
    assert_eq!(
        validate_certificate_at(&leaf, &policy, Some(FIXED_NOT_AFTER + 1)),
        Err(SealError::CertificateExpired)
    );
}
