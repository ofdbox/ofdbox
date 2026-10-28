mod fixtures;

use ofdbox_core::model::integrity::DocEntries;
use ofdbox_core::{OfdError, OfdPackage, ReadOptions};

const ENTRIES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:DocEntries xmlns:ofd="http://www.ofdspec.org/2016" ID="1" CreatorName="OFDBox"
                Version="1.0" CreationDate="2024-01-02">
  <ofd:FileList>
    <ofd:File ID="f1" FileLoc="/OFD.xml"/>
    <ofd:File ID="f2" FileLoc="/Doc_0/Document.xml"/>
  </ofd:FileList>
  <ofd:SignedValueLoc>/Integrity/SignedValue.dat</ofd:SignedValueLoc>
</ofd:DocEntries>"#;

#[test]
fn canonical_doc_entries_parse_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = DocEntries::from_xml(ENTRIES.as_bytes(), options).expect("DocEntries");
        assert_eq!(view.id(), Some("1"));
        assert_eq!(view.creator_name(), Some("OFDBox"));
        assert_eq!(view.version(), Some("1.0"));
        assert_eq!(view.creation_date(), Some("2024-01-02"));
        let files: Vec<_> = view.files().collect();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].id(), Some("f1"));
        assert_eq!(files[0].file_loc(), Some("/OFD.xml"));
        assert_eq!(view.signed_value_loc(), Some("/Integrity/SignedValue.dat"));
    }
}

#[test]
fn writer_emits_canonical_doc_entries() {
    let view = DocEntries::from_xml(ENTRIES.as_bytes(), ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:DocEntries"));
    assert!(xml.contains(r#"CreatorName="OFDBox""#));
    assert!(xml.contains("<ofd:FileList>"));
    assert!(xml.contains(r#"FileLoc="/OFD.xml""#));
    assert!(xml.contains("<ofd:SignedValueLoc>/Integrity/SignedValue.dat</ofd:SignedValueLoc>"));
    let reparsed =
        DocEntries::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    assert_eq!(reparsed.files().count(), 2);
}

#[test]
fn missing_id_is_strict_error() {
    let missing = ENTRIES.replace(r#" ID="1""#, "");
    assert!(DocEntries::from_xml(missing.as_bytes(), ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        DocEntries::from_xml(missing.as_bytes(), ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn package_parses_ofd_entries() {
    let bytes = fixtures::zip_with_entry("OFDEntries.xml", ENTRIES.as_bytes());
    let mut package = OfdPackage::from_bytes(bytes).expect("package");
    let view = package.parse_ofd_entries().expect("OFDEntries");
    assert_eq!(view.file_list.files.len(), 2);
}

#[test]
fn file_loc_must_be_attribute() {
    let with_child = ENTRIES.replace(
        r#"<ofd:File ID="f1" FileLoc="/OFD.xml"/>"#,
        r#"<ofd:File ID="f1"><ofd:FileLoc>/OFD.xml</ofd:FileLoc></ofd:File>"#,
    );
    assert!(matches!(
        DocEntries::from_xml(with_child.as_bytes(), ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));

    let missing = ENTRIES.replace(r#" FileLoc="/OFD.xml""#, "");
    assert!(matches!(
        DocEntries::from_xml(missing.as_bytes(), ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}
