//! 包写出与重打包。
//!
//! 提供 [`PackageEdits`] 与 [`PackageWriteOptions`]，配合 [`crate::OfdPackage`]
//! 的写出方法使用：
//!
//! - [`crate::OfdPackage::write_to`]：原样重打包；
//! - [`crate::OfdPackage::write_with`]：替换、删除、新增 entry 后重打包；
//! - [`crate::OfdPackage::write_decrypted_to`]：解密另存为未加密包。
//!
//! 未修改的 entry 直接复制压缩数据与 ZIP 元数据（压缩方式、时间、权限、
//! extra field、文件注释），包注释也保留。

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, Write};
use std::sync::Arc;

use crate::error::OfdError;
use crate::package::{entry_key, normalize_entry_name};

/// 新增或替换 entry 使用的压缩方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntryCompression {
    /// 不压缩。
    Store,
    /// Deflate，默认。
    #[default]
    Deflate,
}

/// 写包时写入的修改时间。
///
/// 取值范围与 ZIP 的 DOS 时间一致：1980-01-01 至 2107-12-31。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageDateTime {
    /// 年。
    pub year: u16,
    /// 月。
    pub month: u8,
    /// 日。
    pub day: u8,
    /// 时。
    pub hour: u8,
    /// 分。
    pub minute: u8,
    /// 秒。
    pub second: u8,
}

impl PackageDateTime {
    /// 按字段创建时间。
    pub fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    pub(crate) fn to_zip(self) -> Result<zip::DateTime, OfdError> {
        zip::DateTime::from_date_and_time(
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
        )
        .map_err(|_| {
            OfdError::InvalidStructure(format!(
                "时间超出 ZIP 范围: {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                self.year, self.month, self.day, self.hour, self.minute, self.second
            ))
        })
    }
}

impl Default for PackageDateTime {
    fn default() -> Self {
        Self::new(1980, 1, 1, 0, 0, 0)
    }
}

/// 写包选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageWriteOptions {
    /// 未修改的 entry 直接复制压缩数据与 ZIP 元数据；默认 `true`。
    ///
    /// 置为 `false` 时解压后按 [`Self::compression`] 重新压缩，ZIP 元数据
    /// 中只有时间与权限保留。
    pub preserve_entries: bool,
    /// 新增或替换 entry 使用的压缩方式；默认 `Deflate`。
    pub compression: EntryCompression,
    /// 新增或替换 entry 的修改时间；`None` 时替换沿用原条目时间，新增使用 1980-01-01。
    pub last_modified: Option<PackageDateTime>,
    /// 是否保留包内目录条目；默认 `true`。
    pub keep_directories: bool,
}

impl Default for PackageWriteOptions {
    fn default() -> Self {
        Self {
            preserve_entries: true,
            compression: EntryCompression::Deflate,
            last_modified: None,
            keep_directories: true,
        }
    }
}

/// 对包内容的一次编辑集合。
///
/// 名称按包内路径处理：忽略前导 `/`、`\` 归一为 `/`、匹配时大小写不敏感。
/// 物理条目名保持原样，只有编辑目标按上述规则匹配。
#[derive(Debug, Clone, Default)]
pub struct PackageEdits {
    replacements: Vec<(String, Arc<[u8]>)>,
    additions: Vec<(String, Arc<[u8]>)>,
    removals: Vec<String>,
}

impl PackageEdits {
    /// 创建空编辑集合。
    pub fn new() -> Self {
        Self::default()
    }

    /// 是否没有任何编辑。
    pub fn is_empty(&self) -> bool {
        self.replacements.is_empty() && self.additions.is_empty() && self.removals.is_empty()
    }

    /// 清空全部编辑。
    pub fn clear(&mut self) {
        self.replacements.clear();
        self.additions.clear();
        self.removals.clear();
    }

