//! 计算 OFD 包内被文档引用的文件集合。
//!
//! 所有引用都从 `model` 字段读取，不再遍历原始 XML。

use std::collections::{HashSet, VecDeque};

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::OfdError;
use crate::model;
use crate::model::definition::StLoc;
use crate::model_xml;
use crate::package::{OfdPackage, entry_key, parent_dir, resolve_loc};

/// 引用图扩展选项。
///
/// 模型化后不再支持按任意 XML 元素名扩展。
#[derive(Clone, Debug, Default)]
pub struct ReferenceGraphOptions {
    /// 保留字段，当前忽略。
    pub extra_text_refs: Vec<String>,
    /// 保留字段，当前忽略。
    pub extra_attribute_refs: Vec<String>,
}

pub(crate) fn referenced_entries_with(
    package: &mut OfdPackage,
    _options: &ReferenceGraphOptions,
) -> Result<HashSet<String>, OfdError> {
    let mut referenced = HashSet::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    for root in ["OFD.xml", "Encryptions.xml"] {
        if package.contains(root) {
            queue.push_back(root.to_owned());
        }
    }

    while let Some(entry) = queue.pop_front() {
        if !visited.insert(entry_key(&entry)) {
            continue;
        }
        let Ok(bytes) = package.read_entry(&entry) else {
            continue;
        };
        referenced.insert(entry_key(&entry));
        if !entry.to_ascii_lowercase().ends_with(".xml") {
            continue;
        }
        let dir = parent_dir(&entry).unwrap_or("").to_owned();
        let refs = collect_model_refs(&bytes, &dir)?;
        for path in refs {
            if path.contains("..") || !package.contains(&path) {
                continue;
            }
            referenced.insert(entry_key(&path));
            if path.to_ascii_lowercase().ends_with(".xml") {
                queue.push_back(path);
            }
        }
    }

    Ok(referenced)
}

