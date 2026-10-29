//! `OFD.xsd`：包主入口与文档元数据。

use std::io::Write;

use ofdbox_yaserde::{
    __xml::attribute::OwnedAttribute, __xml::namespace::Namespace, __xml::writer::XmlEvent,
    YaSerialize,
};

use super::definition::StLoc;

/// `OFD.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaDeserialize)]
#[yaserde(
  namespace = "http://www.ofdspec.org/2016",
  xmlns = {
    "ofd" = "http://www.ofdspec.org/2016"
  },
  namespace_aliases = {
    "http://www.ofdspec.org" = "http://www.ofdspec.org/2016"
  }
)]
#[yaserde(rename = "OFD")]
pub struct Ofd {
    /// 文件格式版本。
    #[yaserde(attribute = true, rename = "Version")]
    pub version: String,
    /// 文件类型。
    #[yaserde(attribute = true, rename = "DocType", default = "default_doc_type")]
    pub doc_type: String,
    /// 文档入口列表。
    #[yaserde(rename = "DocBody")]
    pub doc_bodies: Vec<DocBody>,
}

/// 文档入口。
impl YaSerialize for Ofd {
    fn serialize<W: Write>(
        &self,
        writer: &mut ofdbox_yaserde::ser::Serializer<W>,
    ) -> ofdbox_yaserde::Result<()> {
        let namespace = crate::model_xml::OFD_NAMESPACE;
        let previous_namespace = writer
            .current_namespace()
            .map(std::string::ToString::to_string);
        writer.push_namespaces(vec![("ofd".to_string(), namespace.to_string())]);
        writer.set_current_namespace(Some(namespace.to_string()));
        let element_name = writer.element_name(Some(namespace), "OFD", None);
        writer
            .write(
                XmlEvent::start_element(element_name.as_str())
                    .ns("ofd", namespace)
                    .attr("Version", &self.version)
                    .attr("DocType", &self.doc_type),
            )
            .map_err(ofdbox_yaserde::Error::custom)?;
        for body in &self.doc_bodies {
            let body_name = writer.element_name(Some(namespace), "DocBody", None);
            writer.set_start_event_name(Some(body_name));
            writer.set_skip_start_end(false);
            body.serialize(writer)?;
        }
        writer
            .write(XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        writer.pop_namespaces();
        writer.set_current_namespace(previous_namespace);
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<OwnedAttribute>,
        namespace: Namespace,
    ) -> ofdbox_yaserde::Result<(Vec<OwnedAttribute>, Namespace)> {
        Ok((attributes, namespace))
    }
}

#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "DocBody")]
pub struct DocBody {
    /// 文档元数据。
    #[yaserde(rename = "DocInfo")]
    pub doc_info: DocInfo,
    /// 文档根文件路径。
    #[yaserde(rename = "DocRoot")]
    pub doc_root: StLoc,
    /// 版本信息。
    #[yaserde(rename = "Versions")]
    pub versions: Option<Versions>,
    /// 签名索引路径。
    #[yaserde(rename = "Signatures")]
    pub signatures: Option<StLoc>,
}

/// 文档元数据。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "DocInfo")]
pub struct DocInfo {
    /// 文档 ID。
    #[yaserde(rename = "DocID")]
    pub doc_id: String,
    /// 标题。
    #[yaserde(rename = "Title")]
    pub title: Option<String>,
    /// 作者。
    #[yaserde(rename = "Author")]
    pub author: Option<String>,
    /// 主题。
    #[yaserde(rename = "Subject")]
    pub subject: Option<String>,
    /// 摘要。
    #[yaserde(rename = "Abstract")]
    pub abstract_text: Option<String>,
    /// 创建日期。
    #[yaserde(rename = "CreationDate")]
    pub creation_date: Option<String>,
    /// 修改日期。
    #[yaserde(rename = "ModDate")]
    pub mod_date: Option<String>,
    /// 文档用途。
    #[yaserde(rename = "DocUsage")]
    pub doc_usage: Option<String>,
    /// 封面路径。
    #[yaserde(rename = "Cover")]
    pub cover: Option<StLoc>,
    /// 关键词。
    #[yaserde(rename = "Keywords")]
    pub keywords: Option<Keywords>,
    /// 创建者。
    #[yaserde(rename = "Creator")]
    pub creator: Option<String>,
    /// 创建者版本。
    #[yaserde(rename = "CreatorVersion")]
    pub creator_version: Option<String>,
    /// 自定义数据。
    #[yaserde(rename = "CustomDatas")]
    pub custom_datas: Option<CustomDatas>,
}

/// 关键词列表。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Keywords")]
pub struct Keywords {
    /// 关键词。
    #[yaserde(rename = "Keyword")]
    pub keywords: Vec<String>,
}

/// 自定义数据列表。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "CustomDatas")]
pub struct CustomDatas {
    /// 自定义数据。
    #[yaserde(rename = "CustomData")]
    pub custom_datas: Vec<CustomData>,
}

/// 一条自定义数据。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "CustomData")]
pub struct CustomData {
    /// 名称。
    #[yaserde(attribute = true, rename = "Name")]
    pub name: String,
    /// 内容。
    #[yaserde(text = true, default = "default_string")]
    pub value: String,
}

/// 版本索引。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Versions")]
pub struct Versions {
    /// 版本。
    #[yaserde(rename = "Version")]
    pub versions: Vec<VersionRef>,
}

/// 版本索引条目。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Version")]
pub struct VersionRef {
    /// 版本 ID。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: String,
    /// 序号。
    #[yaserde(attribute = true, rename = "Index")]
    pub index: i32,
    /// 是否当前版本。
    #[yaserde(attribute = true, rename = "Current", default = "default_false")]
    pub current: bool,
    /// 版本描述文件路径。
    #[yaserde(attribute = true, rename = "BaseLoc")]
    pub base_loc: StLoc,
}

fn default_false() -> bool {
    false
}

fn default_doc_type() -> String {
    "OFD".to_owned()
}

fn default_string() -> String {
    String::new()
}
