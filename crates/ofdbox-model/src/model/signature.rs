//! `Signature.xsd`：签名/签章描述文件。

use super::definition::{StBox, StLoc, StRefId};

/// `Signature.xml` 根对象。
#[derive(Clone, Debug, PartialEq)]
pub struct Signature {
    /// 签名信息。
    pub signed_info: SignedInfo,
    /// 签名值文件路径。
    pub signed_value: StLoc,
}

/// 签名信息。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignedInfo {
    /// 签名提供者。
    pub provider: Provider,
    /// 签名算法。
    pub signature_method: Option<String>,
    /// 签名时间。
    pub signature_date_time: Option<String>,
    /// 引用。
    pub references: References,
    /// 签章注释。
    pub stamp_annots: Vec<StampAnnot>,
    /// 印章。
    pub seal: Option<Seal>,
}

/// 签名提供者。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Provider {
    /// 提供者名称。
    pub provider_name: String,
    /// 版本。
    pub version: Option<String>,
    /// 公司。
    pub company: Option<String>,
}

/// 引用集合。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct References {
    /// 引用。
    pub references: Vec<Reference>,
    /// 校验方法。
    pub check_method: CheckMethod,
}

/// 校验方法。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CheckMethod {
    /// MD5。
    #[default]
    Md5,
    /// SHA1。
    Sha1,
    /// 未知值。
    Other(String),
}

/// 引用。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reference {
    /// 文件路径。
    pub file_ref: StLoc,
    /// 校验值。
    pub check_value: String,
}

/// 签章注释。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StampAnnot {
    /// 签名 ID。
    pub id: String,
    /// 页面引用。
    pub page_ref: StRefId,
    /// 边界。
    pub boundary: StBox,
    /// 裁剪区域。
    pub clip: Option<StBox>,
}

/// 印章。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seal {
    /// 印章基准路径。
    pub base_loc: StLoc,
}
