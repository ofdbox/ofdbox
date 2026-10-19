use std::io::{Cursor, Write};
use std::sync::{Arc, Mutex};

use ofdbox_core::{OfdPackage, ReadOptions};
use ofdbox_render::{
    Backend, BackendFrame, Element, Group, Matrix, OfdFrontend, Paint, PdfBackend, PdfFrontend,
    Point, PoolBudget, RasterTarget, RenderOptions, ResourcePool, Rgba, Scene, Size,
    VelloCpuBackend, WgpuBackend,
};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" DocType="OFD" Version="1.0">
  <ofd:DocBody><ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot></ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData><ofd:PublicRes>PublicRes.xml</ofd:PublicRes></ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;

const CONTENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PathObject ID="p1" Boundary="0 0 100 100" Fill="true" Stroke="false">
      <ofd:FillColor ColorSpace="1" Value="255 0 0"/>
      <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 100 L 0 100 C</ofd:AbbreviatedData>
    </ofd:PathObject>
    <ofd:ImageObject ID="i1" Boundary="10 10 20 20" CTM="20 0 0 20 0 0" ResourceID="2"/>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;

const PUBLIC_RES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:ColorSpaces><ofd:ColorSpace ID="1" Type="RGB"/></ofd:ColorSpaces>
  <ofd:MultiMedias>
    <ofd:MultiMedia ID="2" Type="Image" Format="PNG"><ofd:MediaFile>Images/img.png</ofd:MediaFile></ofd:MultiMedia>
  </ofd:MultiMedias>
</ofd:Res>"#;

fn blue_png() -> Vec<u8> {
    #[derive(Clone, Default)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);
    impl Write for SharedWriter {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("lock").extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let output = SharedWriter::default();
    let mut encoder = png::Encoder::new(output.clone(), 2, 2);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer
        .write_image_data(&[
            0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255,
        ])
        .expect("png data");
    writer.finish().expect("png finish");
    output.0.lock().expect("lock").clone()
}

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .expect("entry");
        writer.write_all(data).expect("write");
    }
    writer.finish().expect("zip").into_inner()
}

fn synthetic_package(content: &str) -> Vec<u8> {
    let png = blue_png();
    zip(&[
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT_XML.as_bytes()),
        ("Doc_0/PublicRes.xml", PUBLIC_RES_XML.as_bytes()),
        ("Doc_0/Images/img.png", &png),
        ("Doc_0/Pages/Page_0/Content.xml", content.as_bytes()),
    ])
}

fn open(bytes: Vec<u8>) -> OfdPackage {
    OfdPackage::from_bytes_with_options(bytes, ReadOptions::LENIENT, Default::default())
        .expect("open package")
}

#[test]
fn renders_path_and_image() {
    let mut package = open(synthetic_package(CONTENT_XML));
    let document = package.parse_document().expect("document");
    let page = document
        .pages()
        .expect("pages")
        .pages()
        .next()
        .expect("page");
    let pool = ResourcePool::new(PoolBudget::default());
    let options = RenderOptions {
        dpi: 25.4,
        ..Default::default()
    };
    let scene = OfdFrontend::new(&mut package, &pool, options)
        .build_page(&page)
        .expect("scene");
    assert_eq!(scene.size, Size::new(100.0, 100.0));

    let mut target = RasterTarget::from_size(scene.size, 25.4).expect("target");
    let mut backend = VelloCpuBackend::with_defaults();
    let mut frame = backend.begin_frame((), &mut target).expect("frame");
    frame.draw(&scene).expect("draw");
    frame.finish().expect("finish");

    assert_eq!(pixel(&target, 50, 50), [255, 0, 0, 255]);
    assert_eq!(pixel(&target, 20, 20), [0, 0, 255, 255]);
    // 图片边界之外恢复为路径填充色。
    assert_eq!(pixel(&target, 5, 5), [255, 0, 0, 255]);
}

fn pixel(target: &RasterTarget, x: u16, y: u16) -> [u8; 4] {
    let index = (usize::from(y) * usize::from(target.width()) + usize::from(x)) * 4;
    target.data()[index..index + 4]
        .try_into()
        .expect("rgba pixel")
}

