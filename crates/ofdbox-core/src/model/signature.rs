//! `Signature.xsd`：签名/签章描述文件。

use super::definition::{StBox, StLoc};

/// `Signature.xml` 根对象。
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
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "SignedInfo")]
pub struct SignedInfo {
    /// 签名提供者。
    #[yaserde(rename = "Provider", default = "default_provider")]
    pub provider: Provider,
    /// 签名算法。
    #[yaserde(rename = "SignatureMethod")]
    pub signature_method: Option<String>,
    /// 签名时间。
    #[yaserde(rename = "SignatureDateTime")]
    pub signature_date_time: Option<String>,
    /// 引用。
    #[yaserde(rename = "References", default = "default_references")]
    pub references: References,
    /// 签章注释。
    #[yaserde(rename = "StampAnnot")]
    pub stamp_annots: Vec<StampAnnot>,
    /// 印章。
    #[yaserde(rename = "Seal")]
    pub seal: Option<Seal>,
}

/// 签名提供者。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Provider")]
pub struct Provider {
    /// 提供者名称。
    #[yaserde(attribute = true, rename = "ProviderName", default = "default_string")]
    pub provider_name: String,
    /// 版本。
    #[yaserde(attribute = true, rename = "Version")]
    pub version: Option<String>,
    /// 公司。
    #[yaserde(attribute = true, rename = "Company")]
    pub company: Option<String>,
    /// 接口协议版本。
    #[yaserde(attribute = true, rename = "ProtocolVer")]
    pub protocol_version: Option<String>,
    /// 组件扩展信息。
    #[yaserde(rename = "ExtendData")]
    pub extend_data: Option<String>,
}

/// 引用集合。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "References")]
pub struct References {
    /// 引用。
    #[yaserde(rename = "Reference")]
    pub references: Vec<Reference>,
    /// 校验方法。
    #[yaserde(
        attribute = true,
        rename = "CheckMethod",
        default = "default_check_method"
    )]
    pub check_method: CheckMethod,
}

/// 校验方法。
#[derive(Clone, Debug, Default, PartialEq, Eq, ofdbox_yaserde::DefaultYaSerde)]
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
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
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
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "StampAnnot")]
pub struct StampAnnot {
    /// 签名 ID。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: String,
    /// 页面引用。
    #[yaserde(attribute = true, rename = "PageRef")]
    pub page_ref: String,
    /// 边界。
    #[yaserde(attribute = true, rename = "Boundary")]
    pub boundary: StBox,
    /// 裁剪区域。
    #[yaserde(attribute = true, rename = "Clip")]
    pub clip: Option<StBox>,
}

/// 印章。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Seal")]
pub struct Seal {
    /// 印章基准路径。
    #[yaserde(rename = "BaseLoc", default = "default_st_loc")]
    pub base_loc: StLoc,
}

fn default_check_method() -> CheckMethod {
    CheckMethod::Md5
}

fn default_string() -> String {
    String::new()
}

fn default_st_loc() -> StLoc {
    StLoc::default()
}

impl Signature {
    /// 签名信息。
    pub fn signed_info(&self) -> Option<&SignedInfo> {
        Some(&self.signed_info)
    }

    /// 签名值路径。
    pub fn signed_value(&self) -> Option<&str> {
        Some(self.signed_value.as_str())
    }
}

impl SignedInfo {
    /// 引用集合。
    pub fn references(&self) -> Option<&References> {
        Some(&self.references)
    }

    /// 签名方法。
    pub fn signature_method(&self) -> Option<&str> {
        self.signature_method.as_deref()
    }

    /// 印章信息。
    pub fn seal(&self) -> Option<&Seal> {
        self.seal.as_ref()
    }

    /// 签名提供者。
    pub fn provider(&self) -> Option<&Provider> {
        Some(&self.provider)
    }

    /// 签名时间。
    pub fn signature_date_time(&self) -> Option<&str> {
        self.signature_date_time.as_deref()
    }
}

impl Provider {
    /// 组件名称。
    pub fn name(&self) -> Option<&str> {
        Some(self.provider_name.as_str())
    }

    /// 组件名称。
    pub fn provider_name(&self) -> Option<&str> {
        Some(self.provider_name.as_str())
    }

    /// 公司。
    pub fn company(&self) -> Option<&str> {
        self.company.as_deref()
    }

    /// 版本。
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// 协议版本。
    pub fn protocol_version(&self) -> Option<&str> {
        self.protocol_version.as_deref()
    }

    /// 扩展信息。
    pub fn extend_data(&self) -> Option<&str> {
        self.extend_data.as_deref()
    }
}

impl References {
    /// 引用。
    pub fn references(&self) -> impl Iterator<Item = &Reference> {
        self.references.iter()
    }

    /// 校验方法。
    pub fn check_method(&self) -> Option<&str> {
        Some(match &self.check_method {
            CheckMethod::Md5 => "MD5",
            CheckMethod::Sha1 => "SHA1",
            CheckMethod::Other(value) => value,
        })
    }
}

impl Reference {
    /// 文件路径。
    pub fn file_ref(&self) -> Option<&str> {
        Some(self.file_ref.as_str())
    }

    /// 校验值。
    pub fn check_value(&self) -> &str {
        &self.check_value
    }
}

impl Seal {
    /// 印章基准路径。
    pub fn base_loc(&self) -> Option<&str> {
        Some(self.base_loc.as_str())
    }
}

fn default_provider() -> Provider {
    Provider::default()
}

fn default_references() -> References {
    References::default()
}
