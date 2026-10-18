use std::fs;
use std::path::PathBuf;

use ofdbox_crypto::sm3_digest;
use ofdbox_seal::{
    CertificatePolicy, SM2_SM3_SIGNATURE_OID, SealError, SealFormat, SealVerifyOptions,
    fingerprint_seal, signer_in_seal, verify_seal, verify_seal_with_options,
};

mod common;

use common::*;

const SEAL_TIME: u64 = 1_654_041_600;
const AFTER_SEAL_EXPIRY: u64 = 1_735_689_600;
const BEFORE_SEAL_START: u64 = 1_600_000_000;
const GM_SEAL_TIME: u64 = 1_704_067_200;

#[test]
fn verifies_synthetic_gm_t_0031_seal() {
    let secret = test_secret();
    let certificate = test_certificate(&secret);
    let seal = signed_gm_seal(&certificate, &secret.to_bytes(), "1234567812345678");

    let result = verify_seal(&seal, SealFormat::GmT0031).expect("verify gm/t 0031");
    assert_eq!(result.format, SealFormat::GmT0031);
    assert_eq!(result.signature_algorithm, SM2_SM3_SIGNATURE_OID);
}

#[test]
fn verifies_synthetic_gb_t_38540_seal() {
    let secret = test_secret();
    let certificate = test_certificate(&secret);
    let seal = signed_gb_seal(&certificate, &secret.to_bytes(), "1234567812345678");

    let result = verify_seal(&seal, SealFormat::GbT38540).expect("verify gb/t 38540");
    assert_eq!(result.format, SealFormat::GbT38540);
    assert_eq!(result.signature_algorithm, SM2_SM3_SIGNATURE_OID);
}

#[test]
fn tampered_seal_fails_verification() {
    let secret = test_secret();
    let certificate = test_certificate(&secret);
    let mut seal = signed_gm_seal(&certificate, &secret.to_bytes(), "1234567812345678");
    let offset = seal
        .windows(PNG.len())
        .position(|window| window == PNG)
        .expect("picture offset");
    seal[offset] ^= 0xff;

    assert_eq!(
        verify_seal(&seal, SealFormat::GmT0031),
        Err(SealError::SignatureInvalid)
    );
}

#[test]
fn format_mismatch_is_rejected_without_trial_verification() {
    let secret = test_secret();
    let certificate = test_certificate(&secret);
    let seal = signed_gm_seal(&certificate, &secret.to_bytes(), "1234567812345678");

    assert_eq!(
        verify_seal(&seal, SealFormat::GbT38540),
        Err(SealError::FormatMismatch {
            expected: SealFormat::GbT38540,
            actual: SealFormat::GmT0031,
        })
    );
    assert_eq!(
        verify_seal(&seal, SealFormat::Unknown),
        Err(SealError::UnsupportedFormat)
    );
}

#[test]
fn unsupported_algorithm_is_rejected() {
    let secret = test_secret();
    let certificate = test_certificate(&secret);
    let seal = signed_gb_seal_with_rsa_oid(&certificate);
    assert_eq!(
        verify_seal(&seal, SealFormat::GbT38540),
        Err(SealError::UnsupportedAlgorithm(
            "1.2.840.113549.1.1.1".to_owned()
        ))
    );
}

#[test]
fn malformed_certificate_is_rejected() {
    let seal = seq(&[
        seal_info(gb_property(), false),
        octets(b"not-a-certificate"),
        oid(OID_SM2_SM3),
        bit_string(b"fictional-signature"),
    ]);
    assert_eq!(
        verify_seal(&seal, SealFormat::GbT38540),
        Err(SealError::InvalidCertificate)
    );
}

#[test]
fn custom_dist_id_is_supported_explicitly() {
    let secret = test_secret();
    let certificate = test_certificate(&secret);
    let seal = signed_gb_seal(&certificate, &secret.to_bytes(), "ofdbox-test-id");

    assert_eq!(
        verify_seal(&seal, SealFormat::GbT38540),
        Err(SealError::SignatureInvalid)
    );
    let options = SealVerifyOptions::new().with_dist_id("ofdbox-test-id");
    verify_seal_with_options(&seal, SealFormat::GbT38540, &options).expect("verify custom id");
}

#[test]
fn unknown_vendor_structure_is_not_verified() {
    let variant = seq(&[
        seal_info(
            seq(&[
                integer(3),
                utf8("厂商变体章"),
                seq(&[octets(b"fictional-certificate")]),
                generalized("20200422020900Z"),
                generalized("20200422020900Z"),
                generalized("20220422020900Z"),
            ]),
            false,
        ),
        oid(OID_SM2_SM3),
        bit_string(b"fictional-signature"),
    ]);
    assert_eq!(
        fingerprint_seal(&variant).expect("fingerprint").format(),
        SealFormat::Unknown
    );
    assert_eq!(
        verify_seal(&variant, SealFormat::GmT0031),
        Err(SealError::FormatMismatch {
            expected: SealFormat::GmT0031,
            actual: SealFormat::Unknown,
        })
    );
}

