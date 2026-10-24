//! 盖章外观：把印章图片写成资源、页面注释，并在 `Signature.xml` 里关联 `StampAnnot`。
//!
//! 这里只做“文档已经声明 `DocumentRes` 与 `Annotations`”的增量写入；模板层面
//! 的索引创建留给调用方（示例会建好这两个入口）。生成的文件同样会被签名保护。

use quick_xml::escape::escape;

use crate::error::OfdError;
use crate::package::{OfdPackage, parent_dir, resolve_loc};
use crate::sign::detect_prefix;

const OFD_NS: &str = crate::OFD_NAMESPACE;

/// 盖章位置与印模。
#[derive(Clone, Debug)]
pub struct StampAppearance {
    /// 目标页 `Page/@ID`。
    pub page_id: String,
    /// 章在页面上的位置，单位毫米：`x y width height`。
    pub boundary: [f64; 4],
    /// 裁剪区域，单位毫米；缺省与 `boundary` 相同。
    pub clip: Option<[f64; 4]>,
    /// 印模图片字节（通常取自 `SESeal` 的 `picture`）。
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
    pub(crate) annot: StampAnnot,
}

pub(crate) fn prepare(
    package: &mut OfdPackage,
    stamp: &StampAppearance,
    document_entry: &str,
) -> Result<StampFiles, OfdError> {
    validate(stamp)?;

    let doc_dir = parent_dir(document_entry).unwrap_or("").to_owned();
    let document = String::from_utf8(package.read_entry(document_entry)?.to_vec())
        .map_err(|error| OfdError::InvalidStructure(format!("Document.xml 不是 UTF-8: {error}")))?;

    let document_res = element_text(&document, "DocumentRes").ok_or_else(|| {
        OfdError::InvalidStructure("盖章需要 Document.xml 声明 DocumentRes".to_owned())
    })?;
    let annotations = element_text(&document, "Annotations").ok_or_else(|| {
        OfdError::InvalidStructure("盖章需要 Document.xml 声明 Annotations".to_owned())
    })?;
    let annotations_entry = resolve_loc(&doc_dir, annotations)?;
    let document_res_entry = resolve_loc(&doc_dir, document_res)?;

    let max_unit = element_text(&document, "MaxUnitID")
        .and_then(|text| text.trim().parse::<i64>().ok())
        .ok_or_else(|| OfdError::InvalidStructure("Document.xml 缺少 MaxUnitID".to_owned()))?;
    let annot_id = max_unit + 1;
    let image_object_id = max_unit + 2;
    let multimedia_id = max_unit + 3;
    let document = set_element_text(&document, "MaxUnitID", &multimedia_id.to_string())
        .ok_or_else(|| OfdError::InvalidStructure("Document.xml 缺少 MaxUnitID".to_owned()))?;

    // 资源目录：优先取 DocumentRes 的 BaseLoc，缺省为 `Res`。
    let mut res = String::new();
    if let Ok(bytes) = package.read_entry(&document_res_entry)
        && let Ok(text) = std::str::from_utf8(&bytes)
    {
        res = element_attribute(text, "Res", "BaseLoc").unwrap_or_default();
    }
    let res_dir = resolve_loc(&doc_dir, if res.is_empty() { "Res" } else { &res })?;
    // 文件名用数字 ID，不把页 ID 带进路径。
    let extension = image_extension(&stamp.image_format);
    let image_name = unique_name(package, &res_dir, &format!("seal_{annot_id}.{extension}"));
    let image_entry = format!("{res_dir}/{image_name}");

    // DocumentRes：追加一张图片资源。
    let mut document_res_xml = match package.read_entry(&document_res_entry) {
        Ok(bytes) => String::from_utf8(bytes.to_vec()).map_err(|error| {
            OfdError::InvalidStructure(format!("DocumentRes 不是 UTF-8: {error}"))
        })?,
        Err(_) => format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="{OFD_NS}" BaseLoc="Res"><ofd:MultiMedias></ofd:MultiMedias></ofd:Res>"#
        ),
    };
    let res_prefix = detect_prefix(&document_res_xml)?.to_owned();
    let media = format!(
        r#"<{res_prefix}MultiMedia ID="{multimedia_id}" Type="Image" Format="{}"><{res_prefix}MediaFile>{}</{res_prefix}MediaFile></{res_prefix}MultiMedia>"#,
        escape(&stamp.image_format),
        escape(&image_name)
    );
    document_res_xml = match insert_before_close(&document_res_xml, "MultiMedias", &media) {
        Some(updated) => updated,
        None => insert_before_close(
            &document_res_xml,
            "Res",
            &format!("<{res_prefix}MultiMedias>{media}</{res_prefix}MultiMedias>"),
        )
        .ok_or_else(|| OfdError::InvalidStructure("DocumentRes 缺少 Res 根元素".to_owned()))?,
    };

    // 注释索引：同一页已有条目时复用其注释文件，避免重复 `<ofd:Page>`。
    let index_dir = parent_dir(&annotations_entry).unwrap_or("").to_owned();
    let mut index_xml = match package.read_entry(&annotations_entry) {
        Ok(bytes) => String::from_utf8(bytes.to_vec()).map_err(|error| {
            OfdError::InvalidStructure(format!("Annotations 不是 UTF-8: {error}"))
        })?,
        Err(_) => format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Annotations xmlns:ofd="{OFD_NS}"></ofd:Annotations>"#
        ),
    };
    let index_prefix = detect_prefix(&index_xml)?.to_owned();
    let page_marker = format!("PageID=\"{}\"", escape(&stamp.page_id));

    let (annot_entry, mut annot_xml, new_page) = match page_file_loc(&index_xml, &page_marker) {
        Some(loc) => {
            let entry = resolve_loc(&index_dir, &loc)?;
            let xml = match package.read_entry(&entry) {
                Ok(bytes) => String::from_utf8(bytes.to_vec()).map_err(|error| {
                    OfdError::InvalidStructure(format!("注释文件不是 UTF-8: {error}"))
                })?,
                Err(_) => format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:PageAnnot xmlns:ofd="{OFD_NS}"></ofd:PageAnnot>"#
                ),
            };
            (entry, xml, false)
        }
        None => {
            let name = unique_name(package, &index_dir, &format!("PageAnnot_{annot_id}.xml"));
            let entry = format!("{index_dir}/{name}");
            let page_entry = format!(
                r#"<{index_prefix}Page PageID="{}"><{index_prefix}FileLoc>{}</{index_prefix}FileLoc></{index_prefix}Page>"#,
                escape(&stamp.page_id),
                escape(&name)
            );
            index_xml = insert_before_close(&index_xml, "Annotations", &page_entry)
                .ok_or_else(|| OfdError::InvalidStructure("Annotations 缺少结束标签".to_owned()))?;
            (
                entry,
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:PageAnnot xmlns:ofd="{OFD_NS}"></ofd:PageAnnot>"#
                ),
                true,
            )
        }
    };
    let annot_prefix = detect_prefix(&annot_xml)?.to_owned();
    let boundary = format_boundary(&stamp.boundary);
    let annot = format!(
        r#"<{annot_prefix}Annot Type="Stamp" ID="{annot_id}" Subtype="Stamp"><{annot_prefix}Appearance Boundary="{boundary}"><{annot_prefix}ImageObject ID="{image_object_id}" ResourceID="{multimedia_id}" Boundary="0 0 {width} {height}" CTM="{width} 0 0 {height} 0 0"/></{annot_prefix}Appearance></{annot_prefix}Annot>"#,
        width = stamp.image_width,
        height = stamp.image_height,
    );
    annot_xml = insert_before_close(&annot_xml, "PageAnnot", &annot)
        .ok_or_else(|| OfdError::InvalidStructure("注释文件缺少 PageAnnot 根元素".to_owned()))?;

    let mut entries = vec![
        (document_res_entry, document_res_xml.into_bytes()),
        (annot_entry, annot_xml.into_bytes()),
        (image_entry, stamp.image.clone()),
    ];
    if new_page {
        entries.push((annotations_entry, index_xml.into_bytes()));
    }

    Ok(StampFiles {
        document_xml: document.into_bytes(),
        entries,
        annot: StampAnnot {
            id: annot_id.to_string(),
            page_id: stamp.page_id.clone(),
            boundary: stamp.boundary,
            clip: Some(stamp.clip.unwrap_or(stamp.boundary)),
        },
    })
}

