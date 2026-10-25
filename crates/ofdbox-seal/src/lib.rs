#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! 电子印章（ESL）的结构识别、制章、显示解析与验签。
//!
//! 印章标准没有声明格式的字段，因此本 crate 通过有界 DER 扫描生成结构指纹，
//! 再结合印章头、属性字段和外层签名容器判断 GM/T 0031—2014 或 GB/T 38540—2020。
//!
//! 模块按标准拆分：`eseal` 放两标准共用的容器、解析、制章入口与验章；
//! `gbt38540` 与 `gmt0031` 只放各自的标准特有结构与编码。

mod certificate;
mod der;
mod error;
mod eseal;
mod fingerprint;
mod gbt38540;
mod gmt0031;
mod oids;
mod report;
mod signature;
mod time;
mod timestamp;

pub use certificate::{
    CertificatePolicy, CertificateValidation, validate_certificate, validate_certificate_at,
};
pub use error::SealError;
pub use eseal::{
    SealBuilder, SealDisplay, SealHeader, SealPicture, SealPictureInput, SealProperty, SealSpec,
    SealStandard, SealVerification, SealVerifyOptions, fingerprint_seal, identify_seal_format,
    parse_seal_for_display, signer_in_seal, verify_seal, verify_seal_with_options,
};
pub use fingerprint::{OuterSignatureShape, SealFingerprint, SealFormat, TimeType};
pub use gbt38540::{
    SesSignature, SesSignatureVerifier, SesVerification, SesVerifyOptions, Sm2SesSignatureProducer,
    parse_ses_signature, verify_ses_signature,
};
pub use gmt0031::sign::{
    GmT0031Signature, GmT0031SignatureVerifier, GmT0031Verification, GmT0031VerifyOptions,
    Sm2GmT0031SignatureProducer, parse_gmt0031_signature, parse_time_info,
    verify_gmt0031_signature,
};
pub use oids::SM2_SM3_SIGNATURE_OID;
pub use report::{
    CertificateRole, CertificateState, CertificateStatus, ReferenceStatus, ReportOptions,
    ReportVerdict, SignatureReport, SignatureState, SignatureStatus, SignatureSummary,
    TimestampStatus, verify_signature_report,
};
pub use timestamp::{
    TimeStampRequest, TimeStampResponse, TimeStampToken, TimeStampVerification,
    TimeStampVerifyOptions, attach_time_stamp, parse_time_stamp_request, parse_time_stamp_response,
    parse_time_stamp_token, verify_time_stamp_token,
};