#[test]
fn validates_seal_certificate_chain_and_validity() {
    let root = test_root_certificate();
    let leaf = test_leaf_certificate();
    let leaf_key = test_leaf_key();
    let seal = signed_gb_seal_with_certs(
        &leaf,
        std::slice::from_ref(&leaf),
        &leaf_key.to_bytes(),
        "1234567812345678",
    );
    let policy = CertificatePolicy::new()
        .with_trust_anchor(root)
        .at_time(SEAL_TIME);
    let options = SealVerifyOptions::new()
        .with_certificate_policy(&policy)
        .with_seal_validity(Some(SEAL_TIME))
        .with_signer_certificate(&leaf);

    let result = verify_seal_with_options(&seal, SealFormat::GbT38540, &options)
        .expect("valid certified seal");
    assert_eq!(result.certificate_chain_length, Some(2));
    assert!(!result.revocation_checked);
}

#[test]
fn validates_gm_t_0031_seal_chain_and_validity() {
    let root = test_root_certificate();
    let leaf = test_leaf_certificate();
    let leaf_key = test_leaf_key();
    let seal = signed_gm_seal_with_certs(
        &leaf,
        std::slice::from_ref(&leaf),
        &leaf_key.to_bytes(),
        "1234567812345678",
    );
    let policy = CertificatePolicy::new()
        .with_trust_anchor(root)
        .at_time(GM_SEAL_TIME);
    let options = SealVerifyOptions::new()
        .with_certificate_policy(&policy)
        .with_seal_validity(Some(GM_SEAL_TIME))
        .with_signer_certificate(&leaf);

    verify_seal_with_options(&seal, SealFormat::GmT0031, &options)
        .expect("valid certified gm seal");
}

#[test]
fn rejects_untrusted_seal_certificate() {
    let leaf = test_leaf_certificate();
    let leaf_key = test_leaf_key();
    let seal = signed_gb_seal_with_certs(
        &leaf,
        std::slice::from_ref(&leaf),
        &leaf_key.to_bytes(),
        "1234567812345678",
    );
    let policy = CertificatePolicy::new().at_time(SEAL_TIME);
    let options = SealVerifyOptions::new().with_certificate_policy(&policy);
    assert_eq!(
        verify_seal_with_options(&seal, SealFormat::GbT38540, &options),
        Err(SealError::UntrustedCertificate)
    );
}

#[test]
fn rejects_expired_and_not_yet_valid_seals() {
    let root = test_root_certificate();
    let leaf = test_leaf_certificate();
    let leaf_key = test_leaf_key();
    let seal = signed_gb_seal_with_certs(
        &leaf,
        std::slice::from_ref(&leaf),
        &leaf_key.to_bytes(),
        "1234567812345678",
    );
    let policy = CertificatePolicy::new()
        .with_trust_anchor(root)
        .at_time(SEAL_TIME);

    let expired = SealVerifyOptions::new()
        .with_certificate_policy(&policy)
        .with_seal_validity(Some(AFTER_SEAL_EXPIRY));
    assert_eq!(
        verify_seal_with_options(&seal, SealFormat::GbT38540, &expired),
        Err(SealError::SealExpired)
    );

    let future = SealVerifyOptions::new()
        .with_certificate_policy(&policy)
        .with_seal_validity(Some(BEFORE_SEAL_START));
    assert_eq!(
        verify_seal_with_options(&seal, SealFormat::GbT38540, &future),
        Err(SealError::SealNotYetValid)
    );
}

#[test]
fn rejects_signer_missing_from_certificate_list() {
    let root = test_root_certificate();
    let leaf = test_leaf_certificate();
    let leaf_key = test_leaf_key();
    let seal = signed_gb_seal_with_certs(
        &leaf,
        std::slice::from_ref(&leaf),
        &leaf_key.to_bytes(),
        "1234567812345678",
    );
    let policy = CertificatePolicy::new()
        .with_trust_anchor(root.clone())
        .at_time(SEAL_TIME);
    let options = SealVerifyOptions::new()
        .with_certificate_policy(&policy)
        .with_signer_certificate(&root);
    assert_eq!(
        verify_seal_with_options(&seal, SealFormat::GbT38540, &options),
        Err(SealError::SignerNotInSeal)
    );
}

#[test]
fn checks_digest_based_certificate_list() {
    let leaf = test_leaf_certificate();
    let leaf_key = test_leaf_key();
    let property = gb_property_with_digests(&[sm3_digest(&leaf)]);
    let seal =
        signed_gb_seal_with_property(property, &leaf, &leaf_key.to_bytes(), "1234567812345678");
    assert!(signer_in_seal(&seal, &leaf).expect("digest list"));
    assert!(!signer_in_seal(&seal, &test_root_certificate()).expect("digest list"));
}

#[test]
fn verifies_real_seals_of_known_formats() {
    let _ = dotenvy::dotenv();
    let Some(dir) = std::env::var_os("OFDBOX_TESTDATA_DIR").map(PathBuf::from) else {
        return;
    };
    if !dir.is_dir() {
        return;
    }
    let mut paths = Vec::new();
    collect_seals(&dir, &mut paths);

    let mut known = 0usize;
    let mut unknown = 0usize;
    for path in &paths {
        let bytes = fs::read(path).expect("read seal sample");
        let format = fingerprint_seal(&bytes)
            .expect("fingerprint real seal")
            .format();
        if format == SealFormat::Unknown {
            unknown += 1;
            continue;
        }
        known += 1;
        verify_seal(&bytes, format).expect("verify real seal");
    }
    if !paths.is_empty() {
        assert_eq!(known + unknown, paths.len());
        assert!(known > 0);
    }
}
