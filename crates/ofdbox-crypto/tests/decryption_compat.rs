use std::io::{Cursor, Read, Write};

use base64::Engine;
use ofdbox_core::{OfdError, OfdPackage, PackageEdits, PackageWriteOptions};
use ofdbox_crypto::{
    EncryptOptions, PasswordBackend, derive_password_kek, encrypt_cbc, encrypt_package,
};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>iv-default</ofd:DocID></ofd:DocInfo>
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

fn entry_bytes(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = ZipArchive::new(Cursor::new(bytes.to_vec())).expect("zip");
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).expect("entry");
        let mut data = Vec::new();
        file.read_to_end(&mut data).expect("read");
        entries.push((file.name().expect("name").into_owned(), data));
    }
    entries
}

fn encrypted_sample() -> Vec<u8> {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
    ]);
    encrypt_package(&plain, &EncryptOptions::password("alice", "pw")).expect("encrypt package")
}

#[test]
fn missing_iv_defaults_to_zero() {
    let password = "pw";
    let kek = derive_password_kek(password);
    let iv = [0u8; 16];
    let file_key = [0x42u8; 16];
    let wrapped = encrypt_cbc(&kek, &iv, &file_key).expect("wrap key");
    let ciphertext = encrypt_cbc(&file_key, &iv, ROOT.as_bytes()).expect("encrypt root");

    let seed = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:DecryptSeed xmlns:ofd="http://www.ofdspec.org/2016" ID="1" EncryptCaseId="1.1.1">
  <ofd:UserInfo UserName="u">
    <ofd:EncryptedWK>{}</ofd:EncryptedWK>
  </ofd:UserInfo>
  <ofd:ExtendParams/>
</ofd:DecryptSeed>"#,
        base64::engine::general_purpose::STANDARD.encode(wrapped)
    );
    let map = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:EncryptedEntries xmlns:ofd="http://www.ofdspec.org/2016" ID="1">
  <ofd:EncryptEntry Path="/OFD.xml" EPath="/ofd.dat"/>
</ofd:EncryptedEntries>"#;
    let encryptions = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Encryptions xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:EncryptInfo ID="1">
    <ofd:Provider Name="test" Version="1"/>
    <ofd:EncryptScope>All</ofd:EncryptScope>
    <ofd:DecryptSeedLoc>/decryptseed.dat</ofd:DecryptSeedLoc>
    <ofd:EntriesMapLoc>/map.xml</ofd:EntriesMapLoc>
  </ofd:EncryptInfo>
</ofd:Encryptions>"#;

    let bytes = zip_with_entries(&[
        ("Encryptions.xml", encryptions.as_bytes()),
        ("decryptseed.dat", seed.as_bytes()),
        ("map.xml", map.as_bytes()),
        ("ofd.dat", &ciphertext),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open");
    package
        .enable_decryption(Box::new(PasswordBackend::new(password)))
        .expect("decrypt without IVValue");
    assert_eq!(
        &*package.read_entry("OFD.xml").expect("root"),
        ROOT.as_bytes()
    );
}

#[test]
fn repack_keeps_ciphertext_and_decryptability() {
    let encrypted = encrypted_sample();
    let before = entry_bytes(&encrypted);

    let mut package = OfdPackage::from_bytes(encrypted).expect("open");
    let repacked = package.to_bytes().expect("repack");
    assert_eq!(entry_bytes(&repacked), before);

    let mut reopened = OfdPackage::from_bytes(repacked).expect("reopen");
    reopened
        .enable_decryption(Box::new(PasswordBackend::new("pw")))
        .expect("enable decryption");
    reopened.parse_root_schema().expect("decrypted root");
    reopened
        .parse_document_schema()
        .expect("decrypted document");
}

#[test]
fn decrypted_save_produces_plain_package() {
    let mut package = OfdPackage::from_bytes(encrypted_sample()).expect("open");
    package
        .enable_decryption(Box::new(PasswordBackend::new("pw")))
        .expect("enable decryption");

    let mut output = Vec::new();
    let report = package
        .write_decrypted_to(&mut Cursor::new(&mut output))
        .expect("write decrypted");

    assert!(report.entries >= 2);
    assert!(
        report
            .removed
            .iter()
            .any(|name| name.eq_ignore_ascii_case("Encryptions.xml"))
    );
    assert!(report.warnings.is_empty());

    let mut plain = OfdPackage::from_bytes(output).expect("open plain");
    assert!(!plain.decryption_enabled());
    plain.parse_root_schema().expect("root without decryption");
    plain
        .parse_document_schema()
        .expect("document without decryption");

    let names = plain.entry_names().expect("entry names");
    assert!(names.iter().any(|name| name == "OFD.xml"));
    assert!(
        !names
            .iter()
            .any(|name| name.eq_ignore_ascii_case("Encryptions.xml"))
    );
    assert!(!names.iter().any(|name| name.ends_with(".dat")));
}

#[test]
fn editing_encrypted_package_is_rejected() {
    let mut package = OfdPackage::from_bytes(encrypted_sample()).expect("open");
    let mut edits = PackageEdits::new();
    edits.replace("OFD.xml", b"<ofd:OFD/>");
    let mut output = Vec::new();
    let error = package
        .write_with(
            &mut Cursor::new(&mut output),
            &edits,
            &PackageWriteOptions::default(),
        )
        .expect_err("editing encrypted package must fail");
    assert!(matches!(error, OfdError::Crypto(_)));
    assert!(output.is_empty());
}