#[test]
fn finds_case_insensitive_resources() {
    const RES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:ColorSpaces><ofd:ColorSpace ID="1" Type="RGB"/></ofd:ColorSpaces>
  <ofd:MultiMedias>
    <ofd:MultiMedia ID="2" Type="Image" Format="PNG"><ofd:MediaFile>Images/IMG.PNG</ofd:MediaFile></ofd:MultiMedia>
  </ofd:MultiMedias>
</ofd:Res>"#;
    const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:ImageObject ID="i1" Boundary="10 10 20 20" ResourceID="2"/>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let png = blue_png();
    let bytes = zip(&[
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT_XML.as_bytes()),
        ("Doc_0/PublicRes.xml", RES.as_bytes()),
        ("Doc_0/Images/img.png", &png),
        ("Doc_0/Pages/Page_0/Content.xml", CONTENT.as_bytes()),
    ]);
    let mut package = open(bytes);
    let document = package.parse_document().expect("document");
    let page = document
        .pages()
        .expect("pages")
        .pages()
        .next()
        .expect("page");
    let pool = ResourcePool::new(PoolBudget::default());
    let scene = OfdFrontend::new(&mut package, &pool, RenderOptions::default())
        .build_page(&page)
        .expect("scene");
    assert!(scene.diagnostics.is_empty(), "{:?}", scene.diagnostics);
    assert!(count_images(&scene) > 0);
}

fn count_images(scene: &Scene) -> usize {
    fn walk(group: &Group) -> usize {
        group
            .children
            .iter()
            .map(|element| match element {
                Element::Group(group) => walk(group),
                Element::Image(_) => 1,
                _ => 0,
            })
            .sum()
    }
    walk(&scene.root)
}

#[test]
fn extracts_glyphs_without_parsable_font() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:TextObject ID="t1" Boundary="0 0 100 20" Font="3" Size="10">
      <ofd:FillColor ColorSpace="1" Value="0 0 0"/>
      <ofd:CGTransform CodePosition="0" CodeCount="2" GlyphCount="2"><ofd:Glyphs>5 6</ofd:Glyphs></ofd:CGTransform>
      <ofd:TextCode X="1" Y="10" DeltaX="5 5">AB</ofd:TextCode>
    </ofd:TextObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let res = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:ColorSpaces><ofd:ColorSpace ID="1" Type="RGB"/></ofd:ColorSpaces>
  <ofd:Fonts><ofd:Font ID="3" FontName="Dummy"><ofd:FontFile>Fonts/dummy.ttf</ofd:FontFile></ofd:Font></ofd:Fonts>
</ofd:Res>"#;
    let bytes = zip(&[
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT_XML.as_bytes()),
        ("Doc_0/PublicRes.xml", res.as_bytes()),
        ("Doc_0/Fonts/dummy.ttf", b"not-a-real-font"),
        ("Doc_0/Pages/Page_0/Content.xml", content.as_bytes()),
    ]);
    let mut package = open(bytes);
    let document = package.parse_document().expect("document");
    let page = document
        .pages()
        .expect("pages")
        .pages()
        .next()
        .expect("page");
    let pool = ResourcePool::new(PoolBudget::default());
    let scene = OfdFrontend::new(&mut package, &pool, RenderOptions::default())
        .build_page(&page)
        .expect("scene");
    let texts = collect_text(&scene);
    assert_eq!(texts, vec![vec![(5, 1.0, 10.0), (6, 6.0, 10.0)]]);
}

fn collect_text(scene: &Scene) -> Vec<Vec<(u32, f32, f32)>> {
    fn walk(group: &Group, output: &mut Vec<Vec<(u32, f32, f32)>>) {
        for element in &group.children {
            match element {
                Element::Group(group) => walk(group, output),
                Element::Text(text) => output.push(
                    text.glyphs
                        .iter()
                        .map(|glyph| (glyph.id, glyph.x, glyph.y))
                        .collect(),
                ),
                _ => {}
            }
        }
    }
    let mut output = Vec::new();
    walk(&scene.root, &mut output);
    output
}

type TextRun = (Matrix, Vec<(u32, f32, f32)>);

fn collect_text_runs(scene: &Scene) -> Vec<TextRun> {
    fn walk(group: &Group, output: &mut Vec<TextRun>) {
        for element in &group.children {
            match element {
                Element::Group(group) => walk(group, output),
                Element::Text(text) => output.push((
                    text.transform,
                    text.glyphs
                        .iter()
                        .map(|glyph| (glyph.id, glyph.x, glyph.y))
                        .collect(),
                )),
                _ => {}
            }
        }
    }
    let mut output = Vec::new();
    walk(&scene.root, &mut output);
    output
}

