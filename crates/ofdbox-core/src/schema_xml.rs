//! typed 模型的 XML 序列化与反序列化。
//!
//! 当前只处理标准 OFD namespace；解析时忽略 namespace 前缀，
//! 写出时在根元素上补默认 namespace。

use std::fmt;

use yaserde::{YaDeserialize, YaSerialize};

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

impl From<yaserde::Error> for XmlError {
    fn from(error: yaserde::Error) -> Self {
        Self(error.to_string())
    }
}

/// 从 XML 文本反序列化。
pub fn from_str<T: YaDeserialize>(xml: &str) -> XmlResult<T> {
    Ok(yaserde::de::from_str(xml)?)
}

/// 从 XML 字节反序列化。
pub fn from_bytes<T: YaDeserialize>(bytes: &[u8]) -> XmlResult<T> {
    let xml = std::str::from_utf8(bytes)
        .map_err(|error| XmlError::new(format!("XML 不是 UTF-8: {error}")))?;
    from_str(xml)
}

/// 序列化为 XML 文本。
pub fn to_string<T: YaSerialize>(value: &T) -> XmlResult<String> {
    let mut xml = yaserde::ser::to_string(value)?;
    inject_default_namespace(&mut xml);
    Ok(xml)
}

/// 序列化为 XML 字节。
pub fn to_bytes<T: YaSerialize>(value: &T) -> XmlResult<Vec<u8>> {
    Ok(to_string(value)?.into_bytes())
}

fn inject_default_namespace(xml: &mut String) {
    let Some(declaration_end) = xml.find("?>") else {
        return;
    };
    let Some(relative_start) = xml[declaration_end + 2..].find('<') else {
        return;
    };
    let start = declaration_end + 2 + relative_start;
    let Some(relative_end) = xml[start..].find('>') else {
        return;
    };
    let end = start + relative_end;

    if xml[start..=end].contains("xmlns=") || xml[start..=end].contains("xmlns:") {
        return;
    }

    let insert_at = if xml[start..=end].ends_with("/>") {
        end - 1
    } else {
        end
    };
    xml.insert_str(insert_at, &format!(" xmlns=\"{OFD_NAMESPACE}\""));
}
