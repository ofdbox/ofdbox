//! OFD 后端：把 [`Scene`] 写成最小但可回读的 OFD 包。

use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::sync::Arc;

use crate::asset::{AssetId, FontAsset, ImageAsset};
use crate::error::RenderError;
use crate::geometry::{Matrix, Point};
use crate::image::ImageFormat;
use crate::paint::{Paint, Rgba};
use crate::path::{FillRule, LineCap, LineJoin, Path, PathSegment, StrokeStyle};
use crate::scene::{Element, Group, ImageElement, PathElement, Scene, TextElement};

/// OFD 后端选项。
#[derive(Clone, Debug)]
pub struct OfdOptions {
    /// 生成工具名称。
    pub creator: String,
}

impl Default for OfdOptions {
    fn default() -> Self {
        Self {
            creator: "OFDBox".to_owned(),
        }
    }
}

/// OFD 后端。
#[derive(Debug, Default)]
pub struct OfdBackend {
    options: OfdOptions,
}

struct FontEntry {
    id: u32,
    file: String,
    bytes: Arc<[u8]>,
}

struct ImageEntry {
    id: u32,
    file: String,
    bytes: Arc<[u8]>,
}

#[allow(dead_code)]
struct PageWriter<'a> {
    next_id: &'a mut u32,
    fonts: &'a mut HashMap<AssetId, u32>,
    font_entries: &'a mut Vec<FontEntry>,
    images: &'a mut HashMap<AssetId, u32>,
    image_entries: &'a mut Vec<ImageEntry>,
    content: String,
    page_width: f64,
    page_height: f64,
}

impl OfdBackend {
    /// 创建默认后端。
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用指定选项。
    pub fn with_options(options: OfdOptions) -> Self {
        Self { options }
    }

    /// 把一页写成 OFD。
    pub fn render(&mut self, scene: &Scene) -> Result<Vec<u8>, RenderError> {
        self.render_pages(std::slice::from_ref(scene))
    }

