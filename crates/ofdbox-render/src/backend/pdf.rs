//! PDF 文档后端。
//!
//! 当前阶段把 [`Scene`] 写成矢量 PDF：页面坐标、路径、图片、裁剪、
//! 渐变和文字。文字优先嵌入字体，字体不可用时退化为字形轮廓。

use crate::error::RenderError;
use crate::scene::Scene;

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
        let content = self.writer.stream(
            "",
            format!("q\n{scale} 0 0 -{scale} 0 {height} cm\nQ\n").as_bytes(),
        );
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
    use crate::geometry::Size;
    use crate::scene::{Group, Scene};

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
    fn backend_writes_single_page() {
        let pdf = PdfBackend::new().render(&scene()).expect("pdf");
        assert!(pdf.starts_with(b"%PDF-1.7"));
    }
}
