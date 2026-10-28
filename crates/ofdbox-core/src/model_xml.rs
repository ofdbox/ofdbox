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
    let mut xml = ofdbox_yaserde::ser::to_string(value)?;
    inject_prefixed_namespace(&mut xml);
    Ok(xml)
}

/// 序列化为 XML 字节。
pub fn to_bytes<T: YaSerialize>(value: &T) -> XmlResult<Vec<u8>> {
    Ok(to_string(value)?.into_bytes())
}

fn inject_prefixed_namespace(xml: &mut String) {
    let Some(declaration_end) = xml.find("?>") else {
        return;
    };
    let Some(relative_start) = xml[declaration_end + 2..].find('<') else {
        return;
    };
    let root_start = declaration_end + 2 + relative_start;
    let Some(relative_end) = xml[root_start..].find('>') else {
        return;
    };
    let root_end = root_start + relative_end;

    let namespace = format!("xmlns:ofd=\"{OFD_NAMESPACE}\"");
    let root = xml[root_start..=root_end].to_owned();
    if !root.contains("xmlns:ofd=") {
        let insert_at = if root.ends_with("/>") {
            root_end - 1
        } else {
            root_end
        };
        xml.insert_str(insert_at, &format!(" {namespace}"));
    }

    let input = xml.clone();
    let mut output = String::with_capacity(input.len() + 64);
    let bytes = input.as_bytes();
    let mut index = 0;
    let mut raw = false;
    while index < bytes.len() {
        if raw {
            if let Some(relative) = input[index..].find("<!--ofdbox-raw-end-->") {
                output.push_str(&input[index..index + relative]);
                index += relative + "<!--ofdbox-raw-end-->".len();
                raw = false;
                continue;
            }
            output.push_str(&input[index..]);
            break;
        }
        if input[index..].starts_with("<!--ofdbox-raw-start-->") {
            index += "<!--ofdbox-raw-start-->".len();
            raw = true;
            continue;
        }
        if bytes[index] != b'<' {
            let Some(next) = input[index..].find('<') else {
                output.push_str(&input[index..]);
                break;
            };
            output.push_str(&input[index..index + next]);
            index += next;
            continue;
        }

        if input[index..].starts_with("<!--") {
            let Some(end) = input[index + 4..].find("-->") else {
                output.push_str(&input[index..]);
                break;
            };
            output.push_str(&input[index..index + 4 + end + 3]);
            index += 4 + end + 3;
            continue;
        }
        if input[index..].starts_with("<![CDATA[") {
            let Some(end) = input[index + 9..].find("]]>") else {
                output.push_str(&input[index..]);
                break;
            };
            output.push_str(&input[index..index + 9 + end + 3]);
            index += 9 + end + 3;
            continue;
        }
        if input[index..].starts_with("<?") {
            let Some(end) = input[index + 2..].find("?>") else {
                output.push_str(&input[index..]);
                break;
            };
            output.push_str(&input[index..index + 2 + end + 2]);
            index += 2 + end + 2;
            continue;
        }
        if input[index..].starts_with("<!") {
            let Some(end) = input[index..].find('>') else {
                output.push_str(&input[index..]);
                break;
            };
            output.push_str(&input[index..=index + end]);
            index += end + 1;
            continue;
        }

        let closing = bytes.get(index + 1) == Some(&b'/');
        let name_start = index + if closing { 2 } else { 1 };
        let mut name_end = name_start;
        while name_end < bytes.len()
            && !matches!(bytes[name_end], b' ' | b'\t' | b'\r' | b'\n' | b'/' | b'>')
        {
            name_end += 1;
        }
        if name_end == name_start {
            output.push('<');
            index += 1;
            continue;
        }
        let name = &input[name_start..name_end];
        if name.contains(':') {
            output.push_str(&input[index..name_end]);
        } else {
            output.push_str(&input[index..name_start]);
            output.push_str("ofd:");
            output.push_str(name);
        }
        index = name_end;
    }
    *xml = output;
}
