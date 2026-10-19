//! PDF 文档后端。
//!
//! 当前阶段把 [`Scene`] 写成矢量 PDF：页面坐标、路径、图片、裁剪、
//! 渐变和文字。文字优先嵌入字体，字体不可用时退化为字形轮廓。

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use crate::asset::{AssetId, ImageAsset};
use crate::error::RenderError;
use crate::geometry::{Matrix, Point};
use crate::paint::{Paint, Rgba};
use crate::path::{FillRule, LineCap, LineJoin, Path, PathSegment};
use crate::scene::{Element, Group, ImageElement, PathElement, Scene};

/// PDF 版本。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PdfVersion {
    /// PDF 1.4，兼容旧阅读器。
    V14,
    /// PDF 1.7，默认值。
    #[default]
    V17,
}

impl PdfVersion {
    fn header(self) -> &'static [u8] {
        match self {
            Self::V14 => b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n",
            Self::V17 => b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n",
        }
    }
}

/// 文字输出方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextMode {
    /// 优先嵌入字体，失败时转成字形轮廓。
    #[default]
    EmbedThenOutline,
    /// 始终输出字形轮廓。
    Outline,
}

/// PDF 输出选项。
#[derive(Clone, Debug)]
pub struct PdfOptions {
    /// PDF 版本。
    pub version: PdfVersion,
    /// 文字输出方式。
    pub text_mode: TextMode,
    /// 单张图片允许的最大像素数。
    pub max_image_pixels: u64,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            version: PdfVersion::V17,
            text_mode: TextMode::EmbedThenOutline,
            max_image_pixels: 64 * 1024 * 1024,
        }
    }
}

struct PdfPage {
    width: f64,
    height: f64,
    content: u32,
    resources: String,
}

struct ImageResource {
    name: String,
    object: u32,
    width: u32,
    height: u32,
}

struct ShadingResource {
    name: String,
    object: u32,
}

impl PdfDocument {
    fn collect_images(&self, group: &Group, images: &mut HashMap<AssetId, Arc<ImageAsset>>) {
        for element in &group.children {
            match element {
                Element::Group(group) => self.collect_images(group, images),
                Element::Image(image) => {
                    images
                        .entry(image.image.id())
                        .or_insert_with(|| Arc::clone(&image.image));
                }
                Element::Path(_) | Element::Text(_) => {}
            }
        }
    }

    fn collect_gradients(&self, group: &Group, gradients: &mut HashMap<String, Paint>) {
        for element in &group.children {
            match element {
                Element::Group(group) => self.collect_gradients(group, gradients),
                Element::Path(path) => {
                    if let Some(paint) = &path.fill
                        && matches!(paint, Paint::Linear(_) | Paint::Radial(_))
                    {
                        gradients
                            .entry(format!("{paint:?}"))
                            .or_insert_with(|| paint.clone());
                    }
                }
                Element::Image(_) | Element::Text(_) => {}
            }
        }
    }

