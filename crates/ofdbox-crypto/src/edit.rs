//! 加密 OFD 的“解密 -> 编辑 -> 重新加密”工作流。

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};

use ofdbox_core::{
    CryptoBackend, OfdError, OfdFile, OfdPackage, OpenOptions, OpenStorage, PackageEdits,
    PackageWriteOptions,
};

use crate::EncryptOptions;

/// 解密已有加密包，交给闭包编辑，再重新加密。
///
/// 注意：
///
/// - 这是整包重新加密，不是逐 entry 原地更新；
/// - 已有签名会失效；
/// - 未识别/未建模内容仍按 `ofdbox-core` 的模型规则处理。
pub fn edit_encrypted<F>(
    input: &[u8],
    backend: Box<dyn CryptoBackend>,
    options: &EncryptOptions,
    edit: F,
) -> Result<Vec<u8>, OfdError>
where
    F: FnOnce(&mut OfdFile) -> Result<(), OfdError>,
{
    let mut package = OfdPackage::from_bytes(input.to_vec())?;
    package.enable_decryption(backend)?;

    let mut plaintext = Cursor::new(Vec::new());
    package.write_decrypted_to(&mut plaintext)?;

    let mut file = OfdFile::open_bytes_with(
        plaintext.into_inner(),
        OpenOptions {
            storage: OpenStorage::Memory,
        },
    )?;
    edit(&mut file)?;
    let edited = file.to_bytes()?;

    crate::encrypt_package(&edited, options)
}

struct OriginalEntry {
    physical: String,
    plaintext: Vec<u8>,
}

/// 解密编辑后重新加密，但保留未修改 entry 的原始加密字节。
///
/// 要求加密包只有一层加密操作；新增 entry 暂不支持。
pub fn edit_encrypted_preserving<F>(
    input: &[u8],
    backend: Box<dyn CryptoBackend>,
    edit: F,
) -> Result<Vec<u8>, OfdError>
where
    F: FnOnce(&mut OfdFile) -> Result<(), OfdError>,
{
    let mut package = OfdPackage::from_bytes(input.to_vec())?;
    package.enable_decryption(backend)?;
    if package.decryption_operation_count() > 1 {
        return Err(OfdError::UnsupportedScheme(
            "多层加密暂不支持逐 entry 保留".to_owned(),
        ));
    }

    let mut plaintext = Cursor::new(Vec::new());
    package.write_decrypted_to(&mut plaintext)?;
    let plaintext = plaintext.into_inner();

    let mut originals: HashMap<String, OriginalEntry> = HashMap::new();
    {
        let mut archive = zip::ZipArchive::new(Cursor::new(&plaintext))?;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let name = entry.name().map(|name| name.into_owned())?;
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            let physical = package
                .encrypted_entry_info(&name)?
                .map(|(physical, _)| physical)
                .unwrap_or_else(|| name.to_string());
            originals.insert(
                name,
                OriginalEntry {
                    physical,
                    plaintext: bytes,
                },
            );
        }
    }

    let mut file = OfdFile::open_bytes_with(
        plaintext,
        OpenOptions {
            storage: OpenStorage::Memory,
        },
    )?;
    edit(&mut file)?;
    let edited = file.to_bytes()?;

    let mut edited_entries: HashMap<String, Vec<u8>> = HashMap::new();
    {
        let mut archive = zip::ZipArchive::new(Cursor::new(&edited))?;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let name = entry.name().map(|name| name.into_owned())?;
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            edited_entries.insert(name, bytes);
        }
    }

    let mut edits = PackageEdits::new();
    for (name, bytes) in &edited_entries {
        let Some(original) = originals.get(name) else {
            return Err(OfdError::InvalidStructure(format!(
                "加密包暂不支持新增 entry: {name}"
            )));
        };
        if *bytes == original.plaintext {
            continue;
        }
        if original.physical != *name {
            let encrypted = package.encrypt_entry(name, bytes)?;
            edits.replace(&original.physical, encrypted);
        } else {
            edits.replace(name, bytes.clone());
        }
    }

    let edited_names: HashSet<&String> = edited_entries.keys().collect();
    for (name, original) in &originals {
        if edited_names.contains(name) {
            continue;
        }
        if original.physical != *name {
            edits.remove(&original.physical);
        } else {
            edits.remove(name);
        }
    }

    let mut output = Cursor::new(Vec::new());
    package.write_with_encrypted(&mut output, &edits, &PackageWriteOptions::default())?;
    Ok(output.into_inner())
}