    /// 把多页写成 OFD。
    pub fn render_pages(&mut self, scenes: &[Scene]) -> Result<Vec<u8>, RenderError> {
        if scenes.is_empty() {
            return Err(RenderError::InvalidPage);
        }
        let mut next_id = 1_u32;
        let mut fonts = HashMap::new();
        let mut font_entries = Vec::new();
        let mut images = HashMap::new();
        let mut image_entries = Vec::new();
        let mut pages = Vec::new();
        for scene in scenes {
            next_id += 1;
            let layer_id = next_id;
            next_id += 1;
            let mut writer = PageWriter {
                next_id: &mut next_id,
                fonts: &mut fonts,
                font_entries: &mut font_entries,
                images: &mut images,
                image_entries: &mut image_entries,
                content: String::new(),
                page_width: scene.size.width,
                page_height: scene.size.height,
            };
            writer.group(&scene.root, Matrix::IDENTITY);
            let content = writer.content;
            pages.push(page_xml(
                layer_id,
                scene.size.width,
                scene.size.height,
                &content,
            ));
        }
        let mut public_res = String::new();
        if !font_entries.is_empty() {
            public_res.push_str("<ofd:PublicRes>PublicRes.xml</ofd:PublicRes>");
        }
        let mut document_res = String::new();
        if !image_entries.is_empty() {
            document_res.push_str("<ofd:DocumentRes>DocumentRes.xml</ofd:DocumentRes>");
        }
        let page_refs = pages
            .iter()
            .enumerate()
            .map(|(index, _)| {
                format!(
                    "<ofd:Page ID=\"{}\" BaseLoc=\"Pages/Page_{}/Content.xml\"/>",
                    index + 1,
                    index
                )
            })
            .collect::<Vec<_>>()
            .join("");
        let document = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:Document xmlns:ofd=\"http://www.ofdspec.org/2016\"><ofd:CommonData><ofd:MaxUnitID>{next_id}</ofd:MaxUnitID><ofd:PageArea><ofd:PhysicalBox>0 0 {} {}</ofd:PhysicalBox></ofd:PageArea>{public_res}{document_res}</ofd:CommonData><ofd:Pages>{page_refs}</ofd:Pages></ofd:Document>",
            scenes[0].size.width, scenes[0].size.height
        );
        let ofd = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:OFD xmlns:ofd=\"http://www.ofdspec.org/2016\" Version=\"1.1\" DocType=\"OFD\"><ofd:DocBody><ofd:DocInfo><ofd:DocID>{}</ofd:DocID><ofd:Creator>{}</ofd:Creator></ofd:DocInfo><ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot></ofd:DocBody></ofd:OFD>",
            document_id(),
            xml_escape(&self.options.creator)
        );
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        write_entry(&mut zip, "OFD.xml", ofd.as_bytes(), options)?;
        write_entry(&mut zip, "Doc_0/Document.xml", document.as_bytes(), options)?;
        for (index, page) in pages.iter().enumerate() {
            write_entry(
                &mut zip,
                &format!("Doc_0/Pages/Page_{index}/Content.xml"),
                page.as_bytes(),
                options,
            )?;
        }
        if !font_entries.is_empty() {
            let mut resources = String::from(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:Res xmlns:ofd=\"http://www.ofdspec.org/2016\" BaseLoc=\"Res\"><ofd:Fonts>",
            );
            for font in &font_entries {
                resources.push_str(&format!(
                    "<ofd:Font ID=\"{}\" FontName=\"Font{}\" FamilyName=\"Font{}\" Charset=\"unicode\"><ofd:FontFile>{}</ofd:FontFile></ofd:Font>",
                    font.id, font.id, font.id, xml_escape(&font.file)
                ));
            }
            resources.push_str("</ofd:Fonts></ofd:Res>");
            write_entry(
                &mut zip,
                "Doc_0/PublicRes.xml",
                resources.as_bytes(),
                options,
            )?;
            for font in &font_entries {
                write_entry(
                    &mut zip,
                    &format!("Doc_0/Res/{}", font.file),
                    &font.bytes,
                    options,
                )?;
            }
        }
        if !image_entries.is_empty() {
            let mut resources = String::from(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:Res xmlns:ofd=\"http://www.ofdspec.org/2016\" BaseLoc=\"Res\"><ofd:MultiMedias>",
            );
            for image in &image_entries {
                resources.push_str(&format!(
                    "<ofd:MultiMedia ID=\"{}\" Type=\"Image\"><ofd:MediaFile>{}</ofd:MediaFile></ofd:MultiMedia>",
                    image.id,
                    xml_escape(&image.file)
                ));
            }
            resources.push_str("</ofd:MultiMedias></ofd:Res>");
            write_entry(
                &mut zip,
                "Doc_0/DocumentRes.xml",
                resources.as_bytes(),
                options,
            )?;
            for image in &image_entries {
                write_entry(
                    &mut zip,
                    &format!("Doc_0/Res/{}", image.file),
                    &image.bytes,
                    options,
                )?;
            }
        }
        let cursor = zip
            .finish()
            .map_err(|error| RenderError::Backend(error.to_string()))?;
        Ok(cursor.into_inner())
    }
}

