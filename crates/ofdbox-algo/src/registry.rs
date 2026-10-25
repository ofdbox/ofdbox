use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::cipher::{Cipher, Sm4Cbc, Sm4Ctr, Sm4Ecb};
use crate::digest::{DigestAlgorithm, Sm3Digest};
use crate::error::AlgorithmError;
use crate::kdf::{Kdf, Sm3Kdf};
use crate::key::PublicKeyRef;
use crate::mac::{HmacSm3, Mac};
use crate::oid::{
    SM2_SIGN_OID, SM2_SM3_SIGN_OID, SM3_HMAC_OID, SM3_KDF_ID, SM3_OID, SM4_CBC_OID, SM4_CTR_OID,
    SM4_ECB_OID,
};
use crate::signature::{RawSignatureAlgorithm, SignatureParams, Sm2RawSignature};
use crate::suite::{SignatureSuite, SuiteMode};

/// 算法注册表：按 `(role, OID)` 查找实现。
#[derive(Default)]
pub struct AlgorithmRegistry {
    digests: HashMap<String, Arc<dyn DigestAlgorithm>>,
    signatures: HashMap<String, Arc<dyn RawSignatureAlgorithm>>,
    suites: HashMap<String, SignatureSuite>,
    ciphers: HashMap<String, Arc<dyn Cipher>>,
    kdfs: HashMap<String, Arc<dyn Kdf>>,
    macs: HashMap<String, Arc<dyn Mac>>,
}

impl AlgorithmRegistry {
    /// 创建空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 国密默认注册表：SM3、SM2-1、SM2+SM3、SM4-CBC、SM3-KDF。
    pub fn gm_default() -> Self {
        Self::new()
            .with_digest(SM3_OID, Sm3Digest)
            .with_signature(SM2_SIGN_OID, Sm2RawSignature)
            .with_signature(SM2_SM3_SIGN_OID, Sm2RawSignature)
            .with_suite(SignatureSuite {
                oid: SM2_SM3_SIGN_OID,
                digest_oid: SM3_OID,
                signature_oid: SM2_SIGN_OID,
                mode: SuiteMode::Integrated,
            })
            .with_cipher(SM4_CBC_OID, Sm4Cbc)
            .with_cipher(SM4_ECB_OID, Sm4Ecb)
            .with_cipher(SM4_CTR_OID, Sm4Ctr)
            .with_kdf(SM3_KDF_ID, Sm3Kdf)
            .with_mac(SM3_HMAC_OID, HmacSm3)
    }

    /// 注册摘要算法。
    pub fn with_digest(
        mut self,
        oid: impl Into<String>,
        algorithm: impl DigestAlgorithm + 'static,
    ) -> Self {
        self.digests.insert(oid.into(), Arc::new(algorithm));
        self
    }

    /// 注册原始签名算法。
    pub fn with_signature(
        mut self,
        oid: impl Into<String>,
        algorithm: impl RawSignatureAlgorithm + 'static,
    ) -> Self {
        self.signatures.insert(oid.into(), Arc::new(algorithm));
        self
    }

    /// 注册复合签名算法。
    pub fn with_suite(mut self, suite: SignatureSuite) -> Self {
        self.suites.insert(suite.oid.to_owned(), suite);
        self
    }

    /// 注册对称密码算法。
    pub fn with_cipher(mut self, oid: impl Into<String>, algorithm: impl Cipher + 'static) -> Self {
        self.ciphers.insert(oid.into(), Arc::new(algorithm));
        self
    }

    /// 注册 KDF。
    pub fn with_kdf(mut self, id: impl Into<String>, algorithm: impl Kdf + 'static) -> Self {
        self.kdfs.insert(id.into(), Arc::new(algorithm));
        self
    }

    /// 注册 MAC。
    pub fn with_mac(mut self, oid: impl Into<String>, algorithm: impl Mac + 'static) -> Self {
        self.macs.insert(oid.into(), Arc::new(algorithm));
        self
    }

    /// 查找摘要算法。
    pub fn digest(&self, oid: &str) -> Result<&dyn DigestAlgorithm, AlgorithmError> {
        self.digests
            .get(oid)
            .map(Arc::as_ref)
            .ok_or_else(|| self.mismatch_or_unknown(oid, "digest"))
    }

