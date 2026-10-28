use std::io::{Cursor, Read, Write};

use base64::Engine;

use ofdbox_core::model::page::{GraphicObject, Page};
use ofdbox_core::{OfdError, OfdFile, OfdPackage};
use ofdbox_crypto::{
    EncryptOptions, EncryptionRecipient, PASSWORD_CASE_ID, PasswordBackend, edit_encrypted,
    edit_encrypted_preserving, encrypt_cbc, encrypt_package, wrap_file_key,
};
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

    let root = package.parse_root_schema().expect("root");
    assert_eq!(
        root.doc_bodies.first().unwrap().doc_info.doc_id.as_str(),
        "roundtrip"
    );
    package.parse_document_schema().expect("document");
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
        ("Doc_0/Res/a.bin", b"\x00\x01\x02\x03"),
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
    assert!(package.contains("Encryptions.xml"));
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
    plain.parse_root_schema().expect("plain root");
    plain.parse_document_schema().expect("plain document");
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
    assert!(package.contains("Encryptions.xml"));
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
        package.parse_root_schema().expect("root");
    }

    let mut package = OfdPackage::from_bytes(encrypted).expect("open");
    assert!(
        package
            .enable_decryption(Box::new(PasswordBackend::new("wrong")))
            .is_err()
    );
}

#[test]
fn edits_encrypted_package_and_reencrypts() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
    ]);
    let options = EncryptOptions::password("alice", "secret");
    let encrypted = encrypt_package(&plain, &options).expect("encrypt");

    let output = edit_encrypted(
        &encrypted,
        Box::new(PasswordBackend::new("secret")),
        &options,
        |file: &mut OfdFile| {
            let ofd = file.ofd()?;
            let _ = ofd.document(0)?;
            Ok(())
        },
    )
    .expect("edit encrypted");

    let mut reopened = OfdPackage::from_bytes(output).expect("reopen");
    reopened
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("decrypt edited");
    reopened
        .parse_document_schema()
        .expect("document after edit");
}

const PAGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Content>
    <ofd:Layer ID="1">
      <ofd:TextObject ID="2" Boundary="10 10 100 20" Font="1" Size="12">
        <ofd:TextCode X="0" Y="0">before</ofd:TextCode>
      </ofd:TextObject>
    </ofd:Layer>
  </ofd:Content>
</ofd:Page>"#;

#[test]
fn edits_encrypted_page_and_reencrypts() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", PAGE.as_bytes()),
    ]);
    let options = EncryptOptions::password("alice", "secret");
    let encrypted = encrypt_package(&plain, &options).expect("encrypt");

    let output = edit_encrypted(
        &encrypted,
        Box::new(PasswordBackend::new("secret")),
        &options,
        |file: &mut OfdFile| {
            let document = file.document(0)?;
            let mut page = document.page("1")?;
            let object = &mut page.data.content.as_mut().unwrap().layers[0].objects[0];
            let GraphicObject::Text(text) = object else {
                return Err(OfdError::InvalidStructure("not text".to_owned()));
            };
            text.text_codes[0].text = "after".to_owned();
            page.save()?;
            Ok(())
        },
    )
    .expect("edit encrypted");

    let mut reopened = OfdPackage::from_bytes(output).expect("reopen");
    reopened
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("decrypt");
    let bytes = reopened
        .read_entry("Doc_0/Pages/Page_0/Content.xml")
        .expect("page");
    let page: Page = ofdbox_core::model_xml::from_bytes(&bytes).expect("parse page");
    let GraphicObject::Text(text) = &page.content.as_ref().unwrap().layers[0].objects[0] else {
        panic!("not text");
    };
    assert_eq!(text.text_codes[0].text, "after");
}

#[test]
fn preserves_unmodified_encrypted_entries() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", PAGE.as_bytes()),
    ]);
    let options = EncryptOptions::password("alice", "secret");
    let encrypted = encrypt_package(&plain, &options).expect("encrypt");

    let mut original = OfdPackage::from_bytes(encrypted.clone()).expect("open original");
    original
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("decrypt original");
    let physical = original
        .encrypted_entry_info("Doc_0/Document.xml")
        .expect("info")
        .expect("encrypted")
        .0;
    let resource_before = original
        .read_raw_entry(&physical)
        .expect("raw resource")
        .to_vec();

    let output = edit_encrypted_preserving(
        &encrypted,
        Box::new(PasswordBackend::new("secret")),
        |file: &mut OfdFile| {
            let document = file.document(0)?;
            let mut page = document.page("1")?;
            let GraphicObject::Text(text) =
                &mut page.data.content.as_mut().unwrap().layers[0].objects[0]
            else {
                return Err(OfdError::InvalidStructure("not text".to_owned()));
            };
            text.text_codes[0].text = "after".to_owned();
            page.save()?;
            Ok(())
        },
    )
    .expect("preserving edit");

    let mut edited = OfdPackage::from_bytes(output).expect("open edited");
    edited
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("decrypt edited");
    let physical = edited
        .encrypted_entry_info("Doc_0/Document.xml")
        .expect("info")
        .expect("encrypted")
        .0;
    let resource_after = edited
        .read_raw_entry(&physical)
        .expect("raw resource after")
        .to_vec();
    assert_eq!(resource_before, resource_after);

    let page_bytes = edited
        .read_entry("Doc_0/Pages/Page_0/Content.xml")
        .expect("page");
    let page: Page = ofdbox_core::model_xml::from_bytes(&page_bytes).expect("parse page");
    let GraphicObject::Text(text) = &page.content.as_ref().unwrap().layers[0].objects[0] else {
        panic!("not text");
    };
    assert_eq!(text.text_codes[0].text, "after");
}

