use cbc::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7};
use sm4::cipher::{Block, BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
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

/// SM4-ECB，PKCS#7 填充。
#[derive(Clone, Copy, Debug, Default)]
pub struct Sm4Ecb;

impl Cipher for Sm4Ecb {
    fn encrypt(
        &self,
        key: &[u8],
        _iv: Option<&[u8]>,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, AlgorithmError> {
        let cipher = sm4::Sm4::new_from_slice(key).map_err(|_| AlgorithmError::InvalidKey)?;
        let mut buffer = pkcs7_pad(plaintext);
        for chunk in buffer.chunks_mut(BLOCK_SIZE) {
            let mut block = Block::<sm4::Sm4>::default();
            block.copy_from_slice(chunk);
            cipher.encrypt_block(&mut block);
            chunk.copy_from_slice(&block);
        }
        Ok(buffer)
    }

    fn decrypt(
        &self,
        key: &[u8],
        _iv: Option<&[u8]>,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, AlgorithmError> {
        if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(BLOCK_SIZE) {
            return Err(AlgorithmError::InvalidCiphertext);
        }
        let cipher = sm4::Sm4::new_from_slice(key).map_err(|_| AlgorithmError::InvalidKey)?;
        let mut buffer = ciphertext.to_vec();
        for chunk in buffer.chunks_mut(BLOCK_SIZE) {
            let mut block = Block::<sm4::Sm4>::default();
            block.copy_from_slice(chunk);
            cipher.decrypt_block(&mut block);
            chunk.copy_from_slice(&block);
        }
        pkcs7_unpad(&buffer)
    }
}

/// SM4-CTR，按 128 位大端计数器递增。
#[derive(Clone, Copy, Debug, Default)]
pub struct Sm4Ctr;

impl Cipher for Sm4Ctr {
    fn encrypt(
        &self,
        key: &[u8],
        iv: Option<&[u8]>,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, AlgorithmError> {
        ctr_xor(key, iv, plaintext)
    }

    fn decrypt(
        &self,
        key: &[u8],
        iv: Option<&[u8]>,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, AlgorithmError> {
        ctr_xor(key, iv, ciphertext)
    }
}

fn ctr_xor(key: &[u8], iv: Option<&[u8]>, data: &[u8]) -> Result<Vec<u8>, AlgorithmError> {
    let iv = iv.ok_or(AlgorithmError::InvalidParameters)?;
    if key.len() != BLOCK_SIZE || iv.len() != BLOCK_SIZE {
        return Err(AlgorithmError::InvalidKey);
    }
    let cipher = sm4::Sm4::new_from_slice(key).map_err(|_| AlgorithmError::InvalidKey)?;
    let mut counter = [0u8; BLOCK_SIZE];
    counter.copy_from_slice(iv);
    let mut output = Vec::with_capacity(data.len());
    for chunk in data.chunks(BLOCK_SIZE) {
        let mut block = Block::<sm4::Sm4>::default();
        block.copy_from_slice(&counter);
        cipher.encrypt_block(&mut block);
        output.extend(
            chunk
                .iter()
                .zip(block.iter())
                .map(|(plain, key)| plain ^ key),
        );
        for byte in counter.iter_mut().rev() {
            let (next, overflow) = byte.overflowing_add(1);
            *byte = next;
            if !overflow {
                break;
            }
        }
    }
    Ok(output)
}

fn pkcs7_pad(data: &[u8]) -> Vec<u8> {
    let padding = BLOCK_SIZE - data.len() % BLOCK_SIZE;
    let mut output = Vec::with_capacity(data.len() + padding);
    output.extend_from_slice(data);
    output.extend(std::iter::repeat_n(padding as u8, padding));
    output
}

fn pkcs7_unpad(data: &[u8]) -> Result<Vec<u8>, AlgorithmError> {
    let padding = *data.last().ok_or(AlgorithmError::InvalidCiphertext)? as usize;
    if padding == 0 || padding > BLOCK_SIZE || padding > data.len() {
        return Err(AlgorithmError::InvalidCiphertext);
    }
    if !data[data.len() - padding..]
        .iter()
        .all(|byte| *byte as usize == padding)
    {
        return Err(AlgorithmError::InvalidCiphertext);
    }
    Ok(data[..data.len() - padding].to_vec())
}
