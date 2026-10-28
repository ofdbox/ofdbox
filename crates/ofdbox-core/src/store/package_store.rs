use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::OfdError;
use crate::package::{OfdPackage, normalize_entry_name};
use crate::store::storage::Storage;

enum Backend {
    ZipReadOnly(Mutex<Box<OfdPackage>>),
    Memory(RefCell<BTreeMap<String, Vec<u8>>>),
    Directory(PathBuf),
}

/// OFD entry 的存储后端。
///
/// - ZIP 后端只读；
/// - 内存后端直接读写 `BTreeMap`；
/// - 目录后端直接读写磁盘文件。
pub struct PackageStore {
    backend: Backend,
}

impl PackageStore {
    /// 打开只读 ZIP。
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_zip(path: impl AsRef<Path>) -> Result<Self, OfdError> {
        Ok(Self {
            backend: Backend::ZipReadOnly(Mutex::new(Box::new(OfdPackage::open(path)?))),
        })
    }

    /// 打开只读 ZIP 字节。
    pub fn open_zip_bytes(bytes: Vec<u8>) -> Result<Self, OfdError> {
        Ok(Self {
            backend: Backend::ZipReadOnly(Mutex::new(Box::new(OfdPackage::from_bytes(bytes)?))),
        })
    }

    /// 创建空存储。
    pub fn create(storage: Storage) -> Result<Self, OfdError> {
        match storage {
            Storage::Memory => Ok(Self {
                backend: Backend::Memory(RefCell::new(BTreeMap::new())),
            }),
            Storage::Directory(path) => {
                std::fs::create_dir_all(&path)?;
                reset_directory(&path)?;
                Ok(Self {
                    backend: Backend::Directory(path),
                })
            }
        }
    }

    /// 从已有 ZIP 解压到指定后端。
    ///
    /// 当前使用 `OfdPackage::read_entry`，得到的是已解密 entry。
    /// 加密包编辑暂未支持重新加密。
    #[cfg(not(target_arch = "wasm32"))]
    pub fn extract_from_zip(path: impl AsRef<Path>, storage: Storage) -> Result<Self, OfdError> {
        let mut package = OfdPackage::open(path)?;
        Self::extract(&mut package, storage)
    }

    /// 从已有 ZIP 字节解压到指定后端。
    pub fn extract_from_bytes(bytes: Vec<u8>, storage: Storage) -> Result<Self, OfdError> {
        let mut package = OfdPackage::from_bytes(bytes)?;
        Self::extract(&mut package, storage)
    }

    fn extract(package: &mut OfdPackage, storage: Storage) -> Result<Self, OfdError> {
        if package.contains("Encryptions.xml") {
            return Err(OfdError::EncryptionRequired);
        }
        let store = Self::create(storage)?;
        let names = package.entry_names()?;
        for name in names {
            if name.ends_with('/') {
                continue;
            }
            let bytes = package.read_entry(&name)?.to_vec();
            store.write_entry(&name, bytes)?;
        }
        Ok(store)
    }

    /// 读取 entry。
    pub fn read_entry(&self, entry: &str) -> Result<Vec<u8>, OfdError> {
        let entry = normalize_entry_name(entry)?;
        match &self.backend {
            Backend::ZipReadOnly(package) => {
                let mut package = package
                    .lock()
                    .map_err(|_| OfdError::InvalidStructure("ZIP 锁已损坏".to_owned()))?;
                Ok(package.read_entry(&entry)?.to_vec())
            }
            Backend::Memory(files) => files
                .borrow()
                .get(&entry)
                .cloned()
                .ok_or_else(|| OfdError::InvalidStructure(format!("entry 不存在: {entry}"))),
            Backend::Directory(root) => Ok(std::fs::read(directory_path(root, &entry)?)?),
        }
    }

