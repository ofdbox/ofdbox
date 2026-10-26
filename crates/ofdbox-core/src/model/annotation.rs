use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::model::ofd::{
    check_attributes, check_children_order, check_namespace, validate_text_element,
};
use crate::model::page::{GraphicObject, validate_graphic_object};
use crate::options::ReadOptions;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

const ANNOT_TYPES: &[&str] = &["Link", "Path", "Highlight", "Stamp", "Watermark"];

/// 注释索引文件的类型化视图。
#[derive(Debug, Clone)]
pub struct Annotations {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Annotations {
    /// 从 XML 字节解析注释索引。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "Annotations" {
            return Err(OfdError::InvalidStructure(
                "注释索引的根元素不是 Annotations".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 遍历按页组织的注释索引。
    pub fn pages(&self) -> impl Iterator<Item = AnnotationsPageEntry> {
        self.doc
            .children_named(self.node, "Page")
            .map(|node| AnnotationsPageEntry {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 严格序列化为注释索引文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::annotation::write_annotations(self)
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
            &["Page"],
            &[],
            &["Page"],
            &mut diagnostics,
        );
        for page in self.doc.children_named(self.node, "Page") {
            validate_annotations_page(&self.doc, page, &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 注释索引中单个页面的视图。
#[derive(Debug, Clone)]
pub struct AnnotationsPageEntry {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl AnnotationsPageEntry {
    /// 引用页面的标识。
    pub fn page_id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "PageID")
    }

    /// 分页注释文件路径。
    pub fn file_loc(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "FileLoc")
            .map(|node| self.doc.text(node).trim())
    }
}

/// 分页注释文件的类型化视图。
#[derive(Debug, Clone)]
pub struct PageAnnot {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl PageAnnot {
    /// 从 XML 字节解析分页注释文件。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "PageAnnot" {
            return Err(OfdError::InvalidStructure(
                "分页注释文件的根元素不是 PageAnnot".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 遍历注释。
    pub fn annots(&self) -> impl Iterator<Item = Annot> {
        self.doc
            .children_named(self.node, "Annot")
            .map(|node| Annot {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 严格序列化为分页注释文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::annotation::write_page_annot(self)
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
            &["Annot"],
            &["Annot"],
            &["Annot"],
            &mut diagnostics,
        );
        for annot in self.doc.children_named(self.node, "Annot") {
            validate_annot(&self.doc, annot, &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 单个注释的视图。
#[derive(Debug, Clone)]
pub struct Annot {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Annot {
    /// 注释标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 注释类型。
    pub fn annot_type(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Type")
    }

    /// 注释创建者。
    pub fn creator(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Creator")
    }

    /// 最近修改日期。
    pub fn last_mod_date(&self) -> Option<&str> {
        self.doc.attribute(self.node, "LastModDate")
    }

    /// 注释子类型。
    pub fn subtype(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Subtype")
    }

    /// 是否显示。
    pub fn visible(&self) -> Option<bool> {
        self.bool_attr("Visible")
    }

    /// Remark 是否随页面打印。
    pub fn print(&self) -> Option<bool> {
        self.bool_attr("Print")
    }

    /// Remark 是否不随页面缩放。
    pub fn no_zoom(&self) -> Option<bool> {
        self.bool_attr("NoZoom")
    }

    /// Remark 是否不随页面旋转。
    pub fn no_rotate(&self) -> Option<bool> {
        self.bool_attr("NoRotate")
    }

    /// Remark 是否只读。
    pub fn read_only(&self) -> Option<bool> {
        self.bool_attr("ReadOnly")
    }

    /// 注释说明内容。
    pub fn remark(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "Remark")
            .map(|node| self.doc.text(node).trim())
    }

    /// 注释参数。
    pub fn parameters(&self) -> impl Iterator<Item = Parameter> + '_ {
        let mut values = Vec::new();
        if let Some(parameters) = self.doc.find_child(self.node, "Parameters") {
            for node in self.doc.children_named(parameters, "Parameter") {
                values.push(Parameter {
                    doc: Arc::clone(&self.doc),
                    node,
                });
            }
        }
        values.into_iter()
    }

    /// 注释外观。
    pub fn appearance(&self) -> Option<Appearance> {
        self.doc
            .find_child(self.node, "Appearance")
            .map(|node| Appearance {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    fn bool_attr(&self, name: &str) -> Option<bool> {
        self.doc.attribute(self.node, name).and_then(parse_bool)
    }
}

/// 注释参数的视图。
#[derive(Debug, Clone)]
pub struct Parameter {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Parameter {
    /// 参数名称。
    pub fn name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Name")
    }

    /// 参数值。
    pub fn value(&self) -> &str {
        self.doc.text(self.node).trim()
    }
}

/// 注释外观的视图。
#[derive(Debug, Clone)]
pub struct Appearance {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Appearance {
    /// 外观边界。
    pub fn boundary(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Boundary")
    }

    /// 外观中的图元。
    pub fn objects(&self) -> impl Iterator<Item = GraphicObject> {
        self.doc
            .element_children(self.node)
            .map(|node| GraphicObject::new(Arc::clone(&self.doc), node))
            .collect::<Vec<_>>()
            .into_iter()
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn validate_annotations_page(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["PageID"], &["PageID"], diagnostics);
    check_children_order(doc, node, &["FileLoc"], &["FileLoc"], &[], diagnostics);
    if let Some(file_loc) = doc.find_child(node, "FileLoc") {
        validate_text_element(doc, file_loc, true, diagnostics);
    }
}

fn validate_annot(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &[
            "ID",
            "Type",
            "Creator",
            "LastModDate",
            "Subtype",
            "Visible",
            "Print",
            "NoZoom",
            "NoRotate",
            "ReadOnly",
        ],
        &["ID", "Type", "Creator", "LastModDate"],
        diagnostics,
    );
    if let Some(annot_type) = doc.attribute(node, "Type")
        && !ANNOT_TYPES.contains(&annot_type)
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Type", doc.path(node)),
            "注释类型不在允许范围内",
        ));
    }
    for name in ["Visible", "Print", "NoZoom", "NoRotate", "ReadOnly"] {
        if let Some(value) = doc.attribute(node, name)
            && parse_bool(value).is_none()
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidValue,
                format!("{}/@{name}", doc.path(node)),
                "该属性必须是布尔值",
            ));
        }
    }
    check_children_order(
        doc,
        node,
        &["Remark", "Parameters", "Appearance"],
        &["Appearance"],
        &[],
        diagnostics,
    );
    if let Some(remark) = doc.find_child(node, "Remark") {
        validate_text_element(doc, remark, false, diagnostics);
    }
    if let Some(parameters) = doc.find_child(node, "Parameters") {
        validate_parameters(doc, parameters, diagnostics);
    }
    if let Some(appearance) = doc.find_child(node, "Appearance") {
        validate_appearance(doc, appearance, diagnostics);
    }
}

fn validate_parameters(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(
        doc,
        node,
        &["Parameter"],
        &["Parameter"],
        &["Parameter"],
        diagnostics,
    );
    for parameter in doc.children_named(node, "Parameter") {
        check_namespace(doc, parameter, diagnostics);
        check_attributes(doc, parameter, &["Name"], &["Name"], diagnostics);
        validate_text_element(doc, parameter, false, diagnostics);
    }
}

fn validate_appearance(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["Boundary"], &[], diagnostics);
    for child in doc.element_children(node) {
        validate_graphic_object(doc, child, diagnostics);
    }
}
