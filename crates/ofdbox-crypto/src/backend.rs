use std::io::Write;

use base64::Engine;
use ofdbox_core::{CryptoBackend, DecryptSeedView, EncryptInfoView, FileKey, OfdError};
use zeroize::Zeroizing;

/// 口令方案标识。
pub const PASSWORD_CASE_ID: &str = "1.1.1";
/// 证书方案标识。
pub const CERTIFICATE_CASE_ID: &str = "1.1.2";
/// 密钥描述文件缺省 IV：16 个 0（0099 附录 C.1）。
const DEFAULT_IV: [u8; 16] = [0u8; 16];

/// 口令解密后端。
pub struct PasswordBackend {
    password: Zeroizing<String>,
    username: Option<String>,
}

impl PasswordBackend {
    /// 创建口令后端，匹配密钥描述文件中的第一个可解密用户。
    pub fn new(password: impl Into<String>) -> Self {
        Self {
            password: Zeroizing::new(password.into()),
            username: None,
        }
    }

    /// 只匹配指定用户名的 `UserInfo`。
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }
}

impl CryptoBackend for PasswordBackend {
    fn supports(&self, scheme_id: &str) -> bool {
        scheme_id == PASSWORD_CASE_ID
    }

    fn unwrap_file_key(
        &self,
        _operation: &EncryptInfoView,
        seed: &DecryptSeedView,
    ) -> Result<FileKey, OfdError> {
        check_case_id(seed, PASSWORD_CASE_ID)?;
        for user in seed.users() {
            if let Some(username) = &self.username
                && user.user_name() != Some(username.as_str())
            {
                continue;
            }
            let Some(wrapped) = user.encrypted_work_key() else {
                continue;
            };
            let iv = match user.iv_value() {
                Some(value) => decode_base64(value)?,
                None => DEFAULT_IV.to_vec(),
            };
            let wrapped = decode_base64(wrapped)?;
            if let Ok(file_key) =
                crate::password::unwrap_file_key(self.password.as_str(), &iv, &wrapped)
            {
                return Ok(FileKey::new(file_key.to_vec(), iv).with_scheme(PASSWORD_CASE_ID));
            }
        }
        Err(OfdError::Crypto("口令无法解出文件密钥".to_owned()))
    }

    fn decrypt(&self, key: &FileKey, data: &[u8]) -> Result<Vec<u8>, OfdError> {
        crate::cipher::decrypt_cbc(key.key(), key.iv(), data)
            .map_err(|_| OfdError::Crypto("SM4 解密失败".to_owned()))
    }

    fn decrypt_to(
        &self,
        key: &FileKey,
        data: &[u8],
        writer: &mut dyn Write,
    ) -> Result<(), OfdError> {
        crate::cipher::decrypt_cbc_to_writer(key.key(), key.iv(), data, writer)
            .map_err(|_| OfdError::Crypto("SM4 解密失败".to_owned()))
    }

    fn digest(&self, method: &str, data: &[u8]) -> Result<Vec<u8>, OfdError> {
        digest(method, data)
    }
}

/// 证书解密后端。
pub struct CertificateBackend {
    private_key: Zeroizing<Vec<u8>>,
    username: Option<String>,
}

impl CertificateBackend {
    /// 创建证书后端，私钥为 32 字节 SM2 标量。
    pub fn new(private_key: impl Into<Vec<u8>>) -> Self {
        Self {
            private_key: Zeroizing::new(private_key.into()),
            username: None,
        }
    }

    /// 只匹配指定用户名的 `UserInfo`。
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }
}

impl CryptoBackend for CertificateBackend {
    fn supports(&self, scheme_id: &str) -> bool {
        scheme_id == CERTIFICATE_CASE_ID
    }

