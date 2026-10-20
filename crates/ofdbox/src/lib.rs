#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! OFDBox 对外门面。
//!
//! 根模块提供常用文档、渲染和错误类型；完整 API 仍可通过
//! [`core`]、[`render`]、[`crypto`]、[`seal`] 子模块访问。

pub use ofdbox_core as core;
pub use ofdbox_core::{CacheOptions, DocumentView, OfdError, OfdPackage, PageView, ReadOptions};

#[cfg(feature = "render")]
pub use ofdbox_render as render;
#[cfg(feature = "render")]
pub use ofdbox_render::{
    OfdOptions, PdfOptions, PdfVersion, RenderOptions, Scene, Size, TextMode, UnsupportedPolicy,
};

#[cfg(feature = "crypto")]
pub use ofdbox_crypto as crypto;
#[cfg(feature = "crypto")]
pub use ofdbox_crypto::{CertificateBackend, PasswordBackend};

#[cfg(feature = "seal")]
pub use ofdbox_seal as seal;
#[cfg(feature = "seal")]
pub use ofdbox_seal::{SealVerifyOptions, verify_seal};

/// 门面统一错误。
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// OFD 结构解析或写出失败。
    #[error(transparent)]
    Core(#[from] OfdError),
    /// 渲染或格式转换失败。
    #[cfg(feature = "render")]
    #[error(transparent)]
    Render(#[from] ofdbox_render::RenderError),
    /// 文件 I/O 失败。
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// 门面结果类型。
pub type Result<T> = std::result::Result<T, Error>;

/// 常用类型预导入。
pub mod prelude {
    pub use crate::{CacheOptions, DocumentView, Error, OfdPackage, PageView, ReadOptions, Result};
    #[cfg(feature = "render")]
    pub use crate::{
        OfdOptions, PdfOptions, PdfVersion, RenderOptions, Scene, Size, TextMode, UnsupportedPolicy,
    };
}