impl PageWriter<'_> {
    fn group(&mut self, group: &Group, parent: Matrix) {
        let transform = group.transform.then(parent);
        for element in &group.children {
            match element {
                Element::Group(child) => self.group(child, transform),
                Element::Path(path) => self.path(path, transform),
                Element::Text(text) => self.text(text, transform),
                Element::Image(image) => self.image(image, transform),
            }
        }
    }

    fn path(&mut self, element: &PathElement, transform: Matrix) {
        if self.content.is_empty() {
            eprintln!("path transform {:?}", transform);
        }
        if element.path.is_empty() {
            return;
        }
        let id = self.id();
        let boundary = (0.0, 0.0, self.page_width, self.page_height);
        let mut attributes = format!(
            "ID=\"{id}\" Boundary=\"{}\" CTM=\"{}\"",
            box_text(boundary),
            matrix_text(transform.linear())
        );
        let mut colors = String::new();
        if let Some(fill) = &element.fill {
            attributes.push_str(" Fill=\"true\"");
            if let Some(color) = solid_color(fill) {
                colors.push_str(&format!("<ofd:FillColor Value=\"{}\"/>", color_text(color)));
            }
        } else {
            attributes.push_str(" Fill=\"false\"");
        }
        if let Some(stroke) = &element.stroke {
            attributes.push_str(" Stroke=\"true\"");
            if let Some(color) = solid_color(&stroke.paint) {
                colors.push_str(&format!(
                    "<ofd:StrokeColor Value=\"{}\"/>",
                    color_text(color)
                ));
            }
            attributes.push_str(&stroke_attributes(&stroke.style));
        } else {
            attributes.push_str(" Stroke=\"false\"");
        }
        self.content.push_str(&format!(
            "<ofd:PathObject {attributes}>{colors}<ofd:AbbreviatedData>{}</ofd:AbbreviatedData></ofd:PathObject>",
            path_data_transformed(&element.path, transform, boundary)
        ));
    }

    fn text(&mut self, element: &TextElement, transform: Matrix) {
        if let Some(text) = &element.text {
            self.text_native(element, transform, text);
        } else {
            self.text_outline(element, transform);
        }
    }

    fn text_native(&mut self, element: &TextElement, transform: Matrix, text: &str) {
        if text.is_empty() {
            return;
        }
        let id = self.id();
        let font = self.font(&element.font);
        let ctm = element.transform.then(transform);
        let boundary = (0.0, 0.0, self.page_width, self.page_height);
        let first = element
            .glyphs
            .first()
            .copied()
            .unwrap_or(crate::scene::PositionedGlyph {
                id: 0,
                x: 0.0,
                y: 0.0,
            });
        let attributes = format!(
            "ID=\"{id}\" Boundary=\"{}\" CTM=\"{}\" Font=\"{font}\" Size=\"{}\"",
            box_text(boundary),
            matrix_text(ctm),
            number(element.size)
        );
        let mut colors = String::new();
        if let Some(fill) = &element.fill
            && let Some(color) = solid_color(fill)
        {
            colors.push_str(&format!("<ofd:FillColor Value=\"{}\"/>", color_text(color)));
        }
        if let Some(stroke) = &element.stroke
            && let Some(color) = solid_color(&stroke.paint)
        {
            colors.push_str(&format!(
                "<ofd:StrokeColor Value=\"{}\"/>",
                color_text(color)
            ));
        }
        self.content.push_str(&format!(
            "<ofd:TextObject {attributes}>{colors}<ofd:TextCode X=\"{}\" Y=\"{}\">{}</ofd:TextCode></ofd:TextObject>",
            number(f64::from(first.x)),
            number(f64::from(first.y)),
            xml_escape(text)
        ));
    }

    fn text_outline(&mut self, element: &TextElement, transform: Matrix) {
        let Ok(face) = ttf_parser::Face::parse(element.font.data(), element.font.face_index())
        else {
            return;
        };
        let units = f64::from(face.units_per_em());
        if units == 0.0 {
            return;
        }
        let scale = element.size / units;
        for glyph in &element.glyphs {
            let Ok(id) = u16::try_from(glyph.id) else {
                continue;
            };
            let mut builder = GlyphPathBuilder::default();
            if face
                .outline_glyph(ttf_parser::GlyphId(id), &mut builder)
                .is_none()
                || builder.path.is_empty()
            {
                continue;
            }
            let glyph_transform = Matrix::scale(scale, -scale)
                .then(Matrix::translate(f64::from(glyph.x), f64::from(glyph.y)))
                .then(element.transform)
                .then(transform);
            let boundary = (0.0, 0.0, self.page_width, self.page_height);
            let mut attributes = String::new();
            let mut colors = String::new();
            if let Some(fill) = &element.fill {
                attributes.push_str(" Fill=\"true\"");
                if let Some(color) = solid_color(fill) {
                    colors.push_str(&format!("<ofd:FillColor Value=\"{}\"/>", color_text(color)));
                }
            }
            if let Some(stroke) = &element.stroke {
                attributes.push_str(" Stroke=\"true\"");
                if let Some(color) = solid_color(&stroke.paint) {
                    colors.push_str(&format!(
                        "<ofd:StrokeColor Value=\"{}\"/>",
                        color_text(color)
                    ));
                }
                attributes.push_str(&stroke_attributes(&stroke.style));
            }
            let object = self.id();
            self.content.push_str(&format!(
                "<ofd:PathObject ID=\"{object}\" Boundary=\"{}\" CTM=\"1 0 0 1 0 0\"{attributes}>{colors}<ofd:AbbreviatedData>{}</ofd:AbbreviatedData></ofd:PathObject>",
                box_text(boundary),
                path_data_transformed(&builder.path, glyph_transform, boundary)
            ));
        }
    }
    fn image(&mut self, element: &ImageElement, transform: Matrix) {
        let id = self.id();
        let resource = self.image_resource(&element.image);
        let (width, height) = element
            .image
            .decoded(u64::MAX)
            .map(|decoded| {
                (
                    f64::from(decoded.width.max(1)),
                    f64::from(decoded.height.max(1)),
                )
            })
            .unwrap_or((1.0, 1.0));
        let boundary =
            transform_bounds(element.transform.then(transform), (0.0, 0.0, width, height));
        let ctm = Matrix::scale(width, height)
            .then(element.transform)
            .then(transform);
        self.content.push_str(&format!(
            "<ofd:ImageObject ID=\"{id}\" Boundary=\"{}\" CTM=\"{}\" ResourceID=\"{resource}\"/>",
            box_text(boundary),
            matrix_text(ctm)
        ));
    }

    fn id(&mut self) -> u32 {
        let id = *self.next_id;
        *self.next_id += 1;
        id
    }

    #[allow(dead_code)]
    fn font(&mut self, font: &Arc<FontAsset>) -> u32 {
        let id = font.id();
        if let Some(resource) = self.fonts.get(&id) {
            return *resource;
        }
        let next = self.id();
        let file = format!("Font_{next}.ttf");
        self.fonts.insert(id, next);
        self.font_entries.push(FontEntry {
            id: next,
            file,
            bytes: Arc::from(font.data()),
        });
        next
    }

    fn image_resource(&mut self, image: &Arc<ImageAsset>) -> u32 {
        let id = image.id();
        if let Some(resource) = self.images.get(&id) {
            return *resource;
        }
        let next = self.id();
        let extension = match image.format() {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Tiff => "tif",
            ImageFormat::Jbig2 => "jb2",
            ImageFormat::Other(name) => name.as_str(),
        };
        let file = format!("Image_{next}.{extension}");
        self.images.insert(id, next);
        self.image_entries.push(ImageEntry {
            id: next,
            file,
            bytes: Arc::from(image.data()),
        });
        next
    }
}

