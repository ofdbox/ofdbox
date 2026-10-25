use cms::attr::MessageDigest;
use cms::cert::CertificateChoices;
use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use der::asn1::ObjectIdentifier;
use der::{Decode, Encode};
use ofdbox_algo::{AlgorithmRegistry, PublicKeyRef, SM2_CURVE_OID, SignatureParams};
use ofdbox_core::{OfdError, SignatureVerifier};

use crate::error::CryptoError;
use crate::signature::DEFAULT_DIST_ID;

/// PKCS#7 的 signedData 标识，兼容外来文件。
const ID_SIGNED_DATA_PKCS7: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.2");
/// GB/T 35275（沿用 GM/T 0010）的 signedData 标识。
const ID_SIGNED_DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.156.10197.6.1.4.2.2");
const ID_MESSAGE_DIGEST: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.4");

/// GB/T 35275 SignedData 验签器。
pub struct Sm2SignedDataVerifier {
    dist_id: String,
}

impl Sm2SignedDataVerifier {
    /// 使用默认签名者标识创建验签器。
    pub fn new() -> Self {
        Self {
            dist_id: DEFAULT_DIST_ID.to_owned(),
        }
    }

    /// 指定签名者标识。
    pub fn with_dist_id(mut self, dist_id: impl Into<String>) -> Self {
        self.dist_id = dist_id.into();
        self
    }
}

impl Default for Sm2SignedDataVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl SignatureVerifier for Sm2SignedDataVerifier {
    fn verify(
        &self,
        _signature_method: &str,
        content: &[u8],
        signed_value: &[u8],
        algorithms: &AlgorithmRegistry,
    ) -> Result<(), OfdError> {
        verify_signed_data_with(content, signed_value, &self.dist_id, algorithms)
            .map_err(|error| OfdError::Crypto(error.to_string()))
    }
}

/// 校验一个 GB/T 35275 SignedData（分离内容），使用默认国密算法注册表。
pub fn verify_signed_data(
    content: &[u8],
    signed_value: &[u8],
    dist_id: &str,
) -> Result<(), CryptoError> {
    verify_signed_data_with(
        content,
        signed_value,
        dist_id,
        &AlgorithmRegistry::gm_default(),
    )
}

/// 校验一个 GB/T 35275 SignedData，算法由注册表按 OID 解析。
pub fn verify_signed_data_with(
    content: &[u8],
    signed_value: &[u8],
    dist_id: &str,
    algorithms: &AlgorithmRegistry,
) -> Result<(), CryptoError> {
    let info = ContentInfo::from_der(signed_value).map_err(|_| CryptoError::InvalidEncoding)?;
    if info.content_type != ID_SIGNED_DATA && info.content_type != ID_SIGNED_DATA_PKCS7 {
        return Err(CryptoError::InvalidEncoding);
    }
    let signed_data: SignedData = info
        .content
        .decode_as()
        .map_err(|_| CryptoError::InvalidEncoding)?;
    let signer = signed_data
        .signer_infos
        .0
        .iter()
        .next()
        .ok_or(CryptoError::InvalidSignature)?;

    let digest_oid = signer.digest_alg.oid.to_string();
    let signature_oid = signer.signature_algorithm.oid.to_string();

    let attributes = signer
        .signed_attrs
        .as_ref()
        .ok_or(CryptoError::InvalidSignature)?;
    let message_digest = attributes
        .iter()
        .find(|attribute| attribute.oid == ID_MESSAGE_DIGEST)
        .ok_or(CryptoError::InvalidSignature)
        .and_then(|attribute| {
            MessageDigest::try_from(attribute).map_err(|_| CryptoError::InvalidSignature)
        })?;

    let digest = algorithms
        .digest(&digest_oid)
        .or_else(|_| {
            let suite = algorithms.suite(&digest_oid)?;
            algorithms.digest(suite.digest_oid)
        })
        .map_err(|_| CryptoError::InvalidSignature)?;
    if message_digest.as_bytes() != digest.digest(content).as_slice() {
        return Err(CryptoError::InvalidSignature);
    }

    let certificate = signed_data
        .certificates
        .as_ref()
        .and_then(|certificates| {
            certificates.0.iter().find_map(|choice| match choice {
                CertificateChoices::Certificate(certificate) => Some(certificate),
                _ => None,
            })
        })
        .ok_or(CryptoError::InvalidCertificate)?;
    let public_key = certificate
        .tbs_certificate()
        .subject_public_key_info()
        .subject_public_key
        .raw_bytes();

    let signed_bytes = attributes
        .to_der()
        .map_err(|_| CryptoError::InvalidEncoding)?;
    let key = PublicKeyRef::new(SM2_CURVE_OID, public_key);
    let params = SignatureParams { dist_id };
    let result = if algorithms.suite(&signature_oid).is_ok() {
        algorithms.verify_suite(
            &signature_oid,
            &key,
            &signed_bytes,
            signer.signature.as_bytes(),
            &params,
        )
    } else {
        algorithms.signature(&signature_oid).and_then(|algorithm| {
            algorithm.verify(&key, &signed_bytes, signer.signature.as_bytes(), &params)
        })
    };
    result.map_err(|_| CryptoError::InvalidSignature)
}

/// 从 GB/T 35275 SignedData 中提取签名者证书 DER。
pub fn signed_data_signer_certificate(signed_value: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let info = ContentInfo::from_der(signed_value).map_err(|_| CryptoError::InvalidEncoding)?;
    let signed_data: SignedData = info
        .content
        .decode_as()
        .map_err(|_| CryptoError::InvalidEncoding)?;
    let certificate = signed_data
        .certificates
        .as_ref()
        .and_then(|certificates| {
            certificates.0.iter().find_map(|choice| match choice {
                CertificateChoices::Certificate(certificate) => Some(certificate),
                _ => None,
            })
        })
        .ok_or(CryptoError::InvalidCertificate)?;
    certificate
        .to_der()
        .map_err(|_| CryptoError::InvalidCertificate)
}

#[cfg(test)]
mod tests {
    use super::{Sm2SignedDataVerifier, verify_signed_data};
    use ofdbox_algo::AlgorithmRegistry;
    use ofdbox_core::SignatureVerifier;

    #[test]
    fn rejects_non_cms_bytes() {
        let verifier = Sm2SignedDataVerifier::new();
        assert!(
            verifier
                .verify(
                    "1.2.156.10197.1.501",
                    b"content",
                    b"not cms",
                    &AlgorithmRegistry::gm_default(),
                )
                .is_err()
        );
        assert!(verify_signed_data(b"content", b"not cms", "1234567812345678").is_err());
    }
}
