//! 签名生成：请求、签名值生产者契约与结构组装。
//!
//! 结构层只负责选取保护文件、计算 `References`、组装 `Signature.xml` /
//! `Signatures.xml` 与写包；签名值由 [`SignedValueProducer`] 的实现产出
//! （SM2 → GB/T 35275，签章 → GB/T 38540）。

use base64::Engine;
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesText, Event};

use crate::error::OfdError;
use crate::model::signature::ReferenceDigest;
use crate::write::ofd::write_start;
use crate::xml::namespace::OFD_NAMESPACE;

/// 默认签名算法标识（SM2 with SM3）。
pub const DEFAULT_SIGNATURE_METHOD: &str = "1.2.156.10197.1.501";
/// 默认摘要方法标识（SM3，OID 形式，与真实签名文件一致）。
pub const DEFAULT_CHECK_METHOD: &str = "1.2.156.10197.1.401";

/// 签名值生产者。
///
/// 结构层通过该 trait 计算摘要并产出 `SignedValue.dat`，不依赖具体密码库。
pub trait SignedValueProducer {
    /// 是否支持指定签名方法。
    fn supports(&self, signature_method: &str) -> bool;

    /// 计算保护文件的摘要，结果写入 `References/CheckValue`。
    fn digest(&self, data: &[u8]) -> Result<Vec<u8>, OfdError>;

    /// 产出签名值文件的内容。
    fn produce(&self, context: &SigningContext<'_>) -> Result<Vec<u8>, OfdError>;

    /// 签名类型，写入 `Signatures.xml` 的 `Signature/@Type`；默认 `Sign`。
    fn signature_type(&self) -> &str {
        "Sign"
    }

    /// 签名值文件名，默认 `SignedValue.dat`。
    fn value_name(&self) -> &str {
        "SignedValue.dat"
    }
}

/// 签名值生产上下文。
pub struct SigningContext<'a> {
    /// 待签名的 `Signature.xml` 字节。
    pub content: &'a [u8],
    /// `Signature.xml` 在包内的绝对路径。
    pub signed_entry: &'a str,
    /// `SignatureMethod`。
    pub signature_method: &'a str,
    /// `SignatureDateTime`，按原样写入。
    pub signed_at: &'a str,
    /// 保护文件的摘要列表。
    pub references: &'a [ReferenceDigest],
}

/// 保护范围。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProtectScope {
    /// 包内全部内容相关文件（0099 附录 B.4），自动排除加密描述与本次签名文件。
    #[default]
    AllContent,
    /// 调用方指定的包内路径。
    Custom(Vec<String>),
}

/// 签名组件信息，对应 `SignedInfo/Provider`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInfo {
    /// 组件名称，必填。
    pub name: String,
    /// 组件制造商。
    pub company: Option<String>,
    /// 组件版本。
    pub version: Option<String>,
    /// 接口协议版本（0099 扩展）。
    pub protocol_version: Option<String>,
    /// 组件扩展信息（0099 扩展）。
    pub extend_data: Option<String>,
}

impl ProviderInfo {
    /// 默认使用 `ofdbox` 与当前 crate 版本。
    pub fn ofdbox() -> Self {
        Self {
            name: "ofdbox".to_owned(),
            company: None,
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            protocol_version: None,
            extend_data: None,
        }
    }

    /// 指定组件名称。
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 指定组件制造商。
    pub fn with_company(mut self, company: impl Into<String>) -> Self {
        self.company = Some(company.into());
        self
    }
}

impl Default for ProviderInfo {
    fn default() -> Self {
        Self::ofdbox()
    }
}

