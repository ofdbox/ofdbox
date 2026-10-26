//! `Attachments.xsd`：附件索引。

use super::definition::StLoc;

/// `Attachments.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attachments {
    /// 附件。
    pub attachments: Vec<Attachment>,
}

/// 附件。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attachment {
    /// 附件 ID。
    pub id: String,
    /// 名称。
    pub name: String,
    /// 格式。
    pub format: Option<String>,
    /// 创建时间。
    pub creation_date: Option<String>,
    /// 修改时间。
    pub mod_date: Option<String>,
    /// 大小。
    pub size: Option<f64>,
    /// 是否可见。
    pub visible: bool,
    /// 用途。
    pub usage: String,
    /// 文件路径。
    pub file_loc: StLoc,
}
