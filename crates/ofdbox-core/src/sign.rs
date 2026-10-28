//! 签名生成：请求、签名值生产者契约与结构组装。
//!
//! 结构层只负责选取保护文件、计算 `References`、组装 `Signature.xml` /
//! `Signatures.xml` 与写包；签名值由 [`SignedValueProducer`] 的实现产出
//! （SM2 → GB/T 35275，签章 → GB/T 38540）。

use base64::Engine;

use crate::error::OfdError;
use crate::schema::definition::{StBox, StLoc};
use crate::schema::ofd::Ofd;
use crate::schema::signature::{
    CheckMethod, Provider, Reference, References, Signature, SignedInfo,
    StampAnnot as SchemaStampAnnot,
};
use crate::schema::signatures::{SignatureRef, SignatureType, Signatures};
use crate::schema_xml;
use crate::stamp::{StampAnnot, StampAppearance};

/// 单个 `Reference` 的摘要校验结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceDigest {
    /// 被保护文件路径。
    pub file_ref: String,
    /// 摘要方法。
    pub check_method: String,
    /// XML 中记录的摘要值。
    pub expected: Vec<u8>,
    /// 实际计算出的摘要值。
    pub actual: Vec<u8>,
}

impl ReferenceDigest {
    /// 摘要是否一致。
    pub fn matches(&self) -> bool {
        self.expected == self.actual
    }
}

/// 默认签名算法标识（SM2 with SM3）。
pub const DEFAULT_SIGNATURE_METHOD: &str = "1.2.156.10197.1.501";
/// 默认摘要方法标识（SM3，OID 形式，与真实签名文件一致）。
pub const DEFAULT_CHECK_METHOD: &str = "1.2.156.10197.1.401";

/// 签名值生产者。
///
/// 结构层通过该 trait 计算摘要并产出 `SignedValue.dat`，不依赖具体密码库。
pub trait SignedValueProducer {
    /// 是否支持指定签名方法。
    fn supports(&self, signature_method: &str) -> bool;

    /// 计算保护文件的摘要，结果写入 `References/CheckValue`。
    fn digest(&self, data: &[u8]) -> Result<Vec<u8>, OfdError>;

    /// 产出签名值文件的内容。
    fn produce(&self, context: &SigningContext<'_>) -> Result<Vec<u8>, OfdError>;

    /// 签名类型，写入 `Signatures.xml` 的 `Signature/@Type`；默认 `Sign`。
    fn signature_type(&self) -> &str {
        "Sign"
    }

    /// 签名值文件名，默认 `SignedValue.dat`。
    fn value_name(&self) -> &str {
        "SignedValue.dat"
    }
}

/// 签名值生产上下文。
pub struct SigningContext<'a> {
    /// 待签名的 `Signature.xml` 字节。
    pub content: &'a [u8],
    /// `Signature.xml` 在包内的绝对路径。
    pub signed_entry: &'a str,
    /// `SignatureMethod`。
    pub signature_method: &'a str,
    /// `SignatureDateTime`，按原样写入。
    pub signed_at: &'a str,
    /// 保护文件的摘要列表。
    pub references: &'a [ReferenceDigest],
}

/// 保护范围。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProtectScope {
    /// 包内全部内容相关文件（0099 附录 B.4），自动排除加密描述与本次签名文件。
    #[default]
    AllContent,
    /// 调用方指定的包内路径。
    Custom(Vec<String>),
}

/// 签名组件信息，对应 `SignedInfo/Provider`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInfo {
    /// 组件名称，必填。
    pub name: String,
    /// 组件制造商。
    pub company: Option<String>,
    /// 组件版本。
    pub version: Option<String>,
    /// 接口协议版本（0099 扩展）。
    pub protocol_version: Option<String>,
    /// 组件扩展信息（0099 扩展）。
    pub extend_data: Option<String>,
}

impl ProviderInfo {
    /// 默认使用 `ofdbox` 与当前 crate 版本。
    pub fn ofdbox() -> Self {
        Self {
            name: "ofdbox".to_owned(),
            company: None,
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            protocol_version: None,
            extend_data: None,
        }
    }

    /// 指定组件名称。
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 指定组件制造商。
    pub fn with_company(mut self, company: impl Into<String>) -> Self {
        self.company = Some(company.into());
        self
    }
}

impl Default for ProviderInfo {
    fn default() -> Self {
        Self::ofdbox()
    }
}

/// 一次签名的请求。
#[derive(Debug, Clone)]
pub struct SigningRequest<'a> {
    /// 保护范围。
    pub scope: ProtectScope,
    /// 是否允许后续继续签名。
    ///
    /// 为 `true` 时不把 `Signatures.xml` 纳入 `References`，否则会让后续签名
    /// 破坏本次签名（0099 附录 B.2）。
    pub allow_append: bool,
    /// `SignatureMethod`，默认 SM2 with SM3。
    pub signature_method: &'a str,
    /// 签名组件信息。
    pub provider: ProviderInfo,
    /// `SignatureDateTime`，按原样写入。
    pub signed_at: String,
    /// 盖章外观；按顺序写入多个 `StampAnnot`、注释与资源。
    pub stamps: Vec<StampAppearance>,
}

