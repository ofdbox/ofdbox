mod fixtures;

use ofdbox_algo::AlgorithmRegistry;
use ofdbox_core::{OfdError, OfdPackage, SignatureVerifier, SignatureVerifiers, SignedValueFormat};

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
    fn format(&self) -> SignedValueFormat {
        // 自定义格式：内容不是 DER，嗅探结果是 Other。
        SignedValueFormat::Other
    }

    fn verify(
        &self,
        _method: &str,
        content: &[u8],
        signed_value: &[u8],
        _algorithms: &AlgorithmRegistry,
    ) -> Result<(), OfdError> {
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
    let verifiers = SignatureVerifiers::new().with(Box::new(FakeVerifier));
    package
        .verify_signature(&signature, &verifiers)
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
    let verifiers = SignatureVerifiers::new().with(Box::new(FakeVerifier));
    package
        .verify_signature(&signature, &verifiers)
        .expect("fake verifier");

    struct Unsupported;
    impl SignatureVerifier for Unsupported {
        fn accepts(&self, _signed_value: &[u8]) -> bool {
            false
        }
        fn verify(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
            _: &AlgorithmRegistry,
        ) -> Result<(), OfdError> {
            Ok(())
        }
    }
    let verifiers = SignatureVerifiers::new().with(Box::new(Unsupported));
    assert!(matches!(
        package.verify_signature(&signature, &verifiers),
        Err(OfdError::UnsupportedSignature(_))
    ));
}
