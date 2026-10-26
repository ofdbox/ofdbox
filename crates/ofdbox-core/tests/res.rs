mod fixtures;

use ofdbox_core::model::res::Res;
use ofdbox_core::{DiagnosticCode, OfdError, OfdPackage, ReadOptions};

const RES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="./">
  <ofd:ColorSpaces>
    <ofd:ColorSpace ID="1" Type="RGB" BitsPerComponent="8">
      <ofd:Palette>
        <ofd:CV>0 0 0</ofd:CV>
        <ofd:CV>255 255 255</ofd:CV>
      </ofd:Palette>
    </ofd:ColorSpace>
  </ofd:ColorSpaces>
  <ofd:DrawParams>
    <ofd:DrawParam ID="2" LineWidth="0.5" Join="Round" Cap="Square" MiterLimit="4">
      <ofd:FillColor Value="0 0 0"/>
    </ofd:DrawParam>
  </ofd:DrawParams>
  <ofd:Fonts>
    <ofd:Font ID="3" FontName="SimSun" FamilyName="宋体" Charset="unicode" Italic="false" Bold="true">
      <ofd:FontFile>Res/font.ttf</ofd:FontFile>
    </ofd:Font>
  </ofd:Fonts>
  <ofd:MultiMedias>
    <ofd:MultiMedia ID="4" Type="Image" Format="PNG">
      <ofd:MediaFile>Res/img.png</ofd:MediaFile>
    </ofd:MultiMedia>
  </ofd:MultiMedias>
  <ofd:CompositeGraphicUnits>
    <ofd:CompositeGraphicUnit ID="5" Width="10" Height="20">
      <ofd:Thumbnail>6</ofd:Thumbnail>
      <ofd:Substitution>7</ofd:Substitution>
      <ofd:Content>
        <ofd:TextObject ID="8" Boundary="0 0 1 1" Font="3" Size="10">
          <ofd:TextCode X="0" Y="0">A</ofd:TextCode>
        </ofd:TextObject>
      </ofd:Content>
    </ofd:CompositeGraphicUnit>
  </ofd:CompositeGraphicUnits>
</ofd:Res>"#;

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>doc-1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>20</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
    <ofd:PublicRes>PublicRes.xml</ofd:PublicRes>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;

fn parse(xml: &str, options: ReadOptions) -> Result<Res, OfdError> {
    Res::from_xml(xml.as_bytes(), options)
}

#[test]
fn canonical_res_parses_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse(RES, options).expect("canonical Res should parse");
        assert_eq!(view.base_loc(), Some("./"));

        let color = view.color_spaces().next().expect("ColorSpace");
        assert_eq!(color.id(), Some("1"));
        assert_eq!(color.color_space_type(), Some("RGB"));
        assert_eq!(color.bits_per_component(), Some(8));
        assert_eq!(
            color.palette().collect::<Vec<_>>(),
            ["0 0 0", "255 255 255"]
        );

        let draw = view.draw_params().next().expect("DrawParam");
        assert_eq!(draw.id(), Some("2"));
        assert_eq!(draw.line_width(), Some(0.5));
        assert_eq!(draw.join(), Some("Round"));
        assert_eq!(draw.cap(), Some("Square"));
        assert_eq!(draw.miter_limit(), Some(4.0));

        let font = view.fonts().next().expect("Font");
        assert_eq!(font.id(), Some("3"));
        assert_eq!(font.font_name(), Some("SimSun"));
        assert_eq!(font.family_name(), Some("宋体"));
        assert_eq!(font.charset(), Some("unicode"));
        assert_eq!(font.bold(), Some(true));
        assert_eq!(font.font_file(), Some("Res/font.ttf"));

        let media = view.multi_medias().next().expect("MultiMedia");
        assert_eq!(media.id(), Some("4"));
        assert_eq!(media.media_type(), Some("Image"));
        assert_eq!(media.format(), Some("PNG"));
        assert_eq!(media.media_file(), Some("Res/img.png"));

        let unit = view
            .composite_graphic_units()
            .next()
            .expect("CompositeGraphicUnit");
        assert_eq!(unit.id(), Some("5"));
        assert_eq!(unit.width(), Some(10.0));
        assert_eq!(unit.height(), Some(20.0));
        assert_eq!(unit.thumbnail(), Some("6"));
        assert_eq!(unit.substitution(), Some("7"));
        let content = unit.content().expect("Content");
        assert_eq!(content.objects().next().unwrap().id(), Some("8"));
    }
}

#[test]
fn multiple_font_containers_are_merged() {
    let multi = RES.replace(
        "</ofd:Res>",
        r#"  <ofd:Fonts><ofd:Font ID="9" FontName="Arial"/></ofd:Fonts></ofd:Res>"#,
    );
    let view = parse(&multi, ReadOptions::LENIENT).expect("lenient Res");
    let ids = view
        .fonts()
        .map(|font| font.id().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["3", "9"]);
}

#[test]
fn writer_emits_canonical_res_and_roundtrips() {
    let view = parse(RES, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer should succeed");
    assert!(xml.contains(r#"xmlns:ofd="http://www.ofdspec.org/2016""#));
    let color = xml.find("<ofd:ColorSpaces>").unwrap();
    let draw = xml.find("<ofd:DrawParams>").unwrap();
    let fonts = xml.find("<ofd:Fonts>").unwrap();
    let media = xml.find("<ofd:MultiMedias>").unwrap();
    let composite = xml.find("<ofd:CompositeGraphicUnits>").unwrap();
    assert!(color < draw);
    assert!(draw < fonts);
    assert!(fonts < media);
    assert!(media < composite);

    let reparsed = Res::from_xml(xml.as_bytes(), ReadOptions::STRICT)
        .expect("canonical output should pass strict parsing");
    assert_eq!(reparsed.fonts().next().unwrap().id(), Some("3"));
}

#[test]
fn missing_font_name_is_strict_error() {
    let missing = RES.replace(r#" FontName="SimSun""#, "");
    let view = parse(&missing, ReadOptions::LENIENT).expect("lenient should accept");
    assert_eq!(view.fonts().next().unwrap().font_name(), None);
    assert!(matches!(
        parse(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
    assert!(matches!(view.to_xml(), Err(OfdError::Conformance(_))));
}

#[test]
fn unknown_container_is_strict_error() {
    let unknown = RES.replace("  <ofd:Fonts>", "  <ofd:VendorRes/><ofd:Fonts>");
    assert!(parse(&unknown, ReadOptions::LENIENT).is_ok());
    match parse(&unknown, ReadOptions::STRICT) {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::UnexpectedElement)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn lenient_normalizes_missing_2016_but_strict_rejects() {
    let dirty = RES.replace("http://www.ofdspec.org/2016", "http://www.ofdspec.org/");
    assert!(parse(&dirty, ReadOptions::LENIENT).is_ok());
    match parse(&dirty, ReadOptions::STRICT) {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::NamespaceVariant)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn package_parses_res_from_document_base() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/PublicRes.xml", RES.as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    package.parse_document().expect("Document");
    let res = package
        .parse_res_from_document("PublicRes.xml")
        .expect("PublicRes");
    assert_eq!(res.fonts().next().unwrap().id(), Some("3"));
}
