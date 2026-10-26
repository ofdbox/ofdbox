//! `Signatures.xsd`：签名索引文件。

use super::definition::StLoc;

/// `Signatures.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Signatures {
    /// 最大签名 ID。
    pub max_sign_id: Option<String>,
    /// 签名索引。
    pub signatures: Vec<SignatureRef>,
}

/// 签名索引条目。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignatureRef {
    /// 签名 ID。
    pub id: String,
    /// 签名类型。
    pub signature_type: SignatureType,
    /// 签名描述文件路径。
    pub base_loc: StLoc,
}

/// 签名类型。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SignatureType {
    /// 印章。
    #[default]
    Seal,
    /// 签名。
    Sign,
    /// 未知值。
    Other(String),
}
