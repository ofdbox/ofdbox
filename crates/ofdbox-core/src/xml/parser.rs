use std::str;

use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::options::ReadOptions;
use crate::xml::namespace::{NamespaceClass, OFD_NAMESPACE, classify};
use crate::xml::tree::{NodeId, QName, Span, XmlAttribute, XmlDocument, XmlNode};

/// 解析一个 XML 条目，构建 arena 语法树。
///
/// 宽松模式只容忍语义偏差；XML 本身不合法时，无论哪种模式都返回
/// [`OfdError::Xml`]。
pub fn parse_xml(bytes: &[u8], options: ReadOptions) -> Result<XmlDocument, OfdError> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = str::from_utf8(bytes)
        .map_err(|error| OfdError::InvalidStructure(format!("XML 不是 UTF-8: {error}")))?;

    let mut reader = NsReader::from_str(text);

    let mut nodes: Vec<XmlNode> = Vec::new();
    let mut stack: Vec<NodeId> = Vec::new();
    let mut root: Option<NodeId> = None;
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    loop {
        let start = reader.buffer_position() as usize;
        let (resolved, event) = reader.read_resolved_event()?;

        match event {
            Event::Start(event) => {
                let (mut node, code) =
                    build_node(&event, &resolved, options, Span { start, end: start })?;
                node.span.end = reader.buffer_position() as usize;
                if let Some(code) = code {
                    diagnostics.push(Diagnostic::new(
                        code,
                        node_path(&stack, &nodes, &node.name.local),
                        "元素命名空间不符合标准",
                    ));
                }
                let id = nodes.len();
                if let Some(parent) = stack.last().copied() {
                    nodes[parent].children.push(id);
                } else if root.is_none() {
                    root = Some(id);
                }
                let parent = stack.last().copied();
                node.parent = parent;
                nodes.push(node);
                stack.push(id);
            }
            Event::Empty(event) => {
                let (mut node, code) =
                    build_node(&event, &resolved, options, Span { start, end: start })?;
                node.span.end = reader.buffer_position() as usize;
                if let Some(code) = code {
                    diagnostics.push(Diagnostic::new(
                        code,
                        node_path(&stack, &nodes, &node.name.local),
                        "元素命名空间不符合标准",
                    ));
                }
                let id = nodes.len();
                let parent = stack.last().copied();
                if let Some(parent) = parent {
                    nodes[parent].children.push(id);
                } else if root.is_none() {
                    root = Some(id);
                }
                node.parent = parent;
                nodes.push(node);
            }
            Event::Text(event) => {
                if let Some(current) = stack.last().copied() {
                    let raw = event.xml10_content();
                    let text = quick_xml::escape::unescape(raw.as_ref()).map_err(|error| {
                        OfdError::InvalidStructure(format!("文本反转义失败: {error:?}"))
                    })?;
                    nodes[current].text.push_str(text.as_ref());
                }
            }
            Event::CData(event) => {
                if let Some(current) = stack.last().copied() {
                    nodes[current].text.push_str(event.into_inner().as_ref());
                }
            }
            Event::GeneralRef(event) => {
                if let Some(current) = stack.last().copied() {
                    let text = resolve_reference(&event)?;
                    nodes[current].text.push_str(&text);
                }
            }
            Event::End(_) => {
                if let Some(id) = stack.pop() {
                    // 元素跨度覆盖完整元素（开始标签到结束标签之后）。
                    nodes[id].span.end = reader.buffer_position() as usize;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if let Some(open) = stack.last().copied() {
        let name = nodes[open].name.local.clone();
        return Err(OfdError::Xml(quick_xml::Error::IllFormed(
            quick_xml::errors::IllFormedError::MissingEndTag(name),
        )));
    }

    if options.strict && !diagnostics.is_empty() {
        return Err(OfdError::Conformance(diagnostics));
    }

    let root = root.ok_or_else(|| OfdError::InvalidStructure("缺少根元素".to_owned()))?;
    Ok(XmlDocument::new(nodes, root, bytes.len()))
}

fn build_node(
    event: &BytesStart<'_>,
    resolved: &ResolveResult<'_>,
    options: ReadOptions,
    span: Span,
) -> Result<(XmlNode, Option<DiagnosticCode>), OfdError> {
    let local = event.local_name().as_ref().to_owned();
    let raw_name = event.name();
    let prefix = prefix_of(raw_name.as_ref());

    let (namespace, code) = match resolved {
        ResolveResult::Bound(namespace) => match classify(Some(namespace.as_ref())) {
            NamespaceClass::Canonical => (Some(OFD_NAMESPACE.to_owned()), None),
            NamespaceClass::KnownVariant => {
                if options.strict {
                    (
                        Some(OFD_NAMESPACE.to_owned()),
                        Some(DiagnosticCode::NamespaceVariant),
                    )
                } else {
                    (Some(OFD_NAMESPACE.to_owned()), None)
                }
            }
            NamespaceClass::Missing | NamespaceClass::Unknown => (
                None,
                options.strict.then_some(DiagnosticCode::MissingNamespace),
            ),
        },
        ResolveResult::Unbound | ResolveResult::Unknown(_) => (
            None,
            options.strict.then_some(DiagnosticCode::MissingNamespace),
        ),
    };

    let mut attributes = Vec::new();
    for attribute in event.attributes() {
        let attribute = attribute
            .map_err(|error| OfdError::InvalidStructure(format!("XML 属性解析失败: {error}")))?;
        let key = attribute.key;
        let key_local = key.local_name().as_ref().to_owned();
        let key_prefix = prefix_of(key.as_ref());
        if key_local == "xmlns" || key_prefix.as_deref() == Some("xmlns") {
            continue;
        }
        let value = attribute
            .normalized_value(quick_xml::XmlVersion::Explicit1_0)
            .map_err(|error| OfdError::InvalidStructure(format!("属性值反转义失败: {error}")))?
            .into_owned();
        attributes.push(XmlAttribute {
            name: QName {
                prefix: key_prefix,
                local: key_local,
                namespace: None,
            },
            value,
        });
    }

    Ok((
        XmlNode {
            name: QName {
                prefix,
                local,
                namespace,
            },
            attributes,
            children: Vec::new(),
            parent: None,
            text: String::new(),
            span,
        },
        code,
    ))
}

fn resolve_reference(event: &quick_xml::events::BytesRef<'_>) -> Result<String, OfdError> {
    if let Some(ch) = event.resolve_char_ref()? {
        return Ok(ch.to_string());
    }
    match event.as_ref() {
        "amp" => Ok("&".to_owned()),
        "lt" => Ok("<".to_owned()),
        "gt" => Ok(">".to_owned()),
        "apos" => Ok("'".to_owned()),
        "quot" => Ok("\"".to_owned()),
        other => Err(OfdError::InvalidStructure(format!("未定义实体: &{other};"))),
    }
}

fn prefix_of(raw: &str) -> Option<String> {
    raw.split_once(':').map(|(prefix, _)| prefix.to_owned())
}

fn node_path(stack: &[NodeId], nodes: &[XmlNode], local: &str) -> String {
    let mut parts: Vec<&str> = stack
        .iter()
        .map(|id| nodes[*id].name.local.as_str())
        .collect();
    parts.push(local);
    format!("/{}", parts.join("/"))
}
