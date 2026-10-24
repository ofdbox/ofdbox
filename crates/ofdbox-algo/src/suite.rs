/// 复合签名的组合方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuiteMode {
    /// 摘要由签名算法内部完成（如 SM2+SM3）。
    Integrated,
    /// 先摘要，再对摘要验签（如 RSA+SM3）。
    Prehashed,
}

/// 复合签名算法：引用一个摘要算法和一个原始签名算法。
#[derive(Clone, Copy, Debug)]
pub struct SignatureSuite {
    /// 复合算法 OID。
    pub oid: &'static str,
    /// 摘要算法 OID。
    pub digest_oid: &'static str,
    /// 原始签名算法 OID。
    pub signature_oid: &'static str,
    /// 组合方式。
    pub mode: SuiteMode,
}