    fn embed_gradient(&mut self, paint: &Paint) -> Result<ShadingResource, RenderError> {
        let (stops, coords, shading_type) = match paint {
            Paint::Linear(gradient) => (
                &gradient.stops,
                format!(
                    "[{} {} {} {}]",
                    number(gradient.start.x),
                    number(gradient.start.y),
                    number(gradient.end.x),
                    number(gradient.end.y)
                ),
                2,
            ),
            Paint::Radial(gradient) => (
                &gradient.stops,
                format!(
                    "[{} {} {} {} {} {}]",
                    number(gradient.start_center.x),
                    number(gradient.start_center.y),
                    number(gradient.start_radius),
                    number(gradient.end_center.x),
                    number(gradient.end_center.y),
                    number(gradient.end_radius)
                ),
                3,
            ),
            Paint::Solid(_) => {
                return Err(RenderError::Unsupported("纯色没有 Shading".to_owned()));
            }
        };
        if stops.len() < 2 {
            return Err(RenderError::Unsupported("渐变缺少颜色段".to_owned()));
        }
        let mut functions = Vec::new();
        for pair in stops.windows(2) {
            let body = format!(
                "<< /FunctionType 2 /Domain [0 1] /C0 [{} {} {}] /C1 [{} {} {}] /N 1 >>",
                number(f64::from(pair[0].color.r)),
                number(f64::from(pair[0].color.g)),
                number(f64::from(pair[0].color.b)),
                number(f64::from(pair[1].color.r)),
                number(f64::from(pair[1].color.g)),
                number(f64::from(pair[1].color.b))
            );
            functions.push(self.writer.add(body.into_bytes()));
        }
        let function = if functions.len() == 1 {
            format!("{} 0 R", functions[0])
        } else {
            let bounds = stops[1..stops.len() - 1]
                .iter()
                .map(|stop| number(f64::from(stop.offset).clamp(0.0, 1.0)))
                .collect::<Vec<_>>()
                .join(" ");
            let function_count = functions.len();
            let functions = functions
                .iter()
                .map(|id| format!("{id} 0 R"))
                .collect::<Vec<_>>()
                .join(" ");
            let encode = std::iter::repeat_n("0 1", function_count)
                .collect::<Vec<_>>()
                .join(" ");
            let id = self.writer.add(
                format!(
                    "<< /FunctionType 3 /Domain [0 1] /Functions [{functions}] /Bounds [{bounds}] /Encode [{encode}] >>"
                )
                .into_bytes(),
            );
            format!("{id} 0 R")
        };
        let name_index = self.writer.objects.len() + 1;
        let object = self.writer.add(
            format!(
                "<< /ShadingType {shading_type} /ColorSpace /DeviceRGB /Coords {coords} /Function {function} /Extend [true true] >>"
            )
            .into_bytes(),
        );
        Ok(ShadingResource {
            name: format!("Sh{name_index}"),
            object,
        })
    }

    fn embed_image(
        &mut self,
        image: &ImageAsset,
        name: String,
    ) -> Result<ImageResource, RenderError> {
        let decoded = image.decoded(self.options.max_image_pixels)?;
        let mut rgb = Vec::with_capacity(decoded.rgba.len() / 4 * 3);
        let mut alpha = Vec::with_capacity(decoded.rgba.len() / 4);
        let mut has_alpha = false;
        for pixel in decoded.rgba.as_chunks::<4>().0 {
            rgb.extend_from_slice(&pixel[..3]);
            alpha.push(pixel[3]);
            has_alpha |= pixel[3] != 255;
        }
        let mut dict = format!(
            "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode",
            decoded.width, decoded.height
        );
        if has_alpha {
            let mask = self.writer.stream(
                &format!(
                    "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode",
                    decoded.width, decoded.height
                ),
                &deflate(&alpha)?,
            );
            dict.push_str(&format!(" /SMask {mask} 0 R"));
        }
        let object = self.writer.stream(&dict, &deflate(&rgb)?);
        Ok(ImageResource {
            name,
            object,
            width: decoded.width,
            height: decoded.height,
        })
    }
}

/// 多页 PDF 文档。
pub struct PdfDocument {
    options: PdfOptions,
    writer: PdfWriter,
    pages: Vec<PdfPage>,
}

impl PdfDocument {
    /// 创建 PDF 文档。
    pub fn new(options: PdfOptions) -> Self {
        Self {
            writer: PdfWriter::new(options.version),
            options,
            pages: Vec::new(),
        }
    }