impl<'a> SigningRequest<'a> {
    /// 使用默认范围与组件信息创建请求。
    pub fn new(signature_method: &'a str, signed_at: impl Into<String>) -> Self {
        Self {
            scope: ProtectScope::AllContent,
            allow_append: true,
            signature_method,
            provider: ProviderInfo::ofdbox(),
            signed_at: signed_at.into(),
            stamps: Vec::new(),
        }
    }

    /// 追加一枚印章。
    pub fn with_stamp(mut self, stamp: StampAppearance) -> Self {
        self.stamps.push(stamp);
        self
    }

    /// 追加多枚印章。
    pub fn with_stamps(mut self, stamps: impl IntoIterator<Item = StampAppearance>) -> Self {
        self.stamps.extend(stamps);
        self
    }
}

/// 签名结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningReport {
    /// 签名标识，如 `s001`。
    pub signature_id: String,
    /// 签名描述文件在包内的路径。
    pub signature_entry: String,
    /// 签名值文件在包内的路径。
    pub signed_value_entry: String,
    /// 签名列表文件在包内的路径。
    pub signatures_entry: String,
    /// 被保护文件的包内路径。
    pub protected: Vec<String>,
}

/// 写 `Signature.xml`。
pub(crate) fn write_signature_xml(
    request: &SigningRequest<'_>,
    references: &[ReferenceDigest],
    signed_value_path: &str,
    stamps: &[StampAnnot],
) -> Result<String, OfdError> {
    let references = references
        .iter()
        .map(|reference| Reference {
            file_ref: StLoc::new(reference.file_ref.clone()),
            check_value: base64::engine::general_purpose::STANDARD.encode(&reference.expected),
        })
        .collect();
    let stamp_annots = stamps
        .iter()
        .map(|stamp| SchemaStampAnnot {
            id: stamp.id.clone(),
            page_ref: stamp.page_id.clone(),
            boundary: StBox::new(
                stamp.boundary[0],
                stamp.boundary[1],
                stamp.boundary[2],
                stamp.boundary[3],
            ),
            clip: stamp
                .clip
                .map(|clip| StBox::new(clip[0], clip[1], clip[2], clip[3])),
        })
        .collect();
    let signature = Signature {
        signed_info: SignedInfo {
            provider: Provider {
                provider_name: request.provider.name.clone(),
                version: request.provider.version.clone(),
                company: request.provider.company.clone(),
                protocol_version: request.provider.protocol_version.clone(),
                extend_data: request.provider.extend_data.clone(),
            },
            signature_method: Some(request.signature_method.to_owned()),
            signature_date_time: Some(request.signed_at.clone()),
            references: References {
                references,
                check_method: CheckMethod::Other(DEFAULT_CHECK_METHOD.to_owned()),
            },
            stamp_annots,
            seal: None,
        },
        signed_value: StLoc::new(signed_value_path),
    };
    schema_xml::to_string(&signature).map_err(|error| OfdError::InvalidStructure(error.to_string()))
}

/// 新建 `Signatures.xml`。
pub(crate) fn new_signatures_xml(
    id: &str,
    base_loc: &str,
    signature_type: &str,
) -> Result<String, OfdError> {
    let index = Signatures {
        max_sign_id: Some(id.to_owned()),
        signatures: vec![SignatureRef {
            id: id.to_owned(),
            signature_type: signature_type_value(signature_type),
            base_loc: StLoc::new(base_loc),
        }],
    };
    schema_xml::to_string(&index).map_err(|error| OfdError::InvalidStructure(error.to_string()))
}

/// 在已有 `Signatures.xml` 末尾追加一条签名记录，并更新 `MaxSignId`。
pub(crate) fn append_signature(
    existing: &[u8],
    id: &str,
    base_loc: &str,
    max_id: &str,
    signature_type: &str,
) -> Result<Vec<u8>, OfdError> {
    let mut index: Signatures = schema_xml::from_bytes(existing)
        .map_err(|error| OfdError::InvalidStructure(error.to_string()))?;
    index.max_sign_id = Some(max_id.to_owned());
    index.signatures.push(SignatureRef {
        id: id.to_owned(),
        signature_type: signature_type_value(signature_type),
        base_loc: StLoc::new(base_loc),
    });
    schema_xml::to_bytes(&index).map_err(|error| OfdError::InvalidStructure(error.to_string()))
}

/// 在 `OFD.xml` 的 `DocBody` 末尾补 `Signatures` 链接。
pub(crate) fn insert_signatures_link(root_xml: &[u8], link: &str) -> Result<Vec<u8>, OfdError> {
    let mut ofd: Ofd = schema_xml::from_bytes(root_xml)
        .map_err(|error| OfdError::InvalidStructure(error.to_string()))?;
    let body = ofd
        .doc_bodies
        .first_mut()
        .ok_or_else(|| OfdError::InvalidStructure("OFD.xml 缺少 DocBody".to_owned()))?;
    body.signatures = Some(StLoc::new(link));
    schema_xml::to_bytes(&ofd).map_err(|error| OfdError::InvalidStructure(error.to_string()))
}

fn signature_type_value(value: &str) -> SignatureType {
    // 写回时保留 Type 属性；Seal/Sign 的默认值不应被省略。
    SignatureType::Other(value.to_owned())
}
