use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::model::ofd::{
    check_attributes, check_children_order, check_namespace, validate_text_element,
};
use crate::options::ReadOptions;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

/// 附件列表的类型化视图。
#[derive(Debug, Clone)]
pub struct Attachments {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Attachments {
    /// 从 XML 字节解析附件列表。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "Attachments" {
            return Err(OfdError::InvalidStructure(
                "附件列表的根元素不是 Attachments".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 遍历附件。
    pub fn attachments(&self) -> impl Iterator<Item = Attachment> {
        self.doc
            .children_named(self.node, "Attachment")
            .map(|node| Attachment {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 严格序列化为附件列表文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::attachment::write_attachments(self)
    }

    /// 严格序列化为 UTF-8 字节。
    pub fn to_bytes(&self) -> Result<Vec<u8>, OfdError> {
        self.to_xml().map(String::into_bytes)
    }

    pub(crate) fn validate_strict(&self) -> Result<(), OfdError> {
        let mut diagnostics = Vec::new();
        check_namespace(&self.doc, self.node, &mut diagnostics);
        check_attributes(&self.doc, self.node, &[], &[], &mut diagnostics);
        check_children_order(
            &self.doc,
            self.node,
            &["Attachment"],
            &[],
            &["Attachment"],
            &mut diagnostics,
        );
        for attachment in self.doc.children_named(self.node, "Attachment") {
            validate_attachment(&self.doc, attachment, &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 单个附件的视图。
#[derive(Debug, Clone)]
pub struct Attachment {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Attachment {
    /// 附件标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 附件名称。
    pub fn name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Name")
    }

    /// 附件格式。
    pub fn format(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Format")
    }

    /// 创建时间。
    pub fn creation_date(&self) -> Option<&str> {
        self.doc.attribute(self.node, "CreationDate")
    }

    /// 修改时间。
    pub fn mod_date(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ModDate")
    }

    /// 附件大小，单位为 KB。
    pub fn size(&self) -> Option<f64> {
        self.doc
            .attribute(self.node, "Size")
            .and_then(|value| value.parse().ok())
    }

    /// 是否可见。
    pub fn visible(&self) -> Option<bool> {
        self.doc
            .attribute(self.node, "Visible")
            .and_then(parse_bool)
    }

    /// 附件用途。
    pub fn usage(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Usage")
    }

    /// 附件内容在包内的路径。
    pub fn file_loc(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "FileLoc")
            .map(|node| self.doc.text(node).trim())
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn validate_attachment(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &[
            "ID",
            "Name",
            "Format",
            "CreationDate",
            "ModDate",
            "Size",
            "Visible",
            "Usage",
        ],
        &["ID", "Name"],
        diagnostics,
    );
    if let Some(size) = doc.attribute(node, "Size")
        && size.parse::<f64>().is_err()
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Size", doc.path(node)),
            "Size 必须是浮点数",
        ));
    }
    if let Some(visible) = doc.attribute(node, "Visible")
        && parse_bool(visible).is_none()
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Visible", doc.path(node)),
            "Visible 必须是布尔值",
        ));
    }
    check_children_order(doc, node, &["FileLoc"], &["FileLoc"], &[], diagnostics);
    if let Some(file_loc) = doc.find_child(node, "FileLoc") {
        validate_text_element(doc, file_loc, true, diagnostics);
    }
}
