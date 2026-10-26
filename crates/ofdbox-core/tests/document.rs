mod fixtures;

use ofdbox_core::model::document::Document;
use ofdbox_core::{DiagnosticCode, OfdError, OfdPackage, ReadOptions};

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>191</ofd:MaxUnitID>
    <ofd:PageArea>
      <ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox>
      <ofd:ApplicationBox>0 0 210 297</ofd:ApplicationBox>
    </ofd:PageArea>
    <ofd:PublicRes>PublicRes.xml</ofd:PublicRes>
    <ofd:DocumentRes>DocumentRes.xml</ofd:DocumentRes>
    <ofd:TemplatePage ID="5" Name="tpl" ZOrder="Background" BaseLoc="Temps/Temp_0.xml"/>
    <ofd:DefaultCS>7</ofd:DefaultCS>
  </ofd:CommonData>
  <ofd:Pages>
    <ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/>
    <ofd:Page ID="2" BaseLoc="Pages/Page_1/Content.xml"/>
  </ofd:Pages>
  <ofd:Permissions>
    <ofd:Edit>true</ofd:Edit>
    <ofd:Annot>false</ofd:Annot>
    <ofd:Print Printable="true" Copies="2"/>
    <ofd:ValidPeriod StartDate="2026-01-01T00:00:00" EndDate="2027-01-01T00:00:00"/>
  </ofd:Permissions>
  <ofd:VPreferences>
    <ofd:PageMode>UseOutlines</ofd:PageMode>
    <ofd:PageLayout>OneColumn</ofd:PageLayout>
    <ofd:TabDisplay>DocTitle</ofd:TabDisplay>
    <ofd:HideToolbar>false</ofd:HideToolbar>
    <ofd:ZoomMode>FitWidth</ofd:ZoomMode>
  </ofd:VPreferences>
  <ofd:Bookmarks>
    <ofd:Bookmark Name="title">
      <ofd:Dest Type="XYZ" PageID="1" Left="0" Top="0"/>
    </ofd:Bookmark>
  </ofd:Bookmarks>
  <ofd:Annotations>Annots/Annotations.xml</ofd:Annotations>
  <ofd:CustomTags>Tags/CustomTags.xml</ofd:CustomTags>
  <ofd:Attachments>Attachments.xml</ofd:Attachments>
  <ofd:Extensions>Extensions.xml</ofd:Extensions>
</ofd:Document>"#;

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>doc-1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

fn parse(xml: &str, options: ReadOptions) -> Result<Document, OfdError> {
    Document::from_xml(xml.as_bytes(), options)
}

#[test]
fn canonical_document_parses_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse(DOCUMENT, options).expect("canonical Document should parse");
        let common = view.common_data().expect("CommonData");
        assert_eq!(common.max_unit_id(), Some("191"));
        let area = common.page_area().expect("PageArea");
        assert_eq!(area.physical_box(), Some("0 0 210 297"));
        assert_eq!(area.application_box(), Some("0 0 210 297"));
        assert_eq!(common.public_res().collect::<Vec<_>>(), ["PublicRes.xml"]);
        assert_eq!(
            common.document_res().collect::<Vec<_>>(),
            ["DocumentRes.xml"]
        );
        let template = common.template_pages().next().expect("TemplatePage");
        assert_eq!(template.id(), Some("5"));
        assert_eq!(template.name(), Some("tpl"));
        assert_eq!(template.z_order(), Some("Background"));
        assert_eq!(template.base_loc(), Some("Temps/Temp_0.xml"));
        assert_eq!(common.default_cs(), Some("7"));

        let pages: Vec<_> = view.pages().expect("Pages").pages().collect();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].id(), Some("1"));
        assert_eq!(pages[0].base_loc(), Some("Pages/Page_0/Content.xml"));

        let permissions = view.permissions().expect("Permissions");
        assert_eq!(permissions.edit(), Some(true));
        assert_eq!(permissions.annot(), Some(false));
        let print = permissions.print().expect("Print");
        assert_eq!(print.printable(), Some(true));
        assert_eq!(print.copies(), Some(2));
        let period = permissions.valid_period().expect("ValidPeriod");
        assert_eq!(period.start_date(), Some("2026-01-01T00:00:00"));
        assert_eq!(period.end_date(), Some("2027-01-01T00:00:00"));

        let prefs = view.v_preferences().expect("VPreferences");
        assert_eq!(prefs.page_mode(), Some("UseOutlines"));
        assert_eq!(prefs.page_layout(), Some("OneColumn"));
        assert_eq!(prefs.tab_display(), Some("DocTitle"));
        assert_eq!(prefs.hide_toolbar(), Some(false));
        assert_eq!(prefs.zoom_mode(), Some("FitWidth"));

        let bookmark = view
            .bookmarks()
            .expect("Bookmarks")
            .bookmarks()
            .next()
            .unwrap();
        assert_eq!(bookmark.name(), Some("title"));
        let dest = bookmark.dest().expect("Dest");
        assert_eq!(dest.dest_type(), Some("XYZ"));
        assert_eq!(dest.page_id(), Some("1"));

        assert_eq!(view.annotations(), Some("Annots/Annotations.xml"));
        assert_eq!(view.custom_tags(), Some("Tags/CustomTags.xml"));
        assert_eq!(view.attachments(), Some("Attachments.xml"));
        assert_eq!(view.extensions(), Some("Extensions.xml"));
    }
}

