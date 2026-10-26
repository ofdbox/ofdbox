use ofdbox_model::model::definition::{StBox, StId, StRefId};
use ofdbox_model::model::page::{
    GraphicObject, GraphicUnit, Layer, LayerType, Page, PageContent, TextCode, TextObject,
};
use ofdbox_model::xml;

#[test]
fn parses_and_writes_page_content() {
    let xml_text = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:Area>
  <ofd:Content>
    <ofd:Layer ID="1" Type="Body">
      <ofd:TextObject ID="2" Boundary="10 10 100 20" Font="3" Size="12">
        <ofd:TextCode X="10" Y="20">Hello OFD</ofd:TextCode>
      </ofd:TextObject>
      <ofd:PathObject ID="4" Boundary="0 0 10 10" Stroke="true" Fill="false" Rule="NonZero">
        <ofd:AbbreviatedData>M 0 0 L 10 10</ofd:AbbreviatedData>
      </ofd:PathObject>
    </ofd:Layer>
  </ofd:Content>
</ofd:Page>"#;

    let parsed: Page = xml::from_str(xml_text).expect("parse page");
    let content = parsed.content.as_ref().expect("content");
    assert_eq!(content.layers.len(), 1);
    assert_eq!(content.layers[0].id, StId(1));
    assert_eq!(content.layers[0].objects.len(), 2);
    let GraphicObject::Text(text) = &content.layers[0].objects[0] else {
        panic!("first object is not text");
    };
    assert_eq!(text.common.id, StId(2));
    assert_eq!(text.text_codes[0].text, "Hello OFD");
    let GraphicObject::Path(path) = &content.layers[0].objects[1] else {
        panic!("second object is not path");
    };
    assert_eq!(path.abbreviated_data, "M 0 0 L 10 10");

    let written = xml::to_string(&parsed).expect("write page");
    assert!(written.contains("<ofd:Page") || written.contains("<Page"));
    assert!(written.contains("TextObject"));
    let reparsed: Page = xml::from_str(&written).expect("reparse page");
    assert_eq!(reparsed, parsed);
}

#[test]
fn builds_page_with_text() {
    let page = Page {
        content: Some(PageContent {
            layers: vec![Layer {
                id: StId(1),
                layer_type: LayerType::Body,
                draw_param: None,
                objects: vec![GraphicObject::Text(TextObject {
                    common: GraphicUnit {
                        id: StId(2),
                        boundary: StBox::new(10.0, 10.0, 100.0, 20.0),
                        ..Default::default()
                    },
                    font: StRefId(3),
                    size: 12.0,
                    text_codes: vec![TextCode {
                        x: Some(10.0),
                        y: Some(20.0),
                        text: "Hello".to_owned(),
                        ..Default::default()
                    }],
                    ..Default::default()
                })],
            }],
        }),
        ..Default::default()
    };
    let written = xml::to_string(&page).expect("serialize page");
    let reparsed: Page = xml::from_str(&written).expect("reparse page");
    assert_eq!(reparsed, page);
}
