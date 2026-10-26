//! OFD 标准结构对象模型。
//!
//! 当前是临时 crate，后续会整体移动到 `ofdbox-core::model`。
//! 本 crate 只定义对象结构，不包含 XML 解析、序列化或容器读写。

#![forbid(unsafe_code)]

pub mod model;

pub use model::{
    annotation, annotations, attachments, custom_tags, definition, document, extensions, ofd, page,
    res, signature, signatures, version,
};
