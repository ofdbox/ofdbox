//! `Annotations.xsd`：按页组织的注释索引。

use super::definition::{StLoc, StRefId};

/// `Annotations.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Annotations {
    /// 页面注释索引。
    pub pages: Vec<AnnotationsPage>,
}

/// 页面注释索引。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnnotationsPage {
    /// 页面 ID。
    pub page_id: StRefId,
    /// 注释文件路径。
    pub file_loc: StLoc,
}
