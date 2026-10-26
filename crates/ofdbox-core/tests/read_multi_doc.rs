mod fixtures;

use ofdbox_core::OfdPackage;
use ofdbox_core::model::page::Page;

const OFD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody><ofd:DocInfo><ofd:DocID>doc-0</ofd:DocID></ofd:DocInfo><ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot></ofd:DocBody>
  <ofd:DocBody><ofd:DocInfo><ofd:DocID>doc-1</ofd:DocID></ofd:DocInfo><ofd:DocRoot>Doc_1/Document.xml</ofd:DocRoot></ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData><ofd:MaxUnitID>2</ofd:MaxUnitID><ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea></ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;

fn page(data: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1"><ofd:PathObject ID="2" Boundary="0 0 1 1"><ofd:AbbreviatedData>{data}</ofd:AbbreviatedData></ofd:PathObject></ofd:Layer></ofd:Content>
</ofd:Page>"#
    )
}

#[test]
fn reads_multiple_documents() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", OFD.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        (
            "Doc_0/Pages/Page_0/Content.xml",
            page("M 0 0 L 1 1").as_bytes(),
        ),
        ("Doc_1/Document.xml", DOCUMENT.as_bytes()),
        (
            "Doc_1/Pages/Page_0/Content.xml",
            page("M 0 0 L 2 2").as_bytes(),
        ),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open");

    let documents = package.documents().expect("documents");
    assert_eq!(documents.len(), 2);
    assert_eq!(documents[0].doc_id.as_deref(), Some("doc-0"));
    assert_eq!(documents[1].doc_id.as_deref(), Some("doc-1"));

    let document = package.parse_document_at(1).expect("document 1");
    let entry = document.pages().unwrap().pages().next().unwrap();
    let page = package.parse_page_for_document(1, &entry).expect("page 1");
    let data = page
        .content()
        .unwrap()
        .layers()
        .next()
        .unwrap()
        .objects()
        .next()
        .unwrap()
        .abbreviated_data()
        .map(str::to_owned);
    assert_eq!(data.as_deref(), Some("M 0 0 L 2 2"));
    let _ = Page::from_xml(
        page.to_xml().unwrap().as_bytes(),
        ofdbox_core::ReadOptions::LENIENT,
    );
}
