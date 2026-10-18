use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use ofdbox_core::{OfdError, OfdPackage, ReadOptions};
use ofdbox_crypto::PasswordBackend;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

#[test]
fn decrypts_hello_enc_document_lazily() {
    let _ = dotenvy::dotenv();
    let Some(dir) = std::env::var_os("OFDBOX_TESTDATA_DIR").map(PathBuf::from) else {
        return;
    };
    let base = dir.join("ofd-files").join("hello-enc");
    let bytes = zip_directory(&base);

    let mut package = OfdPackage::from_bytes_with_options(
        bytes.clone(),
        ReadOptions::LENIENT,
        Default::default(),
    )
    .expect("open encrypted package");
    assert!(matches!(
        package.parse_root(),
        Err(OfdError::EncryptionRequired)
    ));

    package
        .enable_decryption(Box::new(PasswordBackend::new("12345678")))
        .expect("enable decryption");
    assert!(package.decryption_enabled());

    let root = package.parse_root().expect("decrypted OFD.xml");
    assert_eq!(
        root.doc_bodies()
            .next()
            .unwrap()
            .doc_info()
            .unwrap()
            .doc_id(),
        Some("4228962e12c247e2984e79d8334ed10e")
    );
    let document = package.parse_document().expect("decrypted Document.xml");
    let pages = document.pages().expect("Pages");
    let page = pages.pages().next().expect("Page");
    package
        .parse_page_for(&page)
        .expect("decrypted Content.xml");

    package.clear_cache();
    let mut streamed = Vec::new();
    package
        .read_entry_to("Doc_0/Document.xml", &mut streamed)
        .expect("streamed read");
    assert!(streamed.starts_with(b"<?xml"));
    assert_eq!(package.cache_stats().plain_bytes, 0);

    let names = package.entry_names().expect("entry names");
    assert!(names.iter().any(|name| name == "OFD.xml"));
    assert!(names.iter().any(|name| name == "Doc_0/Document.xml"));
    assert!(package.cache_stats().plain_bytes > 0);

    package.close_decryption();
    assert!(!package.decryption_enabled());
    assert_eq!(package.cache_stats().plain_bytes, 0);

    let mut wrong =
        OfdPackage::from_bytes_with_options(bytes, ReadOptions::LENIENT, Default::default())
            .expect("open package");
    assert!(
        wrong
            .enable_decryption(Box::new(PasswordBackend::new("wrong-password")))
            .is_err()
    );
}

fn zip_directory(root: &Path) -> Vec<u8> {
    let mut files = Vec::new();
    visit(root, root, &mut files);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, path) in files {
        writer
            .start_file(name, SimpleFileOptions::default())
            .expect("start zip entry");
        writer
            .write_all(&std::fs::read(path).expect("read sample file"))
            .expect("write zip entry");
    }
    writer.finish().expect("finish zip").into_inner()
}

fn visit(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    for entry in std::fs::read_dir(dir).expect("read dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit(root, &path, files);
        } else {
            let name = path
                .strip_prefix(root)
                .expect("relative path")
                .to_string_lossy()
                .replace('\\', "/");
            files.push((name, path));
        }
    }
}
