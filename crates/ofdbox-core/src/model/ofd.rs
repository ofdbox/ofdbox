use std::collections::HashSet;
use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::options::ReadOptions;
use crate::xml::namespace::OFD_NAMESPACE;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

const DOCBODY_ORDER: &[&str] = &["DocInfo", "DocRoot", "Versions", "Signatures"];
const DOCINFO_ORDER: &[&str] = &[
    "DocID",
    "Title",
    "Author",
    "Subject",
    "Abstract",
    "CreationDate",
    "ModDate",
    "DocUsage",
    "Cover",
    "Keywords",
    "Creator",
    "CreatorVersion",
    "CustomDatas",
];

/// `OFD.xml` 根元素的类型化视图。
#[derive(Debug, Clone)]
pub struct OfdRoot {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl OfdRoot {
    /// 从 XML 字节解析根视图。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "OFD" {
            return Err(OfdError::InvalidStructure(
                "OFD.xml 的根元素不是 OFD".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 底层语法树。
    pub fn document(&self) -> &Arc<XmlDocument> {
        &self.doc
    }

    /// 根节点编号。
    pub fn node(&self) -> NodeId {
        self.node
    }

    /// 文件格式版本属性。
    pub fn version(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Version")
    }

    /// 文件格式子集类型属性。
    pub fn doc_type(&self) -> Option<&str> {
        self.doc.attribute(self.node, "DocType")
    }

    /// 遍历所有 `DocBody`。
    pub fn doc_bodies(&self) -> impl Iterator<Item = DocBody> {
        self.doc
            .children_named(self.node, "DocBody")
            .map(|node| DocBody {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 严格序列化为 `OFD.xml` 文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::ofd::write_root(self)
    }

    /// 严格序列化为 UTF-8 字节。
    pub fn to_bytes(&self) -> Result<Vec<u8>, OfdError> {
        self.to_xml().map(String::into_bytes)
    }

    pub(crate) fn validate_strict(&self) -> Result<(), OfdError> {
        let mut diagnostics = Vec::new();
        check_namespace(&self.doc, self.node, &mut diagnostics);
        check_attributes(
            &self.doc,
            self.node,
            &["Version", "DocType"],
            &["Version", "DocType"],
            &mut diagnostics,
        );
        match self.version() {
            Some("1.0") => {}
            Some(_) => diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidAttribute,
                format!("{}/@Version", self.doc.path(self.node)),
                "Version 必须为 1.0",
            )),
            None => {}
        }
        match self.doc_type() {
            Some("OFD") => {}
            Some(_) => diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidAttribute,
                format!("{}/@DocType", self.doc.path(self.node)),
                "DocType 必须为 OFD",
            )),
            None => {}
        }

        let bodies: Vec<NodeId> = self.doc.children_named(self.node, "DocBody").collect();
        if bodies.is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MissingRequired,
                format!("{}/DocBody", self.doc.path(self.node)),
                "至少需要一个 DocBody",
            ));
        }
        for child in self.doc.element_children(self.node) {
            if self.doc.name(child).local != "DocBody" {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::UnexpectedElement,
                    self.doc.path(child),
                    "OFD 根元素只允许 DocBody",
                ));
            }
        }
        for body in bodies {
            validate_doc_body(&self.doc, body, &mut diagnostics);
        }

        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 一个 `DocBody` 的类型化视图。