const TEXT_RES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:ColorSpaces><ofd:ColorSpace ID="1" Type="RGB"/></ofd:ColorSpaces>
  <ofd:Fonts><ofd:Font ID="3" FontName="Dummy"><ofd:FontFile>Fonts/dummy.ttf</ofd:FontFile></ofd:Font></ofd:Fonts>
</ofd:Res>"#;

fn render_text(content: &str) -> Scene {
    let bytes = zip(&[
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT_XML.as_bytes()),
        ("Doc_0/PublicRes.xml", TEXT_RES.as_bytes()),
        ("Doc_0/Fonts/dummy.ttf", b"not-a-real-font"),
        ("Doc_0/Pages/Page_0/Content.xml", content.as_bytes()),
    ]);
    let mut package = open(bytes);
    let document = package.parse_document().expect("document");
    let page = document
        .pages()
        .expect("pages")
        .pages()
        .next()
        .expect("page");
    let pool = ResourcePool::new(PoolBudget::default());
    OfdFrontend::new(&mut package, &pool, RenderOptions::default())
        .build_page(&page)
        .expect("scene")
}

#[test]
fn inherits_text_code_origin() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:TextObject ID="t1" Boundary="0 0 100 20" Font="3" Size="10">
      <ofd:CGTransform CodePosition="0" CodeCount="1" GlyphCount="1"><ofd:Glyphs>5</ofd:Glyphs></ofd:CGTransform>
      <ofd:TextCode X="3" Y="7">A</ofd:TextCode>
      <ofd:CGTransform CodePosition="1" CodeCount="1" GlyphCount="1"><ofd:Glyphs>6</ofd:Glyphs></ofd:CGTransform>
      <ofd:TextCode>B</ofd:TextCode>
    </ofd:TextObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let scene = render_text(content);
    assert_eq!(
        collect_text(&scene),
        vec![vec![(5, 3.0, 7.0)], vec![(6, 3.0, 7.0)]]
    );
}

#[test]
fn vertical_text_advances_downward() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:TextObject ID="t1" Boundary="0 0 100 100" Font="3" Size="10" ReadDirection="90">
      <ofd:CGTransform CodePosition="0" CodeCount="2" GlyphCount="2"><ofd:Glyphs>5 6</ofd:Glyphs></ofd:CGTransform>
      <ofd:TextCode X="1" Y="2">AB</ofd:TextCode>
    </ofd:TextObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let scene = render_text(content);
    assert_eq!(
        collect_text(&scene),
        vec![vec![(5, 1.0, 2.0), (6, 1.0, 12.0)]]
    );
}

#[test]
fn char_direction_rotates_glyphs() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:TextObject ID="t1" Boundary="0 0 100 100" Font="3" Size="10" CharDirection="90">
      <ofd:CGTransform CodePosition="0" CodeCount="1" GlyphCount="1"><ofd:Glyphs>5</ofd:Glyphs></ofd:CGTransform>
      <ofd:TextCode X="10" Y="20">A</ofd:TextCode>
    </ofd:TextObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let scene = render_text(content);
    let runs = collect_text_runs(&scene);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].1, vec![(5, 0.0, 0.0)]);
    let origin = runs[0].0.apply(Point::new(1.0, 0.0));
    assert!((origin.x - 10.0).abs() < 1e-6);
    assert!((origin.y - 21.0).abs() < 1e-6);
}

#[test]
fn hscale_scales_glyph_and_advance() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:TextObject ID="t1" Boundary="0 0 100 100" Font="3" Size="10" HScale="0.5">
      <ofd:CGTransform CodePosition="0" CodeCount="2" GlyphCount="2"><ofd:Glyphs>5 6</ofd:Glyphs></ofd:CGTransform>
      <ofd:TextCode X="0" Y="0">AB</ofd:TextCode>
    </ofd:TextObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let scene = render_text(content);
    let runs = collect_text_runs(&scene);
    assert_eq!(runs.len(), 2);
    let first = runs[0].0.apply(Point::new(1.0, 0.0));
    let second = runs[1].0.apply(Point::new(1.0, 0.0));
    assert!((first.x - 0.5).abs() < 1e-6);
    assert!((second.x - 5.5).abs() < 1e-6);
}

