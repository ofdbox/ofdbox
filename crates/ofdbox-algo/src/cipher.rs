use cbc::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7};
use zeroize::Zeroizing;

use crate::error::AlgorithmError;

const BLOCK_SIZE: usize = 16;

/// 对称密码算法。
pub trait Cipher: Send + Sync {
    /// 加密。
    fn encrypt(
        &self,
        key: &[u8],
        iv: Option<&[u8]>,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, AlgorithmError>;

    /// 解密。
    fn decrypt(
        &self,
        key: &[u8],
        iv: Option<&[u8]>,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, AlgorithmError>;
}

/// SM4-CBC，PKCS#7 填充。
#[derive(Clone, Copy, Debug, Default)]
pub struct Sm4Cbc;

type Encryptor = cbc::Encryptor<sm4::Sm4>;
type Decryptor = cbc::Decryptor<sm4::Sm4>;

impl Cipher for Sm4Cbc {
    fn encrypt(
        &self,
        key: &[u8],
        iv: Option<&[u8]>,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, AlgorithmError> {
        let iv = iv.ok_or(AlgorithmError::InvalidParameters)?;
        check_key_iv(key, iv)?;
        let encryptor =
            Encryptor::new_from_slices(key, iv).map_err(|_| AlgorithmError::InvalidKey)?;
        let mut buffer = Zeroizing::new(vec![0u8; plaintext.len() + BLOCK_SIZE]);
        buffer[..plaintext.len()].copy_from_slice(plaintext);
        let encrypted = encryptor
            .encrypt_padded::<Pkcs7>(&mut buffer, plaintext.len())
            .map_err(|_| AlgorithmError::InvalidCiphertext)?;
        Ok(encrypted.to_vec())
    }

    fn decrypt(
        &self,
        key: &[u8],
        iv: Option<&[u8]>,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, AlgorithmError> {
        let iv = iv.ok_or(AlgorithmError::InvalidParameters)?;
        check_key_iv(key, iv)?;
        if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(BLOCK_SIZE) {
            return Err(AlgorithmError::InvalidCiphertext);
        }
        let decryptor =
            Decryptor::new_from_slices(key, iv).map_err(|_| AlgorithmError::InvalidKey)?;
        let mut buffer = Zeroizing::new(ciphertext.to_vec());
        let decrypted = decryptor
            .decrypt_padded::<Pkcs7>(&mut buffer)
            .map_err(|_| AlgorithmError::InvalidCiphertext)?;
        Ok(decrypted.to_vec())
    }
}

fn check_key_iv(key: &[u8], iv: &[u8]) -> Result<(), AlgorithmError> {
    if key.len() != BLOCK_SIZE || iv.len() != BLOCK_SIZE {
        return Err(AlgorithmError::InvalidKey);
    }
    Ok(())
}
