mod fixtures;

use ofdbox_core::{
    DiagnosticCode, GraphicObjectKind, OfdError, OfdPackage, PageContentView, ReadOptions,
};

const PAGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Template TemplateID="5" ZOrder="Foreground"/>
  <ofd:PageRes>Pages/Page_0/PageRes.xml</ofd:PageRes>
  <ofd:Area>
    <ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox>
  </ofd:Area>
  <ofd:Content>
    <ofd:Layer ID="1" Type="Body">
      <ofd:TextObject ID="10" Boundary="0 0 100 20" Font="1" Size="12">
        <ofd:FillColor Value="0 0 0"/>
        <ofd:TextCode X="0" Y="0">Hello</ofd:TextCode>
      </ofd:TextObject>
      <ofd:PathObject ID="11" Boundary="0 0 10 10" Stroke="true">
        <ofd:AbbreviatedData>M 0 0 L 10 10</ofd:AbbreviatedData>
      </ofd:PathObject>
      <ofd:ImageObject ID="12" Boundary="0 0 10 10" ResourceID="3"/>
      <ofd:CompositeObject ID="13" Boundary="0 0 10 10" ResourceID="4"/>
      <ofd:PageBlock ID="14">
        <ofd:TextObject ID="15" Boundary="0 0 20 20" Font="1" Size="10">
          <ofd:TextCode X="0" Y="0">Hi</ofd:TextCode>
        </ofd:TextObject>
      </ofd:PageBlock>
    </ofd:Layer>
  </ofd:Content>
  <ofd:Actions>
    <ofd:Action Event="CLICK"><ofd:URI URI="https://example.com"/></ofd:Action>
  </ofd:Actions>
</ofd:Page>"#;

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
    <ofd:MaxUnitID>20</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;

fn parse(xml: &str, options: ReadOptions) -> Result<PageContentView, OfdError> {
    PageContentView::from_xml(xml.as_bytes(), options)
}

#[test]
fn canonical_page_parses_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse(PAGE, options).expect("canonical page should parse");
        let template = view.templates().next().expect("Template");
        assert_eq!(template.template_id(), Some("5"));
        assert_eq!(template.z_order(), Some("Foreground"));
        assert_eq!(
            view.page_res().collect::<Vec<_>>(),
            ["Pages/Page_0/PageRes.xml"]
        );
        assert_eq!(view.area().unwrap().physical_box(), Some("0 0 210 297"));

        let layer = view.content().unwrap().layers().next().expect("Layer");
        assert_eq!(layer.id(), Some("1"));
        assert_eq!(layer.layer_type(), Some("Body"));
        let objects: Vec<_> = layer.objects().collect();
        assert_eq!(objects.len(), 5);
        assert_eq!(objects[0].kind(), GraphicObjectKind::Text);
        assert_eq!(objects[0].id(), Some("10"));
        assert_eq!(objects[0].attribute("Font"), Some("1"));
        assert_eq!(objects[0].attribute("Size"), Some("12"));
        assert_eq!(objects[1].kind(), GraphicObjectKind::Path);
        assert_eq!(objects[2].kind(), GraphicObjectKind::Image);
        assert_eq!(objects[2].attribute("ResourceID"), Some("3"));
        assert_eq!(objects[3].kind(), GraphicObjectKind::Composite);
        assert_eq!(objects[4].kind(), GraphicObjectKind::PageBlock);
        assert_eq!(objects[4].children().count(), 1);

        let action = view.actions().unwrap().actions().next().expect("Action");
        assert_eq!(action.event(), Some("CLICK"));
        assert_eq!(action.uri().unwrap().uri(), Some("https://example.com"));
    }
}

#[test]
fn writer_emits_canonical_page_and_roundtrips() {
    let view = parse(PAGE, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer should succeed");
    assert!(xml.contains(r#"xmlns:ofd="http://www.ofdspec.org/2016""#));
    assert!(xml.contains("<ofd:TextObject"));
    assert!(xml.contains("<ofd:TextCode"));
    assert!(xml.contains("Hello"));

    let reparsed = PageContentView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
        .expect("canonical output should pass strict parsing");
    assert_eq!(
        reparsed
            .content()
            .unwrap()
            .layers()
            .next()
            .unwrap()
            .objects()
            .count(),
        5
    );
}

#[test]
fn missing_graphic_required_attributes_are_strict_errors() {
    let missing = PAGE.replace(r#" Font="1" Size="12""#, "");
    let view = parse(&missing, ReadOptions::LENIENT).expect("lenient should accept");
    assert!(view.content().is_some());
    assert!(matches!(
        parse(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
    assert!(matches!(view.to_xml(), Err(OfdError::Conformance(_))));
}

#[test]
fn unknown_graphic_object_is_strict_error() {
    let unknown = PAGE.replace(
        "    </ofd:Layer>",
        "      <ofd:VendorObject ID=\"99\" Boundary=\"0 0 1 1\"/>\n    </ofd:Layer>",
    );
    assert!(parse(&unknown, ReadOptions::LENIENT).is_ok());
    match parse(&unknown, ReadOptions::STRICT) {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::UnexpectedElement)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn lenient_normalizes_missing_2016_but_strict_rejects() {
    let dirty = PAGE.replace("http://www.ofdspec.org/2016", "http://www.ofdspec.org/");
    assert!(parse(&dirty, ReadOptions::LENIENT).is_ok());
    match parse(&dirty, ReadOptions::STRICT) {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::NamespaceVariant)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn package_parses_page_through_base_loc() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Document.xml", DOCUMENT.as_bytes()),
        ("Pages/Page_0/Content.xml", PAGE.as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    let document = package.parse_document().expect("Document");
    let page = document
        .pages()
        .expect("Pages")
        .pages()
        .next()
        .expect("Page");
    let content = package.parse_page_for(&page).expect("Content");
    assert_eq!(
        content.content().unwrap().layers().next().unwrap().id(),
        Some("1")
    );
}
