//! 注释编辑：在现有 `Annotations.xml` / `PageAnnot` 结构上增删注释。
//!
//! 编辑会重写受影响的 `PageAnnot` 文件和注释索引；已签名文档默认拒绝编辑，
//! 因为会破坏 `References` 摘要。

use std::collections::{HashMap, HashSet};
use std::io::{Seek, Write};

use crate::error::OfdError;
use crate::options::ReadOptions;
use crate::package::{OfdPackage, parent_dir, resolve_loc};
use crate::package_write::{PackageEdits, PackageWriteOptions};
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

/// 单条注释的描述。
#[derive(Clone, Debug)]
pub struct AnnotationSpec {
    /// 注释类型（Link / Path / Highlight / Stamp / Watermark）。
    pub annot_type: String,
    /// 外观边界。
    pub boundary: String,
    /// 创建者。
    pub creator: String,
    /// 最后修改日期。
    pub last_mod_date: String,
    /// 可选图片外观。
    pub image: Option<AnnotationImage>,
    /// 可选文字外观。
    pub text: Option<AnnotationText>,
    /// 可选路径外观。
    pub path: Option<AnnotationPath>,
}

/// 文字外观。
#[derive(Clone, Debug)]
pub struct AnnotationText {
    /// 文字内容。
    pub text: String,
    /// 字体资源 ID。
    pub font: String,
    /// 字号。
    pub size: f64,
}

/// 路径外观。
#[derive(Clone, Debug)]
pub struct AnnotationPath {
    /// OFD 路径缩写数据。
    pub data: String,
}

/// 注释的图片外观。
#[derive(Clone, Debug)]
pub struct AnnotationImage {
    /// 图片数据。
    pub data: Vec<u8>,
    /// 图片格式，如 PNG、JPG。
    pub format: String,
    /// 像素宽度。
    pub width: i64,
    /// 像素高度。
    pub height: i64,
}

impl AnnotationSpec {
    /// 创建注释描述。
    pub fn new(annot_type: impl Into<String>, boundary: impl Into<String>) -> Self {
        Self {
            annot_type: annot_type.into(),
            boundary: boundary.into(),
            creator: "OFDBox".to_owned(),
            last_mod_date: "2026-10-25".to_owned(),
            image: None,
            text: None,
            path: None,
        }
    }

    /// 附加文字外观。
    pub fn with_text(
        mut self,
        text: impl Into<String>,
        font: impl Into<String>,
        size: f64,
    ) -> Self {
        self.text = Some(AnnotationText {
            text: text.into(),
            font: font.into(),
            size,
        });
        self
    }

    /// 附加路径外观。
    pub fn with_path(mut self, data: impl Into<String>) -> Self {
        self.path = Some(AnnotationPath { data: data.into() });
        self
    }

    /// 附加图片外观。
    pub fn with_image(
        mut self,
        data: impl Into<Vec<u8>>,
        format: impl Into<String>,
        width: i64,
        height: i64,
    ) -> Self {
        self.image = Some(AnnotationImage {
            data: data.into(),
            format: format.into(),
            width,
            height,
        });
        self
    }

    /// 指定创建者。
    pub fn with_creator(mut self, creator: impl Into<String>) -> Self {
        self.creator = creator.into();
        self
    }

    /// 指定最后修改日期。
    pub fn with_last_mod_date(mut self, date: impl Into<String>) -> Self {
        self.last_mod_date = date.into();
        self
    }
}

/// 编辑结果。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnnotationEditReport {
    /// 新增数量。
    pub added: usize,
    /// 删除数量。
    pub removed: usize,
    /// 修改数量。
    pub updated: usize,
}

/// 注释修改项；`None` 表示保持不变。
#[derive(Clone, Debug, Default)]
pub struct AnnotationPatch {
    /// 外观边界。
    pub boundary: Option<String>,
    /// 创建者。
    pub creator: Option<String>,
    /// 最后修改日期。
    pub last_mod_date: Option<String>,
    /// 是否可见。
    pub visible: Option<bool>,
    /// 是否打印。
    pub print: Option<bool>,
}

