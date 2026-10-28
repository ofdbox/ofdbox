//! `Attachments.xsd`：附件索引。

use super::definition::StLoc;

/// `Attachments.xml` 根对象。
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
#[yaserde(rename = "Attachments")]
pub struct Attachments {
    /// 附件。
    #[yaserde(rename = "Attachment")]
    pub attachments: Vec<Attachment>,
}

/// 附件。
#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "Attachment")]
pub struct Attachment {
    /// 附件 ID。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: String,
    /// 名称。
    #[yaserde(attribute = true, rename = "Name")]
    pub name: String,
    /// 格式。
    #[yaserde(attribute = true, rename = "Format")]
    pub format: Option<String>,
    /// 创建时间。
    #[yaserde(attribute = true, rename = "CreationDate")]
    pub creation_date: Option<String>,
    /// 修改时间。
    #[yaserde(attribute = true, rename = "ModDate")]
    pub mod_date: Option<String>,
    /// 大小。
    #[yaserde(attribute = true, rename = "Size")]
    pub size: Option<f64>,
    /// 是否可见。
    #[yaserde(attribute = true, rename = "Visible", default = "default_true")]
    pub visible: bool,
    /// 用途。
    #[yaserde(attribute = true, rename = "Usage", default = "default_usage")]
    pub usage: String,
    /// 文件路径。
    #[yaserde(rename = "FileLoc", default = "default_st_loc")]
    pub file_loc: StLoc,
}

fn default_true() -> bool {
    true
}

fn default_usage() -> String {
    "none".to_owned()
}

fn default_st_loc() -> StLoc {
    StLoc::default()
}

impl Default for Attachment {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            format: None,
            creation_date: None,
            mod_date: None,
            size: None,
            visible: true,
            usage: "none".to_owned(),
            file_loc: StLoc::default(),
        }
    }
}
