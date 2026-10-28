//! 注释编辑：在现有 `Annotations.xml` / `PageAnnot` 结构上增删注释。
//!
//! 编辑会重写受影响的 `PageAnnot` 文件和注释索引；已签名文档默认拒绝编辑，
//! 因为会破坏 `References` 摘要。

use std::collections::{HashMap, HashSet};
use std::io::{Seek, Write};

use crate::error::OfdError;
use crate::model::annotation::{Annot, AnnotationType, Appearance, PageAnnot};
use crate::model::annotations::{Annotations, AnnotationsPage};
use crate::model::definition::{StArray, StBox, StId, StLoc, StRefId};
use crate::model::document::Document;
use crate::model::ofd::Ofd;
use crate::model::page::{
    CgTransform, CompositeObject, GraphicObject, GraphicUnit, ImageObject, PageBlock, PathObject,
    TextCode, TextObject,
};
use crate::model::res::{MultiMedia, MultiMediaType, MultiMedias, Res};
use crate::model::signatures::Signatures;
use crate::model_xml;
use crate::package::{OfdPackage, parent_dir, resolve_loc};
use crate::package_write::{PackageEdits, PackageWriteOptions};

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
    /// 可选复合外观。
    pub composite: Option<AnnotationComposite>,
    /// 文字 CGTransform。
    pub cg_transform: Option<AnnotationCgTransform>,
}

/// 复合外观。
#[derive(Clone, Debug)]
pub struct AnnotationComposite {
    /// `CompositeGraphicUnit` 的资源 ID。
    pub resource_id: String,
}

/// 文字的 `CGTransform`。
#[derive(Clone, Debug)]
pub struct AnnotationCgTransform {
    /// 起始字符位置。
    pub code_position: i64,
    /// 字符数量。
    pub code_count: Option<i64>,
    /// 字形数量。
    pub glyph_count: Option<i64>,
    /// 可选 `Glyphs` 文本。
    pub glyphs: Option<String>,
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
            composite: None,
            cg_transform: None,
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

    /// 附加复合外观（引用 `DocumentRes` 中的复合图元）。
    pub fn with_composite(mut self, resource_id: impl Into<String>) -> Self {
        self.composite = Some(AnnotationComposite {
            resource_id: resource_id.into(),
        });
        self
    }

