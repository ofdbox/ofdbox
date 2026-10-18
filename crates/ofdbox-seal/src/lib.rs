#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! 电子印章（ESL）的结构识别与显示解析。
//!
//! 印章标准没有声明格式的字段，因此本 crate 通过有界 DER 扫描生成结构指纹，
//! 再结合印章头、属性字段和外层签名容器判断 GM/T 0031—2014 或 GB/T 38540—2020。
//!
//! 显示和验证是两条独立路径：显示只需要印章头和内嵌图片，不执行任何密码学运算；
//! 格式未知时仍可提取图片，但不得据此认为印章有效。

mod certificate;
mod der;
mod error;
mod fingerprint;
mod oids;
mod seal;
mod signature;
mod time;
mod verify;

pub use certificate::{
    CertificatePolicy, CertificateValidation, validate_certificate, validate_certificate_at,
};
pub use error::SealError;
pub use fingerprint::{OuterSignatureShape, SealFingerprint, SealFormat, TimeType};
pub use oids::SM2_SM3_SIGNATURE_OID;
pub use seal::{
    SealDisplay, SealHeader, SealPicture, SealProperty, fingerprint_seal, identify_seal_format,
    parse_seal_for_display,
};
pub use verify::{
    SealVerification, SealVerifyOptions, signer_in_seal, verify_seal, verify_seal_with_options,
};