fn collect_text_stroke_widths(scene: &Scene) -> Vec<Option<f64>> {
    fn walk(group: &Group, output: &mut Vec<Option<f64>>) {
        for element in &group.children {
            match element {
                Element::Group(group) => walk(group, output),
                Element::Text(text) => {
                    output.push(text.stroke.as_ref().map(|stroke| stroke.style.width));
                }
                _ => {}
            }
        }
    }
    let mut output = Vec::new();
    walk(&scene.root, &mut output);
    output
}

#[test]
fn weight_bolds_text() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:TextObject ID="t1" Boundary="0 0 100 100" Font="3" Size="10" Weight="700">
      <ofd:CGTransform CodePosition="0" CodeCount="1" GlyphCount="1"><ofd:Glyphs>5</ofd:Glyphs></ofd:CGTransform>
      <ofd:TextCode X="0" Y="0">A</ofd:TextCode>
    </ofd:TextObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let scene = render_text(content);
    let widths = collect_text_stroke_widths(&scene);
    assert_eq!(widths.len(), 1);
    assert!(widths[0].is_some_and(|width| width > 0.0));
}

#[test]
fn italic_shears_glyphs() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:TextObject ID="t1" Boundary="0 0 100 100" Font="3" Size="10" Italic="true">
      <ofd:CGTransform CodePosition="0" CodeCount="1" GlyphCount="1"><ofd:Glyphs>5</ofd:Glyphs></ofd:CGTransform>
      <ofd:TextCode X="0" Y="0">A</ofd:TextCode>
    </ofd:TextObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let scene = render_text(content);
    let runs = collect_text_runs(&scene);
    assert_eq!(runs.len(), 1);
    let sheared = runs[0].0.apply(Point::new(0.0, -10.0));
    assert!(sheared.x > 1.0, "{sheared:?}");
    assert!((sheared.y + 10.0).abs() < 1e-6, "{sheared:?}");
}

#[test]
fn empty_text_with_missing_font_is_silent() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:TextObject ID="t1" Boundary="0 0 100 100" Font="99" Size="10">
      <ofd:TextCode X="0" Y="0"></ofd:TextCode>
    </ofd:TextObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let scene = render_text(content);
    assert!(scene.diagnostics.is_empty(), "{:?}", scene.diagnostics);
}

#[test]
fn cgtransform_one_to_many() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:TextObject ID="t1" Boundary="0 0 100 100" Font="3" Size="10">
      <ofd:CGTransform CodePosition="0" CodeCount="1" GlyphCount="2"><ofd:Glyphs>5 6</ofd:Glyphs></ofd:CGTransform>
      <ofd:TextCode X="0" Y="0">A</ofd:TextCode>
    </ofd:TextObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let scene = render_text(content);
    assert_eq!(
        collect_text(&scene),
        vec![vec![(5, 0.0, 0.0), (6, 10.0, 0.0)]]
    );
}

#[test]
fn cgtransform_many_to_one() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:TextObject ID="t1" Boundary="0 0 100 100" Font="3" Size="10">
      <ofd:CGTransform CodePosition="0" CodeCount="2" GlyphCount="1"><ofd:Glyphs>5</ofd:Glyphs></ofd:CGTransform>
      <ofd:TextCode X="0" Y="0">AB</ofd:TextCode>
    </ofd:TextObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let scene = render_text(content);
    assert_eq!(collect_text(&scene), vec![vec![(5, 0.0, 0.0)]]);
}

#[test]
fn cgtransform_many_to_many() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:TextObject ID="t1" Boundary="0 0 100 100" Font="3" Size="10">
      <ofd:CGTransform CodePosition="0" CodeCount="2" GlyphCount="3"><ofd:Glyphs>5 6 7</ofd:Glyphs></ofd:CGTransform>
      <ofd:TextCode X="0" Y="0">AB</ofd:TextCode>
    </ofd:TextObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let scene = render_text(content);
    assert_eq!(
        collect_text(&scene),
        vec![vec![(5, 0.0, 0.0), (6, 10.0, 0.0), (7, 20.0, 0.0)]]
    );
}

#[test]
fn placeholders_report_unsupported() {
    let scene = Scene {
        size: Size::new(1.0, 1.0),
        root: Group::default(),
        diagnostics: Vec::new(),
    };
    assert!(PdfBackend::new().render(&scene).is_err());
    assert!(WgpuBackend::new().draw(&scene).is_err());
    assert!(PdfFrontend::new().build_scene().is_err());
}

