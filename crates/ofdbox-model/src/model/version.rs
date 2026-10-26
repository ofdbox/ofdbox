//! `Version.xsd`：版本描述文件。

use super::definition::StLoc;

/// `DocVersion` 根对象。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocVersion {
    /// 版本 ID。
    pub id: String,
    /// 版本号。
    pub version: Option<String>,
    /// 版本名称。
    pub name: Option<String>,
    /// 创建日期。
    pub creation_date: Option<String>,
    /// 文件列表。
    pub file_list: FileList,
    /// 文档根路径。
    pub doc_root: StLoc,
}

/// 版本文件列表。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileList {
    /// 文件。
    pub files: Vec<File>,
}

/// 版本文件。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct File {
    /// 文件 ID。
    pub id: String,
    /// 文件路径。
    pub file_loc: StLoc,
}