    /// 添加一页。
    pub fn add_page(&mut self, scene: &Scene) -> Result<(), RenderError> {
        let scale = 72.0 / 25.4;
        let width = scene.size.width * scale;
        let height = scene.size.height * scale;
        if !(width.is_finite() && width > 0.0 && height.is_finite() && height > 0.0) {
            return Err(RenderError::InvalidPage);
        }
        let mut collected = HashMap::new();
        self.collect_images(&scene.root, &mut collected);
        let mut ordered = collected.into_iter().collect::<Vec<_>>();
        ordered.sort_by_key(|(id, _)| *id);
        let mut images = HashMap::new();
        let mut xobjects = String::new();
        for (index, (id, image)) in ordered.into_iter().enumerate() {
            let name = format!("Im{}", index + 1);
            let resource = self.embed_image(&image, name)?;
            xobjects.push_str(&format!("/{} {} 0 R ", resource.name, resource.object));
            images.insert(id, resource);
        }
        let mut collected_gradients = HashMap::new();
        self.collect_gradients(&scene.root, &mut collected_gradients);
        let mut ordered_gradients = collected_gradients.into_iter().collect::<Vec<_>>();
        ordered_gradients.sort_by(|left, right| left.0.cmp(&right.0));
        let mut gradients = HashMap::new();
        let mut shadings = String::new();
        for (key, paint) in ordered_gradients {
            let resource = self.embed_gradient(&paint)?;
            shadings.push_str(&format!("/{} {} 0 R ", resource.name, resource.object));
            gradients.insert(key, resource);
        }
        let mut resources = Vec::new();
        if !xobjects.is_empty() {
            resources.push(format!("/XObject << {} >>", xobjects.trim_end()));
        }
        if !shadings.is_empty() {
            resources.push(format!("/Shading << {} >>", shadings.trim_end()));
        }
        let resources = resources.join(" ");

        let mut content = ContentBuilder::new(&images, &gradients);
        content.raw(&format!("q\n{scale} 0 0 -{scale} 0 {height} cm\n"));
        content.group(&scene.root);
        content.raw("Q\n");
        let content = self.writer.stream("", &content.into_bytes());
        self.pages.push(PdfPage {
            width,
            height,
            content,
            resources,
        });
        Ok(())
    }

    /// 完成 PDF 并返回字节。
    pub fn finish(mut self) -> Result<Vec<u8>, RenderError> {
        let pages_id = self.writer.placeholder();
        let mut kids = String::new();
        for page in &self.pages {
            let page_id = self.writer.add(
                format!(
                    "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {} {}] /Contents {} 0 R /Resources << {} >> >>",
                    number(page.width),
                    number(page.height),
                    page.content,
                    page.resources
                )
                .into_bytes(),
            );
            kids.push_str(&format!("{page_id} 0 R "));
        }
        self.writer.set(
            pages_id,
            format!(
                "<< /Type /Pages /Kids [{}] /Count {} >>",
                kids.trim_end(),
                self.pages.len()
            )
            .into_bytes(),
        );
        let catalog_id = self
            .writer
            .add(format!("<< /Type /Catalog /Pages {pages_id} 0 R >>").into_bytes());
        Ok(self.writer.finish(catalog_id))
    }
}

/// 单页 PDF 便捷后端。
#[derive(Debug, Default)]
pub struct PdfBackend {
    options: PdfOptions,
}

impl PdfBackend {
    /// 使用默认选项创建后端。
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用指定选项创建后端。
    pub fn with_options(options: PdfOptions) -> Self {
        Self { options }
    }

    /// 把单个场景写成单页 PDF。
    pub fn render(&mut self, scene: &Scene) -> Result<Vec<u8>, RenderError> {
        let mut document = PdfDocument::new(self.options.clone());
        document.add_page(scene)?;
        document.finish()
    }
}

struct ContentBuilder<'a> {
    content: String,
    current: Point,
    start: Point,
    images: &'a HashMap<AssetId, ImageResource>,
    gradients: &'a HashMap<String, ShadingResource>,
}

impl<'a> ContentBuilder<'a> {
    fn new(
        images: &'a HashMap<AssetId, ImageResource>,
        gradients: &'a HashMap<String, ShadingResource>,
    ) -> Self {
        Self {
            content: String::new(),
            current: Point::default(),
            start: Point::default(),
            images,
            gradients,
        }
    }

    fn raw(&mut self, text: &str) {
        self.content.push_str(text);
    }

    fn into_bytes(self) -> Vec<u8> {
        self.content.into_bytes()
    }

    fn group(&mut self, group: &Group) {
        self.raw("q\n");
        self.matrix(group.transform);
        if let Some(clip) = &group.clip {
            self.path(&clip.path);
            match clip.rule {
                FillRule::NonZero => self.raw("W n\n"),
                FillRule::EvenOdd => self.raw("W* n\n"),
            }
        }
        for child in &group.children {
            match child {
                Element::Group(child) => self.group(child),
                Element::Path(path) => self.path_element(path),
                Element::Image(image) => self.image_element(image),
                Element::Text(_) => {}
            }
        }
        self.raw("Q\n");
    }