fn page_xml(layer_id: u32, width: f64, height: f64, content: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:Page xmlns:ofd=\"http://www.ofdspec.org/2016\"><ofd:Area><ofd:PhysicalBox>0 0 {} {}</ofd:PhysicalBox></ofd:Area><ofd:Content><ofd:Layer ID=\"{layer_id}\" Type=\"Body\">{content}</ofd:Layer></ofd:Content></ofd:Page>",
        number(width),
        number(height)
    )
}

#[allow(dead_code)]
fn path_data(path: &Path, _rule: FillRule) -> String {
    let mut output = String::new();
    for segment in &path.segments {
        match segment {
            PathSegment::MoveTo(point) => {
                output.push_str(&format!("M {} {} ", number(point.x), number(point.y)));
            }
            PathSegment::LineTo(point) => {
                output.push_str(&format!("L {} {} ", number(point.x), number(point.y)));
            }
            PathSegment::QuadraticTo { control, to } => {
                output.push_str(&format!(
                    "Q {} {} {} {} ",
                    number(control.x),
                    number(control.y),
                    number(to.x),
                    number(to.y)
                ));
            }
            PathSegment::CubicTo {
                control1,
                control2,
                to,
            } => {
                output.push_str(&format!(
                    "B {} {} {} {} {} {} ",
                    number(control1.x),
                    number(control1.y),
                    number(control2.x),
                    number(control2.y),
                    number(to.x),
                    number(to.y)
                ));
            }
            PathSegment::ArcTo {
                rx,
                ry,
                rotation,
                large_arc,
                sweep,
                to,
            } => {
                output.push_str(&format!(
                    "A {} {} {} {} {} {} {} ",
                    number(*rx),
                    number(*ry),
                    number(rotation.to_degrees()),
                    u8::from(*large_arc),
                    u8::from(*sweep),
                    number(to.x),
                    number(to.y)
                ));
            }
            PathSegment::Close => output.push_str("C "),
        }
    }
    output.trim_end().to_owned()
}

#[derive(Default)]
struct GlyphPathBuilder {
    path: Path,
}

