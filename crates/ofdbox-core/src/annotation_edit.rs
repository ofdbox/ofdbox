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
}

impl AnnotationSpec {
    /// 创建注释描述。
    pub fn new(annot_type: impl Into<String>, boundary: impl Into<String>) -> Self {
        Self {
            annot_type: annot_type.into(),
            boundary: boundary.into(),
            creator: "OFDBox".to_owned(),
            last_mod_date: "2026-10-25".to_owned(),
        }
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
}

/// 注释编辑器。
pub struct AnnotationEditor<'a> {
    package: &'a mut OfdPackage,
    document_entry: String,
    annotations_entry: String,
    annotations_dir: String,
    pages: Vec<(String, String)>,
    page_ids: HashSet<String>,
    pending: HashMap<String, Vec<(String, String)>>,
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
        let id = self.next_id.to_string();
        self.next_id += 1;
        self.pending
            .entry(page_id.to_owned())
            .or_default()
            .push((id.clone(), build_annotation(&id, spec)));
        Ok(id)
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
            if let Some(pending) = self.pending.remove(&page_id) {
                added += pending.len();
                annots.extend(pending);
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
        Ok(AnnotationEditReport { added, removed })
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

fn build_annotation(id: &str, spec: &AnnotationSpec) -> String {
    format!(
        "<ofd:Annot ID=\"{}\" Type=\"{}\" Creator=\"{}\" LastModDate=\"{}\"><ofd:Appearance Boundary=\"{}\"/></ofd:Annot>",
        escape(id),
        escape(&spec.annot_type),
        escape(&spec.creator),
        escape(&spec.last_mod_date),
        escape(&spec.boundary)
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
