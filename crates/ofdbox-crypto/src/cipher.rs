use std::io::Write;

use cbc::cipher::{Block, BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7};
use zeroize::Zeroizing;

use crate::error::CryptoError;

const BLOCK_SIZE: usize = 16;

type Encryptor = cbc::Encryptor<sm4::Sm4>;
type Decryptor = cbc::Decryptor<sm4::Sm4>;

/// SM4-CBC 加密，PKCS#7 填充。
pub fn encrypt_cbc(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    check_key_iv(key, iv)?;
    let encryptor = Encryptor::new_from_slices(key, iv).map_err(|_| CryptoError::InvalidKey)?;
    let mut buffer = Zeroizing::new(vec![0u8; plaintext.len() + BLOCK_SIZE]);
    buffer[..plaintext.len()].copy_from_slice(plaintext);
    let encrypted = encryptor
        .encrypt_padded::<Pkcs7>(&mut buffer, plaintext.len())
        .map_err(|_| CryptoError::InvalidCiphertext)?;
    Ok(encrypted.to_vec())
}

/// SM4-CBC 解密，PKCS#7 去填充。
pub fn decrypt_cbc(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    check_key_iv(key, iv)?;
    if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(BLOCK_SIZE) {
        return Err(CryptoError::InvalidCiphertext);
    }
    let decryptor = Decryptor::new_from_slices(key, iv).map_err(|_| CryptoError::InvalidKey)?;
    let mut buffer = Zeroizing::new(ciphertext.to_vec());
    let decrypted = decryptor
        .decrypt_padded::<Pkcs7>(&mut buffer)
        .map_err(|_| CryptoError::InvalidCiphertext)?;
    Ok(decrypted.to_vec())
}

/// SM4-CBC 解密并流式写入 sink，仅保留一个小块缓冲。
pub fn decrypt_cbc_to_writer(
    key: &[u8],
    iv: &[u8],
    ciphertext: &[u8],
    writer: &mut dyn Write,
) -> Result<(), CryptoError> {
    check_key_iv(key, iv)?;
    if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(BLOCK_SIZE) {
        return Err(CryptoError::InvalidCiphertext);
    }
    let mut decryptor = Decryptor::new_from_slices(key, iv).map_err(|_| CryptoError::InvalidKey)?;
    let mut offset = 0;
    let mut output = Vec::with_capacity(4096);
    let mut block = Block::<Decryptor>::default();
    while ciphertext.len() - offset > BLOCK_SIZE {
        let available = ciphertext.len() - offset - BLOCK_SIZE;
        let take = (available.min(4096) / BLOCK_SIZE) * BLOCK_SIZE;
        if take == 0 {
            break;
        }
        for chunk in ciphertext[offset..offset + take]
            .as_chunks::<BLOCK_SIZE>()
            .0
        {
            block.copy_from_slice(chunk);
            decryptor.decrypt_block(&mut block);
            output.extend_from_slice(&block);
        }
        writer.write_all(&output)?;
        output.clear();
        offset += take;
    }
    let mut last = ciphertext[offset..].to_vec();
    let plaintext = decryptor
        .decrypt_padded::<Pkcs7>(&mut last)
        .map_err(|_| CryptoError::InvalidCiphertext)?;
    writer.write_all(plaintext)?;
    Ok(())
}

fn check_key_iv(key: &[u8], iv: &[u8]) -> Result<(), CryptoError> {
    if key.len() != BLOCK_SIZE || iv.len() != BLOCK_SIZE {
        return Err(CryptoError::InvalidKey);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{decrypt_cbc, encrypt_cbc};

    #[test]
    fn roundtrip() {
        let key = [0x11u8; 16];
        let iv = [0x22u8; 16];
        let plaintext = b"OFDBox SM4-CBC test";
        let ciphertext = encrypt_cbc(&key, &iv, plaintext).expect("encrypt");
        assert_eq!(ciphertext.len() % 16, 0);
        let decrypted = decrypt_cbc(&key, &iv, &ciphertext).expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn rejects_wrong_key() {
        let ciphertext = encrypt_cbc(&[1u8; 16], &[2u8; 16], b"data").expect("encrypt");
        assert!(decrypt_cbc(&[3u8; 16], &[2u8; 16], &ciphertext).is_err());
    }
}

#[cfg(test)]
mod streaming_tests {
    use super::{decrypt_cbc_to_writer, encrypt_cbc};

    #[test]
    fn streaming_decrypt_matches_buffered() {
        let key = [0x11u8; 16];
        let iv = [0x22u8; 16];
        let plaintext: Vec<u8> = (0..10_000).map(|index| (index % 251) as u8).collect();
        let ciphertext = encrypt_cbc(&key, &iv, &plaintext).expect("encrypt");
        let mut output = Vec::new();
        decrypt_cbc_to_writer(&key, &iv, &ciphertext, &mut output).expect("decrypt");
        assert_eq!(output, plaintext);
    }
}