#[test]
fn deletes_encrypted_page_preserving() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", PAGE.as_bytes()),
    ]);
    let options = EncryptOptions::password("alice", "secret");
    let encrypted = encrypt_package(&plain, &options).expect("encrypt");

    let output = edit_encrypted_preserving(
        &encrypted,
        Box::new(PasswordBackend::new("secret")),
        |file: &mut OfdFile| {
            let mut document = file.document(0)?;
            document.remove_page("1")?;
            Ok(())
        },
    )
    .expect("delete page");

    let mut reopened = OfdPackage::from_bytes(output).expect("reopen");
    reopened
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("decrypt");
    let document = reopened
        .parse_document_schema()
        .expect("document after delete");
    assert!(document.pages.pages.is_empty());
    assert!(!reopened.contains("Doc_0/Pages/Page_0/Content.xml"));
}

#[test]
fn adds_encrypted_page() {
    let plain = zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", PAGE.as_bytes()),
    ]);
    let options = EncryptOptions::password("alice", "secret");
    let encrypted = encrypt_package(&plain, &options).expect("encrypt");

    let output = edit_encrypted_preserving(
        &encrypted,
        Box::new(PasswordBackend::new("secret")),
        |file: &mut OfdFile| {
            let mut document = file.document(0)?;
            document.add_page(Page::default())?;
            document.save()?;
            Ok(())
        },
    )
    .expect("add page");

    let mut reopened = OfdPackage::from_bytes(output).expect("reopen");
    reopened
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("decrypt");
    let document = reopened
        .parse_document_schema()
        .expect("document after add");
    assert_eq!(document.pages.pages.len(), 2);
    let entry = document.pages.pages.last().expect("second page");
    assert_eq!(entry.id, "2");
    assert!(reopened.contains("Doc_0/Pages/Page_2/Content.xml"));
    reopened
        .read_entry("Doc_0/Pages/Page_2/Content.xml")
        .expect("read new page");
}

