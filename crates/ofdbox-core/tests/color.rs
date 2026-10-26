use ofdbox_core::model::color::ColorKind;
use ofdbox_core::model::page::Page;
use ofdbox_core::model::res::Res;
use ofdbox_core::{OfdError, ReadOptions};

const RES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="./">
  <ofd:DrawParams>
    <ofd:DrawParam ID="1">
      <ofd:FillColor Index="2" ColorSpace="5" Alpha="128">
        <ofd:Pattern Width="10" Height="20" XStep="5" YStep="6" ReflectMethod="Row" RelativeTo="Page">
          <ofd:CellContent Thumbnail="9">
            <ofd:PathObject ID="8" Boundary="0 0 1 1">
              <ofd:AbbreviatedData>M 0 0 L 1 1</ofd:AbbreviatedData>
            </ofd:PathObject>
          </ofd:CellContent>
        </ofd:Pattern>
      </ofd:FillColor>
      <ofd:StrokeColor>
        <ofd:AxialShd MapType="Repeat" MapUnit="0.5" Extend="1" StartPoint="0 0" EndPoint="1 0">
          <ofd:Segment Position="0"><ofd:Color Value="0 0 0"/></ofd:Segment>
          <ofd:Segment Position="1"><ofd:Color Value="255 255 255" Alpha="200"/></ofd:Segment>
        </ofd:AxialShd>
      </ofd:StrokeColor>
    </ofd:DrawParam>
    <ofd:DrawParam ID="2">
      <ofd:FillColor>
        <ofd:RadialShd StartPoint="0.5 0.5" StartRadius="0" EndPoint="0.5 0.5" EndRadius="10" Eccentricity="0.5" Angle="30">
          <ofd:Segment Position="0"><ofd:Color Value="0 0 0"/></ofd:Segment>
          <ofd:Segment Position="1"><ofd:Color Value="1 1 1"/></ofd:Segment>
        </ofd:RadialShd>
      </ofd:FillColor>
      <ofd:StrokeColor Value="0 0 0"/>
    </ofd:DrawParam>
    <ofd:DrawParam ID="3">
      <ofd:FillColor>
        <ofd:GouraudShd Extend="0">
          <ofd:Point X="0" Y="0" EdgeFlag="0"><ofd:Color Value="0 0 0"/></ofd:Point>
          <ofd:Point X="1" Y="0" EdgeFlag="1"><ofd:Color Value="1 0 0"/></ofd:Point>
          <ofd:Point X="0" Y="1"><ofd:Color Value="0 1 0"/></ofd:Point>
          <ofd:BackColor Value="1 1 1"/>
        </ofd:GouraudShd>
      </ofd:FillColor>
    </ofd:DrawParam>
    <ofd:DrawParam ID="4">
      <ofd:FillColor>
        <ofd:LaGourandShd VerticesPerRow="2" Extend="1">
          <ofd:Point X="0" Y="0"><ofd:Color Value="0 0 0"/></ofd:Point>
          <ofd:Point X="1" Y="0"><ofd:Color Value="1 0 0"/></ofd:Point>
          <ofd:Point X="0" Y="1"><ofd:Color Value="0 1 0"/></ofd:Point>
          <ofd:Point X="1" Y="1"><ofd:Color Value="0 0 1"/></ofd:Point>
        </ofd:LaGourandShd>
      </ofd:FillColor>
    </ofd:DrawParam>
  </ofd:DrawParams>
</ofd:Res>"#;

const PAGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Content>
    <ofd:Layer ID="1">
      <ofd:TextObject ID="2" Boundary="0 0 10 10" Font="3" Size="10">
        <ofd:FillColor Value="0 0 0" Alpha="255"/>
        <ofd:StrokeColor Value="255 0 0" Alpha="128"/>
        <ofd:TextCode X="0" Y="0">文本</ofd:TextCode>
      </ofd:TextObject>
    </ofd:Layer>
  </ofd:Content>
</ofd:Page>"#;

fn parse_res(xml: &str, options: ReadOptions) -> Result<Res, OfdError> {
    Res::from_xml(xml.as_bytes(), options)
}