#[test]
fn renders_page_block_and_template() {
    const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:PublicRes>PublicRes.xml</ofd:PublicRes>
    <ofd:TemplatePage ID="T1" BaseLoc="Templates/T1.xml" ZOrder="Background"/>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;
    const TEMPLATE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PathObject ID="t1" Boundary="0 0 100 100" Fill="true" Stroke="false">
      <ofd:FillColor ColorSpace="1" Value="0 255 0"/>
      <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 100 L 0 100 C</ofd:AbbreviatedData>
    </ofd:PathObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Template TemplateID="T1"/>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PageBlock ID="pb">
      <ofd:PathObject ID="p1" Boundary="20 20 20 20" Fill="true" Stroke="false">
        <ofd:FillColor ColorSpace="1" Value="255 0 0"/>
        <ofd:AbbreviatedData>M 0 0 L 20 0 L 20 20 L 0 20 C</ofd:AbbreviatedData>
      </ofd:PathObject>
    </ofd:PageBlock>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let bytes = zip(&[
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/PublicRes.xml", PUBLIC_RES_XML.as_bytes()),
        ("Doc_0/Templates/T1.xml", TEMPLATE.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", CONTENT.as_bytes()),
    ]);
    let mut package = open(bytes);
    let document = package.parse_document().expect("document");
    let page = document
        .pages()
        .expect("pages")
        .pages()
        .next()
        .expect("page");
    let pool = ResourcePool::new(PoolBudget::default());
    let options = RenderOptions {
        dpi: 25.4,
        ..Default::default()
    };
    let scene = OfdFrontend::new(&mut package, &pool, options)
        .build_page(&page)
        .expect("scene");
    assert_eq!(scene.size, Size::new(100.0, 100.0));

    let mut target = RasterTarget::from_size(scene.size, 25.4).expect("target");
    let mut backend = VelloCpuBackend::with_defaults();
    let mut frame = backend.begin_frame((), &mut target).expect("frame");
    frame.draw(&scene).expect("draw");
    frame.finish().expect("finish");

    assert_eq!(pixel(&target, 30, 30), [255, 0, 0, 255]);
    assert_eq!(pixel(&target, 75, 75), [0, 255, 0, 255]);
}

fn render_content(content: &str) -> RasterTarget {
    let mut package = open(synthetic_package(content));
    let document = package.parse_document().expect("document");
    let page = document
        .pages()
        .expect("pages")
        .pages()
        .next()
        .expect("page");
    let pool = ResourcePool::new(PoolBudget::default());
    let options = RenderOptions {
        dpi: 25.4,
        ..Default::default()
    };
    let scene = OfdFrontend::new(&mut package, &pool, options)
        .build_page(&page)
        .expect("scene");
    let mut target = RasterTarget::from_size(scene.size, 25.4).expect("target");
    let mut backend = VelloCpuBackend::with_defaults();
    let mut frame = backend.begin_frame((), &mut target).expect("frame");
    frame.draw(&scene).expect("draw");
    frame.finish().expect("finish");
    target
}

fn radial_content(eccentricity: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PathObject ID="p1" Boundary="0 0 100 100" Fill="true" Stroke="false">
      <ofd:FillColor>
        <ofd:RadialShd StartPoint="50 50" EndPoint="50 50" EndRadius="50" Eccentricity="{eccentricity}" Angle="0">
          <ofd:Segment Position="0"><ofd:Color Value="255 0 0"/></ofd:Segment>
          <ofd:Segment Position="1"><ofd:Color Value="0 0 255"/></ofd:Segment>
        </ofd:RadialShd>
      </ofd:FillColor>
      <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 100 L 0 100 C</ofd:AbbreviatedData>
    </ofd:PathObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#
    )
}

type PathStyle = (Option<Rgba>, Option<f64>);

fn collect_path_styles(scene: &Scene) -> Vec<PathStyle> {
    fn walk(group: &Group, output: &mut Vec<PathStyle>) {
        for element in &group.children {
            match element {
                Element::Group(group) => walk(group, output),
                Element::Path(path) => {
                    let fill = match &path.fill {
                        Some(Paint::Solid(color)) => Some(*color),
                        _ => None,
                    };
                    output.push((fill, path.stroke.as_ref().map(|stroke| stroke.style.width)));
                }
                _ => {}
            }
        }
    }
    let mut output = Vec::new();
    walk(&scene.root, &mut output);
    output
}

