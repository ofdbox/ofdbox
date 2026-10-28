use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use ofdbox_core::model::annotation::PageAnnot;
use ofdbox_core::model::annotations::Annotations;
use ofdbox_core::model::attachments::Attachments;
use ofdbox_core::model::custom_tags::CustomTags;
use ofdbox_core::model::document::Document;
use ofdbox_core::model::extensions::Extensions;
use ofdbox_core::model::ofd::Ofd;
use ofdbox_core::model::page::Page;
use ofdbox_core::model::res::Res;
use ofdbox_core::model::signature::Signature;
use ofdbox_core::model::signatures::Signatures;
use ofdbox_core::model::version::DocVersion;
use ofdbox_core::model_xml as xml;
use yaserde::{YaDeserialize, YaSerialize};

#[derive(Default)]
struct Report {
    files: usize,
    entries: usize,
    parsed: usize,
    failures: Vec<String>,
    invalid: usize,
    by_kind: BTreeMap<&'static str, usize>,
}

fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, files);
        } else if path.extension().and_then(|value| value.to_str()) == Some("ofd") {
            files.push(path);
        }
    }
}

fn check<T>(report: &mut Report, kind: &'static str, file: &Path, entry: &str, text: &str)
where
    T: YaDeserialize + YaSerialize + PartialEq,
{
    report.entries += 1;
    match xml::from_str::<T>(text) {
        Ok(value) => {
            report.parsed += 1;
            *report.by_kind.entry(kind).or_default() += 1;
            match xml::to_string(&value) {
                Ok(written) => {
                    if let Err(error) = xml::from_str::<T>(&written) {
                        report.failures.push(format!(
                            "{} :: {entry} :: {kind} :: 序列化后无法重解析: {error}",
                            file.display()
                        ));
                    }
                }
                Err(error) => report.failures.push(format!(
                    "{} :: {entry} :: {kind} :: 序列化失败: {error}",
                    file.display()
                )),
            }
        }
        Err(error) => {
            let message = error.to_string();
            if message.contains("prefix is unbound") || message.contains("not well-formed") {
                report.invalid += 1;
            } else {
                report.failures.push(format!(
                    "{} :: {entry} :: {kind} :: 解析失败: {error}",
                    file.display()
                ));
            }
        }
    }
}

fn classify(entry: &str) -> Option<&'static str> {
    let name = entry.rsplit('/').next().unwrap_or(entry);
    if name == "OFD.xml" {
        Some("ofd")
    } else if name == "Document.xml" {
        Some("document")
    } else if name == "Content.xml" {
        Some("page")
    } else if name == "Annotations.xml" {
        Some("annotations")
    } else if name == "Attachments.xml" {
        Some("attachments")
    } else if name == "CustomTags.xml" {
        Some("custom_tags")
    } else if name == "Extensions.xml" {
        Some("extensions")
    } else if name == "Signatures.xml" {
        Some("signatures")
    } else if name == "Signature.xml" {
        Some("signature")
    } else if name == "PageAnnot.xml" || name.starts_with("Annot_") {
        Some("page_annot")
    } else if name == "DocumentRes.xml"
        || name == "PublicRes.xml"
        || name == "PageRes.xml"
        || name == "Res.xml"
    {
        Some("res")
    } else if entry.contains("/Versions/") && name.ends_with(".xml") && name != "Document.xml" {
        Some("version")
    } else {
        None
    }
}

#[test]
#[ignore = "需要 OFDBOX_TESTDATA_DIR 指向外部 OFD 语料"]
fn parses_real_ofd_xml() {
    let dir = std::env::var("OFDBOX_TESTDATA_DIR")
        .unwrap_or_else(|_| r"D:\OFDBox-Workspace\OFD文件".to_owned());
    let dir = PathBuf::from(dir);
    let mut files = Vec::new();
    walk(&dir, &mut files);
    files.sort();

    let mut report = Report::default();
    for path in &files {
        report.files += 1;
        let Ok(file) = File::open(path) else {
            report
                .failures
                .push(format!("{} :: 无法打开文件", path.display()));
            continue;
        };
        let Ok(mut zip) = zip::ZipArchive::new(file) else {
            report.invalid += 1;
            continue;
        };
        for index in 0..zip.len() {
            let Ok(mut entry) = zip.by_index(index) else {
                continue;
            };
            let Ok(name) = entry.name().map(|name| name.to_string()) else {
                continue;
            };
            if !name.ends_with(".xml") {
                continue;
            }
            let Some(kind) = classify(&name) else {
                continue;
            };
            let mut bytes = Vec::new();
            if entry.read_to_end(&mut bytes).is_err() {
                report
                    .failures
                    .push(format!("{} :: {name} :: 读取失败", path.display()));
                continue;
            }
            let text = match std::str::from_utf8(&bytes) {
                Ok(text) => text,
                Err(_) => {
                    report
                        .failures
                        .push(format!("{} :: {name} :: 非 UTF-8", path.display()));
                    continue;
                }
            };
            match kind {
                "ofd" => check::<Ofd>(&mut report, kind, path, &name, text),
                "document" => check::<Document>(&mut report, kind, path, &name, text),
                "page" => check::<Page>(&mut report, kind, path, &name, text),
                "res" => check::<Res>(&mut report, kind, path, &name, text),
                "page_annot" => check::<PageAnnot>(&mut report, kind, path, &name, text),
                "annotations" => check::<Annotations>(&mut report, kind, path, &name, text),
                "attachments" => check::<Attachments>(&mut report, kind, path, &name, text),
                "custom_tags" => check::<CustomTags>(&mut report, kind, path, &name, text),
                "extensions" => check::<Extensions>(&mut report, kind, path, &name, text),
                "signature" => check::<Signature>(&mut report, kind, path, &name, text),
                "signatures" => check::<Signatures>(&mut report, kind, path, &name, text),
                "version" => check::<DocVersion>(&mut report, kind, path, &name, text),
                _ => {}
            }
        }
    }

    println!(
        "files={} entries={} parsed={} failures={} invalid={}",
        report.files,
        report.entries,
        report.parsed,
        report.failures.len(),
        report.invalid
    );
    for (kind, count) in &report.by_kind {
        println!("  {kind}: {count}");
    }
    let mut categories: BTreeMap<(String, String), usize> = BTreeMap::new();
    for failure in &report.failures {
        let parts: Vec<&str> = failure.split(" :: ").collect();
        let (kind, message) = if parts.len() >= 4 {
            (parts[2].to_owned(), parts[3].to_owned())
        } else {
            ("unknown".to_owned(), failure.clone())
        };
        *categories.entry((kind, message)).or_default() += 1;
    }
    let mut categories: Vec<_> = categories.into_iter().collect();
    categories.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    println!("failure categories={}", categories.len());
    for ((kind, message), count) in categories.iter().take(80) {
        println!("CATEGORY {count:4} {kind:12} {message}");
    }
    for failure in report.failures.iter().take(10) {
        println!("EXAMPLE {failure}");
    }
    assert!(
        report.failures.is_empty(),
        "{} failures",
        report.failures.len()
    );
}
