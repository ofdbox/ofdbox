//! 盖章外观：把印章图片写成资源、页面注释，并在 `Signature.xml` 里关联 `StampAnnot`。
//!
//! 这里只做“文档已经声明 `DocumentRes` 与 `Annotations`”的增量写入；模板层面
//! 的索引创建留给调用方（示例会建好这两个入口）。生成的文件同样会被签名保护。
//!
//! 元素定位与插入全部走 [`XmlDocument`] 语法树和源字节区间，不再按字符串查找标签。

use std::collections::HashMap;

use quick_xml::escape::escape;

use crate::error::OfdError;
use crate::options::ReadOptions;
use crate::package::{OfdPackage, parent_dir, resolve_loc};
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

const OFD_NS: &str = crate::OFD_NAMESPACE;

/// 盖章位置与印模。
#[derive(Clone, Debug)]
pub struct StampAppearance {
    /// 目标页 `Page/@ID`。
    pub page_id: String,
    /// 章在页面上的位置，单位毫米：`x y width height`。
    pub boundary: [f64; 4],
    /// 裁剪区域，单位毫米；缺省不写 `Clip`。
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
    /// 按传入顺序对应的 `StampAnnot`。
    pub(crate) annots: Vec<StampAnnot>,
}

struct AnnotFile {
    entry: String,
    source: String,
    prefix: String,
    fragments: String,
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

    let doc_dir = parent_dir(document_entry).unwrap_or("").to_owned();
    let document = utf8(package.read_entry(document_entry)?.to_vec(), "Document.xml")?;
    let document_doc = parse(&document)?;
    let document_root = document_doc.root();
    let common = child(&document_doc, document_root, "CommonData")
        .ok_or_else(|| OfdError::InvalidStructure("Document.xml 缺少 CommonData".to_owned()))?;
    let document_res = text(&document_doc, child(&document_doc, common, "DocumentRes"))
        .ok_or_else(|| {
            OfdError::InvalidStructure("盖章需要 Document.xml 声明 DocumentRes".to_owned())
        })?;
    let annotations = text(
        &document_doc,
        child(&document_doc, document_root, "Annotations"),
    )
    .ok_or_else(|| {
        OfdError::InvalidStructure("盖章需要 Document.xml 声明 Annotations".to_owned())
    })?;
    let annotations_entry = resolve_loc(&doc_dir, &annotations)?;
    let document_res_entry = resolve_loc(&doc_dir, &document_res)?;

