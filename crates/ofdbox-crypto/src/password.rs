use zeroize::Zeroizing;

use crate::cipher::{decrypt_cbc, encrypt_cbc};
use crate::error::CryptoError;
use crate::kdf::sm3_kdf;

/// 文件加密密钥长度。
pub const FILE_KEY_LEN: usize = 16;
/// 口令派生密钥长度。
pub const PASSWORD_KEK_LEN: usize = 16;

/// 口令方案 1.1.1：由口令派生 KEK。
pub fn derive_password_kek(password: &str) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(sm3_kdf(password.as_bytes(), PASSWORD_KEK_LEN))
}

/// 口令方案 1.1.1：用 KEK + IV 解包文件加密密钥。
pub fn unwrap_file_key(
    password: &str,
    iv: &[u8],
    encrypted_work_key: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let kek = derive_password_kek(password);
    let fek = decrypt_cbc(&kek, iv, encrypted_work_key)?;
    if fek.len() != FILE_KEY_LEN {
        return Err(CryptoError::InvalidKey);
    }
    Ok(Zeroizing::new(fek))
}

/// 口令方案 1.1.1：用 KEK + IV 包装文件加密密钥。
pub fn wrap_file_key(password: &str, iv: &[u8], file_key: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if file_key.len() != FILE_KEY_LEN {
        return Err(CryptoError::InvalidKey);
    }
    let kek = derive_password_kek(password);
    encrypt_cbc(&kek, iv, file_key)
}

#[cfg(test)]
mod tests {
    use super::{derive_password_kek, unwrap_file_key, wrap_file_key};

    #[test]
    fn kek_matches_sample() {
        let kek = derive_password_kek("12345678");
        assert_eq!(
            kek.iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "34d6effbd5bdc7c8020f619bfd8303b5"
        );
    }

    #[test]
    fn wrap_unwrap_roundtrip() {
        let iv = [0x5au8; 16];
        let fek = [0x3cu8; 16];
        let wrapped = wrap_file_key("12345678", &iv, &fek).expect("wrap");
        let unwrapped = unwrap_file_key("12345678", &iv, &wrapped).expect("unwrap");
        assert_eq!(&*unwrapped, &fek);
    }

    #[test]
    fn wrong_password_fails_or_differs() {
        let iv = [0u8; 16];
        let fek = [7u8; 16];
        let wrapped = wrap_file_key("correct", &iv, &fek).expect("wrap");
        if let Ok(key) = unwrap_file_key("wrong", &iv, &wrapped) {
            assert_ne!(&*key, &fek);
        }
    }
}
