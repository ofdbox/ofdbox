use ofdbox_core::{ClipAreaKind, OfdError, PageContentView, ReadOptions};

const PAGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Content>
    <ofd:Layer ID="1">
      <ofd:TextObject ID="10" Boundary="0 0 100 20" Font="1" Size="12">
        <ofd:Actions>
          <ofd:Action Event="CLICK"><ofd:URI URI="https://example.com"/></ofd:Action>
        </ofd:Actions>
        <ofd:Clips>
          <ofd:Clip>
            <ofd:Area DrawParam="2" CTM="1 0 0 1 0 0">
              <ofd:Path Boundary="0 0 100 20">
                <ofd:AbbreviatedData>M 0 0 L 100 20</ofd:AbbreviatedData>
              </ofd:Path>
            </ofd:Area>
          </ofd:Clip>
        </ofd:Clips>
        <ofd:FillColor Value="0 0 0"/>
        <ofd:StrokeColor Value="255 0 0"/>
        <ofd:CGTransform CodePosition="0" CodeCount="2" GlyphCount="2">
          <ofd:Glyphs>1 2</ofd:Glyphs>
        </ofd:CGTransform>
        <ofd:TextCode X="1" Y="2" DeltaX="1 2" DeltaY="0 0">AB</ofd:TextCode>
        <ofd:TextCode X="10" Y="2">CD</ofd:TextCode>
      </ofd:TextObject>
      <ofd:PathObject ID="11" Boundary="0 0 10 10" Stroke="true">
        <ofd:StrokeColor Value="0 1 0"/>
        <ofd:AbbreviatedData>M 0 0 L 10 10</ofd:AbbreviatedData>
      </ofd:PathObject>
      <ofd:ImageObject ID="12" Boundary="0 0 10 10" ResourceID="3">
        <ofd:Border LineWidth="0.5" HorizonalCornerRadius="1" VerticalCornerRadius="2" DashPattern="1 2">
          <ofd:BorderColor Value="0 0 255"/>
        </ofd:Border>
      </ofd:ImageObject>
    </ofd:Layer>
  </ofd:Content>
</ofd:Page>"#;

fn parse(xml: &str, options: ReadOptions) -> Result<PageContentView, OfdError> {
    PageContentView::from_xml(xml.as_bytes(), options)
}

#[test]
fn text_payload_exposes_codes_transforms_actions_and_clips() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse(PAGE, options).expect("canonical page");
        let layer = view.content().unwrap().layers().next().unwrap();
        let objects: Vec<_> = layer.objects().collect();
        let text = &objects[0];

        let codes: Vec<_> = text.text_codes().collect();
        assert_eq!(codes.len(), 2);
        assert_eq!(codes[0].text(), "AB");
        assert_eq!(codes[0].x(), Some(1.0));
        assert_eq!(codes[0].y(), Some(2.0));
        assert_eq!(codes[0].delta_x(), Some("1 2"));
        assert_eq!(codes[1].text(), "CD");

        let transform = text.cg_transforms().next().expect("CGTransform");
        assert_eq!(transform.code_position(), Some(0));
        assert_eq!(transform.code_count(), Some(2));
        assert_eq!(transform.glyph_count(), Some(2));
        assert_eq!(transform.glyphs(), Some("1 2"));

        let action = text.actions().unwrap().actions().next().unwrap();
        assert_eq!(action.event(), Some("CLICK"));

        let clips = text.clips().expect("Clips");
        let area = clips.clips().next().unwrap().areas().next().unwrap();
        assert_eq!(area.kind(), ClipAreaKind::Path);
        assert_eq!(area.draw_param(), Some("2"));
        assert_eq!(area.ctm(), Some("1 0 0 1 0 0"));
        let content = area.content().expect("clip content");
        assert_eq!(content.kind(), ofdbox_core::GraphicObjectKind::Path);
        assert_eq!(content.abbreviated_data(), Some("M 0 0 L 100 20"));
    }
}

#[test]
fn path_and_image_payloads_are_typed() {
    let view = parse(PAGE, ReadOptions::STRICT).unwrap();
    let layer = view.content().unwrap().layers().next().unwrap();
    let objects: Vec<_> = layer.objects().collect();

    assert_eq!(objects[1].abbreviated_data(), Some("M 0 0 L 10 10"));
    assert_eq!(objects[1].stroke_color().unwrap().value(), Some("0 1 0"));

    let border = objects[2].border().expect("Border");
    assert_eq!(border.line_width(), Some(0.5));
    assert_eq!(border.horizontal_corner_radius(), Some(1.0));
    assert_eq!(border.vertical_corner_radius(), Some(2.0));
    assert_eq!(border.dash_pattern(), Some("1 2"));
    assert_eq!(border.border_color().unwrap().value(), Some("0 0 255"));
}

#[test]
fn writer_roundtrips_payloads() {
    let view = parse(PAGE, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    let reparsed =
        PageContentView::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    let layer = reparsed.content().unwrap().layers().next().unwrap();
    let objects: Vec<_> = layer.objects().collect();
    assert_eq!(objects[0].text_codes().count(), 2);
    assert_eq!(
        objects[0]
            .clips()
            .unwrap()
            .clips()
            .next()
            .unwrap()
            .areas()
            .count(),
        1
    );
    assert_eq!(objects[2].border().unwrap().line_width(), Some(0.5));
}

#[test]
fn text_without_text_code_is_strict_error() {
    let missing = PAGE
        .replace(r#"<ofd:TextCode X="10" Y="2">CD</ofd:TextCode>"#, "")
        .replace(
            r#"<ofd:TextCode X="1" Y="2" DeltaX="1 2" DeltaY="0 0">AB</ofd:TextCode>"#,
            "",
        );
    assert!(parse(&missing, ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        parse(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn path_without_abbreviated_data_is_strict_error() {
    let missing = PAGE.replace(
        r#"<ofd:AbbreviatedData>M 0 0 L 10 10</ofd:AbbreviatedData>"#,
        "",
    );
    assert!(parse(&missing, ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        parse(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn clip_area_needs_exactly_one_shape() {
    let empty = PAGE.replace(
        r#"<ofd:Path Boundary="0 0 100 20">
                <ofd:AbbreviatedData>M 0 0 L 100 20</ofd:AbbreviatedData>
              </ofd:Path>"#,
        "",
    );
    assert!(parse(&empty, ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        parse(&empty, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn invalid_border_width_is_strict_error() {
    let invalid = PAGE.replace(r#" LineWidth="0.5""#, r#" LineWidth="thick""#);
    let view = parse(&invalid, ReadOptions::LENIENT).expect("lenient");
    let border = view
        .content()
        .unwrap()
        .layers()
        .next()
        .unwrap()
        .objects()
        .nth(2)
        .unwrap()
        .border()
        .unwrap();
    assert_eq!(border.line_width(), None);
    assert!(matches!(
        parse(&invalid, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}