#[test]
fn draw_param_colors_parse_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse_res(RES, options).expect("canonical colors");
        let params: Vec<_> = view.draw_params().collect();

        let fill = params[0].fill_color().expect("FillColor");
        assert_eq!(fill.kind(), ColorKind::Pattern);
        assert_eq!(fill.index(), Some(2));
        assert_eq!(fill.color_space(), Some("5"));
        assert_eq!(fill.alpha(), Some(128));
        let pattern = fill.pattern().expect("Pattern");
        assert_eq!(pattern.width(), Some(10.0));
        assert_eq!(pattern.height(), Some(20.0));
        assert_eq!(pattern.x_step(), Some(5.0));
        assert_eq!(pattern.y_step(), Some(6.0));
        assert_eq!(pattern.reflect_method(), Some("Row"));
        assert_eq!(pattern.relative_to(), Some("Page"));
        assert_eq!(pattern.thumbnail(), Some("9"));
        assert_eq!(pattern.objects().next().unwrap().id(), Some("8"));

        let stroke = params[0].stroke_color().expect("StrokeColor");
        assert_eq!(stroke.kind(), ColorKind::AxialShd);
        let axial = stroke.axial_shd().expect("AxialShd");
        assert_eq!(axial.map_type(), Some("Repeat"));
        assert_eq!(axial.map_unit(), Some(0.5));
        assert_eq!(axial.extend(), Some(1));
        assert_eq!(axial.start_point(), Some("0 0"));
        assert_eq!(axial.end_point(), Some("1 0"));
        let segments: Vec<_> = axial.segments().collect();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[1].position(), Some(1.0));
        assert_eq!(segments[1].color().unwrap().value(), Some("255 255 255"));
        assert_eq!(segments[1].color().unwrap().alpha(), Some(200));

        let radial = params[1].fill_color().unwrap();
        assert_eq!(radial.kind(), ColorKind::RadialShd);
        let radial = radial.radial_shd().unwrap();
        assert_eq!(radial.eccentricity(), Some(0.5));
        assert_eq!(radial.angle(), Some(30.0));
        assert_eq!(radial.end_radius(), Some(10.0));

        let gouraud = params[2].fill_color().unwrap();
        assert_eq!(gouraud.kind(), ColorKind::GouraudShd);
        let gouraud = gouraud.gouraud_shd().unwrap();
        assert_eq!(gouraud.points().count(), 3);
        assert_eq!(gouraud.points().next().unwrap().edge_flag(), Some(0));
        assert_eq!(gouraud.back_color().unwrap().value(), Some("1 1 1"));

        let la = params[3].fill_color().unwrap();
        assert_eq!(la.kind(), ColorKind::LaGouraudShd);
        let la = la.la_gouraud_shd().unwrap();
        assert_eq!(la.vertices_per_row(), Some(2));
        assert_eq!(la.points().count(), 4);
    }
}

#[test]
fn color_writer_roundtrips_all_kinds() {
    let view = parse_res(RES, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:Pattern "));
    assert!(xml.contains("<ofd:AxialShd "));
    assert!(xml.contains("<ofd:RadialShd "));
    assert!(xml.contains("<ofd:GouraudShd "));
    assert!(xml.contains("<ofd:LaGourandShd "));

    let reparsed = Res::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    let params: Vec<_> = reparsed.draw_params().collect();
    assert_eq!(params[0].fill_color().unwrap().kind(), ColorKind::Pattern);
    assert_eq!(
        params[0].stroke_color().unwrap().kind(),
        ColorKind::AxialShd
    );
    assert_eq!(
        params[2].fill_color().unwrap().kind(),
        ColorKind::GouraudShd
    );
    assert_eq!(
        params[3].fill_color().unwrap().kind(),
        ColorKind::LaGouraudShd
    );
}

#[test]
fn invalid_color_alpha_is_strict_error() {
    let invalid = RES.replace(r#" Alpha="128""#, r#" Alpha="opaque""#);
    let view = parse_res(&invalid, ReadOptions::LENIENT).expect("lenient");
    assert_eq!(
        view.draw_params()
            .next()
            .unwrap()
            .fill_color()
            .unwrap()
            .alpha(),
        None
    );
    assert!(matches!(
        parse_res(&invalid, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn gradient_needs_two_segments_in_strict_mode() {
    let single = RES.replace(
        r#"<ofd:Segment Position="1"><ofd:Color Value="255 255 255" Alpha="200"/></ofd:Segment>"#,
        "",
    );
    assert!(parse_res(&single, ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        parse_res(&single, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn page_graphic_exposes_colors_and_validates_them() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = Page::from_xml(PAGE.as_bytes(), options).expect("Page");
        let object = view
            .content()
            .unwrap()
            .layers()
            .next()
            .unwrap()
            .objects()
            .next()
            .unwrap();
        assert_eq!(object.fill_color().unwrap().value(), Some("0 0 0"));
        assert_eq!(object.stroke_color().unwrap().alpha(), Some(128));
    }

    let invalid = PAGE.replace(r#" Alpha="255""#, r#" Alpha="solid""#);
    assert!(Page::from_xml(invalid.as_bytes(), ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        Page::from_xml(invalid.as_bytes(), ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}
