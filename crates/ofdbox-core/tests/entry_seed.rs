use std::io::{Cursor, Write};

use ofdbox_core::model::decrypt_seed::DecryptSeed;
use ofdbox_core::model::encryption::EncryptInfo;
use ofdbox_core::model::entries_map::EncryptedEntries;
use ofdbox_core::{CryptoBackend, FileKey, OfdError, OfdPackage, ReadOptions};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>entry-key</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>1</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;

const ENCRYPTIONS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Encryptions xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:EncryptInfo ID="1">
    <ofd:Provider Name="test" Version="1"/>
    <ofd:EncryptScope>All</ofd:EncryptScope>
    <ofd:DecryptSeedLoc>/seed1.xml</ofd:DecryptSeedLoc>
    <ofd:EntriesMapLoc>/map1.xml</ofd:EntriesMapLoc>
  </ofd:EncryptInfo>
</ofd:Encryptions>"#;

const SEED1: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:DecryptSeed xmlns:ofd="http://www.ofdspec.org/2016" ID="1" EncryptCaseId="1.1.400">
  <ofd:UserInfo UserName="u">
    <ofd:EncryptedWK>AAAA</ofd:EncryptedWK>
    <ofd:IVValue>AAAAAAAAAAAAAAAAAAAAAA==</ofd:IVValue>
  </ofd:UserInfo>
  <ofd:ExtendParams/>
</ofd:DecryptSeed>"#;

const SEED2: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:DecryptSeed xmlns:ofd="http://www.ofdspec.org/2016" ID="1" EncryptCaseId="1.1.401">
  <ofd:UserInfo UserName="u">
    <ofd:EncryptedWK>AAAA</ofd:EncryptedWK>
    <ofd:IVValue>AAAAAAAAAAAAAAAAAAAAAA==</ofd:IVValue>
  </ofd:UserInfo>
  <ofd:ExtendParams/>
</ofd:DecryptSeed>"#;

const MAP: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:EncryptedEntries xmlns:ofd="http://www.ofdspec.org/2016" ID="1">
  <ofd:EncryptEntry Path="/OFD.xml" EPath="/ofd.dat"/>
  <ofd:EncryptEntry Path="/Doc_0/Document.xml" EPath="/doc.dat" DecryptSeedLoc="/seed2.xml"/>
</ofd:EncryptedEntries>"#;

struct EntryBackend;

impl CryptoBackend for EntryBackend {
    fn supports(&self, scheme_id: &str) -> bool {
        matches!(scheme_id, "1.1.400" | "1.1.401")
    }

    fn unwrap_file_key(
        &self,
        _operation: &EncryptInfo,
        seed: &DecryptSeed,
    ) -> Result<FileKey, OfdError> {
        let scheme = seed.encrypt_case_id().unwrap_or_default().to_owned();
        let byte = if scheme == "1.1.401" { 0x22 } else { 0x11 };
        Ok(FileKey::new(vec![byte; 16], vec![0u8; 16]).with_scheme(scheme))
    }

    fn decrypt(&self, key: &FileKey, data: &[u8]) -> Result<Vec<u8>, OfdError> {
        Ok(data.iter().map(|byte| byte ^ key.key()[0]).collect())
    }
}

fn xor(data: &[u8], key: u8) -> Vec<u8> {
    data.iter().map(|byte| byte ^ key).collect()
}

fn zip_with_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .expect("start entry");
        writer.write_all(data).expect("write entry");
    }
    writer.finish().expect("finish zip").into_inner()
}

fn package(with_seed2: bool) -> Vec<u8> {
    let ofd = xor(ROOT.as_bytes(), 0x11);
    let document = xor(DOCUMENT.as_bytes(), 0x22);
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("Encryptions.xml", ENCRYPTIONS.as_bytes()),
        ("seed1.xml", SEED1.as_bytes()),
        ("map1.xml", MAP.as_bytes()),
        ("ofd.dat", &ofd),
        ("doc.dat", &document),
    ];
    if with_seed2 {
        entries.push(("seed2.xml", SEED2.as_bytes()));
    }
    zip_with_entries(&entries)
}

#[test]
fn entry_level_seed_selects_its_own_key() {
    let mut package = OfdPackage::from_bytes(package(true)).expect("open");
    package
        .enable_decryption(Box::new(EntryBackend))
        .expect("enable decryption");

    assert_eq!(
        &*package.read_entry("OFD.xml").expect("root"),
        ROOT.as_bytes()
    );
    assert_eq!(
        &*package.read_entry("Doc_0/Document.xml").expect("document"),
        DOCUMENT.as_bytes()
    );
}

#[test]
fn missing_entry_seed_fails_only_that_entry() {
    let mut package = OfdPackage::from_bytes(package(false)).expect("open");
    package
        .enable_decryption(Box::new(EntryBackend))
        .expect("enable decryption");

    assert!(package.read_entry("OFD.xml").is_ok());
    assert!(matches!(
        package.read_entry("Doc_0/Document.xml"),
        Err(OfdError::MissingEntry(_))
    ));
}

#[test]
fn strict_mode_accepts_seed_attribute() {
    EncryptedEntries::from_xml(MAP.as_bytes(), ReadOptions::STRICT).expect("strict attribute");
}

#[test]
fn writer_emits_seed_as_attribute() {
    let view = EncryptedEntries::from_xml(MAP.as_bytes(), ReadOptions::LENIENT).expect("parse");
    let xml = view.to_xml().expect("write");
    assert!(
        xml.contains(r#"DecryptSeedLoc="/seed2.xml""#),
        "expected attribute form, got: {xml}"
    );
    assert!(!xml.contains("<ofd:DecryptSeedLoc>"));
    EncryptedEntries::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
}
