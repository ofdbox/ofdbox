//! `Extensions.xsd`：扩展索引。

use super::definition::{StLoc, StRefId};

/// `Extensions.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extensions {
    /// 扩展。
    pub extensions: Vec<Extension>,
}

/// 扩展。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extension {
    /// 应用名。
    pub app_name: String,
    /// 公司。
    pub company: Option<String>,
    /// 应用版本。
    pub app_version: Option<String>,
    /// 日期。
    pub date: Option<String>,
    /// 被扩展对象引用。
    pub ref_id: StRefId,
    /// 扩展内容。
    pub items: Vec<ExtensionItem>,
}

/// 扩展内容。
#[derive(Clone, Debug, PartialEq)]
pub enum ExtensionItem {
    /// 属性。
    Property(Property),
    /// 任意数据。
    Data(String),
    /// 外部数据路径。
    ExtendData(StLoc),
}

/// 扩展属性。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Property {
    /// 属性名。
    pub name: String,
    /// 属性类型。
    pub property_type: Option<String>,
    /// 属性值。
    pub value: String,
}
