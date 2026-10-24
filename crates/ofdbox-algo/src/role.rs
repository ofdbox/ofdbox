/// 算法角色。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlgorithmRole {
    /// 杂凑算法。
    Digest,
    /// 原始签名算法（不含摘要组合）。
    RawSignature,
    /// 复合签名算法（摘要 + 签名）。
    SignatureSuite,
    /// 分组/序列密码。
    Cipher,
    /// 密钥派生。
    Kdf,
    /// 密钥协商。
    KeyAgreement,
    /// 公钥加密。
    KeyEncryption,
    /// 消息认证码。
    Mac,
}

impl AlgorithmRole {
    /// 角色名称。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Digest => "digest",
            Self::RawSignature => "signature",
            Self::SignatureSuite => "signature-suite",
            Self::Cipher => "cipher",
            Self::Kdf => "kdf",
            Self::KeyAgreement => "key-agreement",
            Self::KeyEncryption => "key-encryption",
            Self::Mac => "mac",
        }
    }
}
