mod fixtures;

use ofdbox_core::store::{CreateOptions, OfdFile, OpenOptions, OpenStorage, Storage};

const OFD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>demo</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>2</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;

const PAGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:Area>
  <ofd:Content>
    <ofd:Layer ID="1" Type="Body">
      <ofd:TextObject ID="2" Boundary="10 10 100 20" Font="3" Size="12">
        <ofd:TextCode X="10" Y="20">before</ofd:TextCode>
      </ofd:TextObject>
    </ofd:Layer>
  </ofd:Content>
</ofd:Page>"#;

#[test]
fn loads_saves_and_reloads_page_through_store() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", OFD.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", PAGE.as_bytes()),
    ]);

    let reader = OfdFile::open_bytes_with(
        bytes,
        OpenOptions {
            storage: OpenStorage::Memory,
            ..Default::default()
        },
    )
    .expect("open reader");
    let ofd = reader.ofd().expect("ofd");
    let doc = ofd.document(0).expect("document");

    let entry = &doc.data.pages.pages[0];
    let mut page = entry.load(&doc).expect("page");
    let text = page.data.content.as_mut().expect("content").layers[0].objects[0].clone();
    let ofdbox_core::schema::page::GraphicObject::Text(mut text) = text else {
        panic!("not text");
    };
    text.text_codes[0].text = "after".to_owned();
    page.data.content.as_mut().unwrap().layers[0].objects[0] =
        ofdbox_core::schema::page::GraphicObject::Text(text);
    page.save().expect("save page");

    let dir = tempfile::tempdir().expect("tempdir");
    let output = dir.path().join("output.ofd");
    reader.save(&output).expect("save package");

    let reopened = OfdFile::open(&output).expect("reopen");
    let ofd = reopened.ofd().expect("ofd");
    let doc = ofd.document(0).expect("document");
    let page = doc.page("1").expect("page by id");
    let object = &page.data.content.as_ref().unwrap().layers[0].objects[0];
    let ofdbox_core::schema::page::GraphicObject::Text(text) = object else {
        panic!("not text");
    };
    assert_eq!(text.text_codes[0].text, "after");
}

#[test]
fn page_entry_ref_loads_page() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", OFD.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", PAGE.as_bytes()),
    ]);
    let reader = OfdFile::open_bytes_with(
        bytes,
        OpenOptions {
            storage: OpenStorage::Memory,
            ..Default::default()
        },
    )
    .expect("open");
    let ofd = reader.ofd().expect("ofd");
    let doc = ofd.document(0).expect("document");
    let entry = doc.pages().next().expect("entry");
    let page = entry.load().expect("page");
    assert!(page.data.content.is_some());
}

#[test]
#[ignore]
fn loads_real_ofd_through_store() {
    let dir = std::env::var("OFDBOX_TESTDATA_DIR").expect("OFDBOX_TESTDATA_DIR");
    let path = std::fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "ofd"))
        .expect("ofd file");

    let reader = OfdFile::open(&path).expect("open");
    let ofd = reader.ofd().expect("ofd");
    let doc = ofd.document(0).expect("document");
    let entry = doc.pages().next().expect("page entry");
    let page = entry.load().expect("page");
    assert!(page.data.area.is_some() || page.data.content.is_some());
}

#[test]
fn creates_memory_ofd() {
    let file = OfdFile::create().expect("create");
    let mut doc = file.document(0).expect("document");
    let page = doc
        .add_page(ofdbox_core::schema::page::Page::default())
        .expect("add page");
    assert_eq!(page.location().as_str(), "Doc_0/Pages/Page_1/Content.xml");

    let dir = tempfile::tempdir().expect("tempdir");
    let output = dir.path().join("created.ofd");
    file.save(&output).expect("save");

    let reopened = OfdFile::open(&output).expect("reopen");
    let doc = reopened.document(0).expect("document");
    assert_eq!(doc.data.pages.pages.len(), 1);
    let page = doc.page("1").expect("page");
    assert!(page.data.area.is_none());
}

#[test]
fn creates_directory_ofd() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("source");
    let file = OfdFile::create_with(CreateOptions {
        storage: Storage::Directory(source.clone()),
        ..Default::default()
    })
    .expect("create");
    let mut doc = file.document(0).expect("document");
    doc.add_page(ofdbox_core::schema::page::Page::default())
        .expect("add page");
    assert!(source.join("OFD.xml").is_file());
    assert!(source.join("Doc_0/Pages/Page_1/Content.xml").is_file());

    let output = dir.path().join("created.ofd");
    file.save(&output).expect("save");
    assert!(output.is_file());
}
