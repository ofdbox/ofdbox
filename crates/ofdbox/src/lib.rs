#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! OFDBox 对外门面。
//!
//! 根模块提供常用文档、加密与签章类型；完整 API 仍可通过
//! [`core`]、[`crypto`]、[`seal`] 子模块访问。
//!
//! 页面渲染与格式转换已迁至独立的 `ofdbox-folio`，批量转换命令行
//! 位于 `ofdbox-reader` 仓库。

pub use ofdbox_algo as algo;
pub use ofdbox_core as core;
pub use ofdbox_core::model;
pub use ofdbox_core::model::document::{Document, PageEntry};
pub use ofdbox_core::{
    AnnotationEditReport, AnnotationEditor, AnnotationSpec, CacheOptions, CreateOptions,
    DEFAULT_SIGNATURE_METHOD, EntriesVerification, EntriesWriteOptions, EntriesWriteReport,
    ObjectContext, OfdError, OfdFile, OfdPackage, OfdWriter, OpenOptions, OpenStorage,
    PackageDateTime, PackageEdits, PackageStore, PackageWriteOptions, PageSize, ProtectScope,
    ProviderInfo, ReadOptions, ReferenceGraphOptions, SignatureVerifiers, SignedValueFormat,
    SignedValueProducer, SigningReport, SigningRequest, StampAppearance, Storage, Stored,
    StoredObject,
};

#[cfg(feature = "crypto")]
pub use ofdbox_crypto as crypto;
#[cfg(feature = "crypto")]
pub use ofdbox_crypto::{
    CertificateBackend, EncryptOptions, EncryptionRecipient, EncryptionScheme, PasswordBackend,
    Sm2SignedDataProducer, Sm2SignedDataVerifier, encrypt_package, write_ofd_entries_encrypted,
};

#[cfg(feature = "seal")]
pub use ofdbox_seal as seal;
#[cfg(feature = "seal")]
pub use ofdbox_seal::{
    CertificateRole, CertificateState, CertificateStatus, GmT0031Signature,
    GmT0031SignatureVerifier, GmT0031Verification, GmT0031VerifyOptions, ReferenceStatus,
    ReportOptions, ReportVerdict, SealBuilder, SealFormat, SealPictureInput, SealSpec,
    SealStandard, SealVerifyOptions, SesSignatureVerifier, SesVerifyOptions, SignatureReport,
    SignatureState, SignatureStatus, SignatureSummary, Sm2GmT0031SignatureProducer,
    Sm2SesSignatureProducer, TimeStampRequest, TimeStampResponse, TimeStampToken,
    TimeStampVerification, TimeStampVerifyOptions, TimestampStatus, attach_time_stamp,
    parse_gmt0031_signature, parse_seal_for_display, parse_ses_signature, parse_time_stamp_request,
    parse_time_stamp_response, parse_time_stamp_token, signer_in_seal, verify_gmt0031_signature,
    verify_seal, verify_ses_signature, verify_time_stamp_token,
};

/// 门面统一错误。
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// OFD 结构解析或写出失败。
    #[error(transparent)]
    Core(#[from] OfdError),
    /// 文件 I/O 失败。
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// 门面结果类型。
pub type Result<T> = std::result::Result<T, Error>;

/// 常用类型预导入。
pub mod prelude {
    pub use crate::{
        CacheOptions, CreateOptions, Document, Error, OfdFile, OfdPackage, OpenOptions,
        OpenStorage, PackageEdits, PackageWriteOptions, PageEntry, ReadOptions, Result, Storage,
        Stored,
    };
}
