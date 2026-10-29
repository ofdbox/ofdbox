use ofdbox_core::model::definition::{StBox, StId, StRefId};
use ofdbox_core::model::page::{
    GraphicObject, GraphicUnit, Layer, LayerType, Page, PageContent, TextCode, TextContent,
    TextObject,
};
use ofdbox_core::model_xml as xml;

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
    assert_eq!(text.id, StId(2));
    assert_eq!(text.content.text_codes[0].text, "Hello OFD");
    let GraphicObject::Path(path) = &content.layers[0].objects[1] else {
        panic!("second object is not path");
    };
    assert_eq!(path.content.abbreviated_data, "M 0 0 L 10 10");

    let written = xml::to_string(&parsed).expect("write page");
    assert!(written.contains("<ofd:Page") || written.contains("<Page"));
    assert!(written.contains("TextObject"));
    let reparsed: Page = xml::from_str(&written).expect("reparse page");
    assert_eq!(reparsed, parsed);
}

#[test]
fn clip_path_does_not_serialize_id() {
    let xml_text = r#"<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Content>
    <ofd:Layer ID="1">
      <ofd:PathObject ID="2" Boundary="0 0 100 100">
        <ofd:AbbreviatedData>M 0 0 L 100 100</ofd:AbbreviatedData>
        <ofd:Clips>
          <ofd:Clip>
            <ofd:Area>
              <ofd:Path ID="99" Boundary="0 0 10 10">
                <ofd:AbbreviatedData>M 0 0 L 10 10</ofd:AbbreviatedData>
              </ofd:Path>
            </ofd:Area>
          </ofd:Clip>
        </ofd:Clips>
      </ofd:PathObject>
    </ofd:Layer>
  </ofd:Content>
</ofd:Page>"#;
    let parsed: Page = xml::from_str(xml_text).expect("parse page with clip");
    let written = xml::to_string(&parsed).expect("write page with clip");
    let start = written
        .find("<ofd:Path ")
        .or_else(|| written.find("<Path "))
        .expect("clip Path element");
    let end = start + written[start..].find('>').expect("clip Path tag end");
    let tag = &written[start..=end];
    assert!(!tag.contains("ID="), "clip Path must not have ID: {tag}");
    assert!(tag.contains("Boundary="), "{tag}");
}

#[test]
fn clip_text_does_not_serialize_id() {
    let xml_text = r#"<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Content>
    <ofd:Layer ID="1">
      <ofd:PathObject ID="2" Boundary="0 0 100 100">
        <ofd:AbbreviatedData>M 0 0 L 100 100</ofd:AbbreviatedData>
        <ofd:Clips>
          <ofd:Clip>
            <ofd:Area>
              <ofd:Text ID="99" Boundary="0 0 10 10" Font="1" Size="10">
                <ofd:TextCode>clip</ofd:TextCode>
              </ofd:Text>
            </ofd:Area>
          </ofd:Clip>
        </ofd:Clips>
      </ofd:PathObject>
    </ofd:Layer>
  </ofd:Content>
</ofd:Page>"#;
    let parsed: Page = xml::from_str(xml_text).expect("parse page with text clip");
    let written = xml::to_string(&parsed).expect("write page with text clip");
    let start = written
        .find("<ofd:Text ")
        .or_else(|| written.find("<Text "))
        .expect("clip Text element");
    let end = start + written[start..].find('>').expect("clip Text tag end");
    let tag = &written[start..=end];
    assert!(!tag.contains("ID="), "clip Text must not have ID: {tag}");
    assert!(tag.contains("Font="), "{tag}");
}

#[test]
fn page_block_object_requires_id() {
    let xml_text = r#"<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Content>
    <ofd:Layer ID="1">
      <ofd:PathObject Boundary="0 0 10 10">
        <ofd:AbbreviatedData>M 0 0 L 10 10</ofd:AbbreviatedData>
      </ofd:PathObject>
    </ofd:Layer>
  </ofd:Content>
</ofd:Page>"#;
    assert!(xml::from_str::<Page>(xml_text).is_err());
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
                    id: StId(2),
                    content: TextContent {
                        graphic: GraphicUnit {
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
                    },
                })],
            }],
        }),
        ..Default::default()
    };
    let written = xml::to_string(&page).expect("serialize page");
    assert!(!written.contains("Visible="), "{written}");
    assert!(!written.contains("LineWidth="), "{written}");
    assert!(!written.contains(r#"Type="Body""#), "{written}");
    assert!(!written.contains(r#"Fill=""#), "{written}");
    let reparsed: Page = xml::from_str(&written).expect("reparse page");
    assert_eq!(reparsed, page);
}