    /// 用新字节替换已有 entry。
    ///
    /// 替换保持 entry 在包内的位置；未指定 [`PackageWriteOptions::last_modified`]
    /// 时沿用原条目的时间。包内存在同名重复条目时，全部同名条目都会被替换。
    pub fn replace(&mut self, name: impl AsRef<str>, bytes: impl Into<Vec<u8>>) -> &mut Self {
        self.replacements
            .push((name.as_ref().to_owned(), Arc::from(bytes.into())));
        self
    }

    /// 删除 entry；目标不存在时视为无操作。
    pub fn remove(&mut self, name: impl AsRef<str>) -> &mut Self {
        self.removals.push(name.as_ref().to_owned());
        self
    }

    /// 追加新 entry。
    ///
    /// 与包内已有 entry 同名时写出报错（除非同名条目已被 [`Self::remove`]）。
    /// 名称不能以 `/` 结尾，目录条目只能由原包保留，不能新增。
    pub fn add(&mut self, name: impl AsRef<str>, bytes: impl Into<Vec<u8>>) -> &mut Self {
        self.additions
            .push((name.as_ref().to_owned(), Arc::from(bytes.into())));
        self
    }

    pub(crate) fn normalized(&self) -> Result<NormalizedEdits, OfdError> {
        let mut replacements = HashMap::new();
        for (name, bytes) in &self.replacements {
            let key = file_key(name)?;
            if replacements.insert(key, Arc::clone(bytes)).is_some() {
                return Err(OfdError::InvalidStructure(format!(
                    "替换 entry 重复: {name}"
                )));
            }
        }

        let mut additions = Vec::new();
        let mut seen = HashSet::new();
        for (name, bytes) in &self.additions {
            let key = file_key(name)?;
            if !seen.insert(key.clone()) || replacements.contains_key(&key) {
                return Err(OfdError::InvalidStructure(format!(
                    "新增 entry 重复: {name}"
                )));
            }
            additions.push((normalize_entry_name(name)?, Arc::clone(bytes)));
        }

        let mut removals = HashSet::new();
        for name in &self.removals {
            removals.insert(file_key(name)?);
        }

        Ok(NormalizedEdits {
            replacements,
            additions,
            removals,
        })
    }
}

fn file_key(name: &str) -> Result<String, OfdError> {
    let normalized = normalize_entry_name(name)?;
    if normalized.ends_with('/') {
        return Err(OfdError::InvalidStructure(format!(
            "entry 名不能以 / 结尾: {name}"
        )));
    }
    Ok(entry_key(&normalized))
}

/// 归一化之后的编辑集合，供规划阶段使用。
pub(crate) struct NormalizedEdits {
    pub replacements: HashMap<String, Arc<[u8]>>,
    pub additions: Vec<(String, Arc<[u8]>)>,
    pub removals: HashSet<String>,
}

/// 原包中的一个条目。
pub(crate) struct ExistingEntry {
    pub index: usize,
    pub name: String,
    pub is_dir: bool,
    pub modified: Option<zip::DateTime>,
}

/// 写包规划中的一步。
pub(crate) enum PlanItem {
    CopyRaw {
        index: usize,
        name: String,
    },
    CopyDecoded {
        index: usize,
        name: String,
        time: zip::DateTime,
    },
    Replace {
        name: String,
        bytes: Arc<[u8]>,
        time: zip::DateTime,
    },
    Add {
        name: String,
        bytes: Arc<[u8]>,
        time: zip::DateTime,
    },
}

