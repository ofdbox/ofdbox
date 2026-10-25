use sm2::elliptic_curve::rand_core::TryCryptoRng;
use sm2::pke::{DecryptingKey, EncryptingKey};
use zeroize::Zeroizing;

use crate::error::CryptoError;
use crate::password::FILE_KEY_LEN;

/// 从 X.509 证书中提取 SEC1 编码的 SM2 公钥。
pub fn public_key_from_certificate(certificate_der: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let key = ofdbox_algo::public_key_from_certificate(certificate_der)
        .map_err(|_| CryptoError::InvalidCertificate)?;
    if key.key_bytes.is_empty() {
        return Err(CryptoError::InvalidCertificate);
    }
    Ok(key.key_bytes)
}

/// 证书方案 1.1.2：用证书公钥加密文件加密密钥，输出 SM2Cipher DER。
pub fn encrypt_file_key_with_certificate<R: TryCryptoRng>(
    certificate_der: &[u8],
    file_key: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, CryptoError> {
    if file_key.len() != FILE_KEY_LEN {
        return Err(CryptoError::InvalidKey);
    }
    let public_key = public_key_from_certificate(certificate_der)?;
    let encrypting_key =
        EncryptingKey::from_sec1_bytes(&public_key).map_err(|_| CryptoError::InvalidCertificate)?;
    encrypting_key
        .encrypt_der(rng, file_key)
        .map_err(|_| CryptoError::InvalidKey)
}

/// 证书方案 1.1.2：用 SM2 私钥解包文件加密密钥。
pub fn decrypt_file_key_with_private_key(
    private_key: &[u8],
    encrypted_work_key: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let decrypting_key =
        DecryptingKey::from_slice(private_key).map_err(|_| CryptoError::InvalidKey)?;
    let file_key = decrypting_key
        .decrypt_der(encrypted_work_key)
        .map_err(|_| CryptoError::InvalidCiphertext)?;
    if file_key.len() != FILE_KEY_LEN {
        return Err(CryptoError::InvalidKey);
    }
    Ok(Zeroizing::new(file_key))
}

#[cfg(test)]
mod tests {
    use super::{decrypt_file_key_with_private_key, encrypt_file_key_with_certificate};
    use getrandom::SysRng;
    use sm2::SecretKey;

    #[test]
    fn pke_roundtrip_with_raw_key() {
        let secret = SecretKey::from_slice(&[0x11u8; 32]).expect("secret");
        let public = secret.public_key().to_sec1_bytes();
        let encrypting = sm2::pke::EncryptingKey::from_sec1_bytes(&public).expect("public");
        let file_key = [0x42u8; 16];
        let ciphertext = encrypting
            .encrypt_der(&mut SysRng, &file_key)
            .expect("encrypt");
        let decrypted =
            decrypt_file_key_with_private_key(&secret.to_bytes(), &ciphertext).expect("decrypt");
        assert_eq!(&*decrypted, &file_key);
    }

    #[test]
    fn rejects_invalid_certificate() {
        assert!(
            encrypt_file_key_with_certificate(b"not a certificate", &[0u8; 16], &mut SysRng)
                .is_err()
        );
    }
}
