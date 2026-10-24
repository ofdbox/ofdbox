//! 计算 OFD 包内被文档引用的文件集合。
//!
//! 从 `OFD.xml` 与 `Encryptions.xml` 出发，递归解析 XML 里已知的路径引用，
//! 用于 `OFDEntries.xml` 的防夹带校验：未被引用、也未列入 `FileList` 的文件
//! 就是夹带文件。

use std::collections::{HashSet, VecDeque};

use crate::error::OfdError;
use crate::options::ReadOptions;
use crate::package::{OfdPackage, entry_key, parent_dir, resolve_loc};
use crate::xml::parser::parse_xml;

/// 文本形式给出路径的元素。
const TEXT_REFS: &[&str] = &[
    "DocRoot",
    "Signatures",
    "PublicRes",
    "DocumentRes",
    "Annotations",
    "Attachments",
    "CustomTags",
    "SignedValue",
    "FileLoc",
    "MediaFile",
    "FontFile",
    "DecryptSeedLoc",
    "EntriesMapLoc",
    "DocVersion",
];

/// 以属性形式给出路径的元素属性。
const ATTRIBUTE_REFS: &[&str] = &["BaseLoc", "FileLoc", "ImageLoc", "Annotations"];

/// 名称以 `Loc` 结尾（不区分大小写）的字段按路径处理，兼容厂商自定义写法。
fn is_loc_name(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with("loc")
}

pub(crate) fn referenced_entries(package: &mut OfdPackage) -> Result<HashSet<String>, OfdError> {
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
        let doc = parse_xml(&bytes, ReadOptions::LENIENT)?;
        let dir = parent_dir(&entry).unwrap_or("").to_owned();
        let root = doc.root();
        let res_file = doc.name(root).local == "Res";
        let res_base = if res_file {
            let base = doc.attribute(root, "BaseLoc").unwrap_or("");
            resolve_loc(&dir, base).unwrap_or_else(|_| dir.clone())
        } else {
            dir.clone()
        };

        let mut refs: Vec<(String, String)> = Vec::new();
        for node in doc.nodes() {
            let name = node.name.local.as_str();
            if TEXT_REFS.contains(&name) || is_loc_name(name) {
                let text = node.text.trim();
                if !text.is_empty() {
                    // Res 文件里的路径都相对于 BaseLoc；其余相对于当前文件目录。
                    let base = if res_file || matches!(name, "MediaFile" | "FontFile") {
                        &res_base
                    } else {
                        &dir
                    };
                    refs.push((base.clone(), text.to_owned()));
                }
            }
            if name == "Res" {
                continue;
            }
            for attribute in &node.attributes {
                let attribute_name = attribute.name.local.as_str();
                if ATTRIBUTE_REFS.contains(&attribute_name) || is_loc_name(attribute_name) {
                    let value = attribute.value.trim();
                    if !value.is_empty() {
                        refs.push((dir.clone(), value.to_owned()));
                    }
                }
            }
        }

        for (base, loc) in refs {
            if loc.starts_with('#') {
                continue;
            }
            let Ok(path) = resolve_loc(&base, &loc) else {
                continue;
            };
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
