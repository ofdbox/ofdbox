//! OFD 标准结构对象模型。
//!
//! 当前是临时 crate，后续会整体移动到 `ofdbox-core::model`。
//! 本 crate 定义对象结构，并提供基于 yaserde 的 XML 序列化/反序列化。

#![forbid(unsafe_code)]

pub mod model;
pub mod xml;

pub use model::{
    annotation, annotations, attachments, custom_tags, definition, document, extensions, ofd, page,
    res, signature, signatures, version,
};