fn validate(stamp: &StampAppearance) -> Result<(), OfdError> {
    if stamp.boundary.iter().any(|value| !value.is_finite())
        || stamp.boundary[2] <= 0.0
        || stamp.boundary[3] <= 0.0
    {
        return Err(OfdError::InvalidStructure(
            "盖章区域必须是有限正数".to_owned(),
        ));
    }
    if stamp.clip.is_some_and(|clip| {
        clip.iter().any(|value| !value.is_finite()) || clip[2] <= 0.0 || clip[3] <= 0.0
    }) {
        return Err(OfdError::InvalidStructure(
            "裁剪区域必须是有限正数".to_owned(),
        ));
    }
    if stamp.image.is_empty() {
        return Err(OfdError::InvalidStructure("印模图片不能为空".to_owned()));
    }
    if stamp.image_format.trim().is_empty() {
        return Err(OfdError::InvalidStructure("印模格式不能为空".to_owned()));
    }
    if !stamp.image_width.is_finite()
        || !stamp.image_height.is_finite()
        || stamp.image_width <= 0.0
        || stamp.image_height <= 0.0
    {
        return Err(OfdError::InvalidStructure(
            "印模显示尺寸必须是有限正数".to_owned(),
        ));
    }
    Ok(())
}

fn image_extension(format: &str) -> &'static str {
    match format.trim().to_ascii_lowercase().as_str() {
        "png" => "png",
        "jpg" | "jpeg" => "jpg",
        "gif" => "gif",
        "bmp" => "bmp",
        "svg" => "svg",
        _ => "png",
    }
}

