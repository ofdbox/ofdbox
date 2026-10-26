mod fixtures;

use ofdbox_core::model::extensions::Extensions;
use ofdbox_core::{OfdError, OfdPackage, ReadOptions};

const EXTENSIONS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Extensions xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Extension AppName="ExampleApp" Company="Example" AppVersion="1.0" Date="2024-01-02T03:04:05Z" RefId="10">
    <ofd:Property Name="author" Type="string">张三</ofd:Property>
    <ofd:Data>
      <ofd:Item key="a">1</ofd:Item>
      <ofd:Item key="b">2</ofd:Item>
    </ofd:Data>
    <ofd:ExtendData>Extensions/data.xml</ofd:ExtendData>
  </ofd:Extension>
</ofd:Extensions>"#;

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
  <ofd:Extensions>Extensions.xml</ofd:Extensions>
</ofd:Document>"#;

fn parse(xml: &str, options: ReadOptions) -> Result<Extensions, OfdError> {
    Extensions::from_xml(xml.as_bytes(), options)
}

#[test]
fn canonical_extensions_parse_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse(EXTENSIONS, options).expect("canonical Extensions");
        let extension = view.extensions().next().expect("Extension");
        assert_eq!(extension.app_name(), Some("ExampleApp"));
        assert_eq!(extension.company(), Some("Example"));
        assert_eq!(extension.app_version(), Some("1.0"));
        assert_eq!(extension.date(), Some("2024-01-02T03:04:05Z"));
        assert_eq!(extension.ref_id(), Some("10"));

        let property = extension.properties().next().expect("Property");
        assert_eq!(property.name(), Some("author"));
        assert_eq!(property.value_type(), Some("string"));
        assert_eq!(property.value(), "张三");

        assert_eq!(extension.data().count(), 1);
        assert_eq!(
            extension.extend_datas().collect::<Vec<_>>(),
            ["Extensions/data.xml"]
        );
    }
}

#[test]
fn writer_preserves_data_subtree_and_roundtrips() {
    let view = parse(EXTENSIONS, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:Extensions"));
    assert!(xml.contains(r#"<ofd:Extension AppName="ExampleApp""#));
    assert!(xml.contains(r#"<ofd:Property Name="author" Type="string">张三</ofd:Property>"#));
    assert!(xml.contains("<ofd:Data>"));
    assert!(xml.contains(r#"<ofd:Item key="a">1</ofd:Item>"#));
    assert!(xml.contains("<ofd:ExtendData>Extensions/data.xml</ofd:ExtendData>"));

    let reparsed =
        Extensions::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    let extension = reparsed.extensions().next().unwrap();
    assert_eq!(extension.data().count(), 1);
    assert_eq!(extension.properties().count(), 1);
}

#[test]
fn missing_app_name_is_strict_error() {
    let missing = EXTENSIONS.replace(r#" AppName="ExampleApp""#, "");
    let view = parse(&missing, ReadOptions::LENIENT).expect("lenient");
    assert_eq!(view.extensions().next().unwrap().app_name(), None);
    assert!(matches!(
        parse(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn missing_ref_id_is_strict_error() {
    let missing = EXTENSIONS.replace(r#" RefId="10""#, "");
    assert!(parse(&missing, ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        parse(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn empty_extension_is_strict_error() {
    let empty = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Extensions xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Extension AppName="ExampleApp" RefId="10"/>
</ofd:Extensions>"#;
    assert!(parse(empty, ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        parse(empty, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn package_resolves_extensions() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Extensions.xml", EXTENSIONS.as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    let extensions = package.parse_extensions().expect("Extensions");
    assert_eq!(
        extensions.extensions().next().unwrap().app_name(),
        Some("ExampleApp")
    );
}
