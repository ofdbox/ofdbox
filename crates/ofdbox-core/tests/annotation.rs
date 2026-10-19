mod fixtures;

use ofdbox_core::{
    AnnotationsView, DiagnosticCode, OfdError, OfdPackage, PageAnnotView, ReadOptions,
};

const ANNOTATIONS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Annotations xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Page PageID="10">
    <ofd:FileLoc>Pages/Page_0/Annots/Page_0.xml</ofd:FileLoc>
  </ofd:Page>
</ofd:Annotations>"#;

const PAGE_ANNOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:PageAnnot xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Annot ID="20" Type="Stamp" Creator="seal" LastModDate="2024-01-02" Subtype="seal"
             Visible="true" Print="true" NoZoom="false" NoRotate="false" ReadOnly="true">
    <ofd:Remark>签章</ofd:Remark>
    <ofd:Parameters>
      <ofd:Parameter Name="org">示例单位</ofd:Parameter>
      <ofd:Parameter Name="person">张三</ofd:Parameter>
    </ofd:Parameters>
    <ofd:Appearance Boundary="0 0 100 50">
      <ofd:PathObject ID="21" Boundary="0 0 100 50" Stroke="true" Fill="false">
        <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 50 Z</ofd:AbbreviatedData>
      </ofd:PathObject>
    </ofd:Appearance>
  </ofd:Annot>
</ofd:PageAnnot>"#;

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
  <ofd:Annotations>Document/Annotations.xml</ofd:Annotations>
</ofd:Document>"#;

fn parse_annotations(xml: &str, options: ReadOptions) -> Result<AnnotationsView, OfdError> {
    AnnotationsView::from_xml(xml.as_bytes(), options)
}

fn parse_page_annot(xml: &str, options: ReadOptions) -> Result<PageAnnotView, OfdError> {
    PageAnnotView::from_xml(xml.as_bytes(), options)
}

#[test]
fn canonical_annotations_index_parses_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse_annotations(ANNOTATIONS, options).expect("canonical index");
        let page = view.pages().next().expect("Page");
        assert_eq!(page.page_id(), Some("10"));
        assert_eq!(page.file_loc(), Some("Pages/Page_0/Annots/Page_0.xml"));
    }
}

