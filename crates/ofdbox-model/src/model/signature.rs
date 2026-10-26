//! `Signature.xsd`：签名/签章描述文件。

use super::definition::{StBox, StLoc, StRefId};

/// `Signature.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Signature")]
pub struct Signature {
    /// 签名信息。
    #[yaserde(rename = "SignedInfo")]
    pub signed_info: SignedInfo,
    /// 签名值文件路径。
    #[yaserde(rename = "SignedValue")]
    pub signed_value: StLoc,
}

/// 签名信息。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "SignedInfo")]
pub struct SignedInfo {
    /// 签名提供者。
    #[yaserde(rename = "Provider")]
    pub provider: Provider,
    /// 签名算法。
    #[yaserde(rename = "SignatureMethod")]
    pub signature_method: Option<String>,
    /// 签名时间。
    #[yaserde(rename = "SignatureDateTime")]
    pub signature_date_time: Option<String>,
    /// 引用。
    #[yaserde(rename = "References")]
    pub references: References,
    /// 签章注释。
    #[yaserde(rename = "StampAnnot")]
    pub stamp_annots: Vec<StampAnnot>,
    /// 印章。
    #[yaserde(rename = "Seal")]
    pub seal: Option<Seal>,
}

/// 签名提供者。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Provider")]
pub struct Provider {
    /// 提供者名称。
    #[yaserde(attribute = true, rename = "ProviderName")]
    pub provider_name: String,
    /// 版本。
    #[yaserde(attribute = true, rename = "Version")]
    pub version: Option<String>,
    /// 公司。
    #[yaserde(attribute = true, rename = "Company")]
    pub company: Option<String>,
}

/// 引用集合。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "References")]
pub struct References {
    /// 引用。
    #[yaserde(rename = "Reference")]
    pub references: Vec<Reference>,
    /// 校验方法。
    #[yaserde(attribute = true, rename = "CheckMethod")]
    pub check_method: CheckMethod,
}

/// 校验方法。
#[derive(Clone, Debug, Default, PartialEq, Eq, yaserde::DefaultYaSerde)]
pub enum CheckMethod {
    /// MD5。
    #[default]
    Md5,
    /// SHA1。
    Sha1,
    /// 未知值。
    Other(String),
}

crate::impl_xml_enum_codec!(CheckMethod, Other, {
    Md5 => "MD5",
    Sha1 => "SHA1",
});

/// 引用。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Reference")]
pub struct Reference {
    /// 文件路径。
    #[yaserde(attribute = true, rename = "FileRef")]
    pub file_ref: StLoc,
    /// 校验值。
    #[yaserde(rename = "CheckValue")]
    pub check_value: String,
}

/// 签章注释。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "StampAnnot")]
pub struct StampAnnot {
    /// 签名 ID。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: String,
    /// 页面引用。
    #[yaserde(attribute = true, rename = "PageRef")]
    pub page_ref: StRefId,
    /// 边界。
    #[yaserde(attribute = true, rename = "Boundary")]
    pub boundary: StBox,
    /// 裁剪区域。
    #[yaserde(attribute = true, rename = "Clip")]
    pub clip: Option<StBox>,
}

/// 印章。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Seal")]
pub struct Seal {
    /// 印章基准路径。
    #[yaserde(rename = "BaseLoc")]
    pub base_loc: StLoc,
}
