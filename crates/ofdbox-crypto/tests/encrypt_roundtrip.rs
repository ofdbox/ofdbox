use std::io::{Cursor, Read, Write};

use ofdbox_core::{OfdError, OfdPackage};
use ofdbox_crypto::{EncryptOptions, EncryptionRecipient, PasswordBackend, encrypt_package};
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

#[test]
fn encrypted_package_requires_decryption() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
    ]);
    let encrypted =
        encrypt_package(&plain, &EncryptOptions::password("alice", "secret")).expect("encrypt");
    let mut package = OfdPackage::from_bytes(encrypted).expect("open");
    assert!(matches!(
        package.parse_root(),
        Err(OfdError::EncryptionRequired)
    ));
    assert!(package.read_entry("OFD.xml").is_err());
}

#[test]
fn streams_decrypted_entries() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Res/a.bin", b"\x00\x01\x02\x03"),
    ]);
    let encrypted =
        encrypt_package(&plain, &EncryptOptions::password("alice", "secret")).expect("encrypt");
    let mut package = OfdPackage::from_bytes(encrypted).expect("open");
    package
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("enable");

    let mut streamed = Vec::new();
    package
        .read_entry_to("Doc_0/Res/a.bin", &mut streamed)
        .expect("stream");
    assert_eq!(
        streamed,
        &*package.read_entry("Doc_0/Res/a.bin").expect("read")
    );
}

#[test]
fn parses_generated_encryption_metadata() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
    ]);
    let encrypted =
        encrypt_package(&plain, &EncryptOptions::password("alice", "secret")).expect("encrypt");
    let mut package = OfdPackage::from_bytes(encrypted).expect("open");
    package
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("enable");

    let encryptions = package.parse_encryptions().expect("encryptions");
    let info = encryptions.encrypt_infos().next().expect("info");
    let seed_loc = info.decrypt_seed_loc().expect("seed loc").to_owned();
    let map_loc = info.entries_map_loc().expect("map loc").to_owned();

    let seed = package.parse_decrypt_seed(&seed_loc).expect("seed");
    assert!(seed.users().next().is_some());

    // 生成端写出的 entriesmap.dat 是二进制映射表，不做 XML 解析。
    assert!(!package.read_entry(&map_loc).expect("map").is_empty());
}

#[test]
fn writes_plain_package_from_encrypted() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
    ]);
    let encrypted =
        encrypt_package(&plain, &EncryptOptions::password("alice", "secret")).expect("encrypt");
    let mut package = OfdPackage::from_bytes(encrypted).expect("open");
    package
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("enable");

    let mut output = Cursor::new(Vec::new());
    let report = package
        .write_decrypted_to(&mut output)
        .expect("write decrypted");
    assert!(report.entries > 0);
    assert!(!report.removed.is_empty());

    let mut plain = OfdPackage::from_bytes(output.into_inner()).expect("open plain");
    plain.parse_root().expect("plain root");
    plain.parse_document().expect("plain document");
}

#[test]
fn tampered_ciphertext_is_rejected() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
    ]);
    let encrypted =
        encrypt_package(&plain, &EncryptOptions::password("alice", "secret")).expect("encrypt");
    let corrupted = rewrite_entry(&encrypted, "ofd.dat", |data| {
        data[0] ^= 0xFF;
    });
    let mut package = OfdPackage::from_bytes(corrupted).expect("open");
    package
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("seed intact");
    assert!(package.parse_root().is_err());
}

#[test]
fn tampered_decrypt_seed_is_rejected() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
    ]);
    let encrypted =
        encrypt_package(&plain, &EncryptOptions::password("alice", "secret")).expect("encrypt");
    let corrupted = rewrite_entry(&encrypted, "decryptseed.dat", |data| {
        data.fill(b'x');
    });
    let mut package = OfdPackage::from_bytes(corrupted).expect("open");
    assert!(
        package
            .enable_decryption(Box::new(PasswordBackend::new("secret")))
            .is_err()
    );
}

fn rewrite_entry(bytes: &[u8], target: &str, mutate: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let reader = Cursor::new(bytes.to_vec());
    let mut archive = zip::ZipArchive::new(reader).expect("zip");
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let mut mutate = Some(mutate);
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("entry");
        let name = entry.name().expect("name").into_owned();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).expect("read");
        if name.eq_ignore_ascii_case(target)
            && let Some(mutate) = mutate.take()
        {
            mutate(&mut data);
        }
        writer
            .start_file(name, SimpleFileOptions::default())
            .expect("start");
        writer.write_all(&data).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

#[test]
fn decrypts_with_each_password_recipient() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
    ]);
    let options = EncryptOptions::password("alice", "first")
        .with_recipient(EncryptionRecipient::password("bob", "second"));
    let encrypted = encrypt_package(&plain, &options).expect("encrypt");

    for password in ["first", "second"] {
        let mut package = OfdPackage::from_bytes(encrypted.clone()).expect("open");
        package
            .enable_decryption(Box::new(PasswordBackend::new(password)))
            .expect("decrypt");
        package.parse_root().expect("root");
    }

    let mut package = OfdPackage::from_bytes(encrypted).expect("open");
    assert!(
        package
            .enable_decryption(Box::new(PasswordBackend::new("wrong")))
            .is_err()
    );
}