#[derive(Debug, Clone)]
pub struct DocBody {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl DocBody {
    /// 文档元数据。
    pub fn doc_info(&self) -> Option<DocInfo> {
        self.doc
            .find_child(self.node, "DocInfo")
            .map(|node| DocInfo {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    /// 文档根节点路径，通常是 `Document.xml`。
    pub fn doc_root(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "DocRoot")
            .map(|node| self.doc.text(node))
    }

    /// 版本列表。
    pub fn versions(&self) -> Option<Versions> {
        self.doc
            .find_child(self.node, "Versions")
            .map(|node| Versions {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    /// 签名列表路径。
    pub fn signatures(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "Signatures")
            .map(|node| self.doc.text(node).trim())
    }
}

/// `DocInfo` 的类型化视图。
#[derive(Debug, Clone)]
pub struct DocInfo {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl DocInfo {
    fn value(&self, local: &str) -> Option<&str> {
        self.doc
            .find_child(self.node, local)
            .map(|node| self.doc.text(node).trim())
    }

    /// 文档标识。
    pub fn doc_id(&self) -> Option<&str> {
        self.value("DocID")
    }

    /// 文档标题。
    pub fn title(&self) -> Option<&str> {
        self.value("Title")
    }

    /// 文档作者。
    pub fn author(&self) -> Option<&str> {
        self.value("Author")
    }

    /// 文档主题。
    pub fn subject(&self) -> Option<&str> {
        self.value("Subject")
    }

    /// 文档摘要。
    pub fn abstract_(&self) -> Option<&str> {
        self.value("Abstract")
    }

    /// 创建日期。
    pub fn creation_date(&self) -> Option<&str> {
        self.value("CreationDate")
    }

    /// 修改日期。
    pub fn mod_date(&self) -> Option<&str> {
        self.value("ModDate")
    }

    /// 文档用途。
    pub fn doc_usage(&self) -> Option<&str> {
        self.value("DocUsage")
    }

    /// 封面路径。
    pub fn cover(&self) -> Option<&str> {
        self.value("Cover")
    }

    /// 创建者。
    pub fn creator(&self) -> Option<&str> {
        self.value("Creator")
    }

    /// 创建者版本。
    pub fn creator_version(&self) -> Option<&str> {
        self.value("CreatorVersion")
    }

    /// 关键词列表。
    pub fn keywords(&self) -> impl Iterator<Item = &str> + '_ {
        let mut values = Vec::new();
        if let Some(keywords) = self.doc.find_child(self.node, "Keywords") {
            for keyword in self.doc.children_named(keywords, "Keyword") {
                values.push(self.doc.text(keyword).trim());
            }
        }
        values.into_iter()
    }

    /// 自定义元数据。
    pub fn custom_datas(&self) -> impl Iterator<Item = CustomData> + '_ {
        let mut values = Vec::new();
        if let Some(parent) = self.doc.find_child(self.node, "CustomDatas") {
            for node in self.doc.children_named(parent, "CustomData") {
                values.push(CustomData {
                    doc: Arc::clone(&self.doc),
                    node,
                });
            }
        }
        values.into_iter()
    }
}

/// `Versions` 的类型化视图。
#[derive(Debug, Clone)]
pub struct Versions {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Versions {
    /// 遍历版本条目。
    pub fn versions(&self) -> impl Iterator<Item = VersionRef> {
        self.doc
            .children_named(self.node, "Version")
            .map(|node| VersionRef {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 单个 `Version` 的视图。
#[derive(Debug, Clone)]
pub struct VersionRef {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl VersionRef {
    /// 版本标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 版本号。
    pub fn index(&self) -> Option<i32> {
        self.doc
            .attribute(self.node, "Index")
            .and_then(|value| value.parse().ok())
    }

    /// 是否为默认版本。
    pub fn current(&self) -> bool {
        matches!(
            self.doc.attribute(self.node, "Current"),
            Some("true") | Some("1")
        )
    }

    /// 版本描述文件路径。
    pub fn base_loc(&self) -> Option<&str> {
        self.doc.attribute(self.node, "BaseLoc")
    }
}

/// 单个 `CustomData` 的视图。
#[derive(Debug, Clone)]
pub struct CustomData {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl CustomData {
    /// 自定义元数据名称。
    pub fn name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Name")
    }

    /// 自定义元数据值。
    pub fn value(&self) -> &str {
        self.doc.text(self.node).trim()
    }
}

fn validate_doc_body(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(
        doc,
        node,
        DOCBODY_ORDER,
        &["DocInfo", "DocRoot"],
        &[],
        diagnostics,
    );

    if let Some(info) = doc.find_child(node, "DocInfo") {
        validate_doc_info(doc, info, diagnostics);
    }
    if let Some(root) = doc.find_child(node, "DocRoot") {
        validate_text_element(doc, root, true, diagnostics);
    }
    if let Some(versions) = doc.find_child(node, "Versions") {
        validate_versions(doc, versions, diagnostics);
    }
    if let Some(signatures) = doc.find_child(node, "Signatures") {
        validate_text_element(doc, signatures, false, diagnostics);
    }
}

fn validate_doc_info(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(doc, node, DOCINFO_ORDER, &["DocID"], &[], diagnostics);

    if let Some(doc_id) = doc.find_child(node, "DocID") {
        validate_text_element(doc, doc_id, true, diagnostics);
    }
    if let Some(keywords) = doc.find_child(node, "Keywords") {
        validate_keywords(doc, keywords, diagnostics);
    }
    if let Some(custom_datas) = doc.find_child(node, "CustomDatas") {
        validate_custom_datas(doc, custom_datas, diagnostics);
    }
}

fn validate_keywords(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    let values: Vec<NodeId> = doc.children_named(node, "Keyword").collect();
    if values.is_empty() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            format!("{}/Keyword", doc.path(node)),
            "Keywords 至少需要一个 Keyword",
        ));
    }
    for child in doc.element_children(node) {
        if doc.name(child).local != "Keyword" {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "Keywords 只允许 Keyword",
            ));
        }
    }
}

fn validate_custom_datas(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    for child in doc.element_children(node) {
        if doc.name(child).local != "CustomData" {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "CustomDatas 只允许 CustomData",
            ));
            continue;
        }
        check_namespace(doc, child, diagnostics);
        check_attributes(doc, child, &["Name"], &["Name"], diagnostics);
        if doc.element_children(child).next().is_some() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "CustomData 不应包含子元素",
            ));
        }
    }
}

