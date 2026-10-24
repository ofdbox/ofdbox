#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! OFDBox 对外门面。
//!
//! 根模块提供常用文档、加密与签章类型；完整 API 仍可通过
//! [`core`]、[`crypto`]、[`seal`] 子模块访问。
//!
//! 页面渲染与格式转换已迁至独立的 `ofdbox-folio`，批量转换命令行
//! 位于 `ofdbox-reader` 仓库。

pub use ofdbox_core as core;
pub use ofdbox_core::{
    CacheOptions, DEFAULT_SIGNATURE_METHOD, DocumentView, OfdError, OfdPackage, PackageDateTime,
    PackageEdits, PackageWriteOptions, PageView, ProtectScope, ProviderInfo, ReadOptions,
    SignatureVerifiers, SignedValueFormat, SignedValueProducer, SigningReport, SigningRequest,
};

#[cfg(feature = "crypto")]
pub use ofdbox_crypto as crypto;
#[cfg(feature = "crypto")]
pub use ofdbox_crypto::{
    CertificateBackend, EncryptOptions, EncryptionScheme, PasswordBackend, Sm2SignedDataProducer,
    Sm2SignedDataVerifier, encrypt_package,
};

#[cfg(feature = "seal")]
pub use ofdbox_seal as seal;
#[cfg(feature = "seal")]
pub use ofdbox_seal::{
    SealVerifyOptions, SesSignatureVerifier, SesVerifyOptions, parse_ses_signature, verify_seal,
    verify_ses_signature,
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
        CacheOptions, DocumentView, Error, OfdPackage, PackageEdits, PackageWriteOptions, PageView,
        ReadOptions, Result,
    };
}
