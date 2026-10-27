use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use ofdbox_model::model::annotation::PageAnnot;
use ofdbox_model::model::annotations::Annotations;
use ofdbox_model::model::attachments::Attachments;
use ofdbox_model::model::custom_tags::CustomTags;
use ofdbox_model::model::document::Document;
use ofdbox_model::model::extensions::Extensions;
use ofdbox_model::model::ofd::Ofd;
use ofdbox_model::model::page::Page;
use ofdbox_model::model::res::Res;
use ofdbox_model::model::signature::Signature;
use ofdbox_model::model::signatures::Signatures;
use ofdbox_model::model::version::DocVersion;
use ofdbox_model::xml;
use yaserde::{YaDeserialize, YaSerialize};

#[derive(Default)]
struct Report {
    files: usize,
    entries: usize,
    parsed: usize,
    failures: Vec<String>,
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
        Err(error) => report.failures.push(format!(
            "{} :: {entry} :: {kind} :: 解析失败: {error}",
            file.display()
        )),
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
    } else if name == "DocumentRes.xml" || name == "PublicRes.xml" || name == "PageRes.xml" || name == "Res.xml" {
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
            report.failures.push(format!("{} :: 无法打开文件", path.display()));
            continue;
        };
        let Ok(mut zip) = zip::ZipArchive::new(file) else {
            report.failures.push(format!("{} :: 不是有效 ZIP", path.display()));
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
                report.failures.push(format!("{} :: {name} :: 读取失败", path.display()));
                continue;
            }
            let text = match std::str::from_utf8(&bytes) {
                Ok(text) => text,
                Err(_) => {
                    report.failures.push(format!("{} :: {name} :: 非 UTF-8", path.display()));
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
        "files={} entries={} parsed={} failures={}",
        report.files,
        report.entries,
        report.parsed,
        report.failures.len()
    );
    for (kind, count) in &report.by_kind {
        println!("  {kind}: {count}");
    }
    for failure in report.failures.iter().take(40) {
        println!("FAIL {failure}");
    }
    assert!(report.failures.is_empty(), "{} failures", report.failures.len());
}
