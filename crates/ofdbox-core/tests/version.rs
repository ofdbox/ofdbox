mod fixtures;

use ofdbox_core::{DocVersionView, OfdError, OfdPackage, ReadOptions};

const DOC_VERSION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:DocVersion xmlns:ofd="http://www.ofdspec.org/2016" ID="v2" Version="2.0" Name="第二版" CreationDate="2024-01-02">
  <ofd:FileList>
    <ofd:File ID="f1">Doc_0/Document.xml</ofd:File>
    <ofd:File ID="f2">Doc_0/Pages/Page_0/Content.xml</ofd:File>
  </ofd:FileList>
  <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
</ofd:DocVersion>"#;

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>doc-1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
    <ofd:Versions>
      <ofd:Version ID="v1" Index="1" Current="true" BaseLoc="Versions/Version_0/DocVersion.xml"/>
    </ofd:Versions>
  </ofd:DocBody>
</ofd:OFD>"#;

fn parse(xml: &str, options: ReadOptions) -> Result<DocVersionView, OfdError> {
    DocVersionView::from_xml(xml.as_bytes(), options)
}

#[test]
fn canonical_doc_version_parses_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse(DOC_VERSION, options).expect("canonical DocVersion");
        assert_eq!(view.id(), Some("v2"));
        assert_eq!(view.version(), Some("2.0"));
        assert_eq!(view.name(), Some("第二版"));
        assert_eq!(view.creation_date(), Some("2024-01-02"));
        assert_eq!(view.doc_root(), Some("Doc_0/Document.xml"));

        let files: Vec<_> = view.files().collect();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].id(), Some("f1"));
        assert_eq!(files[0].location(), "Doc_0/Document.xml");
        assert_eq!(files[1].id(), Some("f2"));
    }
}

#[test]
fn writer_emits_canonical_doc_version() {
    let view = parse(DOC_VERSION, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:DocVersion"));
    let file_list = xml.find("<ofd:FileList>").unwrap();
    let doc_root = xml.find("<ofd:DocRoot>").unwrap();
    assert!(file_list < doc_root);
    assert!(xml.contains(r#"<ofd:File ID="f1">Doc_0/Document.xml</ofd:File>"#));

    let reparsed =
        DocVersionView::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    assert_eq!(reparsed.files().count(), 2);
}

#[test]
fn missing_id_is_strict_error() {
    let missing = DOC_VERSION.replace(r#" ID="v2""#, "");
    let view = parse(&missing, ReadOptions::LENIENT).expect("lenient");
    assert_eq!(view.id(), None);
    assert!(matches!(
        parse(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn lenient_normalizes_missing_2016_but_strict_rejects() {
    let dirty = DOC_VERSION.replace("http://www.ofdspec.org/2016", "http://www.ofdspec.org/");
    assert!(parse(&dirty, ReadOptions::LENIENT).is_ok());
    match parse(&dirty, ReadOptions::STRICT) {
        Err(OfdError::Conformance(_)) => {}
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn package_resolves_doc_version() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Versions/Version_0/DocVersion.xml", DOC_VERSION.as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    let version = package
        .parse_doc_version("Versions/Version_0/DocVersion.xml")
        .expect("DocVersion");
    assert_eq!(version.id(), Some("v2"));
    assert_eq!(version.files().count(), 2);
}
