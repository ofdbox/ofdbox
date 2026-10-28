#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! OFD 核心库。
//!
//! 结构：
//!
//! - [`model`]：OFD XML 的纯数据模型；
//! - [`model_xml`]：模型与 XML 的双向转换；
//! - [`store`]：`OfdFile`、`Stored<T>`、`ObjectContext` 与存储后端；
//! - `OfdPackage`：ZIP、加密解密与底层 entry 读写。

/// 为简单的字符串枚举生成 `Display` / `FromStr`，供 XML 属性解析使用。
#[macro_export]
macro_rules! impl_xml_enum_codec {
    ($ty:ty, $other:ident, { $($variant:ident => $name:literal),* $(,)? }) => {
        impl std::fmt::Display for $ty {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$variant => formatter.write_str($name),)*
                    Self::$other(value) => formatter.write_str(value),
                }
            }
        }

        impl std::str::FromStr for $ty {
            type Err = String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($name => Ok(Self::$variant),)*
                    other => Ok(Self::$other(other.to_owned())),
                }
            }
        }
    };
}

mod annotation_edit;
mod cache;
mod crypto;
mod error;
#[allow(missing_docs)]
pub mod model;
#[allow(missing_docs)]
pub mod model_xml;
mod options;
mod package;
mod package_write;
mod refs;
mod sign;
mod stamp;
#[allow(missing_docs)]
pub mod store;
mod writer;

pub use annotation_edit::{
    AnnotationCgTransform, AnnotationComposite, AnnotationEditReport, AnnotationEditor,
    AnnotationImage, AnnotationPatch, AnnotationPath, AnnotationSpec, AnnotationText,
};
pub use crypto::canonical_digest_method;
pub use crypto::{
    CryptoBackend, CryptoRegistry, FileKey, SignatureVerifier, SignatureVerifiers,
    SignedValueFormat, detect_signed_value_format,
};
pub use error::{Diagnostic, DiagnosticCode, OfdError};
pub use options::{CacheOptions, ReadOptions};
pub use package::{
    CacheStats, DocumentEntry, EntriesVerification, EntriesWriteOptions, EntriesWriteReport,
    OfdPackage, ROOT_ENTRY,
};
pub use package_write::{
    DecryptedWriteReport, EntryCompression, PackageDateTime, PackageEdits, PackageWriteOptions,
};
pub use refs::ReferenceGraphOptions;
pub use sign::{
    DEFAULT_CHECK_METHOD, DEFAULT_SIGNATURE_METHOD, ProtectScope, ProviderInfo,
    SignedValueProducer, SigningContext, SigningReport, SigningRequest,
};
pub use stamp::StampAppearance;
pub use store::{
    CreateOptions, ObjectContext, OfdFile, OpenOptions, OpenStorage, PackageStore, Storage, Stored,
    StoredObject,
};
pub use writer::{OfdWriter, PageSize};
