//! 国产密码算法原语、OID 元数据与算法注册表。
//!
//! 本 crate 不依赖任何 `ofdbox-*` crate，可被其他国产文档格式复用。
//! 算法实现只处理字节、公钥和参数；证书链、文档结构、网络 IO 由上层负责。

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod cipher;
mod digest;
mod error;
mod kdf;
mod key;
mod oid;
mod registry;
mod role;
mod signature;
mod suite;

pub use cipher::{Cipher, Sm4Cbc};
pub use digest::{DigestAlgorithm, Sm3Digest};
pub use error::AlgorithmError;
pub use kdf::{Kdf, Sm3Kdf};
pub use key::PublicKeyRef;
pub use oid::*;
pub use registry::AlgorithmRegistry;
pub use role::AlgorithmRole;
pub use signature::{RawSignatureAlgorithm, SignatureParams, Sm2RawSignature};
pub use suite::{SignatureSuite, SuiteMode};
