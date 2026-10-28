mod fixtures;

use ofdbox_core::schema::decrypt_seed::DecryptSeed;
use ofdbox_core::schema::encryption::EncryptInfo;
use ofdbox_core::{CryptoBackend, CryptoRegistry, FileKey, OfdError, OfdPackage};

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>custom-scheme</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const ENCRYPTIONS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Encryptions xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:EncryptInfo ID="9">
    <ofd:Provider Name="Dongle"/>
    <ofd:EncryptScope>All</ofd:EncryptScope>
    <ofd:DecryptSeedLoc>/seed.xml</ofd:DecryptSeedLoc>
    <ofd:EntriesMapLoc>/map.xml</ofd:EntriesMapLoc>
  </ofd:EncryptInfo>
</ofd:Encryptions>"#;

const SEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:DecryptSeed xmlns:ofd="http://www.ofdspec.org/2016" ID="9" EncryptCaseId="1.1.200">
  <ofd:UserInfo UserName="dongle">
    <ofd:EncryptedWK>AAAA</ofd:EncryptedWK>
    <ofd:IVValue>AAAAAAAAAAAAAAAAAAAAAA==</ofd:IVValue>
  </ofd:UserInfo>
  <ofd:ExtendParams/>
</ofd:DecryptSeed>"#;

const MAP: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:EncryptedEntries xmlns:ofd="http://www.ofdspec.org/2016" ID="9">
  <ofd:EncryptEntry Path="/OFD.xml" EPath="/ofd.dat"/>
</ofd:EncryptedEntries>"#;

struct DongleBackend;

impl CryptoBackend for DongleBackend {
    fn supports(&self, scheme_id: &str) -> bool {
        scheme_id == "1.1.200"
    }

    fn unwrap_file_key(
        &self,
        _operation: &EncryptInfo,
        _seed: &DecryptSeed,
    ) -> Result<FileKey, OfdError> {
        Ok(FileKey::new(vec![0u8; 16], vec![0u8; 16]).with_scheme("1.1.200"))
    }

    fn decrypt(&self, _key: &FileKey, data: &[u8]) -> Result<Vec<u8>, OfdError> {
        Ok(data.to_vec())
    }
}

