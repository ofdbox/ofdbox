use std::io::{Cursor, Read, Write};

use ofdbox_core::{EntryCompression, OfdError, OfdPackage, PackageEdits, PackageWriteOptions};
use zip::write::{ExtendedFileOptions, FileOptions};
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>demo</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>1</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;

fn date(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> DateTime {
    DateTime::from_date_and_time(year, month, day, hour, minute, second).expect("valid date")
}

fn build(entries: &[(&str, &[u8], DateTime, u32)], comment: &[u8]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    if !comment.is_empty() {
        writer
            .set_raw_comment(Box::from(comment))
            .expect("archive comment");
    }
    for (name, data, time, mode) in entries {
        let options = FileOptions::<ExtendedFileOptions>::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(*time)
            .unix_permissions(*mode)
            .with_file_comment("entry-comment");
        writer.start_file(*name, options).expect("start entry");
        writer.write_all(data).expect("write entry");
    }
    writer.finish().expect("finish zip").into_inner()
}

fn sample() -> Vec<u8> {
    build(
        &[
            (
                "OFD.xml",
                ROOT.as_bytes(),
                date(2020, 5, 6, 7, 8, 10),
                0o600,
            ),
            (
                "Doc_0/Document.xml",
                DOCUMENT.as_bytes(),
                date(2021, 1, 2, 3, 4, 6),
                0o644,
            ),
        ],
        b"package-comment",
    )
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    name: String,
    method: CompressionMethod,
    time: Option<DateTime>,
    mode: Option<u32>,
    comment: String,
    data: Vec<u8>,
}

fn snapshot(bytes: &[u8]) -> (Vec<u8>, Vec<Snapshot>) {
    let mut archive = ZipArchive::new(Cursor::new(bytes.to_vec())).expect("zip archive");
    let comment = archive.comment().to_vec();
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).expect("entry");
        let mut data = Vec::new();
        file.read_to_end(&mut data).expect("read entry");
        entries.push(Snapshot {
            name: file.name().expect("name").into_owned(),
            method: file.compression(),
            time: file.last_modified(),
            mode: file.unix_mode(),
            comment: file.comment().to_owned(),
            data,
        });
    }
    (comment, entries)
}

fn repack_with(source: Vec<u8>, edits: &PackageEdits, options: &PackageWriteOptions) -> Vec<u8> {
    let mut package = OfdPackage::from_bytes(source).expect("open package");
    let mut output = Vec::new();
    package
        .write_with(&mut Cursor::new(&mut output), edits, options)
        .expect("write package");
    output
}

#[test]
fn repack_preserves_entries_and_metadata() {
    let source = sample();
    let (comment_before, before) = snapshot(&source);

    let mut package = OfdPackage::from_bytes(source).expect("open package");
    let output = package.to_bytes().expect("repack");
    let (comment_after, after) = snapshot(&output);

    assert_eq!(comment_after, comment_before);
    assert_eq!(after.len(), before.len());
    for (actual, expected) in after.iter().zip(&before) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.method, expected.method);
        assert_eq!(actual.time, expected.time);
        assert_eq!(actual.mode, expected.mode);
        assert_eq!(actual.comment, expected.comment);
        assert_eq!(actual.data, expected.data);
    }
}

#[test]
fn repack_is_deterministic() {
    let source = sample();
    let mut first = OfdPackage::from_bytes(source.clone()).expect("open");
    let mut second = OfdPackage::from_bytes(source).expect("open");
    assert_eq!(
        first.to_bytes().expect("repack"),
        second.to_bytes().expect("repack")
    );
}