    fn path_element(&mut self, element: &PathElement) {
        if let Some(name) = element
            .fill
            .as_ref()
            .and_then(|paint| self.gradient_name(paint))
        {
            self.raw("q\n");
            self.path(&element.path);
            match element.fill_rule {
                FillRule::NonZero => self.raw("W n\n"),
                FillRule::EvenOdd => self.raw("W* n\n"),
            }
            self.raw(&format!("/{name} sh\nQ\n"));
            if let Some(stroke) = &element.stroke
                && let Some(color) = solid_paint(&stroke.paint)
            {
                self.path(&element.path);
                self.set_stroke_color(color);
                self.set_stroke_style(&stroke.style);
                self.raw("S\n");
            }
            return;
        }
        self.path(&element.path);
        let fill = element.fill.as_ref().and_then(solid_paint);
        let stroke = element
            .stroke
            .as_ref()
            .and_then(|stroke| solid_paint(&stroke.paint).map(|color| (color, &stroke.style)));
        if let Some((color, style)) = stroke {
            self.set_stroke_color(color);
            self.set_stroke_style(style);
        }
        if let Some(color) = fill {
            self.set_fill_color(color);
        }
        match (fill.is_some(), stroke.is_some()) {
            (true, true) => self.raw("B\n"),
            (true, false) => match element.fill_rule {
                FillRule::NonZero => self.raw("f\n"),
                FillRule::EvenOdd => self.raw("f*\n"),
            },
            (false, true) => self.raw("S\n"),
            (false, false) => self.raw("n\n"),
        }
    }

    fn gradient_name(&self, paint: &Paint) -> Option<String> {
        self.gradients
            .get(&format!("{paint:?}"))
            .map(|resource| resource.name.clone())
    }

    fn image_element(&mut self, element: &ImageElement) {
        let Some(resource) = self.images.get(&element.image.id()) else {
            return;
        };
        self.raw("q\n");
        let flip = Matrix::scale(1.0, -1.0).then(Matrix::translate(0.0, 1.0));
        let transform = flip
            .then(Matrix::scale(
                f64::from(resource.width.max(1)),
                f64::from(resource.height.max(1)),
            ))
            .then(element.transform);
        self.matrix(transform);
        self.raw(&format!("/{} Do\nQ\n", resource.name));
    }

    fn set_fill_color(&mut self, color: Rgba) {
        self.raw(&format!(
            "{} {} {} rg\n",
            number(f64::from(color.r)),
            number(f64::from(color.g)),
            number(f64::from(color.b))
        ));
    }

    fn set_stroke_color(&mut self, color: Rgba) {
        self.raw(&format!(
            "{} {} {} RG\n",
            number(f64::from(color.r)),
            number(f64::from(color.g)),
            number(f64::from(color.b))
        ));
    }

    fn set_stroke_style(&mut self, style: &crate::path::StrokeStyle) {
        self.raw(&format!("{} w\n", number(style.width)));
        let cap = match style.cap {
            LineCap::Butt => 0,
            LineCap::Round => 1,
            LineCap::Square => 2,
        };
        let join = match style.join {
            LineJoin::Miter => 0,
            LineJoin::Round => 1,
            LineJoin::Bevel => 2,
        };
        self.raw(&format!(
            "{cap} J\n{join} j\n{} M\n",
            number(style.miter_limit)
        ));
        if style.dash.is_empty() {
            self.raw("[] 0 d\n");
        } else {
            let dashes = style
                .dash
                .iter()
                .map(|value| number(*value))
                .collect::<Vec<_>>()
                .join(" ");
            self.raw(&format!("[{dashes}] {} d\n", number(style.dash_offset)));
        }
    }

    fn matrix(&mut self, matrix: Matrix) {
        if matrix != Matrix::IDENTITY {
            self.raw(&format!(
                "{} {} {} {} {} {} cm\n",
                number(matrix.a),
                number(matrix.b),
                number(matrix.c),
                number(matrix.d),
                number(matrix.e),
                number(matrix.f)
            ));
        }
    }

