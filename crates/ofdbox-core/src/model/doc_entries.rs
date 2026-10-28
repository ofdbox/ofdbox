//! `OFDEntries.xsd`：完整性保护文件列表。

use super::definition::StLoc;

/// `OFDEntries.xml` 根对象。
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
#[yaserde(rename = "DocEntries")]
pub struct DocEntries {
    /// 文档标识。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: String,
    /// 创建者名称。
    #[yaserde(attribute = true, rename = "CreatorName")]
    pub creator_name: Option<String>,
    /// 版本。
    #[yaserde(attribute = true, rename = "Version")]
    pub version: Option<String>,
    /// 创建日期。
    #[yaserde(attribute = true, rename = "CreationDate")]
    pub creation_date: Option<String>,
    /// 文件列表。
    #[yaserde(rename = "FileList")]
    pub file_list: FileList,
    /// 签名值路径。
    #[yaserde(rename = "SignedValueLoc", default = "default_st_loc")]
    pub signed_value_loc: StLoc,
}

/// 文件列表。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "FileList")]
pub struct FileList {
    /// 文件。
    #[yaserde(rename = "File")]
    pub files: Vec<File>,
}

/// 完整性保护文件。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "File")]
pub struct File {
    /// 文件标识。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: String,
    /// 包内绝对路径。
    #[yaserde(attribute = true, rename = "FileLoc")]
    pub file_loc: StLoc,
}

fn default_st_loc() -> StLoc {
    StLoc::default()
}
