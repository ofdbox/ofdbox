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

/// 为简单的字符串枚举生成 `Display` / `FromStr`，供 yaserde 属性解析使用。
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
