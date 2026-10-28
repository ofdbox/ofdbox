//! 盖章外观：把印章图片写成资源、页面注释，并在 `Signature.xml` 里关联 `StampAnnot`。
//!
//! 这里只做“文档已经声明 DocumentRes 与 Annotations”的增量写入。
//! 所有结构修改都通过 schema 模型完成。

use std::collections::{HashMap, HashSet};

use crate::error::OfdError;
use crate::package::{OfdPackage, parent_dir, resolve_loc};
use crate::schema::annotation::{Annot, AnnotationType, Appearance, PageAnnot};
use crate::schema::annotations::{Annotations, AnnotationsPage};
use crate::schema::definition::{StBox, StId, StLoc, StRefId};
use crate::schema::document::Document;
use crate::schema::page::{GraphicObject, GraphicUnit, ImageObject, PageBlock};
use crate::schema::res::{MultiMedia, MultiMediaType, MultiMedias, Res};
use crate::schema_xml;

/// 盖章位置与印模。
#[derive(Clone, Debug)]
pub struct StampAppearance {
    /// 目标页 `Page/@ID`。
    pub page_id: String,
    /// 章在页面上的位置，单位毫米：`x y width height`。
    pub boundary: [f64; 4],
    /// 裁剪区域，单位毫米；缺省不写 `Clip`。
    pub clip: Option<[f64; 4]>,
    /// 印模图片字节。
    pub image: Vec<u8>,
    /// 图片格式，如 `PNG`、`GIF`、`JPG`、`BMP`、`SVG`。
    pub image_format: String,
    /// 印模显示宽度，单位毫米。
    pub image_width: f64,
    /// 印模显示高度，单位毫米。
    pub image_height: f64,
}

/// `StampAnnot` 需要写进签名的字段。
#[derive(Clone, Debug)]
pub(crate) struct StampAnnot {
    pub(crate) id: String,
    pub(crate) page_id: String,
    pub(crate) boundary: [f64; 4],
    pub(crate) clip: Option<[f64; 4]>,
}

/// 盖章需要写入或替换的包内文件。
pub(crate) struct StampFiles {
    /// `Document.xml` 的替换字节（更新 `MaxUnitID`）。
    pub(crate) document_xml: Vec<u8>,
    /// `(包内路径, 字节)`，可能是新增也可能是替换。
    pub(crate) entries: Vec<(String, Vec<u8>)>,
    /// 按传入顺序对应的 `StampAnnot`。
    pub(crate) annots: Vec<StampAnnot>,
}