#[test]
fn replace_keeps_position_and_time() {
    let source = sample();
    let (_, before) = snapshot(&source);

    let mut edits = PackageEdits::new();
    edits.replace(
        "Doc_0/Document.xml",
        b"<ofd:Document>replaced</ofd:Document>",
    );
    let output = repack_with(source, &edits, &PackageWriteOptions::default());
    let (_, after) = snapshot(&output);

    assert_eq!(after.len(), before.len());
    assert_eq!(after[0], before[0]);
    assert_eq!(after[1].name, before[1].name);
    assert_eq!(after[1].data, b"<ofd:Document>replaced</ofd:Document>");
    assert_eq!(after[1].time, before[1].time);
    assert_eq!(after[1].method, CompressionMethod::Deflated);
}

#[test]
fn remove_drops_entry_and_missing_remove_is_noop() {
    let mut edits = PackageEdits::new();
    edits.remove("Doc_0/Document.xml");
    let output = repack_with(sample(), &edits, &PackageWriteOptions::default());
    let (_, after) = snapshot(&output);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].name, "OFD.xml");

    let mut missing = PackageEdits::new();
    missing.remove("Doc_0/NotThere.xml");
    let output = repack_with(sample(), &missing, &PackageWriteOptions::default());
    assert_eq!(snapshot(&output).1.len(), 2);
}

#[test]
fn add_appends_and_rejects_duplicate() {
    let mut edits = PackageEdits::new();
    edits.add("Doc_0/extra.xml", b"<ofd:Extra/>");
    let output = repack_with(sample(), &edits, &PackageWriteOptions::default());
    let (_, after) = snapshot(&output);
    assert_eq!(after.len(), 3);
    assert_eq!(after[2].name, "Doc_0/extra.xml");
    assert_eq!(after[2].data, b"<ofd:Extra/>");

    let mut duplicate = PackageEdits::new();
    duplicate.add("OFD.xml", b"<ofd:OFD/>");
    let mut package = OfdPackage::from_bytes(sample()).expect("open");
    let mut output = Vec::new();
    let error = package
        .write_with(
            &mut Cursor::new(&mut output),
            &duplicate,
            &PackageWriteOptions::default(),
        )
        .expect_err("duplicate add must fail");
    assert!(matches!(error, OfdError::InvalidStructure(_)));
}

#[test]
fn replace_missing_entry_errors() {
    let mut edits = PackageEdits::new();
    edits.replace("Doc_0/NotThere.xml", b"x");
    let mut package = OfdPackage::from_bytes(sample()).expect("open");
    let mut output = Vec::new();
    let error = package
        .write_with(
            &mut Cursor::new(&mut output),
            &edits,
            &PackageWriteOptions::default(),
        )
        .expect_err("missing replace target must fail");
    assert!(matches!(error, OfdError::InvalidStructure(_)));
    assert!(output.is_empty(), "规划失败时不应写出任何字节");
}

#[test]
fn case_and_separator_insensitive_matching() {
    let mut edits = PackageEdits::new();
    edits.replace("doc_0\\document.xml", b"<ofd:Document>ci</ofd:Document>");
    let output = repack_with(sample(), &edits, &PackageWriteOptions::default());
    let (_, after) = snapshot(&output);
    assert_eq!(after[1].data, b"<ofd:Document>ci</ofd:Document>");
    assert_eq!(after[1].name, "Doc_0/Document.xml");
}

#[test]
fn recompress_mode_rewrites_compression() {
    let options = PackageWriteOptions {
        preserve_entries: false,
        compression: EntryCompression::Store,
        ..PackageWriteOptions::default()
    };
    let output = repack_with(sample(), &PackageEdits::new(), &options);
    let (_, after) = snapshot(&output);
    assert!(
        after
            .iter()
            .all(|entry| entry.method == CompressionMethod::Stored)
    );
    assert_eq!(after[0].data, ROOT.as_bytes());
}

#[test]
fn archive_comment_is_preserved() {
    let output = repack_with(
        sample(),
        &PackageEdits::new(),
        &PackageWriteOptions::default(),
    );
    let (comment, _) = snapshot(&output);
    assert_eq!(comment, b"package-comment");
}
