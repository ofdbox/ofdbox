mod fixtures;

use std::io::Cursor;

use ofdbox_core::model::annotation::PageAnnot;
use ofdbox_core::{AnnotationEditor, AnnotationPatch, AnnotationSpec, OfdPackage, ReadOptions};

const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>annot-edit</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>2</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/><ofd:Page ID="2" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
  <ofd:Annotations>Annots/Annotations.xml</ofd:Annotations>
</ofd:Document>"#;

const PAGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:Area>
  <ofd:Content/>
</ofd:Page>"#;

const ANNOTATIONS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Annotations xmlns:ofd="http://www.ofdspec.org/2016"><ofd:Page PageID="1"><ofd:FileLoc>Page_1.xml</ofd:FileLoc></ofd:Page></ofd:Annotations>"#;

const PAGE_ANNOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:PageAnnot xmlns:ofd="http://www.ofdspec.org/2016"><ofd:Annot ID="1" Type="Highlight" Creator="tester" LastModDate="2026-01-01"><ofd:Remark>keep</ofd:Remark><ofd:Appearance Boundary="10 10 20 20"/></ofd:Annot></ofd:PageAnnot>"#;

fn package() -> Vec<u8> {
    fixtures::zip_with_entries(&[
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", PAGE.as_bytes()),
        ("Doc_0/Annots/Annotations.xml", ANNOTATIONS.as_bytes()),
        ("Doc_0/Annots/Page_1.xml", PAGE_ANNOT.as_bytes()),
    ])
}

fn open() -> OfdPackage {
    OfdPackage::from_bytes_with_options(package(), ReadOptions::LENIENT, Default::default())
        .expect("open")
}

#[test]
fn adds_annotation_and_updates_index() {
    let mut source = open();
    let mut editor = AnnotationEditor::new(&mut source, false).expect("editor");
    let id = editor
        .add(
            "1",
            &AnnotationSpec::new("Highlight", "30 40 50 60").with_creator("writer"),
        )
        .expect("add");
    assert_eq!(id, "3");

    let mut output = Vec::new();
    let report = editor.write(Cursor::new(&mut output)).expect("write");
    assert_eq!(report.added, 1);

    let mut package = OfdPackage::from_bytes(output).expect("reopen");
    let page = package.parse_page_annot("Page_1.xml").expect("page annot");
    assert_eq!(page.annots().count(), 2);
    let index = package.parse_annotations().expect("index");
    assert_eq!(index.pages().count(), 1);
}

#[test]
fn removes_last_annotation_and_page_entry() {
    let mut source = open();
    let mut editor = AnnotationEditor::new(&mut source, false).expect("editor");
    assert!(editor.remove("1"));
    let mut output = Vec::new();
    editor.write(Cursor::new(&mut output)).expect("write");

    let mut package = OfdPackage::from_bytes(output).expect("reopen");
    let index = package.parse_annotations().expect("index");
    assert_eq!(index.pages().count(), 0);
    assert!(!package.contains("Doc_0/Annots/Page_1.xml"));
}

#[test]
fn adds_annotation_to_page_without_index_entry() {
    let mut source = open();
    let mut editor = AnnotationEditor::new(&mut source, false).expect("editor");
    editor
        .add("2", &AnnotationSpec::new("Highlight", "1 2 3 4"))
        .expect("add");
    let mut output = Vec::new();
    editor.write(Cursor::new(&mut output)).expect("write");

    let mut package = OfdPackage::from_bytes(output).expect("reopen");
    let index = package.parse_annotations().expect("index");
    let page = index
        .pages()
        .find(|page| page.page_id() == Some("2"))
        .expect("page 2");
    assert_eq!(page.file_loc(), Some("PageAnnot_2.xml"));
    let annots = package
        .parse_page_annot("PageAnnot_2.xml")
        .expect("page annot");
    assert_eq!(annots.annots().count(), 1);
}

