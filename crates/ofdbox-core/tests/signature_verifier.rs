mod fixtures;

use ofdbox_core::{OfdError, OfdPackage, SignatureVerifier};

const SIGNATURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Signature xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:SignedInfo>
    <ofd:Provider ProviderName="P"/>
    <ofd:SignatureMethod>FAKE</ofd:SignatureMethod>
    <ofd:References CheckMethod="SM3">
      <ofd:Reference FileRef="/OFD.xml"><ofd:CheckValue>AAAA</ofd:CheckValue></ofd:Reference>
    </ofd:References>
  </ofd:SignedInfo>
  <ofd:SignedValue>SignedValue.dat</ofd:SignedValue>
</ofd:Signature>"#;

struct FakeVerifier;

impl SignatureVerifier for FakeVerifier {
    fn supports(&self, method: &str) -> bool {
        method == "FAKE"
    }

    fn verify(&self, _method: &str, content: &[u8], signed_value: &[u8]) -> Result<(), OfdError> {
        if content.windows(6).any(|window| window == b"Signat") && signed_value == b"signature" {
            Ok(())
        } else {
            Err(OfdError::Crypto("fake verify failed".to_owned()))
        }
    }
}

#[test]
fn injected_verifier_receives_signature_bytes() {
    let bytes = fixtures::zip_with_entries(&[
        ("Sig/Signature.xml", SIGNATURE.as_bytes()),
        ("Sig/SignedValue.dat", b"signature"),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("package");
    let signature = package
        .parse_signature_file("Sig/Signature.xml")
        .expect("signature");
    package
        .verify_signature(&signature, &FakeVerifier)
        .expect("verify");
}

#[test]
fn unsupported_signature_method_is_reported() {
    let bytes = fixtures::zip_with_entries(&[
        ("Sig/Signature.xml", SIGNATURE.as_bytes()),
        ("Sig/SignedValue.dat", b"signature"),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("package");
    let signature = package
        .parse_signature_file("Sig/Signature.xml")
        .expect("signature");
    match package.verify_signature(&signature, &FakeVerifier) {
        Ok(()) => {}
        Err(error) => panic!("unexpected error: {error:?}"),
    }
    struct Unsupported;
    impl SignatureVerifier for Unsupported {
        fn supports(&self, _method: &str) -> bool {
            false
        }
        fn verify(&self, _: &str, _: &[u8], _: &[u8]) -> Result<(), OfdError> {
            Ok(())
        }
    }
    assert!(matches!(
        package.verify_signature(&signature, &Unsupported),
        Err(OfdError::UnsupportedSignature(_))
    ));
}
