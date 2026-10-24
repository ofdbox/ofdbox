/// 算法公钥的借用视图。
///
/// `key_bytes` 的编码由算法决定：
/// - SM2：未压缩 SEC1 点（`04 || x || y`）；
/// - 其他算法：对应算法约定的公钥字节。
///
/// 证书/SPKI 到算法公钥的转换由上层完成，避免本 crate 依赖 X.509。
#[derive(Clone, Copy, Debug)]
pub struct PublicKeyRef<'a> {
    /// 公钥算法 OID。
    pub algorithm_oid: &'a str,
    /// 算法公钥字节。
    pub key_bytes: &'a [u8],
}

impl<'a> PublicKeyRef<'a> {
    /// 创建公钥视图。
    pub const fn new(algorithm_oid: &'a str, key_bytes: &'a [u8]) -> Self {
        Self {
            algorithm_oid,
            key_bytes,
        }
    }
}

/// 算法私钥的借用视图。
///
/// `key_bytes` 的编码由算法决定：
/// - SM2：32 字节大端私钥标量；
/// - 其他算法：对应算法约定的私钥字节。
#[derive(Clone, Copy, Debug)]
pub struct SecretKeyRef<'a> {
    /// 私钥算法 OID。
    pub algorithm_oid: &'a str,
    /// 算法私钥字节。
    pub key_bytes: &'a [u8],
}

impl<'a> SecretKeyRef<'a> {
    /// 创建私钥视图。
    pub const fn new(algorithm_oid: &'a str, key_bytes: &'a [u8]) -> Self {
        Self {
            algorithm_oid,
            key_bytes,
        }
    }
}