impl ttf_parser::OutlineBuilder for GlyphPathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(Point::new(f64::from(x), f64::from(y)));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(Point::new(f64::from(x), f64::from(y)));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path.quadratic_to(
            Point::new(f64::from(x1), f64::from(y1)),
            Point::new(f64::from(x), f64::from(y)),
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path.cubic_to(
            Point::new(f64::from(x1), f64::from(y1)),
            Point::new(f64::from(x2), f64::from(y2)),
            Point::new(f64::from(x), f64::from(y)),
        );
    }
    fn close(&mut self) {
        self.path.close();
    }
}
fn path_data_transformed(path: &Path, transform: Matrix, boundary: (f64, f64, f64, f64)) -> String {
    let point = |value: Point| {
        let value = transform.apply(value);
        format!(
            "{} {}",
            number(value.x - boundary.0),
            number(value.y - boundary.1)
        )
    };
    let mut output = String::new();
    for segment in &path.segments {
        match segment {
            PathSegment::MoveTo(value) => output.push_str(&format!("M {} ", point(*value))),
            PathSegment::LineTo(value) => output.push_str(&format!("L {} ", point(*value))),
            PathSegment::QuadraticTo { control, to } => {
                output.push_str(&format!("Q {} {} ", point(*control), point(*to)))
            }
            PathSegment::CubicTo {
                control1,
                control2,
                to,
            } => output.push_str(&format!(
                "B {} {} {} ",
                point(*control1),
                point(*control2),
                point(*to)
            )),
            PathSegment::ArcTo { to, .. } => {
                output.push_str(&format!("A 0 0 0 0 0 {} ", point(*to)))
            }
            PathSegment::Close => output.push_str("C "),
        }
    }
    output.trim_end().to_owned()
}
#[allow(dead_code)]
fn path_boundary(path: &Path, transform: Matrix) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut add = |point: Point| {
        let point = transform.apply(point);
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    };
    for segment in &path.segments {
        match segment {
            PathSegment::MoveTo(point) | PathSegment::LineTo(point) => add(*point),
            PathSegment::QuadraticTo { control, to } => {
                add(*control);
                add(*to);
            }
            PathSegment::CubicTo {
                control1,
                control2,
                to,
            } => {
                add(*control1);
                add(*control2);
                add(*to);
            }
            PathSegment::ArcTo { to, .. } => add(*to),
            PathSegment::Close => {}
        }
    }
    if min_x.is_finite() {
        (
            min_x,
            min_y,
            (max_x - min_x).max(0.01),
            (max_y - min_y).max(0.01),
        )
    } else {
        (0.0, 0.0, 0.0, 0.0)
    }
}

