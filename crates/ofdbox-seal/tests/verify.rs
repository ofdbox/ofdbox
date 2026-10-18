use std::fs;
use std::path::PathBuf;

use ofdbox_seal::{
    SM2_SM3_SIGNATURE_OID, SealError, SealFormat, SealVerifyOptions, fingerprint_seal, verify_seal,
    verify_seal_with_options,
};

mod common;

use common::*;

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
    let options = SealVerifyOptions {
        dist_id: "ofdbox-test-id",
    };
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