/// 先规划再写出，保证编辑目标不存在时不会留下半截 ZIP。
pub(crate) fn plan(
    existing: &[ExistingEntry],
    edits: &NormalizedEdits,
    options: &PackageWriteOptions,
) -> Result<Vec<PlanItem>, OfdError> {
    let existing_keys: HashSet<String> = existing
        .iter()
        .map(|entry| entry_key(&entry.name))
        .collect();

    for key in edits.replacements.keys() {
        if !existing_keys.contains(key) {
            return Err(OfdError::InvalidStructure(format!(
                "要替换的 entry 不存在: {key}"
            )));
        }
    }
    for (name, _) in &edits.additions {
        let key = entry_key(name);
        if existing_keys.contains(&key) && !edits.removals.contains(&key) {
            return Err(OfdError::InvalidStructure(format!(
                "entry 已存在，不能新增: {name}"
            )));
        }
    }

    let preferred = options
        .last_modified
        .map(PackageDateTime::to_zip)
        .transpose()?;

    let mut items = Vec::with_capacity(existing.len() + edits.additions.len());
    for entry in existing {
        let key = entry_key(&entry.name);
        if edits.removals.contains(&key) {
            continue;
        }
        if entry.is_dir {
            if options.keep_directories {
                items.push(PlanItem::CopyRaw {
                    index: entry.index,
                    name: entry.name.clone(),
                });
            }
            continue;
        }
        if let Some(bytes) = edits.replacements.get(&key) {
            items.push(PlanItem::Replace {
                name: entry.name.clone(),
                bytes: Arc::clone(bytes),
                time: preferred.or(entry.modified).unwrap_or_default(),
            });
            continue;
        }
        if options.preserve_entries {
            items.push(PlanItem::CopyRaw {
                index: entry.index,
                name: entry.name.clone(),
            });
        } else {
            items.push(PlanItem::CopyDecoded {
                index: entry.index,
                name: entry.name.clone(),
                time: preferred.or(entry.modified).unwrap_or_default(),
            });
        }
    }
    for (name, bytes) in &edits.additions {
        items.push(PlanItem::Add {
            name: name.clone(),
            bytes: Arc::clone(bytes),
            time: preferred.unwrap_or_default(),
        });
    }
    Ok(items)
}

/// 按选项写一个明文或替换后的 entry。
pub(crate) fn write_plain_entry<W: Write + Seek>(
    zip: &mut zip::ZipWriter<W>,
    name: &str,
    bytes: &[u8],
    options: &PackageWriteOptions,
    time: zip::DateTime,
) -> Result<(), OfdError> {
    let compression = match options.compression {
        EntryCompression::Store => zip::CompressionMethod::Stored,
        EntryCompression::Deflate => zip::CompressionMethod::Deflated,
    };
    let file_options = zip::write::SimpleFileOptions::default()
        .compression_method(compression)
        .last_modified_time(time)
        .unix_permissions(0o644);
    zip.start_file(name, file_options)?;
    zip.write_all(bytes)?;
    Ok(())
}

/// 原样复制一个 entry 的压缩数据与 ZIP 元数据。
///
/// 名称是合法 UTF-8 且与解码结果一致时走 `raw_copy_file_rename`，extra field
/// 也能保住；名称是 CP437/GBK 等非 UTF-8 字节时走 `raw_copy_file_touch`，
/// 保证原始名称字节不被改写（代价是该条目的 extra field 不保留）。
pub(crate) fn copy_raw<R: Read, W: Write + Seek>(
    zip: &mut zip::ZipWriter<W>,
    file: zip::read::ZipFile<'_, R>,
    name: &str,
) -> Result<(), OfdError> {
    let utf8_name = std::str::from_utf8(file.name_raw())
        .map(|raw| raw == name)
        .unwrap_or(false);
    if utf8_name {
        zip.raw_copy_file_rename(file, name)?;
    } else {
        let time = file.last_modified().unwrap_or_default();
        let mode = file.unix_mode();
        zip.raw_copy_file_touch(file, time, mode)?;
    }
    Ok(())
}

/// 解密另存的结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DecryptedWriteReport {
    /// 写出的 entry 数量。
    pub entries: usize,
    /// 被移除的加密描述文件与密文条目。
    pub removed: Vec<String>,
    /// 非致命警告，例如被移除的条目仍被签名或完整性保护引用。
    pub warnings: Vec<String>,
}
