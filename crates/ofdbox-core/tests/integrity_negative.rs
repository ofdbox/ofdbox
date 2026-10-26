use ofdbox_core::model::integrity::DocEntries;
use ofdbox_core::{OfdError, ReadOptions};

const ENTRIES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:DocEntries xmlns:ofd="http://www.ofdspec.org/2016" ID="1" CreatorName="OFDBox"
                Version="1.0" CreationDate="2024-01-02">
  <ofd:FileList>
    <ofd:File ID="f1" FileLoc="/OFD.xml"/>
    <ofd:File ID="f2" FileLoc="/Doc_0/Document.xml"/>
  </ofd:FileList>
  <ofd:SignedValueLoc>/Integrity/SignedValue.dat</ofd:SignedValueLoc>
</ofd:DocEntries>"#;

fn assert_strict_rejects(input: &str) {
    assert!(DocEntries::from_xml(input.as_bytes(), ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        DocEntries::from_xml(input.as_bytes(), ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn missing_signed_value_loc_is_strict_error() {
    let input = ENTRIES.replace(
        "<ofd:SignedValueLoc>/Integrity/SignedValue.dat</ofd:SignedValueLoc>",
        "",
    );
    assert_strict_rejects(&input);
}

#[test]
fn relative_file_loc_is_strict_error() {
    let input = ENTRIES.replace(r#"FileLoc="/OFD.xml""#, r#"FileLoc="OFD.xml""#);
    assert_strict_rejects(&input);
}

#[test]
fn duplicate_file_loc_is_strict_error() {
    let input = ENTRIES.replace(r#"FileLoc="/Doc_0/Document.xml""#, r#"FileLoc="/OFD.xml""#);
    assert_strict_rejects(&input);
}

#[test]
fn duplicate_file_id_is_strict_error() {
    let input = ENTRIES.replace(r#"ID="f2""#, r#"ID="f1""#);
    assert_strict_rejects(&input);
}
