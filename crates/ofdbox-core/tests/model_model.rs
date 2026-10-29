use ofdbox_core::model::definition::{PageArea, StBox, StId, StLoc, StRefId};
use ofdbox_core::model::document::{CommonData, Document, PageEntry, Pages};
use ofdbox_core::model::ofd::{DocBody, DocInfo, Ofd};
use ofdbox_core::model::page::{
    GraphicObject, GraphicUnit, Layer, LayerType, Page, PageContent, TextCode, TextContent,
    TextObject,
};
use ofdbox_core::model::res::{Font, Fonts, Res};

#[test]
fn builds_basic_ofd_objects() {
    let page = Page {
        area: Some(PageArea {
            physical_box: StBox::new(0.0, 0.0, 210.0, 297.0),
            ..Default::default()
        }),
        content: Some(PageContent {
            layers: vec![Layer {
                id: StId(2),
                layer_type: LayerType::Body,
                draw_param: None,
                objects: vec![GraphicObject::Text(TextObject {
                    id: StId(3),
                    content: TextContent {
                        graphic: GraphicUnit {
                            boundary: StBox::new(10.0, 10.0, 100.0, 20.0),
                            ..Default::default()
                        },
                        font: StRefId(1),
                        size: 12.0,
                        text_codes: vec![TextCode {
                            x: Some(10.0),
                            y: Some(20.0),
                            text: "Hello OFD".to_owned(),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                })],
            }],
        }),
        ..Default::default()
    };

    let document = Document {
        common_data: CommonData {
            max_unit_id: StId(3),
            page_area: PageArea {
                physical_box: StBox::new(0.0, 0.0, 210.0, 297.0),
                ..Default::default()
            },
            public_res: Vec::new(),
            document_res: vec![StLoc::new("Res/DocumentRes.xml")],
            template_pages: Vec::new(),
            default_cs: None,
        },
        pages: Pages {
            pages: vec![PageEntry {
                id: "2".to_owned(),
                base_loc: StLoc::new("Pages/Page_0/Content.xml"),
            }],
        },
        outlines: None,
        permissions: None,
        actions: None,
        v_preferences: None,
        bookmarks: None,
        annotations: None,
        custom_tags: None,
        attachments: None,
        extensions: None,
    };

    let ofd = Ofd {
        version: "1.0".to_owned(),
        doc_type: "OFD".to_owned(),
        doc_bodies: vec![DocBody {
            doc_info: DocInfo {
                doc_id: "demo".to_owned(),
                ..Default::default()
            },
            doc_root: StLoc::new("Doc_0/Document.xml"),
            versions: None,
            signatures: None,
        }],
    };

    let res = Res {
        base_loc: StLoc::new("."),
        fonts: Some(Fonts {
            fonts: vec![Font {
                id: StId(1),
                font_name: "SimSun".to_owned(),
                ..Default::default()
            }],
        }),
        ..Default::default()
    };

    assert_eq!(ofd.doc_bodies.len(), 1);
    assert_eq!(
        document.pages.pages[0].base_loc.as_str(),
        "Pages/Page_0/Content.xml"
    );
    assert_eq!(res.fonts.unwrap().fonts[0].font_name, "SimSun");
    let content = page.content.unwrap();
    assert!(matches!(
        content.layers[0].objects[0],
        GraphicObject::Text(_)
    ));
}