impl AnnotationPatch {
    /// 创建空修改项。
    pub fn new() -> Self {
        Self::default()
    }

    /// 修改边界。
    pub fn with_boundary(mut self, boundary: impl Into<String>) -> Self {
        self.boundary = Some(boundary.into());
        self
    }

    /// 修改创建者。
    pub fn with_creator(mut self, creator: impl Into<String>) -> Self {
        self.creator = Some(creator.into());
        self
    }

    /// 修改最后修改日期。
    pub fn with_last_mod_date(mut self, date: impl Into<String>) -> Self {
        self.last_mod_date = Some(date.into());
        self
    }

    /// 修改可见性。
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = Some(visible);
        self
    }

    /// 修改打印属性。
    pub fn with_print(mut self, print: bool) -> Self {
        self.print = Some(print);
        self
    }
}

struct PendingAnnotation {
    id: String,
    spec: AnnotationSpec,
    image_object_id: Option<i64>,
    multimedia_id: Option<i64>,
    object_id: Option<i64>,
}

/// 注释编辑器。
pub struct AnnotationEditor<'a> {
    package: &'a mut OfdPackage,
    document_entry: String,
    annotations_entry: String,
    annotations_dir: String,
    pages: Vec<(String, String)>,
    page_ids: HashSet<String>,
    pending: HashMap<String, Vec<PendingAnnotation>>,
    updates: HashMap<String, AnnotationPatch>,
    removed: HashSet<String>,
    next_id: i64,
    max_unit_id: i64,
}

