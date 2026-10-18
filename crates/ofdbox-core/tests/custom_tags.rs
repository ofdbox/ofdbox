mod fixtures;

use ofdbox_core::{CustomTagsView, DiagnosticCode, OfdError, OfdPackage, ReadOptions};

const CUSTOM_TAGS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:CustomTags xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CustomTag TypeID="ofdls">
    <ofd:SchemaLoc>Tags/ofdls.xsd</ofd:SchemaLoc>
    <ofd:FileLoc>Tags/ofdls.xml</ofd:FileLoc>
  </ofd:CustomTag>
  <ofd:CustomTag TypeID="">
    <ofd:FileLoc>Tags/default.xml</ofd:FileLoc>
  </ofd:CustomTag>
</ofd:CustomTags>"#;

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>doc-1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>20</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="10" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
  <ofd:CustomTags>Tags/CustomTags.xml</ofd:CustomTags>
</ofd:Document>"#;

fn parse(xml: &str, options: ReadOptions) -> Result<CustomTagsView, OfdError> {
    CustomTagsView::from_xml(xml.as_bytes(), options)
}

#[test]
fn canonical_custom_tags_parse_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse(CUSTOM_TAGS, options).expect("canonical CustomTags");
        let tags: Vec<_> = view.custom_tags().collect();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].type_id(), Some("ofdls"));
        assert_eq!(tags[0].schema_loc(), Some("Tags/ofdls.xsd"));
        assert_eq!(tags[0].file_loc(), Some("Tags/ofdls.xml"));
        assert_eq!(tags[1].type_id(), Some(""));
    }
}

#[test]
fn namespace_attribute_variant_is_accepted_in_strict_mode() {
    let variant = CUSTOM_TAGS.replace(
        r#"<ofd:CustomTag TypeID="ofdls">"#,
        r#"<ofd:CustomTag NameSpace="ofdls">"#,
    );
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse(&variant, options).expect("NameSpace variant should parse");
        assert_eq!(view.custom_tags().next().unwrap().type_id(), Some("ofdls"));
    }
}

#[test]
fn missing_type_identifier_is_strict_error() {
    let missing = CUSTOM_TAGS.replace(r#" TypeID="ofdls""#, "");
    let view = parse(&missing, ReadOptions::LENIENT).expect("lenient");
    assert_eq!(view.custom_tags().next().unwrap().type_id(), None);
    match parse(&missing, ReadOptions::STRICT) {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::MissingRequired)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn writer_emits_canonical_custom_tags() {
    let view = parse(CUSTOM_TAGS, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:CustomTags"));
    assert!(xml.contains(r#"<ofd:CustomTag TypeID="ofdls">"#));
    assert!(xml.contains("<ofd:SchemaLoc>Tags/ofdls.xsd</ofd:SchemaLoc>"));
    let reparsed =
        CustomTagsView::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    assert_eq!(reparsed.custom_tags().count(), 2);
}

#[test]
fn package_resolves_custom_tags() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Tags/CustomTags.xml", CUSTOM_TAGS.as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    let tags = package.parse_custom_tags().expect("CustomTags");
    assert_eq!(tags.custom_tags().next().unwrap().type_id(), Some("ofdls"));
}
