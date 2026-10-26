//! OFD 结构模型。

pub mod action;
pub mod annotation;
pub mod attachment;
pub mod color;
pub mod custom_tags;
pub mod decrypt_seed;
pub mod document;
pub mod encryption;
pub mod entries_map;
pub mod extensions;
pub mod integrity;
pub mod ofd;
pub mod outline;
pub mod page;
pub mod region;
pub mod res;
pub mod signature;
pub mod version;

/// 包级对象。
pub mod package {
    pub use crate::package::OfdPackage;
}

/// 常用模型类型。
pub mod prelude {
    pub use crate::model::annotation::{Annot, Annotations, PageAnnot};
    pub use crate::model::document::Document;
    pub use crate::model::page::{
        Border, CgTransform, GraphicObject, Layer, Page, PageContent, TextCode,
    };
    pub use crate::model::res::{DrawParam, Font, MultiMedia, Res};
    pub use crate::model::signature::{Signature, Signatures};
    pub use crate::model::version::Version;
}