pub(crate) fn prepare(
    package: &mut OfdPackage,
    stamps: &[StampAppearance],
    document_entry: &str,
) -> Result<StampFiles, OfdError> {
    if stamps.is_empty() {
        return Err(OfdError::InvalidStructure("至少需要一枚印章".to_owned()));
    }
    for stamp in stamps {
        validate(stamp)?;
    }

    let document_bytes = package.read_entry(document_entry)?.to_vec();
    let mut document: Document = parse_schema(&document_bytes)?;
    let doc_dir = parent_dir(document_entry).unwrap_or("").to_owned();
    let pages: HashSet<String> = document
        .pages
        .pages
        .iter()
        .map(|page| page.id.clone())
        .collect();

    let document_res_loc = document
        .common_data
        .document_res
        .first()
        .ok_or_else(|| {
            OfdError::InvalidStructure("盖章需要 Document.xml 声明 DocumentRes".to_owned())
        })?
        .clone();
    let document_res_entry = resolve_loc(&doc_dir, document_res_loc.as_str())?;
    let mut res: Res = if package.contains(&document_res_entry) {
        parse_schema(&package.read_entry(&document_res_entry)?)?
    } else {
        Res {
            base_loc: StLoc::new("Res"),
            ..Res::default()
        }
    };
    let res_dir = resolve_loc(&doc_dir, res.base_loc.as_str())?;

    let annotations_loc = document.annotations.clone().ok_or_else(|| {
        OfdError::InvalidStructure("盖章需要 Document.xml 声明 Annotations".to_owned())
    })?;
    let annotations_entry = resolve_loc(&doc_dir, annotations_loc.as_str())?;
    let mut annotations: Annotations = if package.contains(&annotations_entry) {
        parse_schema(&package.read_entry(&annotations_entry)?)?
    } else {
        Annotations::default()
    };
    let index_dir = parent_dir(&annotations_entry).unwrap_or("").to_owned();

    let mut page_annots: HashMap<String, (String, PageAnnot)> = HashMap::new();
    for page in &annotations.pages {
        let entry = resolve_loc(&index_dir, page.file_loc.as_str())?;
        if !package.contains(&entry) {
            continue;
        }
        let annot = parse_schema(&package.read_entry(&entry)?)?;
        page_annots.insert(page.page_id.clone(), (entry, annot));
    }

    let mut next_id = i64::from(document.common_data.max_unit_id.0) + 1;
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut created_names: HashSet<String> = HashSet::new();
    let mut annots = Vec::with_capacity(stamps.len());

    for stamp in stamps {
        if !pages.contains(&stamp.page_id) {
            return Err(OfdError::InvalidStructure(format!(
                "Document.xml 中不存在页面 {}",
                stamp.page_id
            )));
        }

        let annot_id = next_id;
        let image_object_id = next_id + 1;
        let multimedia_id = next_id + 2;
        next_id += 3;

        let extension = image_extension(&stamp.image_format);
        let image_name = unique_name(
            package,
            &created_names,
            &res_dir,
            &format!("seal_{annot_id}.{extension}"),
        );
        let image_entry = join_entry(&res_dir, &image_name);
        created_names.insert(image_entry.clone());
        entries.push((image_entry, stamp.image.clone()));

        let multi_medias = res.multi_medias.get_or_insert_with(MultiMedias::default);
        multi_medias.multi_medias.push(MultiMedia {
            id: StId(multimedia_id as u32),
            media_type: MultiMediaType::Image,
            format: Some(stamp.image_format.clone()),
            media_file: StLoc::new(image_name),
        });

        let (annot_entry, page_annot) =
            if let Some((entry, annot)) = page_annots.get_mut(&stamp.page_id) {
                (entry.clone(), annot)
            } else {
                let name = unique_name(
                    package,
                    &created_names,
                    &index_dir,
                    &format!("PageAnnot_{annot_id}.xml"),
                );
                let entry = join_entry(&index_dir, &name);
                created_names.insert(entry.clone());
                annotations.pages.push(AnnotationsPage {
                    page_id: stamp.page_id.clone(),
                    file_loc: StLoc::new(name),
                });
                page_annots.insert(stamp.page_id.clone(), (entry.clone(), PageAnnot::default()));
                let (entry, annot) = page_annots
                    .get_mut(&stamp.page_id)
                    .expect("page annot inserted");
                (entry.clone(), annot)
            };
        let _ = &annot_entry;

        let boundary = StBox::new(
            stamp.boundary[0],
            stamp.boundary[1],
            stamp.boundary[2],
            stamp.boundary[3],
        );
        page_annot.annots.push(Annot {
            id: StId(annot_id as u32),
            annot_type: AnnotationType::Stamp,
            creator: "ofdbox".to_owned(),
            last_mod_date: String::new(),
            visible: true,
            subtype: Some("Stamp".to_owned()),
            print: true,
            no_zoom: false,
            no_rotate: false,
            read_only: true,
            remark: None,
            parameters: None,
            appearance: Appearance {
                boundary: Some(boundary),
                page_block: PageBlock {
                    objects: vec![GraphicObject::Image(ImageObject {
                        common: GraphicUnit {
                            id: StId(image_object_id as u32),
                            boundary: StBox::new(0.0, 0.0, stamp.image_width, stamp.image_height),
                            ..GraphicUnit::default()
                        },
                        resource_id: StRefId(multimedia_id as u32),
                        substitution: None,
                        image_mask: None,
                        border: None,
                    })],
                },
            },
        });

        annots.push(StampAnnot {
            id: annot_id.to_string(),
            page_id: stamp.page_id.clone(),
            boundary: stamp.boundary,
            clip: stamp.clip,
        });
    }

    for (entry, annot) in page_annots.values() {
        entries.push((entry.clone(), serialize_schema(annot)?));
    }
    entries.push((document_res_entry, serialize_schema(&res)?));
    entries.push((annotations_entry, serialize_schema(&annotations)?));

    document.common_data.max_unit_id = StId((next_id - 1) as u32);
    Ok(StampFiles {
        document_xml: serialize_schema(&document)?,
        entries,
        annots,
    })
}

fn parse_schema<T: yaserde::YaDeserialize>(bytes: &[u8]) -> Result<T, OfdError> {
    schema_xml::from_bytes(bytes).map_err(|error| OfdError::InvalidStructure(error.to_string()))
}

fn serialize_schema<T: yaserde::YaSerialize>(value: &T) -> Result<Vec<u8>, OfdError> {
    schema_xml::to_bytes(value).map_err(|error| OfdError::InvalidStructure(error.to_string()))
}

fn validate(stamp: &StampAppearance) -> Result<(), OfdError> {
    if stamp.page_id.is_empty() {
        return Err(OfdError::InvalidStructure(
            "盖章页面 ID 不能为空".to_owned(),
        ));
    }
    if !stamp.boundary.iter().all(|value| value.is_finite()) {
        return Err(OfdError::InvalidStructure(
            "盖章边界必须是有限数值".to_owned(),
        ));
    }
    if stamp.image.is_empty() {
        return Err(OfdError::InvalidStructure("印章图片不能为空".to_owned()));
    }
    if stamp.image_width <= 0.0 || stamp.image_height <= 0.0 {
        return Err(OfdError::InvalidStructure("印章尺寸必须大于 0".to_owned()));
    }
    Ok(())
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

fn unique_name(package: &OfdPackage, used: &HashSet<String>, dir: &str, name: &str) -> String {
    let mut candidate = name.to_owned();
    let mut index = 1;
    while package.contains(&join_entry(dir, &candidate))
        || used.contains(&join_entry(dir, &candidate))
    {
        candidate = format!("{index}_{name}");
        index += 1;
    }
    candidate
}

fn join_entry(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}