/// 一次签名的请求。
#[derive(Debug, Clone)]
pub struct SigningRequest<'a> {
    /// 保护范围。
    pub scope: ProtectScope,
    /// 是否允许后续继续签名。
    ///
    /// 为 `true` 时不把 `Signatures.xml` 纳入 `References`，否则会让后续签名
    /// 破坏本次签名（0099 附录 B.2）。
    pub allow_append: bool,
    /// `SignatureMethod`，默认 SM2 with SM3。
    pub signature_method: &'a str,
    /// 签名组件信息。
    pub provider: ProviderInfo,
    /// `SignatureDateTime`，按原样写入。
    pub signed_at: String,
}

impl<'a> SigningRequest<'a> {
    /// 使用默认范围与组件信息创建请求。
    pub fn new(signature_method: &'a str, signed_at: impl Into<String>) -> Self {
        Self {
            scope: ProtectScope::AllContent,
            allow_append: true,
            signature_method,
            provider: ProviderInfo::ofdbox(),
            signed_at: signed_at.into(),
        }
    }
}

/// 签名结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningReport {
    /// 签名标识，如 `s001`。
    pub signature_id: String,
    /// 签名描述文件在包内的路径。
    pub signature_entry: String,
    /// 签名值文件在包内的路径。
    pub signed_value_entry: String,
    /// 签名列表文件在包内的路径。
    pub signatures_entry: String,
    /// 被保护文件的包内路径。
    pub protected: Vec<String>,
}

/// 写 `Signature.xml`。
pub(crate) fn write_signature_xml(
    request: &SigningRequest<'_>,
    references: &[ReferenceDigest],
    signed_value_path: &str,
) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_start(
        &mut writer,
        "ofd:Signature",
        &[("xmlns:ofd", OFD_NAMESPACE)],
    )?;

    write_start(&mut writer, "ofd:SignedInfo", &[])?;
    let mut attributes = vec![("ProviderName", request.provider.name.as_str())];
    if let Some(company) = request.provider.company.as_deref() {
        attributes.push(("Company", company));
    }
    if let Some(version) = request.provider.version.as_deref() {
        attributes.push(("Version", version));
    }
    if let Some(protocol) = request.provider.protocol_version.as_deref() {
        attributes.push(("ProtocolVer", protocol));
    }
    write_start(&mut writer, "ofd:Provider", &attributes)?;
    if let Some(extend_data) = request.provider.extend_data.as_deref() {
        text_element(&mut writer, "ofd:ExtendData", extend_data)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:Provider")))?;

    text_element(&mut writer, "ofd:SignatureMethod", request.signature_method)?;
    text_element(&mut writer, "ofd:SignatureDateTime", &request.signed_at)?;

    write_start(
        &mut writer,
        "ofd:References",
        &[("CheckMethod", DEFAULT_CHECK_METHOD)],
    )?;
    for reference in references {
        let check_value = base64::engine::general_purpose::STANDARD.encode(&reference.expected);
        write_start(
            &mut writer,
            "ofd:Reference",
            &[("FileRef", reference.file_ref.as_str())],
        )?;
        text_element(&mut writer, "ofd:CheckValue", &check_value)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:Reference")))?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:References")))?;

    writer.write_event(Event::End(BytesEnd::new("ofd:SignedInfo")))?;
    text_element(&mut writer, "ofd:SignedValue", signed_value_path)?;
    writer.write_event(Event::End(BytesEnd::new("ofd:Signature")))?;

    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

/// 新建 `Signatures.xml`。
pub(crate) fn new_signatures_xml(id: &str, base_loc: &str, signature_type: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Signatures xmlns:ofd="{OFD_NAMESPACE}"><ofd:MaxSignId>{id}</ofd:MaxSignId><ofd:Signature ID="{id}" Type="{signature_type}" BaseLoc="{base_loc}"/></ofd:Signatures>"#
    )
}

