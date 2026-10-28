use std::str::FromStr;

use ofdbox_core::model::definition::{ColorArray, DeltaArray};
use ofdbox_core::model::page::{Color, GraphicObject, Page};
use ofdbox_core::model_xml;

#[test]
fn expands_repeated_delta_groups() {
    let values = DeltaArray::from_str("1 2 3 4 g 4 1 2 3").expect("parse delta array");
    assert_eq!(
        values.0,
        vec![1.0, 2.0, 3.0, 4.0, 1.0, 1.0, 1.0, 1.0, 2.0, 3.0]
    );
}

#[test]
fn keeps_standard_delta_lists_unchanged() {
    let values = DeltaArray::from_str("0.9701 -1 2.5").expect("parse delta array");
    assert_eq!(values.0, vec![0.9701, -1.0, 2.5]);
}

#[test]
fn parses_hexadecimal_color_components() {
    let values = ColorArray::from_str("#ee #20 #25").expect("parse hex components");
    assert_eq!(
        values.0,
        vec![238, 32, 37]
    );
}

#[test]
fn parses_delta_attributes_on_text_code() {
    let xml = r#"<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Content>
    <ofd:Layer ID="1">
      <ofd:TextObject ID="2" Boundary="0 0 100 20" Font="1" Size="10">
        <ofd:TextCode X="0" Y="0" DeltaX="1 2 3 4 g 4 1 2 3" DeltaY="g 2 -1">Hi</ofd:TextCode>
      </ofd:TextObject>
    </ofd:Layer>
  </ofd:Content>
</ofd:Page>"#;
    let page: Page = model_xml::from_str(xml).expect("parse page");
    let GraphicObject::Text(text) = &page.content.as_ref().unwrap().layers[0].objects[0] else {
        panic!("expected text object");
    };
    let text_code = &text.text_codes[0];
    assert_eq!(
        text_code.delta_x.as_ref().unwrap().0,
        vec![1.0, 2.0, 3.0, 4.0, 1.0, 1.0, 1.0, 1.0, 2.0, 3.0]
    );
    assert_eq!(text_code.delta_y.as_ref().unwrap().0, vec![-1.0, -1.0]);
}


#[test]
fn parses_and_writes_hex_color_values() {
    let xml = r##"<ofd:Color xmlns:ofd="http://www.ofdspec.org/2016" Value="#ee #20 #25"/>"##;
    let color: Color = model_xml::from_str(xml).expect("parse color");
    assert_eq!(
        color.value.as_ref().unwrap().0,
        vec![238, 32, 37]
    );
    let written = model_xml::to_string(&color).expect("write color");
    assert!(written.contains("Value=\"238 32 37\""), "{written}");
}