#[test]
fn annotations_writer_emits_canonical_index() {
    let view = parse_annotations(ANNOTATIONS, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:Annotations"));
    assert!(xml.contains(r#"xmlns:ofd="http://www.ofdspec.org/2016""#));
    assert!(xml.contains(r#"<ofd:Page PageID="10">"#));
    let reparsed =
        AnnotationsView::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    assert_eq!(
        reparsed.pages().next().unwrap().file_loc(),
        Some("Pages/Page_0/Annots/Page_0.xml")
    );
}

#[test]
fn missing_page_id_is_strict_error() {
    let missing = ANNOTATIONS.replace(r#" PageID="10""#, "");
    let view = parse_annotations(&missing, ReadOptions::LENIENT).expect("lenient");
    assert_eq!(view.pages().next().unwrap().page_id(), None);
    assert!(matches!(
        parse_annotations(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
    assert!(matches!(view.to_xml(), Err(OfdError::Conformance(_))));
}

#[test]
fn canonical_page_annot_parses_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse_page_annot(PAGE_ANNOT, options).expect("canonical PageAnnot");
        let annot = view.annots().next().expect("Annot");
        assert_eq!(annot.id(), Some("20"));
        assert_eq!(annot.annot_type(), Some("Stamp"));
        assert_eq!(annot.creator(), Some("seal"));
        assert_eq!(annot.last_mod_date(), Some("2024-01-02"));
        assert_eq!(annot.subtype(), Some("seal"));
        assert_eq!(annot.visible(), Some(true));
        assert_eq!(annot.no_zoom(), Some(false));
        assert_eq!(annot.read_only(), Some(true));
        assert_eq!(annot.remark(), Some("签章"));

        let parameters: Vec<_> = annot.parameters().collect();
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name(), Some("org"));
        assert_eq!(parameters[0].value(), "示例单位");
        assert_eq!(parameters[1].name(), Some("person"));

        let appearance = annot.appearance().expect("Appearance");
        assert_eq!(appearance.boundary(), Some("0 0 100 50"));
        let object = appearance.objects().next().expect("PathObject");
        assert_eq!(object.id(), Some("21"));
        assert_eq!(object.boundary(), Some("0 0 100 50"));
    }
}

#[test]
fn page_annot_writer_roundtrips() {
    let view = parse_page_annot(PAGE_ANNOT, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:PageAnnot"));
    let annot = xml.find("<ofd:Annot ").unwrap();
    let remark = xml.find("<ofd:Remark>").unwrap();
    let parameters = xml.find("<ofd:Parameters>").unwrap();
    let appearance = xml.find("<ofd:Appearance").unwrap();
    assert!(annot < remark && remark < parameters && parameters < appearance);
    assert!(xml.contains(r#"<ofd:Parameter Name="org">示例单位</ofd:Parameter>"#));
    assert!(xml.contains(r#"<ofd:PathObject ID="21""#));

    let reparsed =
        PageAnnotView::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    let annot = reparsed.annots().next().unwrap();
    assert_eq!(annot.annot_type(), Some("Stamp"));
    assert_eq!(
        annot.appearance().unwrap().objects().next().unwrap().id(),
        Some("21")
    );
}

#[test]
fn missing_creator_is_strict_error() {
    let missing = PAGE_ANNOT.replace(r#" Creator="seal""#, "");
    let view = parse_page_annot(&missing, ReadOptions::LENIENT).expect("lenient");
    assert_eq!(view.annots().next().unwrap().creator(), None);
    assert!(matches!(
        parse_page_annot(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn unknown_annot_type_is_strict_error() {
    let unknown = PAGE_ANNOT.replace(r#"Type="Stamp""#, r#"Type="Sticky""#);
    assert!(parse_page_annot(&unknown, ReadOptions::LENIENT).is_ok());
    match parse_page_annot(&unknown, ReadOptions::STRICT) {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::InvalidValue)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn lenient_normalizes_missing_2016_but_strict_rejects() {
    let dirty = PAGE_ANNOT.replace("http://www.ofdspec.org/2016", "http://www.ofdspec.org/");
    assert!(parse_page_annot(&dirty, ReadOptions::LENIENT).is_ok());
    match parse_page_annot(&dirty, ReadOptions::STRICT) {
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
fn package_resolves_root_relative_annotations() {
    let document = DOCUMENT.replace("Document/Annotations.xml", "Doc_0/Annots/Annotations.xml");
    let annotations =
        ANNOTATIONS.replace("Pages/Page_0/Annots/Page_0.xml", "Doc_0/Annots/Page_0.xml");
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", document.as_bytes()),
        ("Doc_0/Annots/Annotations.xml", annotations.as_bytes()),
        ("Doc_0/Annots/Page_0.xml", PAGE_ANNOT.as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");

    let index = package.parse_annotations().expect("Annotations");
    let page = index.pages().next().expect("Page");
    assert_eq!(page.page_id(), Some("10"));
    let file_loc = page.file_loc().expect("FileLoc").to_owned();
    let page_annot = package
        .parse_page_annot(&file_loc)
        .expect("PageAnnot should resolve root-relative");
    assert_eq!(page_annot.annots().next().unwrap().id(), Some("20"));
}

#[test]
fn package_resolves_annotations_and_page_annot() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Document/Annotations.xml", ANNOTATIONS.as_bytes()),
        (
            "Doc_0/Document/Pages/Page_0/Annots/Page_0.xml",
            PAGE_ANNOT.as_bytes(),
        ),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");

    let annotations = package.parse_annotations().expect("Annotations");
    let page = annotations.pages().next().expect("Page");
    assert_eq!(page.page_id(), Some("10"));
    let file_loc = page.file_loc().expect("FileLoc").to_owned();

    let page_annot = package
        .parse_page_annot(&file_loc)
        .expect("PageAnnot should resolve from annotations base");
    assert_eq!(page_annot.annots().next().unwrap().id(), Some("20"));
}