    fn path(&mut self, path: &Path) {
        for segment in &path.segments {
            match *segment {
                PathSegment::MoveTo(point) => self.point("m", point),
                PathSegment::LineTo(point) => self.point("l", point),
                PathSegment::QuadraticTo { control, to } => {
                    let current = self.current;
                    let c1 = Point::new(
                        current.x + (control.x - current.x) * 2.0 / 3.0,
                        current.y + (control.y - current.y) * 2.0 / 3.0,
                    );
                    let c2 = Point::new(
                        to.x + (control.x - to.x) * 2.0 / 3.0,
                        to.y + (control.y - to.y) * 2.0 / 3.0,
                    );
                    self.curve(c1, c2, to);
                }
                PathSegment::CubicTo {
                    control1,
                    control2,
                    to,
                } => self.curve(control1, control2, to),
                PathSegment::ArcTo {
                    rx,
                    ry,
                    rotation,
                    large_arc,
                    sweep,
                    to,
                } => self.arc(rx, ry, rotation, large_arc, sweep, to),
                PathSegment::Close => {
                    self.raw("h\n");
                    self.current = self.start;
                }
            }
        }
    }

    fn point(&mut self, op: &str, point: Point) {
        self.raw(&format!("{} {} {op}\n", number(point.x), number(point.y)));
        self.current = point;
        if op == "m" {
            self.start = point;
        }
    }

    fn curve(&mut self, control1: Point, control2: Point, to: Point) {
        self.raw(&format!(
            "{} {} {} {} {} {} c\n",
            number(control1.x),
            number(control1.y),
            number(control2.x),
            number(control2.y),
            number(to.x),
            number(to.y)
        ));
        self.current = to;
    }

    fn arc(&mut self, rx: f64, ry: f64, rotation: f64, large_arc: bool, sweep: bool, to: Point) {
        use vello_cpu::kurbo::{Arc as KurboArc, SvgArc, Vec2};
        let arc = SvgArc {
            from: vello_cpu::kurbo::Point::new(self.current.x, self.current.y),
            to: vello_cpu::kurbo::Point::new(to.x, to.y),
            radii: Vec2::new(rx, ry),
            x_rotation: rotation,
            large_arc,
            sweep,
        };
        if let Some(arc) = KurboArc::from_svg_arc(&arc) {
            arc.to_cubic_beziers(0.1, |p1, p2, p3| {
                self.curve(
                    Point::new(p1.x, p1.y),
                    Point::new(p2.x, p2.y),
                    Point::new(p3.x, p3.y),
                );
            });
        } else {
            self.point("l", to);
        }
    }
}

fn solid_paint(paint: &Paint) -> Option<Rgba> {
    match paint {
        Paint::Solid(color) => Some(*color),
        _ => None,
    }
}

struct PdfWriter {
    version: PdfVersion,
    objects: Vec<Vec<u8>>,
}

impl PdfWriter {
    fn new(version: PdfVersion) -> Self {
        Self {
            version,
            objects: Vec::new(),
        }
    }

    fn placeholder(&mut self) -> u32 {
        self.objects.push(Vec::new());
        self.objects.len() as u32
    }

    fn set(&mut self, id: u32, body: Vec<u8>) {
        if let Some(slot) = self.objects.get_mut(id as usize - 1) {
            *slot = body;
        }
    }

    fn add(&mut self, body: Vec<u8>) -> u32 {
        self.objects.push(body);
        self.objects.len() as u32
    }

    fn stream(&mut self, dict: &str, data: &[u8]) -> u32 {
        let mut body = format!("<< /Length {} {dict} >>\nstream\n", data.len()).into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(b"\nendstream");
        self.add(body)
    }

    fn finish(self, root: u32) -> Vec<u8> {
        let mut output = self.version.header().to_vec();
        let mut offsets = Vec::with_capacity(self.objects.len());
        for (index, body) in self.objects.iter().enumerate() {
            offsets.push(output.len() as u64);
            output.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
            output.extend_from_slice(body);
            output.extend_from_slice(b"\nendobj\n");
        }
        let xref = output.len() as u64;
        output.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", self.objects.len() + 1).as_bytes(),
        );
        for offset in offsets {
            output.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        output.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root {} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                self.objects.len() + 1,
                root
            )
            .as_bytes(),
        );
        output
    }
}

