#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! OFD 核心库。
//!
//! 当前阶段覆盖 GB/T 33190-2016 的 `OFD.xml`：
//!
//! - 从 ZIP 容器按需读取 `OFD.xml`，支持文件后端与内存后端；
//! - 宽松模式容忍真实文件中常见的命名空间变体、混合前缀、元素乱序；
//! - 严格模式校验命名空间、顺序、必选字段和未知节点；
//! - 严格写出只从类型化视图产出标准命名空间、标准前缀和 XSD 顺序；
//! - 缓存按需解压的 entry 字节和解析后的 XML 语法树。

mod annotation_edit;
mod cache;
mod crypto;
mod error;
#[allow(missing_docs)]
pub mod model;
mod options;
mod package;
mod package_write;
mod refs;
mod sign;
mod stamp;
mod write;
mod writer;
mod xml;

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
pub use writer::{OfdWriter, PageSize};
pub use xml::namespace::OFD_NAMESPACE;
pub use xml::parser::parse_xml;
pub use xml::tree::{NodeId, QName, Span, XmlAttribute, XmlDocument, XmlNode};