#[test]
fn custom_scheme_backend_is_dispatched() {
    let bytes = fixtures::zip_with_entries(&[
        ("Encryptions.xml", ENCRYPTIONS.as_bytes()),
        ("seed.xml", SEED.as_bytes()),
        ("map.xml", MAP.as_bytes()),
        ("ofd.dat", ROOT.as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    let registry = CryptoRegistry::new().with(Box::new(DongleBackend));
    package
        .enable_decryption(Box::new(registry))
        .expect("custom backend");

    let root = package.parse_root().expect("root");
    assert_eq!(
        root.doc_bodies()
            .next()
            .unwrap()
            .doc_info()
            .unwrap()
            .doc_id(),
        Some("custom-scheme")
    );
    let names = package.entry_names().unwrap();
    assert!(names.iter().any(|name| name == "OFD.xml"));
}

#[test]
fn unregistered_scheme_is_reported() {
    let bytes = fixtures::zip_with_entries(&[
        ("Encryptions.xml", ENCRYPTIONS.as_bytes()),
        ("seed.xml", SEED.as_bytes()),
        ("map.xml", MAP.as_bytes()),
        ("ofd.dat", ROOT.as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    match package.enable_decryption(Box::new(CryptoRegistry::new())) {
        Err(OfdError::UnsupportedScheme(scheme)) => assert_eq!(scheme, "1.1.200"),
        other => panic!("expected UnsupportedScheme, got {other:?}"),
    }
}

const ENCRYPTIONS_LAYERED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Encryptions xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:EncryptInfo ID="1">
    <ofd:Provider Name="Layer"/>
    <ofd:EncryptScope>All</ofd:EncryptScope>
    <ofd:DecryptSeedLoc>/seed1.xml</ofd:DecryptSeedLoc>
    <ofd:EntriesMapLoc>/map1.xml</ofd:EntriesMapLoc>
  </ofd:EncryptInfo>
  <ofd:EncryptInfo ID="2" Relative="1">
    <ofd:Provider Name="Layer"/>
    <ofd:EncryptScope>All</ofd:EncryptScope>
    <ofd:DecryptSeedLoc>/seed2.xml</ofd:DecryptSeedLoc>
    <ofd:EntriesMapLoc>/map2.xml</ofd:EntriesMapLoc>
  </ofd:EncryptInfo>
</ofd:Encryptions>"#;

fn layered_seed(id: &str, scheme: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:DecryptSeed xmlns:ofd="http://www.ofdspec.org/2016" ID="{id}" EncryptCaseId="{scheme}">
  <ofd:UserInfo UserName="u">
    <ofd:EncryptedWK>AAAA</ofd:EncryptedWK>
    <ofd:IVValue>AAAAAAAAAAAAAAAAAAAAAA==</ofd:IVValue>
  </ofd:UserInfo>
  <ofd:ExtendParams/>
</ofd:DecryptSeed>"#
    )
}

fn layered_map(id: &str, epath: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:EncryptedEntries xmlns:ofd="http://www.ofdspec.org/2016" ID="{id}">
  <ofd:EncryptEntry Path="/OFD.xml" EPath="{epath}"/>
</ofd:EncryptedEntries>"#
    )
}

struct LayeredBackend;

impl CryptoBackend for LayeredBackend {
    fn supports(&self, scheme_id: &str) -> bool {
        matches!(scheme_id, "1.1.300" | "1.1.301")
    }

    fn unwrap_file_key(
        &self,
        _operation: &EncryptInfo,
        seed: &DecryptSeed,
    ) -> Result<FileKey, OfdError> {
        let scheme = seed.encrypt_case_id().unwrap_or_default().to_owned();
        let byte = if scheme == "1.1.300" { 0x11 } else { 0x22 };
        Ok(FileKey::new(vec![byte; 16], vec![0u8; 16]).with_scheme(scheme))
    }

    fn decrypt(&self, key: &FileKey, data: &[u8]) -> Result<Vec<u8>, OfdError> {
        Ok(data.iter().map(|byte| byte ^ key.key()[0]).collect())
    }
}

#[test]
fn multi_layer_decryption_uses_relative_chain() {
    let layer1: Vec<u8> = ROOT.bytes().map(|byte| byte ^ 0x11).collect();
    let layer2: Vec<u8> = layer1.iter().map(|byte| byte ^ 0x22).collect();
    let bytes = fixtures::zip_with_entries(&[
        ("Encryptions.xml", ENCRYPTIONS_LAYERED.as_bytes()),
        ("seed1.xml", layered_seed("1", "1.1.300").as_bytes()),
        ("seed2.xml", layered_seed("2", "1.1.301").as_bytes()),
        ("map1.xml", layered_map("1", "/ofd1.dat").as_bytes()),
        ("map2.xml", layered_map("2", "/ofd2.dat").as_bytes()),
        ("ofd1.dat", &layer1),
        ("ofd2.dat", &layer2),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("package");
    package
        .enable_decryption(Box::new(
            CryptoRegistry::new().with(Box::new(LayeredBackend)),
        ))
        .expect("enable");
    let root = package.parse_root().expect("root");
    assert_eq!(
        root.doc_bodies()
            .next()
            .unwrap()
            .doc_info()
            .unwrap()
            .doc_id(),
        Some("custom-scheme")
    );
}

#[test]
fn multi_layer_without_relative_is_rejected() {
    let encryptions = ENCRYPTIONS_LAYERED.replace(r#" Relative="1""#, "");
    let bytes = fixtures::zip_with_entries(&[
        ("Encryptions.xml", encryptions.as_bytes()),
        ("seed1.xml", layered_seed("1", "1.1.300").as_bytes()),
        ("seed2.xml", layered_seed("2", "1.1.301").as_bytes()),
        ("map1.xml", layered_map("1", "/ofd1.dat").as_bytes()),
        ("map2.xml", layered_map("2", "/ofd2.dat").as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("package");
    assert!(matches!(
        package.enable_decryption(Box::new(
            CryptoRegistry::new().with(Box::new(LayeredBackend))
        )),
        Err(OfdError::Crypto(_))
    ));
}