fn format_boundary(boundary: &[f64; 4]) -> String {
    format!(
        "{} {} {} {}",
        boundary[0], boundary[1], boundary[2], boundary[3]
    )
}

fn element_text<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let mut search = 0;
    while let Some(offset) = xml[search..].find(name) {
        let start = search + offset + name.len();
        let rest = &xml[start..];
        if let Some(inner) = rest.strip_prefix('>') {
            let end = inner.find('<')?;
            return Some(inner[..end].trim());
        }
        search = start;
    }
    None
}

fn set_element_text(xml: &str, name: &str, value: &str) -> Option<String> {
    let mut search = 0;
    while let Some(offset) = xml[search..].find(name) {
        let start = search + offset + name.len();
        if let Some(inner) = xml[start..].strip_prefix('>') {
            let end = inner.find('<')?;
            let mut output = String::with_capacity(xml.len() + value.len());
            output.push_str(&xml[..start + 1]);
            output.push_str(value);
            output.push_str(&xml[start + 1 + end..]);
            return Some(output);
        }
        search = start;
    }
    None
}

fn element_attribute(xml: &str, element: &str, attribute: &str) -> Option<String> {
    let start = xml.find(element)?;
    let rest = &xml[start..];
    let end = rest.find('>')?;
    let tag = &rest[..end];
    let marker = format!("{attribute}=\"");
    let attr_start = tag.find(&marker)? + marker.len();
    let attr_end = tag[attr_start..].find('"')?;
    Some(tag[attr_start..attr_start + attr_end].to_owned())
}

/// 找到索引里同一 `PageID` 的 `FileLoc`。
fn page_file_loc(xml: &str, page_marker: &str) -> Option<String> {
    let start = xml.find(page_marker)?;
    let rest = &xml[start..];
    let tag = rest.find("FileLoc>")?;
    let inner = &rest[tag + "FileLoc>".len()..];
    let end = inner.find('<')?;
    Some(inner[..end].trim().to_owned())
}

struct Tag<'a> {
    start: usize,
    end: usize,
    name: &'a str,
    self_closing: bool,
}

fn find_last_tag<'a>(xml: &'a str, name: &str) -> Option<Tag<'a>> {
    let mut closing = None;
    let mut self_closing = None;
    let mut position = 0;
    while let Some(offset) = xml[position..].find('<') {
        let start = position + offset;
        if xml[start..].starts_with("<!--") {
            let comment = xml[start + 4..].find("-->")?;
            position = start + 4 + comment + 3;
            continue;
        }
        let end = start + xml[start..].find('>')?;
        let inner = &xml[start + 1..end];
        let (is_closing, body) = match inner.strip_prefix('/') {
            Some(body) => (true, body.trim()),
            None => (false, inner),
        };
        let tag_name = body.split_whitespace().next().unwrap_or("");
        let tag_name = tag_name.trim_end_matches('/');
        if tag_name == name || tag_name.ends_with(&format!(":{name}")) {
            let tag = Tag {
                start,
                end,
                name: tag_name,
                self_closing: !is_closing && xml[start..end].ends_with('/'),
            };
            if is_closing {
                closing = Some(tag);
            } else if tag.self_closing {
                self_closing = Some(tag);
            }
        }
        position = end + 1;
    }
    closing.or(self_closing)
}

/// 在 `name` 元素的结束标签前插入 `fragment`；自闭合元素则展开后插入。
fn insert_before_close(xml: &str, name: &str, fragment: &str) -> Option<String> {
    let tag = find_last_tag(xml, name)?;
    let mut output = String::with_capacity(xml.len() + fragment.len() + 64);
    output.push_str(&xml[..tag.start]);
    if tag.self_closing {
        // 去掉 `/>` 的 `/`，展开成 `<name>fragment</name>`。
        output.push_str(&xml[tag.start..tag.end - 1]);
        output.push('>');
        output.push_str(fragment);
        output.push_str("</");
        output.push_str(tag.name);
        output.push('>');
        output.push_str(&xml[tag.end + 1..]);
    } else {
        output.push_str(fragment);
        output.push_str(&xml[tag.start..]);
    }
    Some(output)
}

fn unique_name(package: &mut OfdPackage, dir: &str, name: &str) -> String {
    let mut candidate = name.to_owned();
    let mut index = 1;
    while package.contains(&format!("{dir}/{candidate}")) {
        let stem = name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name);
        let extension = name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
        candidate = format!("{stem}_{index}.{extension}");
        index += 1;
    }
    candidate
}