#[test]
fn inherits_draw_param_and_layer_param() {
    const RES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:ColorSpaces><ofd:ColorSpace ID="1" Type="RGB"/></ofd:ColorSpaces>
  <ofd:DrawParams>
    <ofd:DrawParam ID="1" LineWidth="2">
      <ofd:FillColor Value="0 255 0"/>
      <ofd:StrokeColor Value="255 0 0"/>
    </ofd:DrawParam>
    <ofd:DrawParam ID="2" Relative="1" LineWidth="0.5"/>
    <ofd:DrawParam ID="3" Relative="1">
      <ofd:FillColor Value="0 0 255"/>
    </ofd:DrawParam>
  </ofd:DrawParams>
</ofd:Res>"#;
    const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 80</ofd:PhysicalBox></ofd:Area>
  <ofd:Content>
    <ofd:Layer ID="1" DrawParam="2">
      <ofd:PathObject ID="p1" Boundary="0 0 100 40" Fill="true" Stroke="true">
        <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 40 L 0 40 C</ofd:AbbreviatedData>
      </ofd:PathObject>
      <ofd:PathObject ID="p2" Boundary="0 40 100 40" Fill="true" Stroke="false" DrawParam="3">
        <ofd:AbbreviatedData>M 0 40 L 100 40 L 100 80 L 0 80 C</ofd:AbbreviatedData>
      </ofd:PathObject>
    </ofd:Layer>
  </ofd:Content>
</ofd:Page>"#;
    let bytes = zip(&[
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT_XML.as_bytes()),
        ("Doc_0/PublicRes.xml", RES.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", CONTENT.as_bytes()),
    ]);
    let mut package = open(bytes);
    let document = package.parse_document().expect("document");
    let page = document
        .pages()
        .expect("pages")
        .pages()
        .next()
        .expect("page");
    let pool = ResourcePool::new(PoolBudget::default());
    let scene = OfdFrontend::new(&mut package, &pool, RenderOptions::default())
        .build_page(&page)
        .expect("scene");
    let styles = collect_path_styles(&scene);
    assert_eq!(styles.len(), 2);
    assert_eq!(styles[0].0, Some(Rgba::new(0.0, 1.0, 0.0, 1.0)));
    assert!((styles[0].1.unwrap() - 0.5).abs() < 1e-9);
    assert_eq!(styles[1].0, Some(Rgba::new(0.0, 0.0, 1.0, 1.0)));
}

#[test]
fn empty_fill_color_defaults_to_black() {
    const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 40</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PathObject ID="p1" Boundary="0 0 100 40" Fill="true" Stroke="false">
      <ofd:FillColor/>
      <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 40 L 0 40 C</ofd:AbbreviatedData>
    </ofd:PathObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let target = render_content(CONTENT);
    assert_eq!(pixel(&target, 50, 20), [0, 0, 0, 255]);
}

#[test]
fn clip_areas_are_united() {
    const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 40</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PathObject ID="p1" Boundary="0 0 100 40" Fill="true" Stroke="false">
      <ofd:Clips>
        <ofd:Clip>
          <ofd:Area>
            <ofd:PathObject ID="c1" Boundary="10 10 20 20" Fill="true">
              <ofd:AbbreviatedData>M 10 10 L 30 10 L 30 30 L 10 30 C</ofd:AbbreviatedData>
            </ofd:PathObject>
          </ofd:Area>
          <ofd:Area>
            <ofd:PathObject ID="c2" Boundary="60 10 20 20" Fill="true">
              <ofd:AbbreviatedData>M 60 10 L 80 10 L 80 30 L 60 30 C</ofd:AbbreviatedData>
            </ofd:PathObject>
          </ofd:Area>
        </ofd:Clip>
      </ofd:Clips>
      <ofd:FillColor Value="255 0 0"/>
      <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 40 L 0 40 C</ofd:AbbreviatedData>
    </ofd:PathObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let target = render_content(CONTENT);
    assert_eq!(pixel(&target, 20, 20), [255, 0, 0, 255]);
    assert_eq!(pixel(&target, 70, 20), [255, 0, 0, 255]);
    assert_eq!(pixel(&target, 50, 20), [255, 255, 255, 255]);
}

