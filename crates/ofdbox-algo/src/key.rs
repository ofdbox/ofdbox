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

/// 拥有所有权的算法公钥。
#[derive(Clone, Debug)]
pub struct PublicKey {
    /// 公钥算法 OID。
    pub algorithm_oid: String,
    /// 算法公钥字节（SM2 为未压缩 SEC1 点）。
    pub key_bytes: Vec<u8>,
}

impl PublicKey {
    /// 借用视图。
    pub fn as_ref(&self) -> PublicKeyRef<'_> {
        PublicKeyRef::new(&self.algorithm_oid, &self.key_bytes)
    }
}

#[cfg(feature = "x509")]
mod x509 {
    use der::{Decode, Encode};
    use x509_cert::Certificate;
    use x509_cert::spki::SubjectPublicKeyInfoOwned;

    use super::PublicKey;
    use crate::error::AlgorithmError;

    /// 从 `SubjectPublicKeyInfo` DER 提取算法公钥。
    pub fn public_key_from_spki(spki_der: &[u8]) -> Result<PublicKey, AlgorithmError> {
        let spki = SubjectPublicKeyInfoOwned::from_der(spki_der)
            .map_err(|_| AlgorithmError::InvalidKey)?;
        let key_bytes = spki.subject_public_key.raw_bytes();
        if key_bytes.is_empty() {
            return Err(AlgorithmError::InvalidKey);
        }
        let mut algorithm_oid = spki.algorithm.oid.to_string();
        // SM2 通常写成 id-ecPublicKey + SM2 曲线参数；这里归一化为 SM2 OID。
        if algorithm_oid == "1.2.840.10045.2.1"
            && let Some(parameters) = spki.algorithm.parameters
            && let Ok(curve) = parameters.decode_as::<der::asn1::ObjectIdentifier>()
        {
            algorithm_oid = curve.to_string();
        }
        Ok(PublicKey {
            algorithm_oid,
            key_bytes: key_bytes.to_vec(),
        })
    }

    /// 从 X.509 证书提取算法公钥。
    pub fn public_key_from_certificate(
        certificate_der: &[u8],
    ) -> Result<PublicKey, AlgorithmError> {
        let certificate =
            Certificate::from_der(certificate_der).map_err(|_| AlgorithmError::InvalidKey)?;
        let spki = certificate
            .tbs_certificate()
            .subject_public_key_info()
            .to_der()
            .map_err(|_| AlgorithmError::InvalidKey)?;
        public_key_from_spki(&spki)
    }
}

#[cfg(feature = "x509")]
pub use x509::{public_key_from_certificate, public_key_from_spki};
