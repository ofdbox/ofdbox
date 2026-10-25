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
mod model;
mod options;
mod package;
mod package_write;
mod refs;
mod sign;
mod stamp;
mod write;
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
pub use model::action::{
    ActionView, ActionsView, GotoAView, GotoView, MovieView, SoundView, UriView,
};
pub use model::annotation::{
    AnnotView, AnnotationsPageView, AnnotationsView, AppearanceView, PageAnnotView, ParameterView,
};
pub use model::attachment::{AttachmentView, AttachmentsView};
pub use model::color::{
    AxialShdView, ColorKind, ColorView, GouraudShdView, LaGouraudShdView, PatternView, PointView,
    RadialShdView, SegmentView,
};
pub use model::custom_tags::{CustomTagView, CustomTagsView};
pub use model::decrypt_seed::{
    DecryptSeedView, DecryptUserView, ExtendParameterView, ExtendParamsView,
};
pub use model::document::{
    BookmarkView, BookmarksView, CommonDataView, DestView, DocumentView, PageAreaView, PageView,
    PagesView, PermissionsView, PrintView, TemplatePageView, VPreferencesView, ValidPeriodView,
};
pub use model::encryption::{
    EncryptInfoView, EncryptionParameterView, EncryptionProviderView, EncryptionsView,
};
pub use model::entries_map::{EncryptEntryView, EncryptedEntriesView};
pub use model::extensions::{DataView, ExtensionView, ExtensionsView, PropertyView};
pub use model::integrity::{DocEntriesView, DocEntryFileView};
pub use model::ofd::{
    CustomDataView, DocBodyView, DocInfoView, OfdRootView, VersionView, VersionsView,
};
pub use model::outline::{OutlineElemView, OutlinesView};
pub use model::page::{
    BorderView, CGTransformView, ClipAreaKind, ClipAreaView, ClipView, ClipsView, ContentView,
    GraphicObjectKind, GraphicObjectView, GraphicPayloadView, LayerView, PageContentView,
    TemplateView, TextCodeView,
};
pub use model::region::{RegionAreaView, RegionCommandKind, RegionCommandView, RegionView};
pub use model::res::{
    ColorSpaceView, CompositeContentView, CompositeGraphicUnitView, DrawParamView, FontView,
    MultiMediaView, ResView,
};
pub use model::signature::{
    ProviderView, ReferenceDigest, ReferenceView, ReferencesView, SealView, SignatureEntryView,
    SignatureParameterView, SignatureView, SignaturesView, SignedInfoView, StampAnnotView,
};
pub use model::version::{DocVersionView, VersionFileView};
pub use options::{CacheOptions, ReadOptions};
pub use package::{
    CacheStats, EntriesVerification, EntriesWriteOptions, EntriesWriteReport, OfdPackage,
    ROOT_ENTRY,
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
pub use xml::namespace::OFD_NAMESPACE;
pub use xml::parser::parse_xml;
pub use xml::tree::{NodeId, QName, Span, XmlAttribute, XmlDocument, XmlNode};