    fn unwrap_file_key(
        &self,
        _operation: &EncryptInfoView,
        seed: &DecryptSeedView,
    ) -> Result<FileKey, OfdError> {
        check_case_id(seed, CERTIFICATE_CASE_ID)?;
        for user in seed.users() {
            if let Some(username) = &self.username
                && user.user_name() != Some(username.as_str())
            {
                continue;
            }
            let Some(wrapped) = user.encrypted_work_key() else {
                continue;
            };
            let iv = match user.iv_value() {
                Some(value) => decode_base64(value)?,
                None => DEFAULT_IV.to_vec(),
            };
            let wrapped = decode_base64(wrapped)?;
            if let Ok(file_key) = crate::certificate::decrypt_file_key_with_private_key(
                self.private_key.as_slice(),
                &wrapped,
            ) {
                return Ok(FileKey::new(file_key.to_vec(), iv).with_scheme(PASSWORD_CASE_ID));
            }
        }
        Err(OfdError::Crypto("证书私钥无法解出文件密钥".to_owned()))
    }

    fn decrypt(&self, key: &FileKey, data: &[u8]) -> Result<Vec<u8>, OfdError> {
        crate::cipher::decrypt_cbc(key.key(), key.iv(), data)
            .map_err(|_| OfdError::Crypto("SM4 解密失败".to_owned()))
    }

    fn decrypt_to(
        &self,
        key: &FileKey,
        data: &[u8],
        writer: &mut dyn Write,
    ) -> Result<(), OfdError> {
        crate::cipher::decrypt_cbc_to_writer(key.key(), key.iv(), data, writer)
            .map_err(|_| OfdError::Crypto("SM4 解密失败".to_owned()))
    }

    fn digest(&self, method: &str, data: &[u8]) -> Result<Vec<u8>, OfdError> {
        digest(method, data)
    }
}

fn digest(method: &str, data: &[u8]) -> Result<Vec<u8>, OfdError> {
    use md5::Md5;
    use sha1::Sha1;
    use sm3::{Digest, Sm3};
    // 真实文件的 CheckMethod 可能是 OID 或大小写变体，先归一化。
    let method = ofdbox_core::canonical_digest_method(method).unwrap_or(method);
    match method {
        "SM3" => Ok(Sm3::digest(data).to_vec()),
        // MD5/SHA1 只用于校验历史文档的 References，不用于新签名。
        "MD5" => Ok(Md5::digest(data).to_vec()),
        "SHA1" => Ok(Sha1::digest(data).to_vec()),
        other => Err(OfdError::UnsupportedDigest(other.to_owned())),
    }
}

fn check_case_id(seed: &DecryptSeedView, expected: &str) -> Result<(), OfdError> {
    if let Some(case_id) = seed.encrypt_case_id()
        && case_id != expected
    {
        return Err(OfdError::Crypto(format!("加密方案标识不匹配: {case_id}")));
    }
    Ok(())
}

fn decode_base64(value: &str) -> Result<Vec<u8>, OfdError> {
    base64::engine::general_purpose::STANDARD
        .decode(value.trim())
        .map_err(|_| OfdError::Crypto("密钥描述 Base64 无效".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{CertificateBackend, PasswordBackend};
    use ofdbox_core::CryptoBackend;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn sm3_digest_vector() {
        let digest = PasswordBackend::new("unused")
            .digest("SM3", b"abc")
            .expect("digest");
        assert_eq!(
            hex(&digest),
            "66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0"
        );
    }

    #[test]
    fn legacy_digest_vectors() {
        let backend = PasswordBackend::new("unused");
        assert_eq!(
            hex(&backend.digest("MD5", b"abc").expect("md5")),
            "900150983cd24fb0d6963f7d28e17f72"
        );
        assert_eq!(
            hex(&backend.digest("SHA1", b"abc").expect("sha1")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn digest_method_aliases_are_normalized() {
        let backend = PasswordBackend::new("unused");
        let expected = backend.digest("SM3", b"data").expect("sm3");
        for alias in [
            "sm3",
            "1.2.156.10197.1.401",
            "1.2.156.10197.1.401.1",
            "1.2.156.10197.1.401.2",
        ] {
            assert_eq!(
                backend.digest(alias, b"data").expect(alias),
                expected,
                "{alias}"
            );
        }
    }

    #[test]
    fn certificate_backend_rejects_unsupported_digest() {
        assert!(matches!(
            CertificateBackend::new(vec![0u8; 32]).digest("SHA256", b"data"),
            Err(ofdbox_core::OfdError::UnsupportedDigest(_))
        ));
    }
}
