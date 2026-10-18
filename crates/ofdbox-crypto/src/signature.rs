use sm2::SecretKey;
use sm2::dsa::signature::{Signer, Verifier};
use sm2::dsa::{Signature, SigningKey, VerifyingKey};

use crate::error::CryptoError;

/// GB/T 32918 默认签名者标识。
pub const DEFAULT_DIST_ID: &str = "1234567812345678";

/// SM2DSA 签名，输入消息原文，输出 ASN.1 DER 的 `SEQUENCE { r, s }`。
pub fn sign_sm3(private_key: &[u8], message: &[u8], dist_id: &str) -> Result<Vec<u8>, CryptoError> {
    let secret_key = SecretKey::from_slice(private_key).map_err(|_| CryptoError::InvalidKey)?;
    let signing_key = SigningKey::new(dist_id, &secret_key).map_err(|_| CryptoError::InvalidKey)?;
    let signature: Signature = signing_key
        .try_sign(message)
        .map_err(|_| CryptoError::InvalidSignature)?;
    Ok(signature.to_der().to_vec())
}

/// SM2DSA 验签，签名值为 ASN.1 DER 的 `SEQUENCE { r, s }`。
pub fn verify_sm3(
    public_key_sec1: &[u8],
    message: &[u8],
    signature_der: &[u8],
    dist_id: &str,
) -> Result<(), CryptoError> {
    let verifying_key = VerifyingKey::from_sec1_bytes(dist_id, public_key_sec1)
        .map_err(|_| CryptoError::InvalidKey)?;
    let signature =
        Signature::from_der(signature_der).map_err(|_| CryptoError::InvalidSignature)?;
    verifying_key
        .verify(message, &signature)
        .map_err(|_| CryptoError::InvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_DIST_ID, sign_sm3, verify_sm3};
    use sm2::SecretKey;

    #[test]
    fn sign_verify_roundtrip() {
        let secret = SecretKey::from_slice(&[0x22u8; 32]).expect("secret");
        let public = secret.public_key().to_sec1_bytes();
        let message = b"OFDBox signature";
        let signature = sign_sm3(&secret.to_bytes(), message, DEFAULT_DIST_ID).expect("sign");
        verify_sm3(&public, message, &signature, DEFAULT_DIST_ID).expect("verify");
    }

    #[test]
    fn tampered_message_fails() {
        let secret = SecretKey::from_slice(&[0x33u8; 32]).expect("secret");
        let public = secret.public_key().to_sec1_bytes();
        let signature = sign_sm3(&secret.to_bytes(), b"original", DEFAULT_DIST_ID).expect("sign");
        assert!(verify_sm3(&public, b"modified", &signature, DEFAULT_DIST_ID).is_err());
    }
}
