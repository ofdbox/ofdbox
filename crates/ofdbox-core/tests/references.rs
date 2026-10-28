mod fixtures;

use ofdbox_core::schema::decrypt_seed::DecryptSeed;
use ofdbox_core::schema::encryption::EncryptInfo;
use ofdbox_core::{CryptoBackend, FileKey, OfdError, OfdPackage};

struct IdentityDigest;

impl CryptoBackend for IdentityDigest {
    fn unwrap_file_key(
        &self,
        _operation: &EncryptInfo,
        _seed: &DecryptSeed,
    ) -> Result<FileKey, OfdError> {
        Err(OfdError::Crypto("not used".to_owned()))
    }

    fn decrypt(&self, _key: &FileKey, _data: &[u8]) -> Result<Vec<u8>, OfdError> {
        Err(OfdError::Crypto("not used".to_owned()))
    }

    fn digest(&self, _method: &str, data: &[u8]) -> Result<Vec<u8>, OfdError> {
        Ok(data.to_vec())
    }
}

fn signature_xml(check_value: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Signature xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:SignedInfo>
    <ofd:Provider ProviderName="P"/>
    <ofd:References CheckMethod="TEST">
      <ofd:Reference FileRef="/a.txt"><ofd:CheckValue>{check_value}</ofd:CheckValue></ofd:Reference>
    </ofd:References>
  </ofd:SignedInfo>
  <ofd:SignedValue>SignedValue.dat</ofd:SignedValue>
</ofd:Signature>"#
    )
}

#[test]
fn reference_digest_matches() {
    let bytes = fixtures::zip_with_entry("a.txt", b"hello");
    let mut package = OfdPackage::from_bytes(bytes).expect("package");
    let signature = ofdbox_core::schema_xml::from_bytes(signature_xml("aGVsbG8=").as_bytes())
        .expect("signature");
    let checks = package
        .verify_references(&signature, &IdentityDigest)
        .expect("verify");
    assert_eq!(checks.len(), 1);
    assert!(checks[0].matches());
    assert_eq!(checks[0].file_ref, "/a.txt");
}

#[test]
fn reference_digest_mismatch_is_reported() {
    let bytes = fixtures::zip_with_entry("a.txt", b"hello");
    let mut package = OfdPackage::from_bytes(bytes).expect("package");
    let signature = ofdbox_core::schema_xml::from_bytes(signature_xml("d3Jvbmc=").as_bytes())
        .expect("signature");
    let checks = package
        .verify_references(&signature, &IdentityDigest)
        .expect("verify");
    assert!(!checks[0].matches());
}

#[test]
fn missing_reference_file_is_an_error() {
    let bytes = fixtures::zip_with_entry("b.txt", b"hello");
    let mut package = OfdPackage::from_bytes(bytes).expect("package");
    let signature = ofdbox_core::schema_xml::from_bytes(signature_xml("aGVsbG8=").as_bytes())
        .expect("signature");
    assert!(matches!(
        package.verify_references(&signature, &IdentityDigest),
        Err(OfdError::MissingEntry(_))
    ));
}
