use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::model::ofd::{
    check_attributes, check_children_order, check_namespace, validate_text_element,
};
use crate::options::ReadOptions;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

/// 自定义标引列表的类型化视图。
#[derive(Debug, Clone)]
pub struct CustomTagsView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl CustomTagsView {
    /// 从 XML 字节解析自定义标引列表。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "CustomTags" {
            return Err(OfdError::InvalidStructure(
                "自定义标引列表的根元素不是 CustomTags".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 遍历自定义标引入口。
    pub fn custom_tags(&self) -> impl Iterator<Item = CustomTagView> {
        self.doc
            .children_named(self.node, "CustomTag")
            .map(|node| CustomTagView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 严格序列化为自定义标引列表文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::custom_tags::write_custom_tags(self)
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
            &["CustomTag"],
            &[],
            &["CustomTag"],
            &mut diagnostics,
        );
        for tag in self.doc.children_named(self.node, "CustomTag") {
            validate_custom_tag(&self.doc, tag, &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 单个自定义标引入口的视图。
#[derive(Debug, Clone)]
pub struct CustomTagView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl CustomTagView {
    /// 自定义标引的类型标识。
    ///
    /// 标准正文使用 `TypeID`，附录 XSD 写作 `NameSpace`，两种写法都接受。
    pub fn type_id(&self) -> Option<&str> {
        self.doc
            .attribute(self.node, "TypeID")
            .or_else(|| self.doc.attribute(self.node, "NameSpace"))
    }

    /// XSD 中的 `NameSpace` 属性原文。
    pub fn name_space(&self) -> Option<&str> {
        self.doc.attribute(self.node, "NameSpace")
    }

    /// 适用的 Schema 文件路径。
    pub fn schema_loc(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "SchemaLoc")
            .map(|node| self.doc.text(node).trim())
    }

    /// 标引内容文件路径。
    pub fn file_loc(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "FileLoc")
            .map(|node| self.doc.text(node).trim())
    }
}

fn validate_custom_tag(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["TypeID", "NameSpace"], &[], diagnostics);
    if doc.attribute(node, "TypeID").is_none() && doc.attribute(node, "NameSpace").is_none() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            format!("{}/@TypeID", doc.path(node)),
            "缺少类型标识 TypeID 或 NameSpace",
        ));
    }
    check_children_order(
        doc,
        node,
        &["SchemaLoc", "FileLoc"],
        &["FileLoc"],
        &[],
        diagnostics,
    );
    if let Some(schema_loc) = doc.find_child(node, "SchemaLoc") {
        validate_text_element(doc, schema_loc, true, diagnostics);
    }
    if let Some(file_loc) = doc.find_child(node, "FileLoc") {
        validate_text_element(doc, file_loc, true, diagnostics);
    }
}
