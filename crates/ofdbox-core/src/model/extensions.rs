use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::model::ofd::{check_attributes, check_namespace, validate_text_element};
use crate::options::ReadOptions;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

/// 扩展信息列表的类型化视图。
#[derive(Debug, Clone)]
pub struct Extensions {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Extensions {
    /// 从 XML 字节解析扩展信息列表。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "Extensions" {
            return Err(OfdError::InvalidStructure(
                "扩展信息列表的根元素不是 Extensions".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 遍历扩展信息节点。
    pub fn extensions(&self) -> impl Iterator<Item = Extension> {
        self.doc
            .children_named(self.node, "Extension")
            .map(|node| Extension {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 严格序列化为扩展信息列表文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::extensions::write_extensions(self)
    }

    /// 严格序列化为 UTF-8 字节。
    pub fn to_bytes(&self) -> Result<Vec<u8>, OfdError> {
        self.to_xml().map(String::into_bytes)
    }

    pub(crate) fn validate_strict(&self) -> Result<(), OfdError> {
        let mut diagnostics = Vec::new();
        check_namespace(&self.doc, self.node, &mut diagnostics);
        check_attributes(&self.doc, self.node, &[], &[], &mut diagnostics);
        for child in self.doc.element_children(self.node) {
            if self.doc.name(child).local != "Extension" {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::UnexpectedElement,
                    self.doc.path(child),
                    "Extensions 只允许 Extension 子元素",
                ));
            }
        }
        for extension in self.doc.children_named(self.node, "Extension") {
            validate_extension(&self.doc, extension, &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 单个扩展信息节点的视图。
#[derive(Debug, Clone)]
pub struct Extension {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Extension {
    /// 扩展应用程序名称。
    pub fn app_name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "AppName")
    }

    /// 软件厂商标识。
    pub fn company(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Company")
    }

    /// 软件版本。
    pub fn app_version(&self) -> Option<&str> {
        self.doc.attribute(self.node, "AppVersion")
    }

    /// 形成扩展信息的日期时间。
    pub fn date(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Date")
    }

    /// 扩展项引用的文档项目标识。
    pub fn ref_id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "RefId")
    }

    /// 简单扩展属性。
    pub fn properties(&self) -> impl Iterator<Item = Property> {
        self.doc
            .children_named(self.node, "Property")
            .map(|node| Property {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 复杂扩展数据节点。
    pub fn data(&self) -> impl Iterator<Item = Data> {
        self.doc
            .children_named(self.node, "Data")
            .map(|node| Data {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 外部扩展数据文件路径。
    pub fn extend_datas(&self) -> impl Iterator<Item = &str> {
        self.doc
            .children_named(self.node, "ExtendData")
            .map(|node| self.doc.text(node).trim())
            .collect::<Vec<_>>()
            .into_iter()
    }

    pub(crate) fn document(&self) -> &Arc<XmlDocument> {
        &self.doc
    }

    pub(crate) fn node(&self) -> NodeId {
        self.node
    }
}

/// 简单扩展属性的视图。
#[derive(Debug, Clone)]
pub struct Property {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Property {
    /// 属性名称。
    pub fn name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Name")
    }

    /// 属性值类型。
    pub fn value_type(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Type")
    }

    /// 属性值。
    pub fn value(&self) -> &str {
        self.doc.text(self.node).trim()
    }
}

/// 复杂扩展数据的视图。
///
/// `Data` 的内容为 `xs:anyType`，这里保留原始语法树节点供调用方自行解释。
#[derive(Debug, Clone)]
pub struct Data {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Data {
    /// 所属语法树。
    pub fn document(&self) -> &XmlDocument {
        &self.doc
    }

    /// 对应的语法树节点。
    pub fn node(&self) -> NodeId {
        self.node
    }
}

fn validate_extension(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["AppName", "Company", "AppVersion", "Date", "RefId"],
        &["AppName", "RefId"],
        diagnostics,
    );
    let mut has_child = false;
    for child in doc.element_children(node) {
        has_child = true;
        check_namespace(doc, child, diagnostics);
        match doc.name(child).local.as_str() {
            "Property" => validate_property(doc, child, diagnostics),
            "Data" => {}
            "ExtendData" => validate_text_element(doc, child, true, diagnostics),
            other => diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                format!("Extension 不支持子元素: {other}"),
            )),
        }
    }
    if !has_child {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            doc.path(node),
            "Extension 至少需要一个 Property、Data 或 ExtendData",
        ));
    }
}

fn validate_property(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_attributes(doc, node, &["Name", "Type"], &["Name"], diagnostics);
    validate_text_element(doc, node, false, diagnostics);
}