    /// 写入 entry。
    pub fn write_entry(&self, entry: &str, bytes: Vec<u8>) -> Result<(), OfdError> {
        let entry = normalize_entry_name(entry)?;
        match &self.backend {
            Backend::ZipReadOnly(_) => Err(OfdError::ReadOnly),
            Backend::Memory(files) => {
                files.borrow_mut().insert(entry, bytes);
                Ok(())
            }
            Backend::Directory(root) => {
                let path = directory_path(root, &entry)?;
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(path, bytes)?;
                Ok(())
            }
        }
    }

    /// 删除 entry。
    pub fn delete_entry(&self, entry: &str) -> Result<(), OfdError> {
        let entry = normalize_entry_name(entry)?;
        match &self.backend {
            Backend::ZipReadOnly(_) => Err(OfdError::ReadOnly),
            Backend::Memory(files) => {
                files.borrow_mut().remove(&entry);
                Ok(())
            }
            Backend::Directory(root) => {
                let path = directory_path(root, &entry)?;
                if path.exists() {
                    std::fs::remove_file(path)?;
                }
                Ok(())
            }
        }
    }

    /// entry 是否存在。
    pub fn exists(&self, entry: &str) -> bool {
        let Ok(entry) = normalize_entry_name(entry) else {
            return false;
        };
        match &self.backend {
            Backend::ZipReadOnly(package) => package
                .lock()
                .map(|package| package.contains(&entry))
                .unwrap_or(false),
            Backend::Memory(files) => files.borrow().contains_key(&entry),
            Backend::Directory(root) => directory_path(root, &entry)
                .map(|path| path.is_file())
                .unwrap_or(false),
        }
    }

    /// 列出所有 entry。
    pub fn entry_names(&self) -> Result<Vec<String>, OfdError> {
        match &self.backend {
            Backend::ZipReadOnly(package) => {
                let package = package
                    .lock()
                    .map_err(|_| OfdError::InvalidStructure("ZIP 锁已损坏".to_owned()))?;
                package.entry_names()
            }
            Backend::Memory(files) => Ok(files.borrow().keys().cloned().collect()),
            Backend::Directory(root) => {
                let mut entries = Vec::new();
                collect_directory(root, root, &mut entries)?;
                entries.sort();
                Ok(entries)
            }
        }
    }

    /// 把当前后端打包成 OFD 字节。
    pub fn to_bytes(&self) -> Result<Vec<u8>, OfdError> {
        if matches!(self.backend, Backend::ZipReadOnly(_)) {
            return Err(OfdError::ReadOnly);
        }

        let names = self.entry_names()?;
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for name in names {
            if name.ends_with('/') {
                continue;
            }
            let bytes = self.read_entry(&name)?;
            zip.start_file(name, options)?;
            zip.write_all(&bytes)?;
        }

        let cursor = zip.finish()?;
        Ok(cursor.into_inner())
    }

    /// 打包并写回文件。
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), OfdError> {
        let bytes = self.to_bytes()?;
        write_atomic(path.as_ref(), &bytes)
    }
}

impl std::fmt::Debug for PackageStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PackageStore")
            .finish_non_exhaustive()
    }
}

fn reset_directory(root: &Path) -> Result<(), OfdError> {
    for item in std::fs::read_dir(root)? {
        let item = item?;
        let path = item.path();
        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn directory_path(root: &Path, entry: &str) -> Result<PathBuf, OfdError> {
    let mut path = root.to_path_buf();
    for component in entry.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(OfdError::InvalidStructure(format!(
                "非法 entry 路径: {entry}"
            )));
        }
        path.push(component);
    }
    Ok(path)
}

fn collect_directory(
    root: &Path,
    current: &Path,
    entries: &mut Vec<String>,
) -> Result<(), OfdError> {
    for item in std::fs::read_dir(current)? {
        let item = item?;
        let path = item.path();
        if path.is_dir() {
            collect_directory(root, &path, entries)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| OfdError::InvalidStructure(error.to_string()))?;
            entries.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), OfdError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output.ofd");
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temporary = parent.join(format!(".{file_name}.{}.{unique}.tmp", std::process::id()));

    let result = (|| {
        std::fs::write(&temporary, bytes)?;
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        std::fs::rename(&temporary, path)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
