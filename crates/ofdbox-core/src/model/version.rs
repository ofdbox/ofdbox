use std::sync::Arc;

use crate::error::{Diagnostic, OfdError};
use crate::model::ofd::{
    check_attributes, check_children_order, check_namespace, validate_text_element,
};
use crate::options::ReadOptions;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

/// 版本描述文件的类型化视图。
#[derive(Debug, Clone)]
pub struct Version {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Version {
    /// 从 XML 字节解析版本描述文件。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "DocVersion" {
            return Err(OfdError::InvalidStructure(
                "版本描述文件的根元素不是 DocVersion".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 版本标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 版本号。
    pub fn version(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Version")
    }

    /// 版本名称。
    pub fn name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Name")
    }

    /// 版本创建日期。
    pub fn creation_date(&self) -> Option<&str> {
        self.doc.attribute(self.node, "CreationDate")
    }

    /// 版本包含的文件。
    pub fn files(&self) -> impl Iterator<Item = VersionFile> {
        let mut values = Vec::new();
        if let Some(file_list) = self.doc.find_child(self.node, "FileList") {
            for node in self.doc.children_named(file_list, "File") {
                values.push(VersionFile {
                    doc: Arc::clone(&self.doc),
                    node,
                });
            }
        }
        values.into_iter()
    }

    /// 版本对应的文档根节点路径。
    pub fn doc_root(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "DocRoot")
            .map(|node| self.doc.text(node).trim())
    }

    /// 严格序列化为版本描述文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::version::write_doc_version(self)
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
            &["ID", "Version", "Name", "CreationDate"],
            &["ID"],
            &mut diagnostics,
        );
        check_children_order(
            &self.doc,
            self.node,
            &["FileList", "DocRoot"],
            &["FileList", "DocRoot"],
            &[],
            &mut diagnostics,
        );
        if let Some(file_list) = self.doc.find_child(self.node, "FileList") {
            validate_file_list(&self.doc, file_list, &mut diagnostics);
        }
        if let Some(doc_root) = self.doc.find_child(self.node, "DocRoot") {
            validate_text_element(&self.doc, doc_root, true, &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 版本中单个文件的视图。
#[derive(Debug, Clone)]
pub struct VersionFile {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl VersionFile {
    /// 文件标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 文件路径。
    pub fn location(&self) -> &str {
        self.doc.text(self.node).trim()
    }
}

fn validate_file_list(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(doc, node, &["File"], &["File"], &["File"], diagnostics);
    for file in doc.children_named(node, "File") {
        check_namespace(doc, file, diagnostics);
        check_attributes(doc, file, &["ID"], &["ID"], diagnostics);
        validate_text_element(doc, file, true, diagnostics);
    }
}
