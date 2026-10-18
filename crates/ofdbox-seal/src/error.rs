use std::fmt;

use crate::fingerprint::SealFormat;

/// 电子印章结构解析与验签错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealError {
    /// DER 数据在预期位置提前结束。
    Truncated(&'static str),
    /// 结构规模超出解析上限。
    LimitExceeded(&'static str),
    /// 结构不符合电子印章的 ASN.1 定义。
    Invalid(&'static str),
    /// 调用方要求验证的格式不是已知标准。
    UnsupportedFormat,
    /// 印章的实际结构与调用方要求的格式不一致。
    FormatMismatch {
        /// 调用方要求的格式。
        expected: SealFormat,
        /// 结构指纹识别出的格式。
        actual: SealFormat,
    },
    /// 印章签名算法不受支持。
    UnsupportedAlgorithm(String),
    /// 印章外层缺少制章者证书。
    MissingCertificate,
    /// 制章者证书无法解析或缺少公钥。
    InvalidCertificate,
    /// 印章签名验证失败。
    SignatureInvalid,
    /// 证书尚未生效。
    CertificateNotYetValid,
    /// 证书已过期。
    CertificateExpired,
    /// 证书链没有连到任何信任锚。
    UntrustedCertificate,
    /// 证书签名验证失败。
    CertificateSignatureInvalid,
    /// 证书签名算法不受支持。
    UnsupportedCertificateAlgorithm(String),
    /// 证书已被 CRL 吊销。
    CertificateRevoked,
    /// 需要吊销检查但没有适用的 CRL 或吊销状态未知。
    RevocationUnknown,
    /// 证书密钥用法不符合要求。
    CertificateKeyUsage,
    /// 证书链超出路径长度约束。
    CertificatePathLength,
    /// 证书链结构不合法。
    InvalidCertificateChain,
    /// 印章尚未生效。
    SealNotYetValid,
    /// 印章已过期。
    SealExpired,
    /// 签章人证书不在印章的证书列表中。
    SignerNotInSeal,
    /// 不支持的签章者证书信息类型。
    UnsupportedCertificateListType(i64),
    /// 未提供验证时间，且当前平台无法读取系统时间。
    VerificationTimeRequired,
}

impl fmt::Display for SealError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated(what) => write!(formatter, "DER 数据不完整：{what}"),
            Self::LimitExceeded(what) => write!(formatter, "DER 结构超出限制：{what}"),
            Self::Invalid(what) => write!(formatter, "印章结构无效：{what}"),
            Self::UnsupportedFormat => write!(formatter, "印章格式未知，无法按已知标准验证"),
            Self::FormatMismatch { expected, actual } => write!(
                formatter,
                "印章格式不匹配：期望 {}，实际为 {}",
                expected.name(),
                actual.name()
            ),
            Self::UnsupportedAlgorithm(algorithm) => {
                write!(formatter, "不支持的印章签名算法：{algorithm}")
            }
            Self::MissingCertificate => formatter.write_str("印章缺少制章者证书"),
            Self::InvalidCertificate => formatter.write_str("制章者证书不合法"),
            Self::SignatureInvalid => formatter.write_str("印章签名验证失败"),
            Self::CertificateNotYetValid => formatter.write_str("证书尚未生效"),
            Self::CertificateExpired => formatter.write_str("证书已过期"),
            Self::UntrustedCertificate => formatter.write_str("证书链未连接到信任锚"),
            Self::CertificateSignatureInvalid => formatter.write_str("证书签名验证失败"),
            Self::UnsupportedCertificateAlgorithm(algorithm) => {
                write!(formatter, "不支持的证书签名算法：{algorithm}")
            }
            Self::CertificateRevoked => formatter.write_str("证书已被吊销"),
            Self::RevocationUnknown => formatter.write_str("无法确定证书吊销状态"),
            Self::CertificateKeyUsage => formatter.write_str("证书密钥用法不符合要求"),
            Self::CertificatePathLength => formatter.write_str("证书链超出路径长度约束"),
            Self::InvalidCertificateChain => formatter.write_str("证书链结构不合法"),
            Self::SealNotYetValid => formatter.write_str("印章尚未生效"),
            Self::SealExpired => formatter.write_str("印章已过期"),
            Self::SignerNotInSeal => formatter.write_str("签章人证书不在印章证书列表中"),
            Self::UnsupportedCertificateListType(list_type) => {
                write!(formatter, "不支持的签章者证书信息类型：{list_type}")
            }
            Self::VerificationTimeRequired => {
                formatter.write_str("当前平台无法读取系统时间，必须显式提供验证时间")
            }
        }
    }
}

impl std::error::Error for SealError {}
