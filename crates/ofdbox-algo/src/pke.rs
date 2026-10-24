//! SM2 公钥加密（SM2-3）。

use sm2::elliptic_curve::rand_core::TryCryptoRng;
use sm2::pke::{DecryptingKey, EncryptingKey};

use crate::error::AlgorithmError;

/// 用 SEC1 编码的 SM2 公钥加密，输出 SM2Cipher DER。
pub fn sm2_encrypt<R: TryCryptoRng>(
    public_key_sec1: &[u8],
    plaintext: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, AlgorithmError> {
    let encrypting =
        EncryptingKey::from_sec1_bytes(public_key_sec1).map_err(|_| AlgorithmError::InvalidKey)?;
    encrypting
        .encrypt_der(rng, plaintext)
        .map_err(|_| AlgorithmError::InvalidKey)
}

/// 用 SM2 私钥解密 SM2Cipher DER。
pub fn sm2_decrypt(private_key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, AlgorithmError> {
    let decrypting =
        DecryptingKey::from_slice(private_key).map_err(|_| AlgorithmError::InvalidKey)?;
    decrypting
        .decrypt_der(ciphertext)
        .map_err(|_| AlgorithmError::InvalidCiphertext)
}