impl<'a> AnnotationEditor<'a> {
    /// 创建编辑器。
    ///
    /// `Document.xml` 必须已声明 `Annotations`；`allow_signed` 为 `false` 时，
    /// 已签名文档会拒绝编辑。
    pub fn new(package: &'a mut OfdPackage, allow_signed: bool) -> Result<Self, OfdError> {
        let document = package.parse_document()?;
        let annotations_loc = document
            .annotations()
            .ok_or_else(|| {
                OfdError::InvalidStructure("Document.xml 未声明 Annotations".to_owned())
            })?
            .to_owned();
        let document_entry = package
            .document_entry_name()
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位 Document.xml".to_owned()))?
            .to_owned();
        let base = parent_dir(&document_entry).unwrap_or("");
        let annotations_entry = resolve_loc(base, &annotations_loc)?;
        if !package.contains(&annotations_entry) {
            return Err(OfdError::MissingEntry(annotations_entry));
        }
        if !allow_signed
            && package
                .parse_signatures()
                .map(|index| index.signatures().next().is_some())
                .unwrap_or(false)
        {
            return Err(OfdError::InvalidStructure(
                "文档已签名，注释编辑会破坏签名；需要显式允许".to_owned(),
            ));
        }

        let index = package.parse_annotations()?;
        let mut pages = Vec::new();
        for page in index.pages() {
            let page_id = page
                .page_id()
                .ok_or_else(|| OfdError::InvalidStructure("注释索引缺少 PageID".to_owned()))?;
            let file_loc = page
                .file_loc()
                .ok_or_else(|| OfdError::InvalidStructure("注释索引缺少 FileLoc".to_owned()))?;
            pages.push((page_id.to_owned(), file_loc.to_owned()));
        }
        let page_ids = document
            .pages()
            .map(|pages| {
                pages
                    .pages()
                    .filter_map(|page| page.id().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let max_unit_id = document
            .common_data()
            .and_then(|common| common.max_unit_id().map(str::to_owned))
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        let annotations_dir = parent_dir(&annotations_entry).unwrap_or("").to_owned();

        Ok(Self {
            package,
            document_entry,
            annotations_entry,
            annotations_dir,
            pages,
            page_ids,
            pending: HashMap::new(),
            updates: HashMap::new(),
            removed: HashSet::new(),
            next_id: max_unit_id + 1,
            max_unit_id,
        })
    }

    /// 新增注释，返回分配到的 ID。
    pub fn add(&mut self, page_id: &str, spec: &AnnotationSpec) -> Result<String, OfdError> {
        if !self.page_ids.contains(page_id) {
            return Err(OfdError::InvalidStructure(format!("页面不存在: {page_id}")));
        }
        if !matches!(
            spec.annot_type.as_str(),
            "Link" | "Path" | "Highlight" | "Stamp" | "Watermark"
        ) {
            return Err(OfdError::InvalidStructure(format!(
                "不支持的注释类型: {}",
                spec.annot_type
            )));
        }
        if spec.image.is_some() && (spec.text.is_some() || spec.path.is_some()) {
            return Err(OfdError::InvalidStructure(
                "注释只能选择一种外观".to_owned(),
            ));
        }
        let id = self.next_id;
        let (image_object_id, multimedia_id, object_id) = if let Some(image) = &spec.image {
            if image.data.is_empty() || image.width <= 0 || image.height <= 0 {
                return Err(OfdError::InvalidStructure("注释图片参数无效".to_owned()));
            }
            self.next_id += 3;
            (Some(id + 1), Some(id + 2), None)
        } else if spec.text.is_some() || spec.path.is_some() {
            self.next_id += 2;
            (None, None, Some(id + 1))
        } else {
            self.next_id += 1;
            (None, None, None)
        };
        let id = id.to_string();
        self.pending
            .entry(page_id.to_owned())
            .or_default()
            .push(PendingAnnotation {
                id: id.clone(),
                spec: spec.clone(),
                image_object_id,
                multimedia_id,
                object_id,
            });
        Ok(id)
    }

    /// 修改已有注释。
    pub fn update(&mut self, id: &str, patch: AnnotationPatch) -> Result<(), OfdError> {
        if self.removed.contains(id) {
            return Err(OfdError::InvalidStructure(format!(
                "注释已被删除，不能再修改: {id}"
            )));
        }
        self.updates.insert(id.to_owned(), patch);
        Ok(())
    }

    /// 删除注释；返回是否记录了删除。
    pub fn remove(&mut self, id: &str) -> bool {
        self.removed.insert(id.to_owned())
    }

    /// 写出编辑结果。
    pub fn write(
        mut self,
        mut writer: impl Write + Seek,
    ) -> Result<AnnotationEditReport, OfdError> {
        let mut edits = PackageEdits::new();
        let mut pages = self.pages.clone();
        let mut added = 0usize;
        let mut removed = 0usize;
        let mut updated = 0usize;

        let needs_image = self
            .pending
            .values()
            .flatten()
            .any(|pending| pending.spec.image.is_some());
        let mut res_state = if needs_image {
            Some(load_document_res(self.package, &self.document_entry)?)
        } else {
            None
        };

        let mut page_ids: Vec<String> = pages.iter().map(|(id, _)| id.clone()).collect();
        for page_id in self.pending.keys() {
            if !page_ids.contains(page_id) {
                page_ids.push(page_id.clone());
            }
        }

        for page_id in page_ids {
            let existing = pages.iter().find(|(id, _)| id == &page_id).cloned();
            let (mut annots, entry) = if let Some((_, file_loc)) = &existing {
                let entry = resolve_loc(&self.annotations_dir, file_loc)?;
                let bytes = self.package.read_entry(&entry)?;
                (parse_existing_annots(&bytes)?, Some(entry))
            } else {
                (Vec::new(), None)
            };

            let before = annots.len();
            annots.retain(|(id, _)| !self.removed.contains(id));
            removed += before - annots.len();
            for (id, raw) in &mut annots {
                if let Some(patch) = self.updates.get(id) {
                    *raw = rewrite_annotation(raw, patch)?;
                    updated += 1;
                }
            }
            if let Some(pending) = self.pending.remove(&page_id) {
                added += pending.len();
                for pending in pending {
                    let mut spec = pending.spec;
                    let mut image_ref = None;
                    if let (Some(state), Some(image)) = (res_state.as_mut(), spec.image.take()) {
                        let extension = image_extension(&image.format);
                        let name = unique_name(
                            self.package,
                            &state.dir,
                            &format!("annotation_{}.{extension}", pending.id),
                        );
                        let entry = format!("{}/{}", state.dir, name);
                        state.media.push_str(&format!(
                            "<{prefix}MultiMedia ID=\"{}\" Type=\"Image\" Format=\"{}\"><{prefix}MediaFile>{}</{prefix}MediaFile></{prefix}MultiMedia>",
                            pending.multimedia_id.unwrap_or(0),
                            escape(&image.format),
                            escape(&name),
                            prefix = state.prefix
                        ));
                        state.images.push((entry, image.data));
                        image_ref = Some((
                            pending.image_object_id.unwrap_or(0),
                            pending.multimedia_id.unwrap_or(0),
                            image.width,
                            image.height,
                        ));
                    }
                    let raw = build_annotation(&pending.id, &spec, image_ref, pending.object_id);
                    annots.push((pending.id, raw));
                }
            }

            if annots.is_empty() {
                if let Some(entry) = entry {
                    edits.remove(entry);
                    pages.retain(|(id, _)| id != &page_id);
                }
                continue;
            }

            let xml = build_page_annot(&annots);
            let entry = match entry {
                Some(entry) => entry,
                None => {
                    let file_loc = self.unique_page_file(&page_id);
                    pages.push((page_id.clone(), file_loc.clone()));
                    resolve_loc(&self.annotations_dir, &file_loc)?
                }
            };
            if self.package.contains(&entry) {
                edits.replace(entry, xml.into_bytes());
            } else {
                edits.add(entry, xml.into_bytes());
            }
        }

        if let Some(state) = res_state {
            let updated = append_media(&state.source, &state.media)?;
            if self.package.contains(&state.entry) {
                edits.replace(&state.entry, updated.into_bytes());
            } else {
                edits.add(&state.entry, updated.into_bytes());
            }
            for (entry, data) in state.images {
                edits.add(entry, data);
            }
        }

        let index = build_annotations_index(&pages);
        edits.replace(&self.annotations_entry, index.into_bytes());

        if self.next_id - 1 > self.max_unit_id
            && let Some(updated) =
                bump_max_unit_id(self.package, &self.document_entry, self.next_id - 1)?
        {
            edits.replace(&self.document_entry, updated);
        }

        self.package
            .write_with(&mut writer, &edits, &PackageWriteOptions::default())?;
        Ok(AnnotationEditReport {
            added,
            removed,
            updated,
        })
    }

    fn unique_page_file(&self, page_id: &str) -> String {
        let mut name = format!("PageAnnot_{page_id}.xml");
        let mut counter = 1;
        while resolve_loc(&self.annotations_dir, &name)
            .map(|entry| self.package.contains(&entry))
            .unwrap_or(false)
        {
            name = format!("PageAnnot_{page_id}_{counter}.xml");
            counter += 1;
        }
        name
    }
}

fn parse_existing_annots(bytes: &[u8]) -> Result<Vec<(String, String)>, OfdError> {
    let doc = parse_xml(bytes, ReadOptions::LENIENT)?;
    let root = doc.root();
    let mut annots = Vec::new();
    for node in doc.children_named(root, "Annot") {
        let id = doc.attribute(node, "ID").unwrap_or("").to_owned();
        let span = doc.span(node);
        let raw = std::str::from_utf8(&bytes[span.start..span.end])
            .map_err(|_| OfdError::InvalidStructure("注释不是 UTF-8".to_owned()))?;
        annots.push((id, raw.to_owned()));
    }
    Ok(annots)
}

fn build_annotation(
    id: &str,
    spec: &AnnotationSpec,
    image: Option<(i64, i64, i64, i64)>,
    object_id: Option<i64>,
) -> String {
    let boundary = escape(&spec.boundary);
    let appearance = if let Some((image_object_id, multimedia_id, width, height)) = image {
        format!(
            "<ofd:Appearance Boundary=\"{boundary}\"><ofd:ImageObject ID=\"{image_object_id}\" ResourceID=\"{multimedia_id}\" Boundary=\"0 0 {width} {height}\" CTM=\"{width} 0 0 {height} 0 0\"/></ofd:Appearance>"
        )
    } else if let (Some(text), Some(object_id)) = (&spec.text, object_id) {
        format!(
            "<ofd:Appearance Boundary=\"{boundary}\"><ofd:TextObject ID=\"{object_id}\" Boundary=\"{boundary}\" Font=\"{}\" Size=\"{}\"><ofd:TextCode X=\"0\" Y=\"0\">{}</ofd:TextCode></ofd:TextObject></ofd:Appearance>",
            escape(&text.font),
            text.size,
            escape(&text.text)
        )
    } else if let (Some(path), Some(object_id)) = (&spec.path, object_id) {
        format!(
            "<ofd:Appearance Boundary=\"{boundary}\"><ofd:PathObject ID=\"{object_id}\" Boundary=\"{boundary}\"><ofd:AbbreviatedData>{}</ofd:AbbreviatedData></ofd:PathObject></ofd:Appearance>",
            escape(&path.data)
        )
    } else {
        format!("<ofd:Appearance Boundary=\"{boundary}\"/>")
    };
    format!(
        "<ofd:Annot ID=\"{}\" Type=\"{}\" Creator=\"{}\" LastModDate=\"{}\">{appearance}</ofd:Annot>",
        escape(id),
        escape(&spec.annot_type),
        escape(&spec.creator),
        escape(&spec.last_mod_date)
    )
}

fn build_page_annot(annots: &[(String, String)]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:PageAnnot xmlns:ofd=\"http://www.ofdspec.org/2016\">",
    );
    for (_, raw) in annots {
        xml.push_str(raw);
    }
    xml.push_str("</ofd:PageAnnot>");
    xml
}

fn build_annotations_index(pages: &[(String, String)]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:Annotations xmlns:ofd=\"http://www.ofdspec.org/2016\">",
    );
    for (page_id, file_loc) in pages {
        xml.push_str(&format!(
            "<ofd:Page PageID=\"{}\"><ofd:FileLoc>{}</ofd:FileLoc></ofd:Page>",
            escape(page_id),
            escape(file_loc)
        ));
    }
    xml.push_str("</ofd:Annotations>");
    xml
}

fn bump_max_unit_id(
    package: &mut OfdPackage,
    document_entry: &str,
    value: i64,
) -> Result<Option<Vec<u8>>, OfdError> {
    let bytes = package.read_entry(document_entry)?;
    let doc = parse_xml(&bytes, ReadOptions::LENIENT)?;
    let root = doc.root();
    let Some(common) = doc.find_child(root, "CommonData") else {
        return Ok(None);
    };
    let Some(node) = doc.find_child(common, "MaxUnitID") else {
        return Ok(None);
    };
    let span = doc.span(node);
    let text = format!("{value}");
    let mut output = Vec::with_capacity(bytes.len() + text.len());
    output.extend_from_slice(&bytes[..span.start]);
    output.extend_from_slice(text.as_bytes());
    output.extend_from_slice(&bytes[span.end..]);
    Ok(Some(output))
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn rewrite_annotation(raw: &str, patch: &AnnotationPatch) -> Result<String, OfdError> {
    let doc = parse_xml(raw.as_bytes(), ReadOptions::LENIENT)?;
    let root = doc.root();
    let mut overrides = Vec::new();
    if let Some(creator) = &patch.creator {
        overrides.push(("Creator", creator.as_str()));
    }
    if let Some(date) = &patch.last_mod_date {
        overrides.push(("LastModDate", date.as_str()));
    }
    if let Some(visible) = patch.visible {
        overrides.push(("Visible", if visible { "true" } else { "false" }));
    }
    if let Some(print) = patch.print {
        overrides.push(("Print", if print { "true" } else { "false" }));
    }
    let (start_tag, open_start, open_end) = rewrite_start_tag(raw, &doc, root, &overrides, 0)?;
    let mut output = String::new();
    output.push_str(&raw[..open_start]);
    output.push_str(&start_tag);
    if raw[..=open_end].ends_with("/>") {
        output.push_str(&raw[open_end + 1..]);
        return Ok(output);
    }

    let close_start = raw[open_start..]
        .rfind("</")
        .map(|offset| open_start + offset)
        .ok_or_else(|| OfdError::InvalidStructure("注释缺少结束标签".to_owned()))?;
    let body_start = open_end + 1;
    let mut body = raw[body_start..close_start].to_owned();
    if let Some(boundary) = &patch.boundary
        && let Some(appearance) = doc.find_child(root, "Appearance")
    {
        let (tag, appearance_start, appearance_end) = rewrite_start_tag(
            &body,
            &doc,
            appearance,
            &[("Boundary", boundary.as_str())],
            body_start,
        )?;
        let mut updated_body = String::new();
        updated_body.push_str(&body[..appearance_start]);
        updated_body.push_str(&tag);
        updated_body.push_str(&body[appearance_end + 1..]);
        body = updated_body;
    }
    output.push_str(&body);
    output.push_str(&raw[close_start..]);
    Ok(output)
}

/// 重写元素开始标签，返回 `(新开始标签, 在 raw 中的起点, `>` 的位置)`。
fn rewrite_start_tag(
    raw: &str,
    doc: &XmlDocument,
    node: NodeId,
    overrides: &[(&str, &str)],
    base: usize,
) -> Result<(String, usize, usize), OfdError> {
    let span = doc.span(node);
    let span_start = span.start - base;
    let span_end = span.end - base;
    let open_start = span_start;
    let open_end = raw[span_start..span_end]
        .find('>')
        .map(|offset| span_start + offset)
        .ok_or_else(|| OfdError::InvalidStructure("元素开始标签不完整".to_owned()))?;
    let self_closing = raw[..=open_end].ends_with("/>");
    let name = doc.name(node);
    let tag_name = match &name.prefix {
        Some(prefix) => format!("{prefix}:{}", name.local),
        None => name.local.clone(),
    };
    let mut attributes: Vec<(String, String)> = doc
        .node(node)
        .attributes
        .iter()
        .map(|attribute| {
            let key = match &attribute.name.prefix {
                Some(prefix) => format!("{prefix}:{}", attribute.name.local),
                None => attribute.name.local.clone(),
            };
            (key, attribute.value.clone())
        })
        .collect();
    for (key, value) in overrides {
        if let Some(entry) = attributes
            .iter_mut()
            .find(|(name, _)| name == key || name.ends_with(&format!(":{key}")))
        {
            entry.1 = (*value).to_owned();
        } else {
            attributes.push(((*key).to_owned(), (*value).to_owned()));
        }
    }
    let mut tag = format!("<{tag_name}");
    for (key, value) in attributes {
        tag.push_str(&format!(" {key}=\"{}\"", escape(&value)));
    }
    tag.push_str(if self_closing { "/>" } else { ">" });
    Ok((tag, open_start, open_end))
}

struct ResState {
    entry: String,
    source: String,
    prefix: String,
    dir: String,
    media: String,
    images: Vec<(String, Vec<u8>)>,
}

fn load_document_res(package: &mut OfdPackage, document_entry: &str) -> Result<ResState, OfdError> {
    let doc_dir = parent_dir(document_entry).unwrap_or("").to_owned();
    let document = package.read_entry(document_entry)?;
    let doc = parse_xml(&document, ReadOptions::LENIENT)?;
    let root = doc.root();
    let common = doc
        .find_child(root, "CommonData")
        .ok_or_else(|| OfdError::InvalidStructure("Document.xml 缺少 CommonData".to_owned()))?;
    let loc = doc
        .find_child(common, "DocumentRes")
        .map(|node| doc.text(node).trim().to_owned())
        .ok_or_else(|| {
            OfdError::InvalidStructure("图片注释需要 Document.xml 声明 DocumentRes".to_owned())
        })?;
    let entry = resolve_loc(&doc_dir, &loc)?;
    let source = match package.read_entry(&entry) {
        Ok(bytes) => std::str::from_utf8(&bytes)
            .map_err(|_| OfdError::InvalidStructure("DocumentRes 不是 UTF-8".to_owned()))?
            .to_owned(),
        Err(_) => r#"<?xml version="1.0" encoding="UTF-8"?><ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res"><ofd:MultiMedias></ofd:MultiMedias></ofd:Res>"#.to_owned(),
    };
    let res_doc = parse_xml(source.as_bytes(), ReadOptions::LENIENT)?;
    let prefix = match &res_doc.name(res_doc.root()).prefix {
        Some(prefix) => format!("{prefix}:"),
        None => String::new(),
    };
    let base_loc = res_doc
        .attribute(res_doc.root(), "BaseLoc")
        .unwrap_or("Res");
    let dir = resolve_loc(&doc_dir, base_loc)?;
    Ok(ResState {
        entry,
        source,
        prefix,
        dir,
        media: String::new(),
        images: Vec::new(),
    })
}

fn append_media(source: &str, media: &str) -> Result<String, OfdError> {
    if media.is_empty() {
        return Ok(source.to_owned());
    }
    let doc = parse_xml(source.as_bytes(), ReadOptions::LENIENT)?;
    let root = doc.root();
    let prefix = match &doc.name(root).prefix {
        Some(prefix) => format!("{prefix}:"),
        None => String::new(),
    };
    let fragment = format!("<{prefix}MultiMedias>{media}</{prefix}MultiMedias>");
    if let Some(multimedias) = doc.find_child(root, "MultiMedias") {
        let span = doc.span(multimedias);
        let open_end = source[span.start..span.end]
            .find('>')
            .map(|offset| span.start + offset)
            .ok_or_else(|| OfdError::InvalidStructure("MultiMedias 开始标签不完整".to_owned()))?;
        if source[..=open_end].ends_with("/>") {
            let mut output = String::new();
            output.push_str(&source[..span.start]);
            output.push_str(&fragment);
            output.push_str(&source[span.end..]);
            return Ok(output);
        }
        let close_start = source[span.start..span.end]
            .rfind("</")
            .map(|offset| span.start + offset)
            .ok_or_else(|| OfdError::InvalidStructure("MultiMedias 缺少结束标签".to_owned()))?;
        let mut output = String::new();
        output.push_str(&source[..close_start]);
        output.push_str(media);
        output.push_str(&source[close_start..]);
        return Ok(output);
    }
    let span = doc.span(root);
    let close_start = source[span.start..span.end]
        .rfind("</")
        .map(|offset| span.start + offset)
        .ok_or_else(|| OfdError::InvalidStructure("DocumentRes 缺少结束标签".to_owned()))?;
    let mut output = String::new();
    output.push_str(&source[..close_start]);
    output.push_str(&fragment);
    output.push_str(&source[close_start..]);
    Ok(output)
}

fn image_extension(format: &str) -> String {
    match format.to_ascii_uppercase().as_str() {
        "PNG" => "png",
        "JPG" | "JPEG" => "jpg",
        "GIF" => "gif",
        "BMP" => "bmp",
        "SVG" => "svg",
        _ => "bin",
    }
    .to_owned()
}

fn unique_name(package: &OfdPackage, dir: &str, name: &str) -> String {
    let mut candidate = name.to_owned();
    let mut counter = 1;
    while resolve_loc(dir, &candidate)
        .map(|entry| package.contains(&entry))
        .unwrap_or(false)
    {
        candidate = format!("{counter}_{name}");
        counter += 1;
    }
    candidate
}
