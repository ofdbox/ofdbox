//! `CustomTags.xsd`：自定义标引索引。

use super::definition::StLoc;

/// `CustomTags.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomTags {
    /// 标引。
    pub custom_tags: Vec<CustomTag>,
}

/// 自定义标引。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomTag {
    /// 命名空间。
    pub namespace: String,
    /// 约束文件路径。
    pub schema_loc: Option<StLoc>,
    /// 标引内容路径。
    pub file_loc: StLoc,
}