    let pages: std::collections::HashSet<String> = child(&document_doc, document_root, "Pages")
        .map(|pages_node| {
            document_doc
                .children_named(pages_node, "Page")
                .filter_map(|page| document_doc.attribute(page, "ID").map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    for stamp in stamps {
        if !pages.contains(&stamp.page_id) {
            return Err(OfdError::InvalidStructure(format!(
                "Document.xml 中不存在页面 {}",
                stamp.page_id
            )));
        }
    }

    let max_node = child(&document_doc, common, "MaxUnitID")
        .ok_or_else(|| OfdError::InvalidStructure("Document.xml 缺少 MaxUnitID".to_owned()))?;
    let max_unit = document_doc
        .text(max_node)
        .trim()
        .parse::<i64>()
        .map_err(|_| OfdError::InvalidStructure("Document.xml 的 MaxUnitID 不是数字".to_owned()))?;
    // 每枚章占三个单元：Annot、ImageObject、MultiMedia。
    let mut next_id = max_unit + 1;

    // DocumentRes：读取源文件，资源片段在最后统一插入。
    let document_res_source = match package.read_entry(&document_res_entry) {
        Ok(bytes) => utf8(bytes.to_vec(), "DocumentRes")?,
        Err(_) => skeleton_res(),
    };
    let document_res_doc = parse(&document_res_source)?;
    let res_root = document_res_doc.root();
    let res_prefix = prefix(&document_res_doc, res_root);

    // 资源目录：优先取 DocumentRes 的 BaseLoc，缺省为 `Res`。
    let base_loc = document_res_doc
        .attribute(res_root, "BaseLoc")
        .unwrap_or("Res")
        .to_owned();
    let res_dir = resolve_loc(&doc_dir, &base_loc)?;

    // 注释索引：读取一次，新增页面条目在最后统一插入。
    let index_source = match package.read_entry(&annotations_entry) {
        Ok(bytes) => utf8(bytes.to_vec(), "Annotations")?,
        Err(_) => format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Annotations xmlns:ofd="{OFD_NS}"></ofd:Annotations>"#
        ),
    };
    let index_doc = parse(&index_source)?;
    let index_root = index_doc.root();
    let index_prefix = prefix(&index_doc, index_root);
    let index_dir = parent_dir(&annotations_entry).unwrap_or("").to_owned();
    let mut created_pages: HashMap<String, String> = HashMap::new();
    let mut new_pages = String::new();

    let mut annot_files: Vec<AnnotFile> = Vec::new();
    let mut media = String::new();
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();
    let mut annots = Vec::with_capacity(stamps.len());

    for stamp in stamps {
        let annot_id = next_id;
        let image_object_id = next_id + 1;
        let multimedia_id = next_id + 2;
        next_id += 3;

        let extension = image_extension(&stamp.image_format);
        let image_name = unique_name(package, &res_dir, &format!("seal_{annot_id}.{extension}"));
        let image_entry = format!("{res_dir}/{image_name}");
        media.push_str(&format!(
            r#"<{res_prefix}MultiMedia ID="{multimedia_id}" Type="Image" Format="{}"><{res_prefix}MediaFile>{}</{res_prefix}MediaFile></{res_prefix}MultiMedia>"#,
            escape(&stamp.image_format),
            escape(&image_name)
        ));
        images.push((image_entry, stamp.image.clone()));

        let annot_entry = if let Some(entry) = created_pages.get(&stamp.page_id) {
            entry.clone()
        } else if let Some(loc) = page_file(&index_doc, &stamp.page_id) {
            resolve_loc(&index_dir, &loc)?
        } else {
            let name = unique_name(package, &index_dir, &format!("PageAnnot_{annot_id}.xml"));
            let entry = format!("{index_dir}/{name}");
            new_pages.push_str(&format!(
                r#"<{index_prefix}Page PageID="{}"><{index_prefix}FileLoc>{}</{index_prefix}FileLoc></{index_prefix}Page>"#,
                escape(&stamp.page_id),
                escape(&name)
            ));
            created_pages.insert(stamp.page_id.clone(), entry.clone());
            entry
        };

        let index = match annot_files
            .iter()
            .position(|file| file.entry == annot_entry)
        {
            Some(index) => index,
            None => {
                let source = match package.read_entry(&annot_entry) {
                    Ok(bytes) => utf8(bytes.to_vec(), "注释文件")?,
                    Err(_) => format!(
                        r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:PageAnnot xmlns:ofd="{OFD_NS}"></ofd:PageAnnot>"#
                    ),
                };
                let doc = parse(&source)?;
                let prefix = prefix(&doc, doc.root());
                annot_files.push(AnnotFile {
                    entry: annot_entry.clone(),
                    source,
                    prefix,
                    fragments: String::new(),
                });
                annot_files.len() - 1
            }
        };
        let annot_prefix = annot_files[index].prefix.clone();
        let boundary = format_boundary(&stamp.boundary);
        annot_files[index].fragments.push_str(&format!(
            r#"<{annot_prefix}Annot Type="Stamp" ID="{annot_id}" Subtype="Stamp"><{annot_prefix}Appearance Boundary="{boundary}"><{annot_prefix}ImageObject ID="{image_object_id}" ResourceID="{multimedia_id}" Boundary="0 0 {width} {height}" CTM="{width} 0 0 {height} 0 0"/></{annot_prefix}Appearance></{annot_prefix}Annot>"#,
            width = stamp.image_width,
            height = stamp.image_height,
        ));

        annots.push(StampAnnot {
            id: annot_id.to_string(),
            page_id: stamp.page_id.clone(),
            boundary: stamp.boundary,
            clip: stamp.clip,
        });
    }

    let document = replace_text(
        &document,
        &document_doc,
        max_node,
        &(next_id - 1).to_string(),
    )?;

    let document_res = if let Some(multi) = child(&document_res_doc, res_root, "MultiMedias") {
        insert_children(&document_res_source, &document_res_doc, multi, &media)?
    } else {
        insert_children(
            &document_res_source,
            &document_res_doc,
            res_root,
            &format!("<{res_prefix}MultiMedias>{media}</{res_prefix}MultiMedias>"),
        )?
    };

    let index_source = if new_pages.is_empty() {
        index_source
    } else {
        insert_children(&index_source, &index_doc, index_root, &new_pages)?
    };

    let mut entries = vec![(document_res_entry, document_res.into_bytes())];
    if !new_pages.is_empty() {
        entries.push((annotations_entry, index_source.into_bytes()));
    }
    for file in annot_files {
        let doc = parse(&file.source)?;
        let xml = insert_children(&file.source, &doc, doc.root(), &file.fragments)?;
        entries.push((file.entry, xml.into_bytes()));
    }
    entries.extend(images);

    Ok(StampFiles {
        document_xml: document.into_bytes(),
        entries,
        annots,
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

fn utf8(bytes: Vec<u8>, what: &str) -> Result<String, OfdError> {
    String::from_utf8(bytes)
        .map_err(|error| OfdError::InvalidStructure(format!("{what} 不是 UTF-8: {error}")))
}

fn parse(xml: &str) -> Result<XmlDocument, OfdError> {
    parse_xml(xml.as_bytes(), ReadOptions::LENIENT)
}

fn prefix(doc: &XmlDocument, node: NodeId) -> String {
    doc.name(node)
        .prefix
        .as_ref()
        .map(|prefix| format!("{prefix}:"))
        .unwrap_or_default()
}

fn child(doc: &XmlDocument, parent: NodeId, local: &str) -> Option<NodeId> {
    doc.find_child(parent, local)
}

fn text(doc: &XmlDocument, node: Option<NodeId>) -> Option<String> {
    node.map(|node| doc.text(node).trim().to_owned())
}

fn page_file(doc: &XmlDocument, page_id: &str) -> Option<String> {
    for page in doc.children_named(doc.root(), "Page") {
        if doc.attribute(page, "PageID") == Some(page_id) {
            return doc
                .find_child(page, "FileLoc")
                .map(|node| doc.text(node).trim().to_owned());
        }
    }
    None
}

/// 把元素文本替换为 `value`，保留开始/结束标签。
fn replace_text(
    source: &str,
    doc: &XmlDocument,
    node: NodeId,
    value: &str,
) -> Result<String, OfdError> {
    let span = doc.span(node);
    let element = &source[span.start..span.end];
    if element.trim_end().ends_with("/>") {
        return Err(OfdError::InvalidStructure(
            "自闭合元素没有文本可替换".to_owned(),
        ));
    }
    let open_end = element
        .find('>')
        .ok_or_else(|| OfdError::InvalidStructure("元素开始标签不完整".to_owned()))?;
    let rest = &element[open_end + 1..];
    let close = rest
        .find('<')
        .ok_or_else(|| OfdError::InvalidStructure("元素结束标签不完整".to_owned()))?;
    let mut output = String::with_capacity(source.len() + value.len());
    output.push_str(&source[..span.start + open_end + 1]);
    output.push_str(value);
    output.push_str(&source[span.start + open_end + 1 + close..]);
    Ok(output)
}

/// 在元素结束标签前插入子元素；自闭合元素展开后插入。
fn insert_children(
    source: &str,
    doc: &XmlDocument,
    node: NodeId,
    fragment: &str,
) -> Result<String, OfdError> {
    if fragment.is_empty() {
        return Ok(source.to_owned());
    }
    let span = doc.span(node);
    let element = &source[span.start..span.end];
    let name = doc.name(node);
    let full = match &name.prefix {
        Some(prefix) => format!("{prefix}:{}", name.local),
        None => name.local.clone(),
    };
    let mut output = String::with_capacity(source.len() + fragment.len() + full.len() + 3);
    output.push_str(&source[..span.start]);
    if element.trim_end().ends_with("/>") {
        let slash = element
            .rfind('/')
            .ok_or_else(|| OfdError::InvalidStructure("自闭合标签不完整".to_owned()))?;
        output.push_str(&element[..slash]);
        output.push('>');
        output.push_str(fragment);
        output.push_str("</");
        output.push_str(&full);
        output.push('>');
    } else {
        let close = element
            .rfind("</")
            .ok_or_else(|| OfdError::InvalidStructure("元素缺少结束标签".to_owned()))?;
        output.push_str(&element[..close]);
        output.push_str(fragment);
        output.push_str(&element[close..]);
    }
    output.push_str(&source[span.end..]);
    Ok(output)
}

fn skeleton_res() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="{OFD_NS}" BaseLoc="Res"><ofd:MultiMedias></ofd:MultiMedias></ofd:Res>"#
    )
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
