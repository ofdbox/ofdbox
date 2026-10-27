use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::Mutex;

use crate::error::OfdError;
use crate::package::{OfdPackage, normalize_entry_name};
use crate::package_write::{PackageEdits, PackageWriteOptions};

struct StoreState {
    package: OfdPackage,
    files: HashMap<String, Vec<u8>>,
    removed: HashSet<String>,
}

/// 共享的 OFD 容器与暂存修改。
///
/// `OfdPackage` 继续负责 ZIP、缓存、解密和底层读取；
/// `PackageStore` 负责共享访问、暂存写入、暂存删除和最终写回。
pub struct PackageStore {
    state: Mutex<StoreState>,
}

impl PackageStore {
    /// 用已有包创建存储。
    pub fn new(package: OfdPackage) -> Self {
        Self {
            state: Mutex::new(StoreState {
                package,
                files: HashMap::new(),
                removed: HashSet::new(),
            }),
        }
    }

    /// 从文件打开存储。
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, OfdError> {
        Ok(Self::new(OfdPackage::open(path)?))
    }

    /// 从内存字节打开存储。
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, OfdError> {
        Ok(Self::new(OfdPackage::from_bytes(bytes)?))
    }

    /// 读取 entry。
    pub fn read_entry(&self, entry: &str) -> Result<Vec<u8>, OfdError> {
        let entry = normalize_entry_name(entry)?;
        let mut state = self.lock()?;
        if state.removed.contains(&entry) {
            return Err(OfdError::InvalidStructure(format!("entry 已删除: {entry}")));
        }
        if let Some(bytes) = state.files.get(&entry) {
            return Ok(bytes.clone());
        }
        Ok(state.package.read_entry(&entry)?.to_vec())
    }

    /// 暂存 entry 内容。
    pub fn write_entry(&self, entry: &str, bytes: Vec<u8>) -> Result<(), OfdError> {
        let entry = normalize_entry_name(entry)?;
        let mut state = self.lock()?;
        state.removed.remove(&entry);
        state.files.insert(entry, bytes);
        Ok(())
    }

    /// 暂存删除 entry。
    pub fn delete_entry(&self, entry: &str) -> Result<(), OfdError> {
        let entry = normalize_entry_name(entry)?;
        let mut state = self.lock()?;
        state.files.remove(&entry);
        if state.package.contains(&entry) {
            state.removed.insert(entry);
        }
        Ok(())
    }

    /// entry 是否存在。
    pub fn exists(&self, entry: &str) -> bool {
        let Ok(entry) = normalize_entry_name(entry) else {
            return false;
        };
        let Ok(state) = self.lock() else {
            return false;
        };
        if state.removed.contains(&entry) {
            return false;
        }
        state.files.contains_key(&entry) || state.package.contains(&entry)
    }

    /// 生成写回后的完整 OFD 字节。
    pub fn to_bytes(&self) -> Result<Vec<u8>, OfdError> {
        let mut state = self.lock()?;
        let mut edits = PackageEdits::new();

        for (entry, bytes) in &state.files {
            if state.package.contains(entry) {
                edits.replace(entry, bytes.clone());
            } else {
                edits.add(entry, bytes.clone());
            }
        }
        for entry in &state.removed {
            if state.package.contains(entry) {
                edits.remove(entry);
            }
        }

        let mut cursor = Cursor::new(Vec::new());
        state
            .package
            .write_with(&mut cursor, &edits, &PackageWriteOptions::default())?;
        Ok(cursor.into_inner())
    }

    /// 写回文件。
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), OfdError> {
        let bytes = self.to_bytes()?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, StoreState>, OfdError> {
        self.state
            .lock()
            .map_err(|_| OfdError::InvalidStructure("package store 锁已损坏".to_owned()))
    }
}

impl std::fmt::Debug for PackageStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PackageStore")
            .finish_non_exhaustive()
    }
}
