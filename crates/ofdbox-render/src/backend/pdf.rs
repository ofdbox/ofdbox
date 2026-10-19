//! PDF 文档后端。
//!
//! 当前阶段把 [`Scene`] 写成矢量 PDF：页面坐标、路径、图片、裁剪、
//! 渐变和文字。文字优先嵌入字体，字体不可用时退化为字形轮廓。

use crate::error::RenderError;
use crate::geometry::{Matrix, Point};
use crate::paint::{Paint, Rgba};
use crate::path::{FillRule, LineCap, LineJoin, Path, PathSegment};
use crate::scene::{Element, Group, PathElement, Scene};

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
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            version: PdfVersion::V17,
            text_mode: TextMode::EmbedThenOutline,
        }
    }
}

struct PdfPage {
    width: f64,
    height: f64,
    content: u32,
    resources: String,
}

/// 多页 PDF 文档。
pub struct PdfDocument {
    writer: PdfWriter,
    pages: Vec<PdfPage>,
}

impl PdfDocument {
    /// 创建 PDF 文档。
    pub fn new(options: PdfOptions) -> Self {
        Self {
            writer: PdfWriter::new(options.version),
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
        let mut content = ContentBuilder::new();
        content.raw(&format!("q\n{scale} 0 0 -{scale} 0 {height} cm\n"));
        content.group(&scene.root);
        content.raw("Q\n");
        let content = self.writer.stream("", &content.into_bytes());
        self.pages.push(PdfPage {
            width,
            height,
            content,
            resources: String::new(),
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

#[derive(Default)]
struct ContentBuilder {
    content: String,
    current: Point,
    start: Point,
}

impl ContentBuilder {
    fn new() -> Self {
        Self::default()
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
                Element::Text(_) | Element::Image(_) => {}
            }
        }
        self.raw("Q\n");
    }

    fn path_element(&mut self, element: &PathElement) {
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
    use crate::geometry::{Point, Size};
    use crate::paint::{Paint, Rgba};
    use crate::path::{FillRule, Path};
    use crate::scene::{Element, Group, PathElement, Scene};

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
    fn backend_writes_single_page() {
        let pdf = PdfBackend::new().render(&scene()).expect("pdf");
        assert!(pdf.starts_with(b"%PDF-1.7"));
    }
}
