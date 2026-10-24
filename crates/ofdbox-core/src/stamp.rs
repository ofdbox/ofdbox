//! 盖章外观：把印章图片写成资源、页面注释，并在 `Signature.xml` 里关联 `StampAnnot`。
//!
//! 这里只做“文档已经声明 `DocumentRes` 与 `Annotations`”的增量写入；模板层面
//! 的索引创建留给调用方（示例会建好这两个入口）。生成的文件同样会被签名保护。

use crate::error::OfdError;
use crate::package::{OfdPackage, parent_dir, resolve_loc};

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

const OFD_NS: &str = "http://www.ofdspec.org/2016";

pub(crate) fn prepare(
    package: &mut OfdPackage,
    stamp: &StampAppearance,
    document_entry: &str,
) -> Result<StampFiles, OfdError> {
    if stamp.boundary[2] <= 0.0 || stamp.boundary[3] <= 0.0 {
        return Err(OfdError::InvalidStructure("盖章区域必须为正数".to_owned()));
    }
    if stamp.image.is_empty() {
        return Err(OfdError::InvalidStructure("印模图片不能为空".to_owned()));
    }

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
        .unwrap_or(0);
    let annot_id = max_unit + 1;
    let image_object_id = max_unit + 2;
    let multimedia_id = max_unit + 3;
    let document = set_element_text(&document, "MaxUnitID", &(multimedia_id).to_string())
        .ok_or_else(|| OfdError::InvalidStructure("Document.xml 缺少 MaxUnitID".to_owned()))?;

    // 资源目录：优先取 DocumentRes 的 BaseLoc，缺省为 `Res`。
    let mut res = String::new();
    if let Ok(bytes) = package.read_entry(&document_res_entry)
        && let Ok(text) = std::str::from_utf8(&bytes)
    {
        res = element_attribute(text, "Res", "BaseLoc").unwrap_or_default();
    }
    let res_dir = resolve_loc(&doc_dir, if res.is_empty() { "Res" } else { &res })?;
    let image_name = unique_name(package, &res_dir, &format!("seal_{}.png", stamp.page_id));
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
    let entry = format!(
        r#"<ofd:MultiMedia ID="{multimedia_id}" Type="Image" Format="{}"><ofd:MediaFile>{image_name}</ofd:MediaFile></ofd:MultiMedia>"#,
        stamp.image_format
    );
    document_res_xml = insert_before_close(&document_res_xml, "MultiMedias", &entry)
        .ok_or_else(|| OfdError::InvalidStructure("DocumentRes 缺少 MultiMedias".to_owned()))?;

    // 注释索引：追加一个页面入口。
    let index_dir = parent_dir(&annotations_entry).unwrap_or("").to_owned();
    let annot_name = unique_name(
        package,
        &index_dir,
        &format!("PageAnnot_{}.xml", stamp.page_id),
    );
    let annot_entry = format!("{index_dir}/{annot_name}");
    let mut index_xml = match package.read_entry(&annotations_entry) {
        Ok(bytes) => String::from_utf8(bytes.to_vec()).map_err(|error| {
            OfdError::InvalidStructure(format!("Annotations 不是 UTF-8: {error}"))
        })?,
        Err(_) => format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Annotations xmlns:ofd="{OFD_NS}"></ofd:Annotations>"#
        ),
    };
    let page_entry = format!(
        r#"<ofd:Page PageID="{}"><ofd:FileLoc>{annot_name}</ofd:FileLoc></ofd:Page>"#,
        stamp.page_id
    );
    index_xml = insert_before_close(&index_xml, "Annotations", &page_entry)
        .ok_or_else(|| OfdError::InvalidStructure("Annotations 缺少结束标签".to_owned()))?;

    let boundary = format_boundary(&stamp.boundary);
    let clip_rect = stamp.clip.unwrap_or(stamp.boundary);
    let annot_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:PageAnnot xmlns:ofd="{OFD_NS}"><ofd:Annot Type="Stamp" ID="{annot_id}" Subtype="Stamp"><ofd:Appearance Boundary="{boundary}"><ofd:ImageObject ID="{image_object_id}" ResourceID="{multimedia_id}" Boundary="0 0 {width} {height}" CTM="{width} 0 0 {height} 0 0"/></ofd:Appearance></ofd:Annot></ofd:PageAnnot>"#,
        width = stamp.image_width,
        height = stamp.image_height,
    );

    Ok(StampFiles {
        document_xml: document.into_bytes(),
        entries: vec![
            (document_res_entry, document_res_xml.into_bytes()),
            (annotations_entry, index_xml.into_bytes()),
            (annot_entry, annot_xml.into_bytes()),
            (image_entry, stamp.image.clone()),
        ],
        annot: StampAnnot {
            id: annot_id.to_string(),
            page_id: stamp.page_id.clone(),
            boundary: stamp.boundary,
            clip: Some(clip_rect),
        },
    })
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

fn insert_before_close(xml: &str, name: &str, fragment: &str) -> Option<String> {
    let closing = xml.rfind(name)?;
    let close_start = xml[..closing].rfind('<')?;
    let mut output = String::with_capacity(xml.len() + fragment.len());
    output.push_str(&xml[..close_start]);
    output.push_str(fragment);
    output.push_str(&xml[close_start..]);
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
