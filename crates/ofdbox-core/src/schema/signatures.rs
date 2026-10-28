//! `Signatures.xsd`：签名索引文件。

use super::definition::StLoc;

/// `Signatures.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Signatures")]
pub struct Signatures {
    /// 最大签名 ID。
    #[yaserde(rename = "MaxSignId")]
    pub max_sign_id: Option<String>,
    /// 签名索引。
    #[yaserde(rename = "Signature")]
    pub signatures: Vec<SignatureRef>,
}

/// 签名索引条目。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Signature")]
pub struct SignatureRef {
    /// 签名 ID。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: String,
    /// 签名类型。
    #[yaserde(attribute = true, rename = "Type", default = "default_signature_type")]
    pub signature_type: SignatureType,
    /// 签名描述文件路径。
    #[yaserde(attribute = true, rename = "BaseLoc")]
    pub base_loc: StLoc,
}

/// 签名类型。
#[derive(Clone, Debug, Default, PartialEq, Eq, yaserde::DefaultYaSerde)]
pub enum SignatureType {
    /// 印章。
    #[default]
    Seal,
    /// 签名。
    Sign,
    /// 未知值。
    Other(String),
}

crate::impl_xml_enum_codec!(SignatureType, Other, {
    Seal => "Seal",
    Sign => "Sign",
});

fn default_signature_type() -> SignatureType {
    SignatureType::Seal
}


impl Signatures {
    /// 最大签名 ID。
    pub fn max_sign_id(&self) -> Option<&str> {
        self.max_sign_id.as_deref()
    }

    /// 签名索引。
    pub fn signatures(&self) -> impl Iterator<Item = &SignatureRef> {
        self.signatures.iter()
    }
}

impl SignatureRef {
    /// 签名标识。
    pub fn id(&self) -> Option<&str> {
        Some(self.id.as_str())
    }

    /// 签名类型。
    pub fn signature_type(&self) -> Option<&str> {
        Some(match &self.signature_type {
            SignatureType::Seal => "Seal",
            SignatureType::Sign => "Sign",
            SignatureType::Other(value) => value,
        })
    }

    /// 签名描述文件路径。
    pub fn base_loc(&self) -> Option<&str> {
        Some(self.base_loc.as_str())
    }
}
