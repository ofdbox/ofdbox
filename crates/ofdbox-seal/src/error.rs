use std::fmt;

/// 电子印章结构解析错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealError {
    /// DER 数据在预期位置提前结束。
    Truncated(&'static str),
    /// 结构规模超出解析上限。
    LimitExceeded(&'static str),
    /// 结构不符合电子印章的 ASN.1 定义。
    Invalid(&'static str),
}

impl fmt::Display for SealError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated(what) => write!(formatter, "DER 数据不完整：{what}"),
            Self::LimitExceeded(what) => write!(formatter, "DER 结构超出限制：{what}"),
            Self::Invalid(what) => write!(formatter, "印章结构无效：{what}"),
        }
    }
}

impl std::error::Error for SealError {}
