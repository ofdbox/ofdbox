//! 包的存放位置与打开方式。

use std::path::PathBuf;

use crate::schema::definition::PageArea;

/// 创建新 OFD 时使用的存储后端。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Storage {
    /// 全部 entry 存放在内存中。
    #[default]
    Memory,
    /// entry 按目录结构写入磁盘。
    Directory(PathBuf),
}

/// 打开已有 OFD 时使用的存储后端。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OpenStorage {
    /// 直接读取源 ZIP，只读。
    #[default]
    ZipReadOnly,
    /// 解压到内存后编辑。
    Memory,
    /// 解压到目录后编辑。
    Directory(PathBuf),
}

/// 创建新 OFD 的选项。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreateOptions {
    /// 存储后端。
    pub storage: Storage,
    /// 文档 ID。
    pub doc_id: Option<String>,
    /// 默认页面区域。
    pub page_area: Option<PageArea>,
}

/// 打开已有 OFD 的选项。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenOptions {
    /// 打开方式。
    pub storage: OpenStorage,
    /// 预留的口令。
    pub password: Option<String>,
}