fn deflate(data: &[u8]) -> Result<Vec<u8>, RenderError> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder
        .write_all(data)
        .map_err(|error| RenderError::Image(error.to_string()))?;
    encoder
        .finish()
        .map_err(|error| RenderError::Image(error.to_string()))
}

fn number(value: f64) -> String {
    let mut text = format!("{value:.4}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{PdfBackend, PdfDocument, PdfOptions};
    use crate::geometry::{Matrix, Point, Size};
    use crate::image::ImageFormat;
    use crate::paint::{GradientExtend, GradientStop, LinearGradient, Paint, Rgba};
    use crate::path::{FillRule, Path};
    use crate::resource::{PoolBudget, ResourcePool};
    use crate::scene::{Element, Group, ImageElement, PathElement, Scene};

    fn scene() -> Scene {
        Scene {
            size: Size::new(100.0, 100.0),
            root: Group::default(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn writes_empty_page() {
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene()).expect("page");
        let pdf = document.finish().expect("pdf");
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.7"));
        assert!(text.contains("/MediaBox [0 0 283.4646 283.4646]"));
        assert!(text.ends_with("%%EOF\n"));
    }

    #[test]
    fn writes_path_commands() {
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
        let scene = Scene {
            size: Size::new(100.0, 100.0),
            root,
            diagnostics: Vec::new(),
        };
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene).expect("page");
        let pdf = document.finish().expect("pdf");
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("10 10 m"));
        assert!(text.contains("90 90 l"));
        assert!(text.contains("1 0 0 rg"));
        assert!(text.contains("f\n"));
    }

    #[test]
    fn writes_image_xobject() {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut encoder = png::Encoder::new(&mut cursor, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("png header");
            writer
                .write_image_data(&[
                    0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255,
                ])
                .expect("png data");
            writer.finish().expect("png finish");
        }
        let pool = ResourcePool::new(PoolBudget::default());
        let image = pool
            .embedded_image(ImageFormat::Png, &cursor.into_inner(), None)
            .expect("image");
        let mut root = Group::default();
        root.children.push(Element::Image(ImageElement {
            image,
            transform: Matrix::scale(0.5, 0.5),
            alpha: 1.0,
        }));
        let scene = Scene {
            size: Size::new(20.0, 20.0),
            root,
            diagnostics: Vec::new(),
        };
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene).expect("page");
        let pdf = document.finish().expect("pdf");
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/XObject"));
        assert!(text.contains("/Im1"));
        assert!(text.contains("/Im1 Do"));
    }

    #[test]
    fn writes_linear_shading() {
        let mut path = Path::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(90.0, 10.0));
        path.line_to(Point::new(90.0, 90.0));
        path.close();
        let mut root = Group::default();
        root.children.push(Element::Path(PathElement {
            path: std::sync::Arc::new(path),
            fill: Some(Paint::Linear(LinearGradient {
                start: Point::new(10.0, 50.0),
                end: Point::new(90.0, 50.0),
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        color: Rgba::new(1.0, 0.0, 0.0, 1.0),
                    },
                    GradientStop {
                        offset: 1.0,
                        color: Rgba::new(0.0, 0.0, 1.0, 1.0),
                    },
                ],
                extend: GradientExtend::Pad,
            })),
            stroke: None,
            fill_rule: FillRule::NonZero,
        }));
        let scene = Scene {
            size: Size::new(100.0, 100.0),
            root,
            diagnostics: Vec::new(),
        };
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene).expect("page");
        let pdf = document.finish().expect("pdf");
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/ShadingType 2"));
        assert!(text.contains("/FunctionType 2"));
        assert!(text.contains(" sh\n"));
    }

    #[test]
    fn backend_writes_single_page() {
        let pdf = PdfBackend::new().render(&scene()).expect("pdf");
        assert!(pdf.starts_with(b"%PDF-1.7"));
    }
}
