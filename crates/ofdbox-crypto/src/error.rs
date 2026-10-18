/// 密码运算错误。
///
/// 错误信息不包含密钥、口令或明文。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CryptoError {
    /// 密钥长度或内容不合法。
    #[error("密钥不合法")]
    InvalidKey,
    /// 密钥派生参数不合法。
    #[error("密钥派生参数不合法")]
    InvalidKdfParameters,
    /// 密文格式或填充不合法。
    #[error("密文不合法")]
    InvalidCiphertext,
    /// 签名值不合法。
    #[error("签名不合法")]
    InvalidSignature,
    /// 证书解析失败。
    #[error("证书不合法")]
    InvalidCertificate,
    /// 编码或 ASN.1 结构不合法。
    #[error("密码数据结构不合法")]
    InvalidEncoding,
    /// I/O 错误。
    #[error("I/O 错误: {0}")]
    Io(String),
}

impl From<std::io::Error> for CryptoError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}