fn collect_model_refs(bytes: &[u8], dir: &str) -> Result<Vec<String>, OfdError> {
    let mut refs = Vec::new();
    let Some(root) = root_name(bytes) else {
        return Ok(refs);
    };
    if root == "OFD"
        && let Ok(ofd) = parse::<model::ofd::Ofd>(bytes)
    {
        for body in &ofd.doc_bodies {
            push_loc(&mut refs, dir, &body.doc_root);
            if let Some(signatures) = &body.signatures {
                push_loc(&mut refs, dir, signatures);
            }
        }
        return Ok(refs);
    }
    if root == "Document"
        && let Ok(document) = parse::<model::document::Document>(bytes)
    {
        for loc in &document.common_data.public_res {
            push_loc(&mut refs, dir, loc);
        }
        for loc in &document.common_data.document_res {
            push_loc(&mut refs, dir, loc);
        }
        for template in &document.common_data.template_pages {
            push_loc(&mut refs, dir, &template.base_loc);
        }
        for page in &document.pages.pages {
            push_loc(&mut refs, dir, &page.base_loc);
        }
        for loc in [
            document.annotations.as_ref(),
            document.custom_tags.as_ref(),
            document.attachments.as_ref(),
            document.extensions.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            push_loc(&mut refs, dir, loc);
        }
        return Ok(refs);
    }
    if root == "Annotations"
        && let Ok(annotations) = parse::<model::annotations::Annotations>(bytes)
    {
        for page in &annotations.pages {
            push_loc(&mut refs, dir, &page.file_loc);
        }
        return Ok(refs);
    }
    if root == "Attachments"
        && let Ok(attachments) = parse::<model::attachments::Attachments>(bytes)
    {
        for item in &attachments.attachments {
            push_loc(&mut refs, dir, &item.file_loc);
        }
        return Ok(refs);
    }
    if root == "CustomTags"
        && let Ok(tags) = parse::<model::custom_tags::CustomTags>(bytes)
    {
        for tag in &tags.custom_tags {
            if let Some(loc) = &tag.schema_loc {
                push_loc(&mut refs, dir, loc);
            }
            push_loc(&mut refs, dir, &tag.file_loc);
        }
        return Ok(refs);
    }
    if root == "Extensions"
        && let Ok(extensions) = parse::<model::extensions::Extensions>(bytes)
    {
        for extension in &extensions.extensions {
            for item in &extension.items {
                if let model::extensions::ExtensionItem::ExtendData(loc) = item {
                    push_loc(&mut refs, dir, loc);
                }
            }
        }
        return Ok(refs);
    }
    if root == "Page"
        && let Ok(page) = parse::<model::page::Page>(bytes)
    {
        for loc in &page.page_res {
            push_loc(&mut refs, dir, loc);
        }
        return Ok(refs);
    }
    if root == "Signatures"
        && let Ok(index) = parse::<model::signatures::Signatures>(bytes)
    {
        for signature in &index.signatures {
            push_loc(&mut refs, dir, &signature.base_loc);
        }
        return Ok(refs);
    }
    if root == "Signature"
        && let Ok(signature) = parse::<model::signature::Signature>(bytes)
    {
        push_loc(&mut refs, dir, &signature.signed_value);
        for reference in &signature.signed_info.references.references {
            push_loc(&mut refs, dir, &reference.file_ref);
        }
        if let Some(seal) = &signature.signed_info.seal {
            push_loc(&mut refs, dir, &seal.base_loc);
        }
        return Ok(refs);
    }
    if root == "DocEntries"
        && let Ok(entries) = parse::<model::doc_entries::DocEntries>(bytes)
    {
        for file in &entries.file_list.files {
            push_loc(&mut refs, dir, &file.file_loc);
        }
        push_loc(&mut refs, dir, &entries.signed_value_loc);
        return Ok(refs);
    }
    if root == "Encryptions"
        && let Ok(encryptions) = parse::<model::encryption::Encryptions>(bytes)
    {
        for info in &encryptions.encrypt_infos {
            if let Some(loc) = &info.decrypt_seed_loc {
                push_loc(&mut refs, dir, loc);
            }
            if let Some(loc) = &info.entries_map_loc {
                push_loc(&mut refs, dir, loc);
            }
        }
        return Ok(refs);
    }
    if root == "DocVersion"
        && let Ok(version) = parse::<model::version::DocVersion>(bytes)
    {
        for file in &version.file_list.files {
            push_loc(&mut refs, dir, &file.file_loc);
        }
        push_loc(&mut refs, dir, &version.doc_root);
        return Ok(refs);
    }
    if root == "Res"
        && let Ok(res) = parse::<model::res::Res>(bytes)
    {
        let res_dir = resolve_loc(dir, res.base_loc.as_str()).unwrap_or_else(|_| dir.to_owned());
        if let Some(fonts) = &res.fonts {
            for font in &fonts.fonts {
                if let Some(loc) = &font.font_file {
                    push_loc(&mut refs, &res_dir, loc);
                }
            }
        }
        if let Some(media) = &res.multi_medias {
            for item in &media.multi_medias {
                push_loc(&mut refs, &res_dir, &item.media_file);
            }
        }
        return Ok(refs);
    }
    Ok(refs)
}

fn root_name(bytes: &[u8]) -> Option<String> {
    let mut reader = Reader::from_reader(bytes);
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                let name = start.local_name().as_ref().to_owned();
                return Some(name);
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn parse<T: yaserde::YaDeserialize>(bytes: &[u8]) -> Result<T, OfdError> {
    model_xml::from_bytes(bytes).map_err(|error| OfdError::InvalidStructure(error.to_string()))
}

fn push_loc(refs: &mut Vec<String>, base: &str, loc: &StLoc) {
    let value = loc.as_str().trim();
    if value.is_empty() || value.starts_with('#') {
        return;
    }
    if let Ok(path) = resolve_loc(base, value) {
        refs.push(path);
    }
}
