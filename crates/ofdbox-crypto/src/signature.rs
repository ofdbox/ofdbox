use ofdbox_algo::{
    PublicKeyRef, RawSignatureAlgorithm, SM2_CURVE_OID, SignatureParams, Sm2RawSignature,
};
use sm2::SecretKey;
use sm2::dsa::signature::Signer;
use sm2::dsa::{Signature, SigningKey};

use crate::error::CryptoError;

pub use ofdbox_algo::DEFAULT_DIST_ID;

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
    verify_with(public_key_sec1, message, signature_der, dist_id)
}

/// SM2DSA 验签，签名值为 `r || s` 各 32 字节的原始编码。
pub fn verify_sm3_raw(
    public_key_sec1: &[u8],
    message: &[u8],
    signature: &[u8],
    dist_id: &str,
) -> Result<(), CryptoError> {
    if signature.len() != 64 {
        return Err(CryptoError::InvalidSignature);
    }
    verify_with(public_key_sec1, message, signature, dist_id)
}

fn verify_with(
    public_key_sec1: &[u8],
    message: &[u8],
    signature: &[u8],
    dist_id: &str,
) -> Result<(), CryptoError> {
    let key = PublicKeyRef::new(SM2_CURVE_OID, public_key_sec1);
    let params = SignatureParams { dist_id };
    Sm2RawSignature
        .verify(&key, message, signature, &params)
        .map_err(|_| CryptoError::InvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_DIST_ID, sign_sm3, verify_sm3, verify_sm3_raw};
    use sm2::SecretKey;
    use sm2::dsa::Signature;

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

    #[test]
    fn raw_signature_encoding_roundtrip() {
        let secret = SecretKey::from_slice(&[0x55u8; 32]).expect("secret");
        let public = secret.public_key().to_sec1_bytes();
        let message = b"OFDBox raw signature";
        let der = sign_sm3(&secret.to_bytes(), message, DEFAULT_DIST_ID).expect("sign");
        let signature = Signature::from_der(&der).expect("der signature");
        verify_sm3_raw(&public, message, &signature.to_bytes(), DEFAULT_DIST_ID)
            .expect("verify raw");
        assert!(verify_sm3_raw(&public, message, &der, DEFAULT_DIST_ID).is_err());
    }
}
