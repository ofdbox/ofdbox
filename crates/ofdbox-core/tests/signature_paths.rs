mod fixtures;

use ofdbox_core::{
    CryptoBackend, DecryptSeedView, EncryptInfoView, FileKey, OfdError, OfdPackage, ReadOptions,
};

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>paths</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
    <ofd:Signatures>Doc_0/Signs/Signatures.xml</ofd:Signatures>
  </ofd:DocBody>
</ofd:OFD>"#;

const SIGNATURES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Signatures xmlns:ofd="http://www.ofdspec.org/2016"><ofd:MaxSignId>1</ofd:MaxSignId><ofd:Signature ID="1" Type="Sign" BaseLoc="Doc_0/Signs/Sign_0/Signature.xml"/></ofd:Signatures>"#;

const SIGNATURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Signature xmlns:ofd="http://www.ofdspec.org/2016"><ofd:SignedInfo><ofd:Provider ProviderName="test"/><ofd:SignatureMethod>1.2.156.10197.1.501</ofd:SignatureMethod><ofd:SignatureDateTime>20260101000000</ofd:SignatureDateTime><ofd:References CheckMethod="SM3"><ofd:Reference FileRef="/Doc_0/"><ofd:CheckValue>AAAA</ofd:CheckValue></ofd:Reference><ofd:Reference FileRef="/OFD.xml"><ofd:CheckValue>AAAA</ofd:CheckValue></ofd:Reference></ofd:References></ofd:SignedInfo><ofd:SignedValue>Doc_0/Signs/Sign_0/SignedValue.dat</ofd:SignedValue></ofd:Signature>"#;

struct ZeroBackend;

impl CryptoBackend for ZeroBackend {
    fn unwrap_file_key(
        &self,
        _operation: &EncryptInfoView,
        _seed: &DecryptSeedView,
    ) -> Result<FileKey, OfdError> {
        Err(OfdError::Crypto("unused".to_owned()))
    }

    fn decrypt(&self, _key: &FileKey, _data: &[u8]) -> Result<Vec<u8>, OfdError> {
        Err(OfdError::Crypto("unused".to_owned()))
    }

    fn digest(&self, _method: &str, _data: &[u8]) -> Result<Vec<u8>, OfdError> {
        Ok(vec![0u8; 32])
    }
}

fn package() -> Vec<u8> {
    fixtures::zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Signs/Signatures.xml", SIGNATURES.as_bytes()),
        ("Doc_0/Signs/Sign_0/Signature.xml", SIGNATURE.as_bytes()),
        ("Doc_0/Signs/Sign_0/SignedValue.dat", b"value"),
    ])
}

#[test]
fn resolves_full_path_base_loc() {
    let mut package =
        OfdPackage::from_bytes_with_options(package(), ReadOptions::LENIENT, Default::default())
            .expect("open");
    let index = package.parse_signatures().expect("signatures");
    let entry = index.signatures().next().expect("entry");
    let signature = package
        .parse_signature(entry.base_loc().expect("BaseLoc"))
        .expect("full-path BaseLoc");
    assert_eq!(
        signature.signed_value(),
        Some("Doc_0/Signs/Sign_0/SignedValue.dat")
    );
}

#[test]
fn skips_directory_references() {
    let mut package =
        OfdPackage::from_bytes_with_options(package(), ReadOptions::LENIENT, Default::default())
            .expect("open");
    let index = package.parse_signatures().expect("signatures");
    let entry = index.signatures().next().expect("entry");
    let signature = package
        .parse_signature(entry.base_loc().expect("BaseLoc"))
        .expect("signature");
    let checks = package
        .verify_references(&signature, &ZeroBackend)
        .expect("references");
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].file_ref, "/OFD.xml");
}
