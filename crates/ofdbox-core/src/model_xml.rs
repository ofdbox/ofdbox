//! typed 模型的 XML 序列化与反序列化。
//!
//! 当前只处理标准 OFD namespace；解析时忽略 namespace 前缀，
//! 写出时统一使用 `ofd:` 前缀和 OFD namespace。

use std::fmt;

use ofdbox_yaserde::{YaDeserialize, YaSerialize};

/// OFD 标准命名空间。
pub const OFD_NAMESPACE: &str = "http://www.ofdspec.org/2016";

/// XML 处理错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlError(String);

impl XmlError {
    /// 创建错误。
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    /// 错误信息。
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for XmlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for XmlError {}

/// XML 处理结果。
pub type XmlResult<T> = Result<T, XmlError>;

impl From<ofdbox_yaserde::Error> for XmlError {
    fn from(error: ofdbox_yaserde::Error) -> Self {
        Self(error.to_string())
    }
}

/// 从 XML 文本反序列化。
pub fn from_str<T: YaDeserialize>(xml: &str) -> XmlResult<T> {
    Ok(ofdbox_yaserde::de::from_str(xml)?)
}

/// 从 XML 字节反序列化。
pub fn from_bytes<T: YaDeserialize>(bytes: &[u8]) -> XmlResult<T> {
    let xml = std::str::from_utf8(bytes)
        .map_err(|error| XmlError::new(format!("XML 不是 UTF-8: {error}")))?;
    from_str(xml)
}

/// 序列化为 XML 文本。
pub fn to_string<T: YaSerialize>(value: &T) -> XmlResult<String> {
    Ok(ofdbox_yaserde::ser::to_string(value)?)
}

/// 解析 OFD 元素名，前缀由序列化上下文决定。
pub(crate) fn ofd_name<W: std::io::Write>(
    writer: &ofdbox_yaserde::ser::Serializer<W>,
    local: &str,
) -> String {
    writer.element_name(Some(OFD_NAMESPACE), local, None)
}

/// 序列化为 XML 字节。
pub fn to_bytes<T: YaSerialize>(value: &T) -> XmlResult<Vec<u8>> {
    Ok(to_string(value)?.into_bytes())
}
