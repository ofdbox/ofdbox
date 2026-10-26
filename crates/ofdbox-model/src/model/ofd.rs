//! `OFD.xsd`：包主入口与文档元数据。

use super::definition::StLoc;

/// `OFD.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ofd {
    /// 文件格式版本。
    pub version: String,
    /// 文件类型。
    pub doc_type: String,
    /// 文档入口列表。
    pub doc_bodies: Vec<DocBody>,
}

/// 文档入口。
#[derive(Clone, Debug, PartialEq)]
pub struct DocBody {
    /// 文档元数据。
    pub doc_info: DocInfo,
    /// 文档根文件路径。
    pub doc_root: StLoc,
    /// 版本信息。
    pub versions: Option<Versions>,
    /// 签名索引路径。
    pub signatures: Option<StLoc>,
}

/// 文档元数据。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocInfo {
    /// 文档 ID。
    pub doc_id: String,
    /// 标题。
    pub title: Option<String>,
    /// 作者。
    pub author: Option<String>,
    /// 主题。
    pub subject: Option<String>,
    /// 摘要。
    pub abstract_: Option<String>,
    /// 创建日期。
    pub creation_date: Option<String>,
    /// 修改日期。
    pub mod_date: Option<String>,
    /// 文档用途。
    pub doc_usage: Option<String>,
    /// 封面路径。
    pub cover: Option<StLoc>,
    /// 关键词。
    pub keywords: Option<Keywords>,
    /// 创建者。
    pub creator: Option<String>,
    /// 创建者版本。
    pub creator_version: Option<String>,
    /// 自定义数据。
    pub custom_datas: Option<CustomDatas>,
}

/// 关键词列表。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keywords {
    /// 关键词。
    pub keywords: Vec<String>,
}

/// 自定义数据列表。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomDatas {
    /// 自定义数据。
    pub custom_datas: Vec<CustomData>,
}

/// 一条自定义数据。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomData {
    /// 名称。
    pub name: String,
    /// 内容。
    pub value: String,
}

/// 版本索引。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Versions {
    /// 版本。
    pub versions: Vec<VersionRef>,
}

/// 版本索引条目。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VersionRef {
    /// 版本 ID。
    pub id: String,
    /// 序号。
    pub index: i32,
    /// 是否当前版本。
    pub current: bool,
    /// 版本描述文件路径。
    pub base_loc: StLoc,
}