    /// 为文字外观附加 `CGTransform`。
    pub fn with_cg_transform(
        mut self,
        code_position: i64,
        code_count: Option<i64>,
        glyph_count: Option<i64>,
        glyphs: Option<String>,
    ) -> Self {
        self.cg_transform = Some(AnnotationCgTransform {
            code_position,
            code_count,
            glyph_count,
            glyphs,
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
        let ofd = parse_schema::<Ofd>(&package.read_entry("OFD.xml")?)?;
        let body = ofd
            .doc_bodies
            .first()
            .ok_or_else(|| OfdError::InvalidStructure("OFD.xml 缺少 DocBody".to_owned()))?;
        let document_entry = resolve_loc("", body.doc_root.as_str())?;
        let document = parse_schema::<Document>(&package.read_entry(&document_entry)?)?;
        let annotations_loc = document
            .annotations
            .as_ref()
            .ok_or_else(|| {
                OfdError::InvalidStructure("Document.xml 未声明 Annotations".to_owned())
            })?
            .to_string();
        let base = parent_dir(&document_entry).unwrap_or("");
        let annotations_entry = resolve_loc(base, &annotations_loc)?;
        if !package.contains(&annotations_entry) {
            return Err(OfdError::MissingEntry(annotations_entry));
        }
        if !allow_signed && has_signatures(package, &body.signatures)? {
            return Err(OfdError::InvalidStructure(
                "文档已签名，注释编辑会破坏签名；需要显式允许".to_owned(),
            ));
        }

        let page_ids: HashSet<String> = document
            .pages
            .pages
            .iter()
            .map(|page| page.id.to_string())
            .collect();
        let max_unit_id = i64::from(document.common_data.max_unit_id.0);
        let annotations_dir = parent_dir(&annotations_entry).unwrap_or("").to_owned();

        Ok(Self {
            package,
            document_entry,
            annotations_entry,
            annotations_dir,
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
        let appearances = [
            spec.image.is_some(),
            spec.text.is_some(),
            spec.path.is_some(),
            spec.composite.is_some(),
        ]
        .into_iter()
        .filter(|selected| *selected)
        .count();
        if appearances > 1 {
            return Err(OfdError::InvalidStructure(
                "注释只能选择一种外观".to_owned(),
            ));
        }
        if spec.cg_transform.is_some() && spec.text.is_none() {
            return Err(OfdError::InvalidStructure(
                "CGTransform 只能用于文字外观".to_owned(),
            ));
        }
        let id = self.next_id;
        let (image_object_id, multimedia_id, object_id) = if let Some(image) = &spec.image {
            if image.data.is_empty() || image.width <= 0 || image.height <= 0 {
                return Err(OfdError::InvalidStructure("注释图片参数无效".to_owned()));
            }
            self.next_id += 3;
            (Some(id + 1), Some(id + 2), None)
        } else if spec.text.is_some() || spec.path.is_some() || spec.composite.is_some() {
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
        let mut added = 0usize;
        let mut removed = 0usize;
        let mut updated = 0usize;

        let mut annotations: Annotations =
            parse_schema(&self.package.read_entry(&self.annotations_entry)?)?;
        let mut pages: Vec<(String, String)> = annotations
            .pages
            .iter()
            .map(|page| (page.page_id.clone(), page.file_loc.to_string()))
            .collect();
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
            let (mut page_annot, entry) = if let Some((_, file_loc)) = &existing {
                let entry = resolve_loc(&self.annotations_dir, file_loc)?;
                let bytes = self.package.read_entry(&entry)?;
                (parse_schema::<PageAnnot>(&bytes)?, Some(entry))
            } else {
                (PageAnnot::default(), None)
            };

            let before = page_annot.annots.len();
            page_annot
                .annots
                .retain(|annot| !self.removed.contains(&annot.id.to_string()));
            removed += before - page_annot.annots.len();

            for annot in &mut page_annot.annots {
                if let Some(patch) = self.updates.get(&annot.id.to_string()) {
                    apply_patch(annot, patch)?;
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
                        let media = MultiMedia {
                            id: StId(pending.multimedia_id.unwrap_or(0) as u32),
                            media_type: MultiMediaType::Image,
                            format: Some(image.format.clone()),
                            media_file: StLoc::new(name),
                        };
                        state
                            .res
                            .multi_medias
                            .get_or_insert_with(MultiMedias::default)
                            .multi_medias
                            .push(media);
                        state.images.push((entry, image.data));
                        image_ref = Some((
                            pending.image_object_id.unwrap_or(0),
                            pending.multimedia_id.unwrap_or(0),
                            image.width,
                            image.height,
                        ));
                    }
                    page_annot.annots.push(build_annotation(
                        &pending.id,
                        &spec,
                        image_ref,
                        pending.object_id,
                    )?);
                }
            }

            if page_annot.annots.is_empty() {
                if let Some(entry) = entry {
                    edits.remove(entry);
                    pages.retain(|(id, _)| id != &page_id);
                    annotations.pages.retain(|page| page.page_id != page_id);
                }
                continue;
            }

            let bytes = serialize_schema(&page_annot)?;
            let entry = match entry {
                Some(entry) => entry,
                None => {
                    let file_loc = self.unique_page_file(&page_id);
                    annotations.pages.push(AnnotationsPage {
                        page_id: page_id.clone(),
                        file_loc: StLoc::new(file_loc.clone()),
                    });
                    pages.push((page_id.clone(), file_loc.clone()));
                    resolve_loc(&self.annotations_dir, &file_loc)?
                }
            };
            if self.package.contains(&entry) {
                edits.replace(entry, bytes);
            } else {
                edits.add(entry, bytes);
            }
        }

        if let Some(state) = res_state {
            edits.replace(state.entry.clone(), serialize_schema(&state.res)?);
            for (entry, data) in state.images {
                edits.add(entry, data);
            }
        }

        annotations.pages = pages
            .iter()
            .map(|(page_id, file_loc)| AnnotationsPage {
                page_id: page_id.clone(),
                file_loc: StLoc::new(file_loc.clone()),
            })
            .collect();
        edits.replace(
            self.annotations_entry.clone(),
            serialize_schema(&annotations)?,
        );

        if self.next_id - 1 > self.max_unit_id {
            let mut document: Document =
                parse_schema(&self.package.read_entry(&self.document_entry)?)?;
            document.common_data.max_unit_id = StId((self.next_id - 1) as u32);
            edits.replace(self.document_entry.clone(), serialize_schema(&document)?);
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

fn parse_schema<T: ofdbox_yaserde::YaDeserialize>(bytes: &[u8]) -> Result<T, OfdError> {
    model_xml::from_bytes(bytes).map_err(|error| OfdError::InvalidStructure(error.to_string()))
}

fn has_signatures(
    package: &mut OfdPackage,
    signatures: &Option<crate::model::definition::StLoc>,
) -> Result<bool, OfdError> {
    let Some(location) = signatures else {
        return Ok(false);
    };
    let entry = resolve_loc("", location.as_str())?;
    if !package.contains(&entry) {
        return Ok(false);
    }
    let index = parse_schema::<Signatures>(&package.read_entry(&entry)?)?;
    Ok(!index.signatures.is_empty())
}

fn apply_patch(annot: &mut Annot, patch: &AnnotationPatch) -> Result<(), OfdError> {
    if let Some(creator) = &patch.creator {
        annot.creator = creator.clone();
    }
    if let Some(date) = &patch.last_mod_date {
        annot.last_mod_date = date.clone();
    }
    if let Some(visible) = patch.visible {
        annot.visible = visible;
    }
    if let Some(print) = patch.print {
        annot.print = print;
    }
    if let Some(boundary) = &patch.boundary {
        annot.appearance.boundary = Some(parse_boundary(boundary)?);
    }
    Ok(())
}

fn build_annotation(
    id: &str,
    spec: &AnnotationSpec,
    image: Option<(i64, i64, i64, i64)>,
    object_id: Option<i64>,
) -> Result<Annot, OfdError> {
    let boundary = parse_boundary(&spec.boundary)?;
    let objects = if let Some((image_object_id, multimedia_id, width, height)) = image {
        vec![GraphicObject::Image(ImageObject {
            common: GraphicUnit {
                id: StId(image_object_id as u32),
                boundary: StBox::new(0.0, 0.0, width as f64, height as f64),
                ctm: Some(StArray(vec![
                    width as f64,
                    0.0,
                    0.0,
                    height as f64,
                    0.0,
                    0.0,
                ])),
                ..GraphicUnit::default()
            },
            resource_id: StRefId(multimedia_id as u32),
            substitution: None,
            image_mask: None,
            border: None,
        })]
    } else if let (Some(text), Some(object_id)) = (&spec.text, object_id) {
        let mut cg_transforms = Vec::new();
        if let Some(transform) = &spec.cg_transform {
            cg_transforms.push(CgTransform {
                code_position: transform.code_position as i32,
                code_count: transform.code_count.unwrap_or(1) as i32,
                glyph_count: transform.glyph_count.unwrap_or(1) as i32,
                glyphs: transform.glyphs.clone(),
            });
        }
        vec![GraphicObject::Text(TextObject {
            common: GraphicUnit {
                id: StId(object_id as u32),
                boundary,
                ..GraphicUnit::default()
            },
            font: StRefId(text.font.parse::<u32>().unwrap_or(0)),
            size: text.size,
            text_codes: vec![TextCode {
                x: Some(0.0),
                y: Some(0.0),
                text: text.text.clone(),
                ..TextCode::default()
            }],
            cg_transforms,
            ..TextObject::default()
        })]
    } else if let (Some(path), Some(object_id)) = (&spec.path, object_id) {
        vec![GraphicObject::Path(PathObject {
            common: GraphicUnit {
                id: StId(object_id as u32),
                boundary,
                ..GraphicUnit::default()
            },
            abbreviated_data: path.data.clone(),
            ..PathObject::default()
        })]
    } else if let (Some(composite), Some(object_id)) = (&spec.composite, object_id) {
        vec![GraphicObject::Composite(CompositeObject {
            common: GraphicUnit {
                id: StId(object_id as u32),
                boundary,
                ..GraphicUnit::default()
            },
            resource_id: StRefId(composite.resource_id.parse::<u32>().unwrap_or(0)),
        })]
    } else {
        Vec::new()
    };

    Ok(Annot {
        id: StId(id.parse::<u32>().unwrap_or(0)),
        annot_type: spec
            .annot_type
            .parse::<AnnotationType>()
            .unwrap_or_else(|_| AnnotationType::Other(spec.annot_type.clone())),
        creator: spec.creator.clone(),
        last_mod_date: spec.last_mod_date.clone(),
        visible: true,
        subtype: None,
        print: true,
        no_zoom: false,
        no_rotate: false,
        read_only: true,
        remark: None,
        parameters: None,
        appearance: Appearance {
            boundary: Some(boundary),
            page_block: PageBlock { objects },
        },
    })
}

fn parse_boundary(value: &str) -> Result<StBox, OfdError> {
    let values: Vec<f64> = value
        .split_whitespace()
        .map(|item| {
            item.parse::<f64>()
                .map_err(|_| OfdError::InvalidStructure(format!("无效边界: {value}")))
        })
        .collect::<Result<_, _>>()?;
    if values.len() != 4 {
        return Err(OfdError::InvalidStructure(format!(
            "边界需要 4 个数: {value}"
        )));
    }
    Ok(StBox::new(values[0], values[1], values[2], values[3]))
}

struct ResState {
    entry: String,
    dir: String,
    res: Res,
    images: Vec<(String, Vec<u8>)>,
}

fn load_document_res(package: &mut OfdPackage, document_entry: &str) -> Result<ResState, OfdError> {
    let document: Document = parse_schema(&package.read_entry(document_entry)?)?;
    let doc_dir = parent_dir(document_entry).unwrap_or("");
    let loc = document
        .common_data
        .document_res
        .first()
        .ok_or_else(|| {
            OfdError::InvalidStructure("图片注释需要 Document.xml 声明 DocumentRes".to_owned())
        })?
        .clone();
    let entry = resolve_loc(doc_dir, loc.as_str())?;
    let res: Res = if package.contains(&entry) {
        parse_schema(&package.read_entry(&entry)?)?
    } else {
        Res {
            base_loc: StLoc::new("Res"),
            ..Res::default()
        }
    };
    let dir = resolve_loc(doc_dir, res.base_loc.as_str())?;
    Ok(ResState {
        entry,
        dir,
        res,
        images: Vec::new(),
    })
}

fn serialize_schema<T: ofdbox_yaserde::YaSerialize>(value: &T) -> Result<Vec<u8>, OfdError> {
    model_xml::to_bytes(value).map_err(|error| OfdError::InvalidStructure(error.to_string()))
}

fn image_extension(format: &str) -> &'static str {
    match format.to_ascii_uppercase().as_str() {
        "PNG" => "png",
        "JPG" | "JPEG" => "jpg",
        "GIF" => "gif",
        "BMP" => "bmp",
        "SVG" => "svg",
        _ => "bin",
    }
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
