use std::sync::Arc;

use crate::error::{Diagnostic, OfdError};
use crate::model::ofd::{
    check_attributes, check_children_unordered, check_namespace, validate_text_element,
};
use crate::options::ReadOptions;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

/// 完整性保护描述文件 `OFDEntries.xml` 的类型化视图。
///
/// 按附录 D 表 D.1：`CreatorName`/`Version`/`CreationDate` 是根元素属性，
/// `File/@FileLoc` 也是属性（不是子元素）。
#[derive(Debug, Clone)]
pub struct DocEntriesView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl DocEntriesView {
    /// 从 XML 字节解析完整性保护描述。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "DocEntries" {
            return Err(OfdError::InvalidStructure(
                "完整性描述文件的根元素不是 DocEntries".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 文档标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 创建者名称。
    pub fn creator_name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "CreatorName")
    }

    /// 版本。
    pub fn version(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Version")
    }

    /// 创建日期。
    pub fn creation_date(&self) -> Option<&str> {
        self.doc.attribute(self.node, "CreationDate")
    }

    /// 防夹带文件列表。
    pub fn files(&self) -> impl Iterator<Item = DocEntryFileView> {
        let mut values = Vec::new();
        if let Some(list) = self.doc.find_child(self.node, "FileList") {
            for node in self.doc.children_named(list, "File") {
                values.push(DocEntryFileView {
                    doc: Arc::clone(&self.doc),
                    node,
                });
            }
        }
        values.into_iter()
    }

    /// 防夹带文件列表的签名值路径。
    pub fn signed_value_loc(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "SignedValueLoc")
            .map(|node| self.doc.text(node).trim())
    }

    /// 严格序列化为完整性描述文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::integrity::write_doc_entries(self)
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
            &["ID", "CreatorName", "Version", "CreationDate"],
            &["ID"],
            &mut diagnostics,
        );
        check_children_unordered(
            &self.doc,
            self.node,
            &["FileList", "SignedValueLoc"],
            &["FileList"],
            &[],
            &mut diagnostics,
        );
        if let Some(list) = self.doc.find_child(self.node, "FileList") {
            validate_file_list(&self.doc, list, &mut diagnostics);
        }
        if let Some(value) = self.doc.find_child(self.node, "SignedValueLoc") {
            validate_text_element(&self.doc, value, true, &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 防夹带文件列表中的单个文件。
#[derive(Debug, Clone)]
pub struct DocEntryFileView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl DocEntryFileView {
    /// 文件标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 包内文件路径。
    pub fn file_loc(&self) -> Option<&str> {
        self.doc.attribute(self.node, "FileLoc")
    }
}

fn validate_file_list(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_unordered(doc, node, &["File"], &["File"], &["File"], diagnostics);
    for file in doc.children_named(node, "File") {
        check_namespace(doc, file, diagnostics);
        check_attributes(
            doc,
            file,
            &["ID", "FileLoc"],
            &["ID", "FileLoc"],
            diagnostics,
        );
        check_children_unordered(doc, file, &[], &[], &[], diagnostics);
    }
}