#[test]
fn tiles_pattern_fill() {
    const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 30</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PathObject ID="p1" Boundary="0 0 100 20" Fill="true" Stroke="false">
      <ofd:FillColor>
        <ofd:Pattern Width="10" Height="10" XStep="10" YStep="10" RelativeTo="Object">
          <ofd:CellContent>
            <ofd:PathObject ID="cell" Boundary="0 0 5 10" Fill="true" Stroke="false">
              <ofd:FillColor Value="255 0 0"/>
              <ofd:AbbreviatedData>M 0 0 L 5 0 L 5 10 L 0 10 C</ofd:AbbreviatedData>
            </ofd:PathObject>
          </ofd:CellContent>
        </ofd:Pattern>
      </ofd:FillColor>
      <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 20 L 0 20 C</ofd:AbbreviatedData>
    </ofd:PathObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let target = render_content(CONTENT);
    assert_eq!(pixel(&target, 2, 5), [255, 0, 0, 255]);
    assert_eq!(pixel(&target, 7, 5), [255, 255, 255, 255]);
    assert_eq!(pixel(&target, 12, 5), [255, 0, 0, 255]);
}

fn radial_repeat_content(map_type: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 200 150</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PathObject ID="p1" Boundary="0 0 200 150" Fill="true" Stroke="false">
      <ofd:FillColor>
        <ofd:RadialShd StartPoint="40 70" EndPoint="50 90" StartRadius="10" EndRadius="70" MapType="{map_type}">
          <ofd:Segment Position="0"><ofd:Color Value="255 255 0"/></ofd:Segment>
          <ofd:Segment Position="1"><ofd:Color Value="0 0 255"/></ofd:Segment>
        </ofd:RadialShd>
      </ofd:FillColor>
      <ofd:AbbreviatedData>M 0 0 L 200 0 L 200 150 L 0 150 C</ofd:AbbreviatedData>
    </ofd:PathObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#
    )
}

#[test]
fn renders_radial_repeat_and_reflect() {
    let repeat = render_content(&radial_repeat_content("Repeat"));
    let reflect = render_content(&radial_repeat_content("Reflect"));
    assert_ne!(pixel(&repeat, 40, 70), [255, 255, 255, 255]);
    assert_ne!(pixel(&repeat, 120, 100), pixel(&reflect, 120, 100));
}

#[test]
fn radial_extend_zero_clips_to_end_ellipse() {
    const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 40</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PathObject ID="p1" Boundary="0 0 100 40" Fill="true" Stroke="false">
      <ofd:FillColor>
        <ofd:RadialShd StartPoint="50 20" EndPoint="50 20" StartRadius="0" EndRadius="25">
          <ofd:Segment Position="0"><ofd:Color Value="255 255 0"/></ofd:Segment>
          <ofd:Segment Position="1"><ofd:Color Value="0 0 255"/></ofd:Segment>
        </ofd:RadialShd>
      </ofd:FillColor>
      <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 40 L 0 40 C</ofd:AbbreviatedData>
    </ofd:PathObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let target = render_content(CONTENT);
    let center = pixel(&target, 50, 20);
    assert!(center[0] > 200 && center[1] > 200, "{center:?}");
    assert_eq!(pixel(&target, 90, 20), [255, 255, 255, 255]);
}

#[test]
fn axial_extend_zero_clips_to_axis_band() {
    const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 40</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PathObject ID="p1" Boundary="0 0 100 40" Fill="true" Stroke="false">
      <ofd:FillColor>
        <ofd:AxialShd StartPoint="20 20" EndPoint="80 20" Extend="0">
          <ofd:Segment Position="0"><ofd:Color Value="255 0 0"/></ofd:Segment>
          <ofd:Segment Position="1"><ofd:Color Value="0 0 255"/></ofd:Segment>
        </ofd:AxialShd>
      </ofd:FillColor>
      <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 40 L 0 40 C</ofd:AbbreviatedData>
    </ofd:PathObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let target = render_content(CONTENT);
    assert_eq!(pixel(&target, 10, 20), [255, 255, 255, 255]);
    let middle = pixel(&target, 50, 20);
    assert!(middle[0] > 100 && middle[2] > 100, "{middle:?}");
    assert_eq!(pixel(&target, 90, 20), [255, 255, 255, 255]);
}

