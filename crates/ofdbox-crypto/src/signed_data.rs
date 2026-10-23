use cms::attr::MessageDigest;
use cms::cert::CertificateChoices;
use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use der::asn1::ObjectIdentifier;
use der::{Decode, Encode};
use ofdbox_core::{OfdError, SignatureVerifier};
use sm3::{Digest, Sm3};

use crate::error::CryptoError;
use crate::signature::{DEFAULT_DIST_ID, verify_sm3};

/// PKCS#7 的 signedData 标识，兼容外来文件。
const ID_SIGNED_DATA_PKCS7: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.2");
/// GB/T 35275（沿用 GM/T 0010）的 signedData 标识。
const ID_SIGNED_DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.156.10197.6.1.4.2.2");
const ID_SM3: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.156.10197.1.401");
/// SM2 with SM3 的组合标识，OFD `SignatureMethod` 与 38540 用它。
const ID_SM2_SIGN: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.156.10197.1.501");
/// 纯 SM2 签名算法标识，35275 的 `SignerInfo.signatureAlgorithm` 用它。
const ID_SM2_SIGN_ALG: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.156.10197.1.301.1");
const ID_MESSAGE_DIGEST: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.4");

/// GB/T 35275 SignedData 的 SM2/SM3 验签器。
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
    fn supports(&self, signature_method: &str) -> bool {
        signature_method == "1.2.1"
            || signature_method == ID_SM2_SIGN.to_string()
            || signature_method == ID_SM2_SIGN_ALG.to_string()
            || signature_method.contains("SM2")
    }

    fn verify(
        &self,
        _signature_method: &str,
        content: &[u8],
        signed_value: &[u8],
    ) -> Result<(), OfdError> {
        verify_signed_data(content, signed_value, &self.dist_id)
            .map_err(|error| OfdError::Crypto(error.to_string()))
    }
}

/// 校验一个 GB/T 35275 SignedData（分离内容）。
pub fn verify_signed_data(
    content: &[u8],
    signed_value: &[u8],
    dist_id: &str,
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
    // 35275 的签名算法写纯 SM2（.301.1），部分实现写组合标识（.501），两者等价。
    if signer.digest_alg.oid != ID_SM3
        || (signer.signature_algorithm.oid != ID_SM2_SIGN
            && signer.signature_algorithm.oid != ID_SM2_SIGN_ALG)
    {
        return Err(CryptoError::InvalidSignature);
    }

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
    let actual_digest = Sm3::digest(content);
    if message_digest.as_bytes() != actual_digest.as_slice() {
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
    verify_sm3(
        public_key,
        &signed_bytes,
        signer.signature.as_bytes(),
        dist_id,
    )
}

#[cfg(test)]
mod tests {
    use super::{Sm2SignedDataVerifier, verify_signed_data};
    use ofdbox_core::SignatureVerifier;

    #[test]
    fn rejects_non_cms_bytes() {
        let verifier = Sm2SignedDataVerifier::new();
        assert!(verifier.supports("1.2.156.10197.1.501"));
        assert!(
            verifier
                .verify("1.2.156.10197.1.501", b"content", b"not cms")
                .is_err()
        );
        assert!(verify_signed_data(b"content", b"not cms", "1234567812345678").is_err());
    }
}
