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
        }
    }
}

impl std::error::Error for SealError {}
