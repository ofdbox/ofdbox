//! 从零创建 OFD 文档的写入 API。
//!
//! `OfdWriter` 直接在内存中构造文档、页面、图元与资源，最后一次性打包，
//! 不依赖“先生成字节再重新打开”的流程，也不需要 dirty 跟踪。

use std::io::{Cursor, Write};
use std::path::Path;

use crate::error::OfdError;

const OFD_NS: &str = "http://www.ofdspec.org/2016";

/// 页面尺寸（毫米）。
#[derive(Clone, Copy, Debug)]
pub struct PageSize {
    /// 宽。
    pub width: f64,
    /// 高。
    pub height: f64,
}

impl PageSize {
    /// A4。
    pub fn a4() -> Self {
        Self {
            width: 210.0,
            height: 297.0,
        }
    }

    /// 自定义尺寸。
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

struct WriterText {
    id: i64,
    boundary: String,
    font: String,
    size: f64,
    text_codes: Vec<(f64, f64, String)>,
}

struct WriterPage {
    size: PageSize,
    layer_id: i64,
    texts: Vec<WriterText>,
}

struct WriterFont {
    id: i64,
    name: String,
    file: Option<String>,
    data: Option<Vec<u8>>,
}

/// OFD 写入器。
pub struct OfdWriter {
    doc_id: String,
    title: Option<String>,
    creator: Option<String>,
    pages: Vec<WriterPage>,
    fonts: Vec<WriterFont>,
    next_id: i64,
}

impl Default for OfdWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl OfdWriter {
    /// 新建写入器。
    pub fn new() -> Self {
        Self {
            doc_id: "ofd-doc".to_owned(),
            title: None,
            creator: None,
            pages: Vec::new(),
            fonts: Vec::new(),
            next_id: 1,
        }
    }

    /// 设置 DocID。
    pub fn doc_id(mut self, value: impl Into<String>) -> Self {
        self.doc_id = value.into();
        self
    }

    /// 设置标题。
    pub fn title(mut self, value: impl Into<String>) -> Self {
        self.title = Some(value.into());
        self
    }

    /// 设置创建者。
    pub fn creator(mut self, value: impl Into<String>) -> Self {
        self.creator = Some(value.into());
        self
    }

    /// 追加一页，返回页序号（从 0 开始）。
    pub fn add_page(&mut self, size: PageSize) -> usize {
        let layer_id = self.allocate();
        self.pages.push(WriterPage {
            size,
            layer_id,
            texts: Vec::new(),
        });
        self.pages.len() - 1
    }

    /// 添加系统字体，返回字体 ID。
    pub fn add_font(&mut self, name: impl Into<String>) -> String {
        let id = self.allocate();
        self.fonts.push(WriterFont {
            id,
            name: name.into(),
            file: None,
            data: None,
        });
        id.to_string()
    }

    /// 添加内嵌字体，返回字体 ID。
    pub fn add_embedded_font(
        &mut self,
        name: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> String {
        let id = self.allocate();
        let file = format!("Font_{id}.ttf");
        self.fonts.push(WriterFont {
            id,
            name: name.into(),
            file: Some(file),
            data: Some(data.into()),
        });
        id.to_string()
    }

    /// 在指定页面添加文字图元，返回图元 ID。
    pub fn add_text(
        &mut self,
        page_index: usize,
        boundary: impl Into<String>,
        font: impl Into<String>,
        size: f64,
        text_codes: &[(f64, f64, &str)],
    ) -> Result<String, OfdError> {
        if page_index >= self.pages.len() {
            return Err(OfdError::InvalidStructure(format!(
                "页面不存在: {page_index}"
            )));
        }
        let id = self.allocate();
        let page = &mut self.pages[page_index];
        page.texts.push(WriterText {
            id,
            boundary: boundary.into(),
            font: font.into(),
            size,
            text_codes: text_codes
                .iter()
                .map(|(x, y, text)| (*x, *y, (*text).to_owned()))
                .collect(),
        });
        Ok(id.to_string())
    }

    /// 生成 OFD 字节。
    pub fn to_bytes(self) -> Result<Vec<u8>, OfdError> {
        if self.pages.is_empty() {
            return Err(OfdError::InvalidStructure("至少需要一页".to_owned()));
        }
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        entries.push(("OFD.xml".to_owned(), self.ofd_xml().into_bytes()));
        entries.push((
            "Doc_0/Document.xml".to_owned(),
            self.document_xml().into_bytes(),
        ));
        if !self.fonts.is_empty() {
            entries.push((
                "Doc_0/Res/DocumentRes.xml".to_owned(),
                self.document_res_xml().into_bytes(),
            ));
            for font in &self.fonts {
                if let (Some(file), Some(data)) = (&font.file, &font.data) {
                    entries.push((format!("Doc_0/Res/{file}"), data.clone()));
                }
            }
        }
        for (index, page) in self.pages.iter().enumerate() {
            entries.push((
                format!("Doc_0/Pages/Page_{index}/Content.xml"),
                page_xml(page),
            ));
        }

        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            zip.start_file(name, zip::write::SimpleFileOptions::default())?;
            zip.write_all(&bytes)?;
        }
        Ok(zip.finish()?.into_inner())
    }

