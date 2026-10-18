use ofdbox_core::{DocumentView, OfdError, ReadOptions, RegionCommandKind};

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>20</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="10" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
  <ofd:Actions>
    <ofd:Action Event="CLICK">
      <ofd:Region>
        <ofd:Area Start="0 0">
          <ofd:Move Point1="0 0"/>
          <ofd:Line Point1="10 0"/>
          <ofd:QuadraticBezier Point1="10 5" Point2="5 10"/>
          <ofd:CubicBezier Point1="5 10" Point2="2 8" Point3="0 5"/>
          <ofd:Arc SweepDirection="true" LargeArc="false" RotationAngle="30" EllipseSize="5 3" EndPoint="0 0"/>
          <ofd:Close/>
        </ofd:Area>
      </ofd:Region>
      <ofd:URI URI="https://example.com"/>
    </ofd:Action>
  </ofd:Actions>
</ofd:Document>"#;

fn parse(xml: &str, options: ReadOptions) -> Result<DocumentView, OfdError> {
    DocumentView::from_xml(xml.as_bytes(), options)
}

fn region_commands(view: &DocumentView) -> Vec<ofdbox_core::RegionCommandView> {
    view.actions()
        .unwrap()
        .actions()
        .next()
        .unwrap()
        .region()
        .unwrap()
        .areas()
        .next()
        .unwrap()
        .commands()
        .collect()
}

#[test]
fn region_commands_parse_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse(DOCUMENT, options).expect("canonical region");
        let commands = region_commands(&view);
        assert_eq!(commands.len(), 6);
        assert_eq!(commands[0].kind(), RegionCommandKind::Move);
        assert_eq!(commands[0].point1(), Some("0 0"));
        assert_eq!(commands[1].kind(), RegionCommandKind::Line);
        assert_eq!(commands[2].kind(), RegionCommandKind::QuadraticBezier);
        assert_eq!(commands[2].point2(), Some("5 10"));
        assert_eq!(commands[3].kind(), RegionCommandKind::CubicBezier);
        assert_eq!(commands[3].point3(), Some("0 5"));
        assert_eq!(commands[4].kind(), RegionCommandKind::Arc);
        assert_eq!(commands[4].sweep_direction(), Some(true));
        assert_eq!(commands[4].large_arc(), Some(false));
        assert_eq!(commands[4].rotation_angle(), Some(30.0));
        assert_eq!(commands[4].ellipse_size(), Some("5 3"));
        assert_eq!(commands[4].end_point(), Some("0 0"));
        assert_eq!(commands[5].kind(), RegionCommandKind::Close);
    }
}

#[test]
fn region_writer_roundtrips() {
    let view = parse(DOCUMENT, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:Region>"));
    assert!(xml.contains(r#"<ofd:Area Start="0 0">"#));
    assert!(xml.contains("<ofd:QuadraticBezier"));
    assert!(xml.contains("<ofd:Close"));

    let reparsed =
        DocumentView::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    assert_eq!(region_commands(&reparsed).len(), 6);
}

#[test]
fn missing_area_start_is_strict_error() {
    let missing = DOCUMENT.replace(r#" Start="0 0""#, "");
    assert!(parse(&missing, ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        parse(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn unknown_region_command_is_strict_error() {
    let unknown = DOCUMENT.replace("<ofd:Close/>", "<ofd:Spline/>");
    assert!(parse(&unknown, ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        parse(&unknown, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn invalid_arc_rotation_is_strict_error() {
    let invalid = DOCUMENT.replace(r#"RotationAngle="30""#, r#"RotationAngle="turn""#);
    let view = parse(&invalid, ReadOptions::LENIENT).expect("lenient");
    let commands = region_commands(&view);
    assert_eq!(commands[4].rotation_angle(), None);
    assert!(matches!(
        parse(&invalid, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}
