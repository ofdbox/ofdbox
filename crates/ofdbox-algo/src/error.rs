use std::fmt;

/// 算法层错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlgorithmError {
    /// 指定 OID 未注册。
    UnknownAlgorithm {
        /// 算法 OID。
        oid: String,
        /// 调用方期望的角色。
        role: &'static str,
    },
    /// OID 已注册，但角色与调用方期望不一致。
    RoleMismatch {
        /// 算法 OID。
        oid: String,
        /// 调用方期望的角色。
        expected: &'static str,
        /// 实际注册的角色。
        actual: &'static str,
    },
    /// 公钥无效。
    InvalidKey,
    /// 签名值无效。
    InvalidSignature,
    /// 密文或填充无效。
    InvalidCiphertext,
    /// 参数无效。
    InvalidParameters,
    /// 当前实现不支持该操作。
    Unsupported,
}

impl fmt::Display for AlgorithmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAlgorithm { oid, role } => {
                write!(formatter, "未注册的算法：{oid}（角色 {role}）")
            }
            Self::RoleMismatch {
                oid,
                expected,
                actual,
            } => write!(
                formatter,
                "算法角色不匹配：{oid}，期望 {expected}，实际 {actual}"
            ),
            Self::InvalidKey => formatter.write_str("公钥或密钥无效"),
            Self::InvalidSignature => formatter.write_str("签名值无效"),
            Self::InvalidCiphertext => formatter.write_str("密文或填充无效"),
            Self::InvalidParameters => formatter.write_str("算法参数无效"),
            Self::Unsupported => formatter.write_str("当前实现不支持该操作"),
        }
    }
}

impl std::error::Error for AlgorithmError {}