#[test]
fn updates_annotation_preserving_unknown_content() {
    let mut source = open();
    let mut editor = AnnotationEditor::new(&mut source, false).expect("editor");
    editor
        .update(
            "1",
            AnnotationPatch::new()
                .with_boundary("30 40 50 60")
                .with_creator("reviewer")
                .with_visible(true),
        )
        .expect("update");
    let mut output = Vec::new();
    let report = editor.write(Cursor::new(&mut output)).expect("write");
    assert_eq!(report.updated, 1);

    let mut package = OfdPackage::from_bytes(output).expect("reopen");
    let raw = package
        .read_entry("Doc_0/Annots/Page_1.xml")
        .expect("page annot");
    let text = std::str::from_utf8(&raw).expect("utf8");
    assert!(text.contains(r#"Creator="reviewer""#), "{text}");
    assert!(text.contains(r#"Visible="true""#), "{text}");
    assert!(text.contains(r#"Boundary="30 40 50 60""#), "{text}");
    assert!(text.contains("<ofd:Remark>keep</ofd:Remark>"), "{text}");
}

const DOCUMENT_WITH_RES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>2</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
    <ofd:DocumentRes>DocumentRes.xml</ofd:DocumentRes>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
  <ofd:Annotations>Annots/Annotations.xml</ofd:Annotations>
</ofd:Document>"#;

const DOCUMENT_RES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res"><ofd:MultiMedias/></ofd:Res>"#;

#[test]
fn adds_image_annotation_and_resource() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT_WITH_RES.as_bytes()),
        ("Doc_0/DocumentRes.xml", DOCUMENT_RES.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", PAGE.as_bytes()),
        ("Doc_0/Annots/Annotations.xml", ANNOTATIONS.as_bytes()),
        ("Doc_0/Annots/Page_1.xml", PAGE_ANNOT.as_bytes()),
    ]);
    let mut source =
        OfdPackage::from_bytes_with_options(bytes, ReadOptions::LENIENT, Default::default())
            .expect("open");
    let mut editor = AnnotationEditor::new(&mut source, false).expect("editor");
    editor
        .add(
            "1",
            &AnnotationSpec::new("Stamp", "10 10 100 100").with_image(
                b"fake-png".to_vec(),
                "PNG",
                40,
                40,
            ),
        )
        .expect("add image");
    let mut output = Vec::new();
    editor.write(Cursor::new(&mut output)).expect("write");

    let mut package = OfdPackage::from_bytes(output).expect("reopen");
    let res = package
        .parse_res_from_document("DocumentRes.xml")
        .expect("document res");
    let media: Vec<_> = res.multi_medias().collect();
    assert_eq!(media.len(), 1);
    let file = media[0].media_file().expect("media file");
    assert_eq!(file, "annotation_3.png");
    assert!(package.contains("Doc_0/Res/annotation_3.png"));

    let raw = package
        .read_entry("Doc_0/Annots/Page_1.xml")
        .expect("page annot");
    let text = std::str::from_utf8(&raw).expect("utf8");
    assert!(text.contains("ImageObject"), "{text}");
    assert!(text.contains("ResourceID="), "{text}");
}

#[test]
fn adds_text_and_path_appearances() {
    let mut source = open();
    let mut editor = AnnotationEditor::new(&mut source, false).expect("editor");
    editor
        .add(
            "1",
            &AnnotationSpec::new("Highlight", "10 10 100 30").with_text("review", "1", 12.0),
        )
        .expect("text");
    editor
        .add(
            "1",
            &AnnotationSpec::new("Path", "10 40 100 80").with_path("M 10 40 L 100 80"),
        )
        .expect("path");
    let mut output = Vec::new();
    editor.write(Cursor::new(&mut output)).expect("write");

    let mut package = OfdPackage::from_bytes(output).expect("reopen");
    let raw = package
        .read_entry("Doc_0/Annots/Page_1.xml")
        .expect("page annot");
    let text = std::str::from_utf8(&raw).expect("utf8");
    assert!(text.contains("<ofd:TextObject"), "{text}");
    assert!(
        text.contains("<ofd:TextCode X=\"0\" Y=\"0\">review</ofd:TextCode>"),
        "{text}"
    );
    assert!(text.contains("<ofd:PathObject"), "{text}");
    assert!(
        text.contains("<ofd:AbbreviatedData>M 10 40 L 100 80</ofd:AbbreviatedData>"),
        "{text}"
    );

    PageAnnot::from_xml(&raw, ReadOptions::STRICT).expect("strict page annot");
}

#[test]
fn adds_cg_transform_and_composite_appearance() {
    let mut source = open();
    let mut editor = AnnotationEditor::new(&mut source, false).expect("editor");
    editor
        .add(
            "1",
            &AnnotationSpec::new("Highlight", "10 10 100 30")
                .with_text("abc", "1", 12.0)
                .with_cg_transform(0, Some(3), Some(3), Some("abc".to_owned())),
        )
        .expect("text cg");
    editor
        .add(
            "1",
            &AnnotationSpec::new("Stamp", "10 40 100 80").with_composite("1"),
        )
        .expect("composite");
    let mut output = Vec::new();
    editor.write(Cursor::new(&mut output)).expect("write");

    let mut package = OfdPackage::from_bytes(output).expect("reopen");
    let raw = package
        .read_entry("Doc_0/Annots/Page_1.xml")
        .expect("page annot");
    let text = std::str::from_utf8(&raw).expect("utf8");
    assert!(
        text.contains("<ofd:CGTransform CodePosition=\"0\" CodeCount=\"3\" GlyphCount=\"3\">"),
        "{text}"
    );
    assert!(text.contains("<ofd:Glyphs>abc</ofd:Glyphs>"), "{text}");
    assert!(
        text.contains("<ofd:CompositeObject ID=\"") && text.contains("ResourceID=\"1\""),
        "{text}"
    );
    PageAnnot::from_xml(&raw, ReadOptions::STRICT).expect("strict page annot");
}

#[test]
fn rejects_cg_transform_without_text() {
    let mut source = open();
    let mut editor = AnnotationEditor::new(&mut source, false).expect("editor");
    assert!(
        editor
            .add(
                "1",
                &AnnotationSpec::new("Highlight", "1 2 3 4").with_cg_transform(
                    0,
                    Some(1),
                    Some(1),
                    None
                ),
            )
            .is_err()
    );
}
