mod fixtures;

use std::io::Write;

use ofdbox_core::{OfdError, OfdPackage, ReadOptions};

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>doc-1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>1</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;

#[test]
fn from_bytes_parses_root() {
    let bytes = fixtures::zip_with_entry("OFD.xml", ROOT.as_bytes());
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    assert!(package.contains("OFD.xml"));
    assert_eq!(package.entry_count(), 1);
    let view = package.parse_root().expect("parse root");
    assert_eq!(view.version(), Some("1.0"));
    assert_eq!(
        view.doc_bodies().next().unwrap().doc_root(),
        Some("Document.xml")
    );
}

#[test]
fn open_file_parses_root() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.ofd");
    let bytes = fixtures::zip_with_entry("OFD.xml", ROOT.as_bytes());
    std::fs::File::create(&path)
        .unwrap()
        .write_all(&bytes)
        .unwrap();

    let mut package = OfdPackage::open(&path).expect("open package");
    let view = package.parse_root().expect("parse root");
    assert_eq!(view.doc_type(), Some("OFD"));
}

#[test]
fn backslash_entry_names_are_resolved() {
    let root = ROOT.replace(
        "<ofd:DocRoot>Document.xml</ofd:DocRoot>",
        "<ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>",
    );
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", root.as_bytes()),
        ("Doc_0\\Document.xml", DOCUMENT.as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    assert!(package.contains("Doc_0/Document.xml"));
    assert!(package.contains("doc_0/document.xml"));
    let names = package.entry_names().expect("entry names");
    assert!(names.iter().any(|name| name == "OFD.xml"));
    assert!(names.iter().any(|name| name == "Doc_0/Document.xml"));
    let document = package.parse_document().expect("document");
    assert!(document.pages().is_some());
    assert!(
        !package
            .read_entry("Doc_0/Document.xml")
            .expect("read entry")
            .is_empty()
    );
}

#[test]
fn missing_root_entry_is_reported() {
    let bytes = fixtures::zip_with_entry("Other.xml", b"<x/>");
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    match package.parse_root() {
        Err(OfdError::MissingEntry(name)) => assert_eq!(name, "OFD.xml"),
        other => panic!("expected MissingEntry, got {other:?}"),
    }
}

#[test]
fn stored_entry_with_bad_crc_is_readable() {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("data.txt", options).expect("entry");
    writer.write_all(b"AAAA").expect("write");
    let mut bytes = writer.finish().expect("finish").into_inner();
    let position = bytes
        .windows(4)
        .position(|window| window == b"AAAA")
        .expect("payload");
    bytes[position..position + 4].copy_from_slice(b"BBBB");

    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    assert_eq!(
        &package.read_entry("data.txt").expect("read entry")[..],
        b"BBBB"
    );
}

#[test]
fn invalid_zip_is_reported() {
    match OfdPackage::from_bytes(vec![1, 2, 3, 4]) {
        Err(OfdError::Zip(_)) => {}
        other => panic!("expected Zip error, got {other:?}"),
    }
}

#[test]
fn cache_tracks_and_clears() {
    let bytes = fixtures::zip_with_entry("OFD.xml", ROOT.as_bytes());
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    package.parse_root().unwrap();
    let stats = package.cache_stats();
    assert!(stats.entry_bytes > 0);
    assert!(stats.xml_bytes > 0);

    package.clear_cache();
    let stats = package.cache_stats();
    assert_eq!(stats.entry_bytes, 0);
    assert_eq!(stats.xml_bytes, 0);
}

#[test]
fn strict_package_rejects_dirty_root() {
    let dirty = ROOT.replace("http://www.ofdspec.org/2016", "http://www.ofdspec.org/");
    let bytes = fixtures::zip_with_entry("OFD.xml", dirty.as_bytes());
    let mut package =
        OfdPackage::from_bytes_with_options(bytes, ReadOptions::STRICT, Default::default())
            .expect("open package");
    assert!(matches!(
        package.parse_root(),
        Err(OfdError::Conformance(_))
    ));
}
