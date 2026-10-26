mod fixtures;

use ofdbox_core::{OfdPackage, OfdWriter, PageSize};

#[test]
fn writes_multi_page_document() {
    let mut writer = OfdWriter::new()
        .doc_id("writer-demo")
        .title("Writer Demo")
        .creator("OFDBox");

    let font = writer.add_font("SimSun");
    let embedded = writer.add_embedded_font("SimHei", b"fake-ttf".to_vec());

    let page0 = writer.add_page(PageSize::a4());
    let page1 = writer.add_page(PageSize::a4());
    writer
        .add_text(
            page0,
            "10 10 100 20",
            &font,
            12.0,
            &[(10.0, 20.0, "page 1")],
        )
        .expect("text 0");
    writer
        .add_text(
            page1,
            "10 10 100 20",
            &embedded,
            12.0,
            &[(10.0, 20.0, "page 2")],
        )
        .expect("text 1");

    let bytes = writer.to_bytes().expect("build");
    let mut package = OfdPackage::from_bytes(bytes).expect("open");

    let documents = package.documents().expect("documents");
    assert_eq!(documents.len(), 1);
    let document = package.parse_document_at(0).expect("document");
    assert_eq!(document.pages().unwrap().pages().count(), 2);

    let entry = document.pages().unwrap().pages().next().unwrap();
    let page = package.parse_page_for_document(0, &entry).expect("page 0");
    let text = page
        .content()
        .unwrap()
        .layers()
        .next()
        .unwrap()
        .objects()
        .next()
        .unwrap()
        .text_codes()
        .next()
        .unwrap()
        .text()
        .to_owned();
    assert_eq!(text, "page 1");

    let res = package
        .parse_res_for_document(0, "Res/DocumentRes.xml")
        .expect("res");
    let fonts: Vec<_> = res.fonts().collect();
    assert_eq!(fonts.len(), 2);
    assert!(fonts[0].font_file().is_none());
    let file = fonts[1].font_file().expect("embedded font file");
    assert!(package.contains(&format!("Doc_0/Res/{file}")));
}
