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
fn missing_root_entry_is_reported() {
    let bytes = fixtures::zip_with_entry("Other.xml", b"<x/>");
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    match package.parse_root() {
        Err(OfdError::MissingEntry(name)) => assert_eq!(name, "OFD.xml"),
        other => panic!("expected MissingEntry, got {other:?}"),
    }
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
