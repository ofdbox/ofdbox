use std::io::{Cursor, Write};
use std::sync::{Arc, Mutex};

use ofdbox_core::{OfdPackage, ReadOptions};
use ofdbox_render::{
    Backend, BackendFrame, Element, Group, OfdFrontend, PdfBackend, PdfFrontend, PoolBudget,
    RasterTarget, RenderOptions, ResourcePool, Scene, Size, VelloCpuBackend, WgpuBackend,
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
