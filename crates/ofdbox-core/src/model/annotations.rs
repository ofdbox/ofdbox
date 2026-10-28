//! `Annotations.xsd`：按页组织的注释索引。

use super::definition::StLoc;

/// `Annotations.xml` 根对象。
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
#[yaserde(rename = "Annotations")]
pub struct Annotations {
    /// 页面注释索引。
    #[yaserde(rename = "Page")]
    pub pages: Vec<AnnotationsPage>,
}

/// 页面注释索引。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Page")]
pub struct AnnotationsPage {
    /// 页面 ID。
    #[yaserde(attribute = true, rename = "PageID")]
    pub page_id: String,
    /// 注释文件路径。
    #[yaserde(rename = "FileLoc")]
    pub file_loc: StLoc,
}