fn transform_bounds(transform: Matrix, bounds: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let points = [
        transform.apply(Point::new(bounds.0, bounds.1)),
        transform.apply(Point::new(bounds.0 + bounds.2, bounds.1)),
        transform.apply(Point::new(bounds.0, bounds.1 + bounds.3)),
        transform.apply(Point::new(bounds.0 + bounds.2, bounds.1 + bounds.3)),
    ];
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

#[allow(dead_code)]
fn glyph_text(font: &FontAsset, glyphs: &[crate::scene::PositionedGlyph]) -> Option<String> {
    let face = ttf_parser::Face::parse(font.data(), font.face_index()).ok()?;
    let cmap = face.tables().cmap?;
    let mut reverse = HashMap::<u16, char>::new();
    for subtable in cmap.subtables {
        if !subtable.is_unicode() {
            continue;
        }
        subtable.codepoints(|codepoint| {
            if let Some(character) = char::from_u32(codepoint)
                && let Some(glyph) = subtable.glyph_index(codepoint)
            {
                reverse.entry(glyph.0).or_insert(character);
            }
        });
    }
    let mut output = String::new();
    for glyph in glyphs {
        let id = u16::try_from(glyph.id).ok()?;
        output.push(*reverse.get(&id).unwrap_or(&'\u{FFFD}'));
    }
    Some(output)
}

fn solid_color(paint: &Paint) -> Option<Rgba> {
    match paint {
        Paint::Solid(color) => Some(*color),
        Paint::Linear(gradient) => gradient.stops.first().map(|stop| stop.color),
        Paint::Radial(gradient) => gradient.stops.first().map(|stop| stop.color),
    }
}

fn color_text(color: Rgba) -> String {
    format!(
        "{} {} {}",
        (color.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.b.clamp(0.0, 1.0) * 255.0).round() as u8
    )
}

fn stroke_attributes(style: &StrokeStyle) -> String {
    let mut attributes = format!(" LineWidth=\"{}\"", number(style.width));
    attributes.push_str(&format!(
        " Cap=\"{}\" Join=\"{}\" MiterLimit=\"{}\"",
        match style.cap {
            LineCap::Butt => "Butt",
            LineCap::Round => "Round",
            LineCap::Square => "Square",
        },
        match style.join {
            LineJoin::Miter => "Miter",
            LineJoin::Round => "Round",
            LineJoin::Bevel => "Bevel",
        },
        number(style.miter_limit)
    ));
    if !style.dash.is_empty() {
        attributes.push_str(&format!(
            " DashPattern=\"{}\" DashOffset=\"{}\"",
            style
                .dash
                .iter()
                .map(|value| number(*value))
                .collect::<Vec<_>>()
                .join(" "),
            number(style.dash_offset)
        ));
    }
    attributes
}

fn box_text(bounds: (f64, f64, f64, f64)) -> String {
    format!(
        "{} {} {} {}",
        number(bounds.0),
        number(bounds.1),
        number(bounds.2),
        number(bounds.3)
    )
}

trait LinearMatrix {
    fn linear(self) -> Matrix;
}

impl LinearMatrix for Matrix {
    fn linear(self) -> Matrix {
        Matrix::new(self.a, self.b, self.c, self.d, 0.0, 0.0)
    }
}
fn matrix_text(matrix: Matrix) -> String {
    format!(
        "{} {} {} {} {} {}",
        number(matrix.a),
        number(matrix.b),
        number(matrix.c),
        number(matrix.d),
        number(matrix.e),
        number(matrix.f)
    )
}

fn number(value: f64) -> String {
    let mut text = format!("{value:.4}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text == "-0" { "0".to_owned() } else { text }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn document_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{nanos:032x}")
}

fn write_entry(
    zip: &mut zip::ZipWriter<Cursor<Vec<u8>>>,
    name: &str,
    data: &[u8],
    options: zip::write::SimpleFileOptions,
) -> Result<(), RenderError> {
    zip.start_file(name, options)
        .map_err(|error| RenderError::Backend(error.to_string()))?;
    zip.write_all(data)
        .map_err(|error| RenderError::Backend(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::OfdBackend;
    use crate::paint::{Paint, Rgba};
    use crate::path::{FillRule, Path};
    use crate::resource::{PoolBudget, ResourcePool};
    use crate::scene::{Element, Group, PathElement, PositionedGlyph, Scene, TextElement};
    use crate::{Matrix, Point, Size};
    use ofdbox_core::OfdPackage;

    fn scene() -> Scene {
        let mut path = Path::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(90.0, 10.0));
        path.line_to(Point::new(90.0, 90.0));
        path.close();
        let mut root = Group::default();
        root.children.push(Element::Path(PathElement {
            path: std::sync::Arc::new(path),
            fill: Some(Paint::Solid(Rgba::new(1.0, 0.0, 0.0, 1.0))),
            stroke: None,
            fill_rule: FillRule::NonZero,
        }));
        if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/arial.ttf") {
            let pool = ResourcePool::new(PoolBudget::default());
            let font = pool.embedded_font(&bytes, 0).expect("font");
            let face = ttf_parser::Face::parse(&bytes, 0).expect("face");
            let glyph = face.glyph_index('A').expect("glyph");
            root.children.push(Element::Text(TextElement {
                font,
                size: 10.0,
                transform: Matrix::translate(10.0, 50.0),
                glyphs: vec![PositionedGlyph {
                    id: u32::from(glyph.0),
                    x: 0.0,
                    y: 0.0,
                }],
                text: None,
                fill: Some(Paint::Solid(Rgba::BLACK)),
                stroke: None,
            }));
        }
        Scene {
            size: Size::new(100.0, 100.0),
            root,
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn writes_readable_package() {
        let bytes = OfdBackend::new().render(&scene()).expect("ofd");
        assert!(bytes.starts_with(b"PK"));
        let mut package = OfdPackage::from_bytes(bytes).expect("package");
        let names = package.entry_names().expect("names");
        assert!(names.iter().any(|name| name == "OFD.xml"));
        assert!(names.iter().any(|name| name == "Doc_0/Document.xml"));
        assert!(
            names
                .iter()
                .any(|name| name == "Doc_0/Pages/Page_0/Content.xml")
        );
        let content = package
            .read_entry("Doc_0/Pages/Page_0/Content.xml")
            .expect("content");
        let content = String::from_utf8_lossy(&content);
        assert!(content.contains("PathObject"));
        assert!(content.contains("AbbreviatedData"));
        assert!(content.matches("PathObject").count() >= 2);
        package.parse_document().expect("document");
    }
}