/// 在已有 `Signatures.xml` 末尾追加一条签名记录，并更新 `MaxSignId`。
pub(crate) fn append_signature(
    existing: &[u8],
    id: &str,
    base_loc: &str,
    max_id: &str,
    signature_type: &str,
) -> Result<Vec<u8>, OfdError> {
    let text = std::str::from_utf8(existing).map_err(|error| {
        OfdError::InvalidStructure(format!("Signatures.xml 不是 UTF-8: {error}"))
    })?;
    let prefix = detect_prefix(text)?;
    let root = format!("<{prefix}Signatures");
    let entry =
        format!(r#"<{prefix}Signature ID="{id}" Type="{signature_type}" BaseLoc="{base_loc}"/>"#);
    let close = format!("</{prefix}Signatures>");
    let end = text
        .find(&close)
        .ok_or_else(|| OfdError::InvalidStructure("Signatures.xml 缺少结束标签".to_owned()))?;

    let mut output = String::with_capacity(text.len() + entry.len() + 32);
    output.push_str(&text[..end]);

    let max_tag = format!("<{prefix}MaxSignId>");
    let max_close = format!("</{prefix}MaxSignId>");
    if let Some(start) = output.find(&max_tag)
        && let Some(inner) = output[start + max_tag.len()..].find(&max_close)
    {
        let value_start = start + max_tag.len();
        let value_end = value_start + inner;
        output.replace_range(value_start..value_end, max_id);
    }

    output.push_str(&entry);
    output.push_str(&text[end..]);

    if !output.contains(&max_tag) {
        // 没有 MaxSignId：插到根开始标签之后。
        let root_at = output
            .find(&root)
            .ok_or_else(|| OfdError::InvalidStructure("Signatures.xml 根元素无效".to_owned()))?;
        let tag_end = output[root_at..]
            .find('>')
            .map(|offset| root_at + offset + 1)
            .ok_or_else(|| OfdError::InvalidStructure("Signatures.xml 根元素无效".to_owned()))?;
        let insert = format!("<{prefix}MaxSignId>{max_id}</{prefix}MaxSignId>");
        output.insert_str(tag_end, &insert);
    }

    Ok(output.into_bytes())
}

/// 在 `OFD.xml` 的 `DocBody` 末尾补 `Signatures` 链接。
pub(crate) fn insert_signatures_link(root_xml: &[u8], link: &str) -> Result<Vec<u8>, OfdError> {
    let text = std::str::from_utf8(root_xml)
        .map_err(|error| OfdError::InvalidStructure(format!("OFD.xml 不是 UTF-8: {error}")))?;
    let prefix = detect_prefix(text)?;
    let close = format!("</{prefix}DocBody>");
    let end = text
        .find(&close)
        .ok_or_else(|| OfdError::InvalidStructure("OFD.xml 缺少 DocBody".to_owned()))?;
    let escaped = quick_xml::escape::escape(link);
    let element = format!("<{prefix}Signatures>{escaped}</{prefix}Signatures>");
    let mut output = String::with_capacity(text.len() + element.len());
    output.push_str(&text[..end]);
    output.push_str(&element);
    output.push_str(&text[end..]);
    Ok(output.into_bytes())
}

/// 取根元素前缀（含冒号），无前缀返回空串。
fn detect_prefix(xml: &str) -> Result<String, OfdError> {
    let mut rest = xml;
    loop {
        let start = rest
            .find('<')
            .ok_or_else(|| OfdError::InvalidStructure("XML 缺少根元素".to_owned()))?;
        let after = &rest[start + 1..];
        if after.starts_with('?') || after.starts_with('!') {
            rest = &after[after.find('>').map(|i| i + 1).unwrap_or(after.len())..];
            continue;
        }
        let name_end = after
            .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
            .unwrap_or(after.len());
        let name = &after[..name_end];
        return Ok(match name.split_once(':') {
            Some((prefix, _)) => format!("{prefix}:"),
            None => String::new(),
        });
    }
}

fn text_element(writer: &mut Writer<Vec<u8>>, name: &str, value: &str) -> Result<(), OfdError> {
    write_start(writer, name, &[])?;
    writer.write_event(Event::Text(BytesText::new(value)))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}