#[test]
fn interpolates_gouraud_triangle() {
    const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PathObject ID="p1" Boundary="0 0 100 100" Fill="true" Stroke="false">
      <ofd:FillColor>
        <ofd:GouraudShd>
          <ofd:Point X="0" Y="0"><ofd:Color Value="255 0 0"/></ofd:Point>
          <ofd:Point X="100" Y="0"><ofd:Color Value="0 255 0"/></ofd:Point>
          <ofd:Point X="0" Y="100"><ofd:Color Value="0 0 255"/></ofd:Point>
        </ofd:GouraudShd>
      </ofd:FillColor>
      <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 100 L 0 100 C</ofd:AbbreviatedData>
    </ofd:PathObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let target = render_content(CONTENT);
    let center = pixel(&target, 33, 33);
    assert!(center[0] > 60 && center[0] < 120, "{center:?}");
    assert!(center[1] > 60 && center[1] < 120, "{center:?}");
    assert!(center[2] > 60 && center[2] < 120, "{center:?}");
}

#[test]
fn renders_lagouraud_grid() {
    const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PathObject ID="p1" Boundary="0 0 100 100" Fill="true" Stroke="false">
      <ofd:FillColor>
        <ofd:LaGourandShd VerticesPerRow="2">
          <ofd:Point X="0" Y="0"><ofd:Color Value="255 0 0"/></ofd:Point>
          <ofd:Point X="100" Y="0"><ofd:Color Value="255 0 0"/></ofd:Point>
          <ofd:Point X="0" Y="100"><ofd:Color Value="255 0 0"/></ofd:Point>
          <ofd:Point X="100" Y="100"><ofd:Color Value="255 0 0"/></ofd:Point>
        </ofd:LaGourandShd>
      </ofd:FillColor>
      <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 100 L 0 100 C</ofd:AbbreviatedData>
    </ofd:PathObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let target = render_content(CONTENT);
    assert_eq!(pixel(&target, 25, 75), [255, 0, 0, 255]);
    assert_eq!(pixel(&target, 75, 25), [255, 0, 0, 255]);
}

#[test]
fn renders_elliptical_radial_gradient() {
    let circle = render_content(&radial_content("0"));
    assert_eq!(pixel(&circle, 90, 50), pixel(&circle, 50, 90));

    let ellipse = render_content(&radial_content("0.8"));
    assert_ne!(pixel(&ellipse, 90, 50), pixel(&ellipse, 50, 90));
}

#[test]
fn uses_default_color_space_for_palette_index() {
    const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:PageArea><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:PageArea>
    <ofd:PublicRes>PublicRes.xml</ofd:PublicRes>
    <ofd:DefaultCS>1</ofd:DefaultCS>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;
    const RES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:ColorSpaces>
    <ofd:ColorSpace ID="1" Type="RGB">
      <ofd:Palette><ofd:CV>#ff #00 #00</ofd:CV></ofd:Palette>
    </ofd:ColorSpace>
  </ofd:ColorSpaces>
</ofd:Res>"#;
    const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 100 100</ofd:PhysicalBox></ofd:Area>
  <ofd:Content><ofd:Layer ID="1">
    <ofd:PathObject ID="p1" Boundary="0 0 100 100" Fill="true" Stroke="false">
      <ofd:FillColor Index="0"/>
      <ofd:AbbreviatedData>M 0 0 L 100 0 L 100 100 L 0 100 C</ofd:AbbreviatedData>
    </ofd:PathObject>
  </ofd:Layer></ofd:Content>
</ofd:Page>"#;
    let bytes = zip(&[
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/PublicRes.xml", RES.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", CONTENT.as_bytes()),
    ]);
    let mut package = open(bytes);
    let document = package.parse_document().expect("document");
    let page = document
        .pages()
        .expect("pages")
        .pages()
        .next()
        .expect("page");
    let pool = ResourcePool::new(PoolBudget::default());
    let options = RenderOptions {
        dpi: 25.4,
        ..Default::default()
    };
    let scene = OfdFrontend::new(&mut package, &pool, options)
        .build_page(&page)
        .expect("scene");

    let mut target = RasterTarget::from_size(scene.size, 25.4).expect("target");
    let mut backend = VelloCpuBackend::with_defaults();
    let mut frame = backend.begin_frame((), &mut target).expect("frame");
    frame.draw(&scene).expect("draw");
    frame.finish().expect("finish");

    assert_eq!(pixel(&target, 50, 50), [255, 0, 0, 255]);
}