#[test]
fn lenient_accepts_unordered_children_but_strict_rejects() {
    let unordered = DOCUMENT
        .replace(
            "<ofd:MaxUnitID>191</ofd:MaxUnitID>
    <ofd:PageArea>",
            "<ofd:PageArea>",
        )
        .replace(
            "</ofd:PageArea>
    <ofd:PublicRes>",
            "</ofd:PageArea>
    <ofd:MaxUnitID>191</ofd:MaxUnitID>
    <ofd:PublicRes>",
        );
    assert!(parse(&unordered, ReadOptions::LENIENT).is_ok());
    match parse(&unordered, ReadOptions::STRICT) {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::OutOfOrder)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn lenient_normalizes_missing_2016_but_strict_rejects() {
    let dirty = DOCUMENT.replace("http://www.ofdspec.org/2016", "http://www.ofdspec.org/");
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
fn outlines_and_actions_parse_and_write() {
    let with_outlines = DOCUMENT.replace(
        "  <ofd:Permissions>",
        r#"  <ofd:Outlines>
    <ofd:OutlineElem Title="第一章" Count="1" Expanded="true">
      <ofd:Actions>
        <ofd:Action Event="CLICK">
          <ofd:Goto><ofd:Dest Type="Fit" PageID="1"/></ofd:Goto>
        </ofd:Action>
      </ofd:Actions>
      <ofd:OutlineElem Title="1.1"/>
    </ofd:OutlineElem>
  </ofd:Outlines>
  <ofd:Permissions>"#,
    );
    let with_actions = with_outlines.replace(
        "  <ofd:VPreferences>",
        r#"  <ofd:Actions>
    <ofd:Action Event="DO">
      <ofd:URI URI="https://example.com"/>
    </ofd:Action>
  </ofd:Actions>
  <ofd:VPreferences>"#,
    );

    let view = parse(&with_actions, ReadOptions::LENIENT).expect("lenient should accept");
    let outline = view
        .outlines()
        .expect("Outlines")
        .outline_elems()
        .next()
        .expect("OutlineElem");
    assert_eq!(outline.title(), Some("第一章"));
    assert_eq!(outline.count(), Some(1));
    assert_eq!(outline.expanded(), Some(true));
    let action = outline
        .actions()
        .expect("Actions")
        .actions()
        .next()
        .expect("Action");
    assert_eq!(action.event(), Some("CLICK"));
    assert!(action.goto().expect("Goto").dest().is_some());
    assert_eq!(outline.children().count(), 1);
    let document_action = view
        .actions()
        .expect("Document Actions")
        .actions()
        .next()
        .expect("Action");
    assert_eq!(
        document_action.uri().expect("URI").uri(),
        Some("https://example.com")
    );

    let strict = parse(&with_actions, ReadOptions::STRICT).expect("strict should accept");
    let xml = strict.to_xml().expect("strict writer");
    let reparsed = Document::from_xml(xml.as_bytes(), ReadOptions::STRICT)
        .expect("strict output should roundtrip");
    assert!(reparsed.outlines().is_some());
    assert!(reparsed.actions().is_some());
}

#[test]
fn action_without_event_is_strict_error() {
    let with_outlines = DOCUMENT.replace(
        "  <ofd:Permissions>",
        r#"  <ofd:Outlines>
    <ofd:OutlineElem Title="第一章">
      <ofd:Actions><ofd:Action><ofd:Goto><ofd:Dest Type="Fit" PageID="1"/></ofd:Goto></ofd:Action></ofd:Actions>
    </ofd:OutlineElem>
  </ofd:Outlines>
  <ofd:Permissions>"#,
    );
    let view = parse(&with_outlines, ReadOptions::LENIENT).expect("lenient should accept");
    assert!(view.outlines().is_some());
    assert!(matches!(
        parse(&with_outlines, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
    assert!(matches!(view.to_xml(), Err(OfdError::Conformance(_))));
}

#[test]
fn action_region_parses_in_strict_mode() {
    let with_region = DOCUMENT.replace(
        "  <ofd:VPreferences>",
        r#"  <ofd:Actions>
    <ofd:Action Event="CLICK">
      <ofd:Region><ofd:Area Start="0 0"><ofd:Move Point1="0 0"/></ofd:Area></ofd:Region>
      <ofd:URI URI="https://example.com"/>
    </ofd:Action>
  </ofd:Actions>
  <ofd:VPreferences>"#,
    );
    let view = parse(&with_region, ReadOptions::STRICT).expect("region should be modeled");
    let region = view
        .actions()
        .unwrap()
        .actions()
        .next()
        .unwrap()
        .region()
        .expect("Region");
    assert_eq!(region.areas().next().unwrap().start(), Some("0 0"));
    view.to_xml().expect("strict writer");
}

#[test]
fn strict_rejects_unknown_top_level_element() {
    let with_forms = DOCUMENT.replace("  <ofd:Annotations>", "  <ofd:Forms/><ofd:Annotations>");
    assert!(parse(&with_forms, ReadOptions::LENIENT).is_ok());
    match parse(&with_forms, ReadOptions::STRICT) {
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
fn missing_required_sections_are_reported() {
    let missing = DOCUMENT.replace(
        "  <ofd:CommonData>\n    <ofd:MaxUnitID>191</ofd:MaxUnitID>\n    <ofd:PageArea>\n      <ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox>\n      <ofd:ApplicationBox>0 0 210 297</ofd:ApplicationBox>\n    </ofd:PageArea>\n    <ofd:PublicRes>PublicRes.xml</ofd:PublicRes>\n    <ofd:DocumentRes>DocumentRes.xml</ofd:DocumentRes>\n    <ofd:TemplatePage ID=\"5\" Name=\"tpl\" ZOrder=\"Background\" BaseLoc=\"Temps/Temp_0.xml\"/>\n    <ofd:DefaultCS>7</ofd:DefaultCS>\n  </ofd:CommonData>\n",
        "",
    );
    let view = parse(&missing, ReadOptions::LENIENT).expect("lenient should accept");
    assert!(view.common_data().is_none());
    assert!(matches!(view.to_xml(), Err(OfdError::Conformance(_))));
    assert!(matches!(
        parse(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn writer_emits_canonical_document_order() {
    let view = parse(DOCUMENT, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer should succeed");
    assert!(xml.contains(r#"xmlns:ofd="http://www.ofdspec.org/2016""#));
    let common = xml.find("<ofd:CommonData>").unwrap();
    let pages = xml.find("<ofd:Pages>").unwrap();
    let permissions = xml.find("<ofd:Permissions>").unwrap();
    let prefs = xml.find("<ofd:VPreferences>").unwrap();
    let bookmarks = xml.find("<ofd:Bookmarks>").unwrap();
    let annotations = xml.find("<ofd:Annotations>").unwrap();
    let custom_tags = xml.find("<ofd:CustomTags>").unwrap();
    let attachments = xml.find("<ofd:Attachments>").unwrap();
    let extensions = xml.find("<ofd:Extensions>").unwrap();
    assert!(common < pages);
    assert!(pages < permissions);
    assert!(permissions < prefs);
    assert!(prefs < bookmarks);
    assert!(bookmarks < annotations);
    assert!(annotations < custom_tags);
    assert!(custom_tags < attachments);
    assert!(attachments < extensions);

    let reparsed = Document::from_xml(xml.as_bytes(), ReadOptions::STRICT)
        .expect("canonical output should pass strict parsing");
    assert_eq!(
        reparsed.pages().unwrap().pages().next().unwrap().id(),
        Some("1")
    );
}

#[test]
fn package_parses_document_through_doc_root() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Document.xml", DOCUMENT.as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    let view = package.parse_document().expect("parse Document.xml");
    assert_eq!(view.pages().unwrap().pages().count(), 2);
}