fn validate_versions(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    let versions: Vec<NodeId> = doc.children_named(node, "Version").collect();
    if versions.is_empty() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            format!("{}/Version", doc.path(node)),
            "Versions 至少需要一个 Version",
        ));
    }
    for child in doc.element_children(node) {
        if doc.name(child).local != "Version" {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "Versions 只允许 Version",
            ));
            continue;
        }
        check_namespace(doc, child, diagnostics);
        check_attributes(
            doc,
            child,
            &["ID", "Index", "Current", "BaseLoc"],
            &["ID", "Index", "BaseLoc"],
            diagnostics,
        );
        if let Some(index) = doc.attribute(child, "Index")
            && index.parse::<i32>().is_err()
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidAttribute,
                format!("{}/@Index", doc.path(child)),
                "Index 必须是整数",
            ));
        }
        if doc.element_children(child).next().is_some() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "Version 不应包含子元素",
            ));
        }
    }
}

pub(crate) fn validate_text_element(
    doc: &XmlDocument,
    node: NodeId,
    required: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_namespace(doc, node, diagnostics);
    if doc.element_children(node).next().is_some() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnexpectedElement,
            doc.path(node),
            "该元素不应包含子元素",
        ));
    }
    if required && doc.text(node).trim().is_empty() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            doc.path(node),
            "该元素不能为空",
        ));
    }
}

pub(crate) fn check_namespace(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    match doc.name(node).namespace.as_deref() {
        Some(OFD_NAMESPACE) => {}
        Some(_) => diagnostics.push(Diagnostic::new(
            DiagnosticCode::NamespaceVariant,
            doc.path(node),
            "命名空间 URI 不是标准值",
        )),
        None => diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingNamespace,
            doc.path(node),
            "元素缺少 OFD 命名空间",
        )),
    }
}

pub(crate) fn check_attributes(
    doc: &XmlDocument,
    node: NodeId,
    allowed: &[&str],
    required: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = HashSet::new();
    for attribute in &doc.node(node).attributes {
        if attribute.name.prefix.is_some() || !allowed.contains(&attribute.name.local.as_str()) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedAttribute,
                format!("{}/@{}", doc.path(node), attribute.name.local),
                "属性不在允许范围内",
            ));
        }
        seen.insert(attribute.name.local.as_str());
    }
    for name in required {
        if !seen.contains(name) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MissingRequired,
                format!("{}/@{}", doc.path(node), name),
                "缺少必选属性",
            ));
        }
    }
}

/// 校验子元素集合，不要求顺序。适用于标准正文没有给出 `xs:sequence` 的结构。
pub(crate) fn check_children_unordered(
    doc: &XmlDocument,
    node: NodeId,
    allowed: &[&str],
    required: &[&str],
    repeated: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = HashSet::new();
    for child in doc.element_children(node) {
        check_namespace(doc, child, diagnostics);
        let name = doc.name(child).local.as_str();
        if !allowed.contains(&name) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "元素不在允许范围内",
            ));
            continue;
        }
        if !seen.insert(name) && !repeated.contains(&name) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DuplicateElement,
                doc.path(child),
                "单值元素重复出现",
            ));
        }
    }
    for name in required {
        if !seen.contains(name) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MissingRequired,
                format!("{}/{}", doc.path(node), name),
                "缺少必选元素",
            ));
        }
    }
}

pub(crate) fn check_children_order(
    doc: &XmlDocument,
    node: NodeId,
    order: &[&str],
    required: &[&str],
    repeated: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut last_index = 0usize;
    let mut seen = HashSet::new();
    for child in doc.element_children(node) {
        check_namespace(doc, child, diagnostics);
        let name = doc.name(child).local.as_str();
        let Some(index) = order.iter().position(|item| *item == name) else {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "元素不在允许范围内",
            ));
            continue;
        };
        if index < last_index {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::OutOfOrder,
                doc.path(child),
                "子元素顺序不符合 XSD sequence",
            ));
        }
        last_index = index;
        if !seen.insert(name) && !repeated.contains(&name) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DuplicateElement,
                doc.path(child),
                "单值元素重复出现",
            ));
        }
    }
    for name in required {
        if !seen.contains(name) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MissingRequired,
                format!("{}/{}", doc.path(node), name),
                "缺少必选元素",
            ));
        }
    }
}
