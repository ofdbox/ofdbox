use std::io::{Cursor, Write};

use ofdbox_core::{OfdError, OfdPackage};
use ofdbox_crypto::{EncryptOptions, PasswordBackend, encrypt_package};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>roundtrip</ofd:DocID></ofd:DocInfo>
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

#[test]
fn encrypts_and_reads_back() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Res/a.bin", b"\x00\x01\x02\x03"),
    ]);
    let options = EncryptOptions::password("alice", "secret");
    let encrypted = encrypt_package(&plain, &options).expect("encrypt");

    let mut package = OfdPackage::from_bytes(encrypted).expect("open encrypted");
    package
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("enable");

    let root = package.parse_root().expect("root");
    assert_eq!(
        root.doc_bodies()
            .next()
            .unwrap()
            .doc_info()
            .unwrap()
            .doc_id(),
        Some("roundtrip")
    );
    package.parse_document().expect("document");
    assert_eq!(
        &*package.read_entry("Doc_0/Res/a.bin").expect("resource"),
        b"\x00\x01\x02\x03"
    );
    let names = package.entry_names().unwrap();
    assert!(names.iter().any(|name| name == "Encryptions.xml"));
    assert!(names.iter().any(|name| name == "decryptseed.dat"));
    assert!(names.iter().any(|name| name == "entriesmap.dat"));
}

#[test]
fn wrong_password_fails() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
    ]);
    let encrypted =
        encrypt_package(&plain, &EncryptOptions::password("alice", "secret")).expect("encrypt");
    let mut package = OfdPackage::from_bytes(encrypted).expect("open");
    assert!(
        package
            .enable_decryption(Box::new(PasswordBackend::new("wrong")))
            .is_err()
    );
}

#[test]
fn rejects_already_encrypted_input() {
    let plain = zip_with_entries(&[("Encryptions.xml", b"<Encryptions/>")]);
    assert!(matches!(
        encrypt_package(&plain, &EncryptOptions::password("alice", "secret")),
        Err(OfdError::Crypto(_))
    ));
}

fn zip_with_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .expect("start");
        writer.write_all(data).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}
