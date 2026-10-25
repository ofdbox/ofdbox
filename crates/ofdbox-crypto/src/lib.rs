#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! GM/T 0099—2020 所需的密码原语与 OFD 密钥处理。
//!
//! 本 crate 只负责密码运算：
//!
//! - SM3 杂凑与 GB/T 32918.3 KDF；
//! - SM4-CBC + PKCS#7；
//! - 口令方案与证书方案的文件密钥包装/解包；
//! - SM2 C1C3C2 加解密和 SM2DSA 签名/验签。
//!
//! OFD 描述结构和透明读取由 `ofdbox-core` 负责。

mod backend;
mod certificate;
mod cipher;
mod digest;
mod encrypt;
mod entries;
mod error;
mod kdf;
mod password;
mod producer;
mod signature;
mod signed_data;

pub use backend::{CERTIFICATE_CASE_ID, CertificateBackend, PASSWORD_CASE_ID, PasswordBackend};
pub use certificate::{
    decrypt_file_key_with_private_key, encrypt_file_key_with_certificate,
    public_key_from_certificate,
};
pub use cipher::{decrypt_cbc, encrypt_cbc};
pub use digest::sm3_digest;
#[cfg(not(target_arch = "wasm32"))]
pub use encrypt::encrypt_package;
pub use encrypt::{
    EncryptOptions, EncryptionFilter, EncryptionRecipient, EncryptionScheme,
    encrypt_package_with_rng,
};
pub use entries::write_ofd_entries_encrypted;
pub use error::CryptoError;
pub use kdf::sm3_kdf;
pub use password::{
    FILE_KEY_LEN, PASSWORD_KEK_LEN, derive_password_kek, unwrap_file_key, wrap_file_key,
};
pub use producer::Sm2SignedDataProducer;
pub use signature::{DEFAULT_DIST_ID, sign_sm3, verify_sm3, verify_sm3_raw};
pub use signed_data::{Sm2SignedDataVerifier, verify_signed_data};