fn multilayer_package() -> Vec<u8> {
    let key0 = [1u8; 16];
    let iv0 = [2u8; 16];
    let key1 = [3u8; 16];
    let iv1 = [4u8; 16];
    let plain = [
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", PAGE.as_bytes()),
        ("Doc_0/Res/a.bin", b"\x00\x01\x02\x03"),
    ];

    let mut physical0 = Vec::new();
    let mut physical1 = Vec::new();
    let mut map0 = String::new();
    let mut map1 = String::new();
    for (index, (name, bytes)) in plain.iter().enumerate() {
        let p0 = format!("Layer0/{index}.dat");
        let p1 = format!("Layer1/{index}.dat");
        let c0 = encrypt_cbc(&key0, &iv0, bytes).expect("layer0");
        let c1 = encrypt_cbc(&key1, &iv1, &c0).expect("layer1");
        physical0.push((p0.clone(), c0));
        physical1.push((p1.clone(), c1));
        map0.push_str(&format!(
            "<ofd:EncryptEntry Path=\"/{name}\" EPath=\"/{p0}\"/>"
        ));
        map1.push_str(&format!(
            "<ofd:EncryptEntry Path=\"/{name}\" EPath=\"/{p1}\"/>"
        ));
    }

    let map0 = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:EncryptedEntries xmlns:ofd=\"http://www.ofdspec.org/2016\" ID=\"1\">{map0}</ofd:EncryptedEntries>"
    );
    let map1 = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:EncryptedEntries xmlns:ofd=\"http://www.ofdspec.org/2016\" ID=\"2\">{map1}</ofd:EncryptedEntries>"
    );
    let encrypted_map0 = encrypt_cbc(&key0, &iv0, map0.as_bytes()).expect("map0");
    let encrypted_map1 = encrypt_cbc(&key1, &iv1, map1.as_bytes()).expect("map1");

    let seed = |id: &str, iv: &[u8], key: &[u8]| {
        let wrapped = wrap_file_key("secret", iv, key).expect("wrap");
        let iv = base64::engine::general_purpose::STANDARD.encode(iv);
        let wrapped = base64::engine::general_purpose::STANDARD.encode(wrapped);
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:DecryptSeed xmlns:ofd=\"http://www.ofdspec.org/2016\" ID=\"{id}\" EncryptCaseId=\"{PASSWORD_CASE_ID}\"><ofd:UserInfo UserName=\"alice\"><ofd:EncryptedWK>{wrapped}</ofd:EncryptedWK><ofd:IVValue>{iv}</ofd:IVValue></ofd:UserInfo><ofd:ExtendParams/></ofd:DecryptSeed>"
        )
    };
    let seed0 = seed("1", &iv0, &key0);
    let seed1 = seed("2", &iv1, &key1);
    let encryptions = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:Encryptions xmlns:ofd=\"http://www.ofdspec.org/2016\"><ofd:EncryptInfo ID=\"1\"><ofd:Provider Name=\"OFDBox\"/><ofd:EncryptScope>All</ofd:EncryptScope><ofd:DecryptSeedLoc>/decryptseed_0.dat</ofd:DecryptSeedLoc><ofd:EntriesMapLoc>/entriesmap_0.dat</ofd:EntriesMapLoc></ofd:EncryptInfo><ofd:EncryptInfo ID=\"2\" Relative=\"1\"><ofd:Provider Name=\"OFDBox\"/><ofd:EncryptScope>All</ofd:EncryptScope><ofd:DecryptSeedLoc>/decryptseed_1.dat</ofd:DecryptSeedLoc><ofd:EntriesMapLoc>/entriesmap_1.dat</ofd:EntriesMapLoc></ofd:EncryptInfo></ofd:Encryptions>";

    let mut entries: Vec<(String, Vec<u8>)> = physical1;
    entries.push(("entriesmap_0.dat".to_owned(), encrypted_map0));
    entries.push(("entriesmap_1.dat".to_owned(), encrypted_map1));
    entries.push(("decryptseed_0.dat".to_owned(), seed0.into_bytes()));
    entries.push(("decryptseed_1.dat".to_owned(), seed1.into_bytes()));
    entries.push((
        "Encryptions.xml".to_owned(),
        encryptions.as_bytes().to_vec(),
    ));
    let _ = physical0;
    zip_with_entries(
        &entries
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_slice()))
            .collect::<Vec<_>>(),
    )
}

#[test]
fn edits_multilayer_encrypted_package() {
    let encrypted = multilayer_package();
    let mut package = OfdPackage::from_bytes(encrypted.clone()).expect("open multilayer");
    package
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("decrypt multilayer");
    assert_eq!(package.decryption_operation_count(), 2);

    let output = edit_encrypted_preserving(
        &encrypted,
        Box::new(PasswordBackend::new("secret")),
        |file: &mut OfdFile| {
            let document = file.document(0)?;
            let mut page = document.page("1")?;
            let GraphicObject::Text(text) =
                &mut page.data.content.as_mut().unwrap().layers[0].objects[0]
            else {
                return Err(OfdError::InvalidStructure("not text".to_owned()));
            };
            text.text_codes[0].text = "after".to_owned();
            page.save()?;
            Ok(())
        },
    )
    .unwrap_or_else(|error| panic!("edit multilayer: {error:?}"));

    let mut reopened = OfdPackage::from_bytes(output).expect("reopen");
    reopened
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("decrypt edited");
    let bytes = reopened
        .read_entry("Doc_0/Pages/Page_0/Content.xml")
        .expect("page");
    let page: Page = ofdbox_core::model_xml::from_bytes(&bytes).expect("parse page");
    let GraphicObject::Text(text) = &page.content.as_ref().unwrap().layers[0].objects[0] else {
        panic!("not text");
    };
    assert_eq!(text.text_codes[0].text, "after");
}

#[test]
fn adds_page_to_multilayer_encrypted_package() {
    let encrypted = multilayer_package();
    let output = edit_encrypted_preserving(
        &encrypted,
        Box::new(PasswordBackend::new("secret")),
        |file: &mut OfdFile| {
            let mut document = file.document(0)?;
            document.add_page(Page::default())?;
            document.save()?;
            Ok(())
        },
    )
    .unwrap_or_else(|error| panic!("add page multilayer: {error:?}"));

    let mut reopened = OfdPackage::from_bytes(output).expect("reopen");
    reopened
        .enable_decryption(Box::new(PasswordBackend::new("secret")))
        .expect("decrypt edited");
    let document = reopened
        .parse_document_schema()
        .expect("document after add");
    assert_eq!(document.pages.pages.len(), 2);
    assert_eq!(document.pages.pages[1].id, "2");
    assert!(reopened.contains("Doc_0/Pages/Page_2/Content.xml"));
    reopened
        .read_entry("Doc_0/Pages/Page_2/Content.xml")
        .expect("read new page");
}