    /// 保存到文件。
    pub fn save(self, path: impl AsRef<Path>) -> Result<(), OfdError> {
        let bytes = self.to_bytes()?;
        if let Some(parent) = path.as_ref().parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)?;
        Ok(())
    }

    fn allocate(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn ofd_xml(&self) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:OFD xmlns:ofd=\"{OFD_NS}\" Version=\"1.0\" DocType=\"OFD\"><ofd:DocBody><ofd:DocInfo><ofd:DocID>{}</ofd:DocID>{}{}</ofd:DocInfo><ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot></ofd:DocBody></ofd:OFD>",
            escape(&self.doc_id),
            self.title
                .as_ref()
                .map(|title| format!("<ofd:Title>{}</ofd:Title>", escape(title)))
                .unwrap_or_default(),
            self.creator
                .as_ref()
                .map(|creator| format!("<ofd:Creator>{}</ofd:Creator>", escape(creator)))
                .unwrap_or_default(),
        )
    }

    fn document_xml(&self) -> String {
        let first = self.pages[0].size;
        let mut pages = String::new();
        for index in 0..self.pages.len() {
            pages.push_str(&format!(
                "<ofd:Page ID=\"{}\" BaseLoc=\"Pages/Page_{index}/Content.xml\"/>",
                index + 1
            ));
        }
        let document_res = if self.fonts.is_empty() {
            String::new()
        } else {
            "<ofd:DocumentRes>Res/DocumentRes.xml</ofd:DocumentRes>".to_owned()
        };
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:Document xmlns:ofd=\"{OFD_NS}\"><ofd:CommonData><ofd:MaxUnitID>{}</ofd:MaxUnitID><ofd:PageArea><ofd:PhysicalBox>0 0 {} {}</ofd:PhysicalBox></ofd:PageArea>{document_res}</ofd:CommonData><ofd:Pages>{pages}</ofd:Pages></ofd:Document>",
            self.next_id - 1,
            first.width,
            first.height
        )
    }

    fn document_res_xml(&self) -> String {
        let mut fonts = String::new();
        for font in &self.fonts {
            fonts.push_str(&format!(
                "<ofd:Font ID=\"{}\" FontName=\"{}\"",
                font.id,
                escape(&font.name)
            ));
            if let Some(file) = &font.file {
                fonts.push_str(&format!(
                    "><ofd:FontFile>{}</ofd:FontFile></ofd:Font>",
                    escape(file)
                ));
            } else {
                fonts.push_str("/>");
            }
        }
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:Res xmlns:ofd=\"{OFD_NS}\" BaseLoc=\".\"><ofd:Fonts>{fonts}</ofd:Fonts></ofd:Res>"
        )
    }
}

fn page_xml(page: &WriterPage) -> Vec<u8> {
    let mut content = String::new();
    for text in &page.texts {
        content.push_str(&format!(
            "<ofd:TextObject ID=\"{}\" Boundary=\"{}\" Font=\"{}\" Size=\"{}\">",
            text.id,
            escape(&text.boundary),
            escape(&text.font),
            text.size
        ));
        for (x, y, value) in &text.text_codes {
            content.push_str(&format!(
                "<ofd:TextCode X=\"{x}\" Y=\"{y}\">{}</ofd:TextCode>",
                escape(value)
            ));
        }
        content.push_str("</ofd:TextObject>");
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:Page xmlns:ofd=\"{OFD_NS}\"><ofd:Area><ofd:PhysicalBox>0 0 {} {}</ofd:PhysicalBox></ofd:Area><ofd:Content><ofd:Layer ID=\"{}\" Type=\"Body\">{content}</ofd:Layer></ofd:Content></ofd:Page>",
        page.size.width, page.size.height, page.layer_id
    );
    xml.into_bytes()
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
