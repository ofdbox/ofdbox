use sm2::dsa::signature::Verifier;
use sm2::dsa::{Signature, VerifyingKey};

use crate::error::AlgorithmError;
use crate::key::PublicKeyRef;
use crate::oid::{SM2_CURVE_OID, SM2_SIGN_OID};

/// GB/T 32918 默认签名者标识。
pub const DEFAULT_DIST_ID: &str = "1234567812345678";

/// 签名参数。
#[derive(Clone, Copy, Debug)]
pub struct SignatureParams<'a> {
    /// SM2 签名者标识。
    pub dist_id: &'a str,
}

impl<'a> Default for SignatureParams<'a> {
    fn default() -> Self {
        Self {
            dist_id: DEFAULT_DIST_ID,
        }
    }
}

/// 原始签名算法：对消息本身验签，摘要由算法内部完成。
pub trait RawSignatureAlgorithm: Send + Sync {
    /// 验证签名。
    fn verify(
        &self,
        key: &PublicKeyRef<'_>,
        message: &[u8],
        signature: &[u8],
        params: &SignatureParams<'_>,
    ) -> Result<(), AlgorithmError>;
}

/// SM2-1 数字签名算法。
#[derive(Clone, Copy, Debug, Default)]
pub struct Sm2RawSignature;

impl RawSignatureAlgorithm for Sm2RawSignature {
    fn verify(
        &self,
        key: &PublicKeyRef<'_>,
        message: &[u8],
        signature: &[u8],
        params: &SignatureParams<'_>,
    ) -> Result<(), AlgorithmError> {
        if key.algorithm_oid != SM2_CURVE_OID && key.algorithm_oid != SM2_SIGN_OID {
            return Err(AlgorithmError::InvalidKey);
        }
        let verifying_key = VerifyingKey::from_sec1_bytes(params.dist_id, key.key_bytes)
            .map_err(|_| AlgorithmError::InvalidKey)?;
        // 兼容 DER `SEQUENCE { r, s }` 与 `r || s` 两种编码。
        if let Ok(signature) = Signature::from_der(signature) {
            return verifying_key
                .verify(message, &signature)
                .map_err(|_| AlgorithmError::InvalidSignature);
        }
        if signature.len() != 64 {
            return Err(AlgorithmError::InvalidSignature);
        }
        let r: [u8; 32] = signature[..32]
            .try_into()
            .map_err(|_| AlgorithmError::InvalidSignature)?;
        let s: [u8; 32] = signature[32..]
            .try_into()
            .map_err(|_| AlgorithmError::InvalidSignature)?;
        let signature =
            Signature::from_scalars(r, s).map_err(|_| AlgorithmError::InvalidSignature)?;
        verifying_key
            .verify(message, &signature)
            .map_err(|_| AlgorithmError::InvalidSignature)
    }
}