    /// 查找原始签名算法。
    pub fn signature(&self, oid: &str) -> Result<&dyn RawSignatureAlgorithm, AlgorithmError> {
        self.signatures
            .get(oid)
            .map(Arc::as_ref)
            .ok_or_else(|| self.mismatch_or_unknown(oid, "signature"))
    }

    /// 查找复合签名算法。
    pub fn suite(&self, oid: &str) -> Result<&SignatureSuite, AlgorithmError> {
        self.suites
            .get(oid)
            .ok_or_else(|| self.mismatch_or_unknown(oid, "signature-suite"))
    }

    /// 查找对称密码算法。
    pub fn cipher(&self, oid: &str) -> Result<&dyn Cipher, AlgorithmError> {
        self.ciphers
            .get(oid)
            .map(Arc::as_ref)
            .ok_or_else(|| self.mismatch_or_unknown(oid, "cipher"))
    }

    /// 查找 KDF。
    pub fn kdf(&self, id: &str) -> Result<&dyn Kdf, AlgorithmError> {
        self.kdfs
            .get(id)
            .map(Arc::as_ref)
            .ok_or_else(|| self.mismatch_or_unknown(id, "kdf"))
    }

    /// 查找 MAC。
    pub fn mac(&self, oid: &str) -> Result<&dyn Mac, AlgorithmError> {
        self.macs
            .get(oid)
            .map(Arc::as_ref)
            .ok_or_else(|| self.mismatch_or_unknown(oid, "mac"))
    }

    /// 按复合算法 OID 验签。
    pub fn verify_suite(
        &self,
        suite_oid: &str,
        key: &PublicKeyRef<'_>,
        message: &[u8],
        signature: &[u8],
        params: &SignatureParams<'_>,
    ) -> Result<(), AlgorithmError> {
        let suite = self.suite(suite_oid)?;
        let signature_algorithm = self.signature(suite.signature_oid)?;
        match suite.mode {
            SuiteMode::Integrated => signature_algorithm.verify(key, message, signature, params),
            SuiteMode::Prehashed => {
                let digest = self.digest(suite.digest_oid)?;
                signature_algorithm.verify(key, &digest.digest(message), signature, params)
            }
        }
    }

    /// 按复合算法 OID 生成签名。
    pub fn sign_suite(
        &self,
        suite_oid: &str,
        key: &crate::key::SecretKeyRef<'_>,
        message: &[u8],
        params: &SignatureParams<'_>,
    ) -> Result<Vec<u8>, AlgorithmError> {
        let suite = self.suite(suite_oid)?;
        let signature_algorithm = self.signature(suite.signature_oid)?;
        match suite.mode {
            SuiteMode::Integrated => signature_algorithm.sign(key, message, params),
            SuiteMode::Prehashed => {
                let digest = self.digest(suite.digest_oid)?;
                signature_algorithm.sign(key, &digest.digest(message), params)
            }
        }
    }

    fn mismatch_or_unknown(&self, oid: &str, expected: &'static str) -> AlgorithmError {
        let actual = if self.digests.contains_key(oid) {
            Some("digest")
        } else if self.signatures.contains_key(oid) {
            Some("signature")
        } else if self.suites.contains_key(oid) {
            Some("signature-suite")
        } else if self.ciphers.contains_key(oid) {
            Some("cipher")
        } else if self.kdfs.contains_key(oid) {
            Some("kdf")
        } else if self.macs.contains_key(oid) {
            Some("mac")
        } else {
            None
        };
        match actual {
            Some(actual) => AlgorithmError::RoleMismatch {
                oid: oid.to_owned(),
                expected,
                actual,
            },
            None => AlgorithmError::UnknownAlgorithm {
                oid: oid.to_owned(),
                role: expected,
            },
        }
    }
}

impl fmt::Debug for AlgorithmRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AlgorithmRegistry")
            .field("digests", &self.digests.len())
            .field("signatures", &self.signatures.len())
            .field("suites", &self.suites.len())
            .field("ciphers", &self.ciphers.len())
            .field("kdfs", &self.kdfs.len())
            .field("macs", &self.macs.len())
            .finish()
    }
}
