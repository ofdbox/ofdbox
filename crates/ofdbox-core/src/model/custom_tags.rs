//! `CustomTags.xsd`：自定义标引索引。

use super::definition::StLoc;

/// `CustomTags.xml` 根对象。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(
  namespace = "http://www.ofdspec.org/2016",
  xmlns = {
    "ofd" = "http://www.ofdspec.org/2016"
  },
  namespace_aliases = {
    "http://www.ofdspec.org" = "http://www.ofdspec.org/2016"
  }
)]
#[yaserde(rename = "CustomTags")]
pub struct CustomTags {
    /// 标引。
    #[yaserde(rename = "CustomTag")]
    pub custom_tags: Vec<CustomTag>,
}

/// 自定义标引。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "CustomTag")]
pub struct CustomTag {
    /// 命名空间（旧版属性）。
    #[yaserde(attribute = true, rename = "NameSpace")]
    pub namespace: String,
    /// 标引类型 ID（扩展属性）。
    #[yaserde(attribute = true, rename = "TypeID")]
    pub type_id: Option<String>,
    /// 约束文件路径。
    #[yaserde(rename = "SchemaLoc")]
    pub schema_loc: Option<StLoc>,
    /// 标引内容路径。
    #[yaserde(rename = "FileLoc")]
    pub file_loc: StLoc,
}
