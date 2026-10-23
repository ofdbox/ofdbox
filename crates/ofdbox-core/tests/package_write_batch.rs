mod fixtures;

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use ofdbox_core::{OfdError, OfdPackage};
use zip::ZipArchive;

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            collect(&path, files);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ofd"))
        {
            files.push(path);
        }
    }
}

/// 一条 entry 的「原始名 + 压缩数据」。
type EntryRecord = (Vec<u8>, Vec<u8>);

/// 按物理顺序收集「原始名 + 压缩数据」。名称用原始字节，避免 CP437/GBK 名称
/// 在解码成字符串后无法回查；用原始压缩数据比对，避免源文件 CRC 错误干扰。
fn entry_map(bytes: &[u8]) -> Result<Vec<EntryRecord>, OfdError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes.to_vec()))?;
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index_raw(index)?;
        if file.is_dir() {
            continue;
        }
        let name = file.name_raw().to_vec();
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        entries.push((name, data));
    }
    Ok(entries)
}

#[test]
fn corpus_repack_roundtrips() {
    let Some(dir) = fixtures::testdata_dir() else {
        return;
    };
    let mut files = Vec::new();
    collect(&dir, &mut files);
    assert!(!files.is_empty(), "no .ofd files under fixture dir");

    let mut repacked = 0usize;
    let mut unreadable = 0usize;
    let mut failed = Vec::new();
    for path in files {
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };
        let mut package = match OfdPackage::from_bytes(bytes.clone()) {
            Ok(package) => package,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };
        let output = match package.to_bytes() {
            Ok(output) => output,
            Err(error) => {
                failed.push(format!("{}: 重打包失败 {error}", path.display()));
                continue;
            }
        };

        let expected = match entry_map(&bytes) {
            Ok(map) => map,
            Err(error) => {
                failed.push(format!("{}: 读取原包失败 {error}", path.display()));
                continue;
            }
        };
        let actual = match entry_map(&output) {
            Ok(map) => map,
            Err(error) => {
                failed.push(format!("{}: 重打包结果无法读取 {error}", path.display()));
                continue;
            }
        };
        if expected != actual {
            failed.push(format!("{}: 重打包后条目内容不一致", path.display()));
            continue;
        }
        repacked += 1;
    }

    assert!(
        failed.is_empty(),
        "重打包回归失败 {} 份:\n{}",
        failed.len(),
        failed.join("\n")
    );
    assert!(repacked >= 150, "重打包成功数过少: {repacked}");
    assert!(unreadable <= 10, "不可读语料过多: {unreadable}");
}
