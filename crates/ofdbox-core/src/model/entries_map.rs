use std::sync::Arc;

use crate::error::{Diagnostic, OfdError};
use crate::model::ofd::{
    check_attributes, check_children_unordered, check_namespace, validate_text_element,
};
use crate::options::ReadOptions;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

/// 明密文映射表的类型化视图。
#[derive(Debug, Clone)]
pub struct EncryptedEntriesView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl EncryptedEntriesView {
    /// 从 XML 字节解析明密文映射表。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if !matches!(
            doc.name(node).local.as_str(),
            "EncryptedEntries" | "EncryptEntries"
        ) {
            return Err(OfdError::InvalidStructure(
                "明密文映射表的根元素不是 EncryptedEntries".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 加密操作标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 遍历明密文对应关系。
    pub fn entries(&self) -> impl Iterator<Item = EncryptEntryView> {
        self.doc
            .children_named(self.node, "EncryptEntry")
            .map(|node| EncryptEntryView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 严格序列化为明密文映射表文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::entries_map::write_entries_map(self)
    }

    /// 严格序列化为 UTF-8 字节。
    pub fn to_bytes(&self) -> Result<Vec<u8>, OfdError> {
        self.to_xml().map(String::into_bytes)
    }

    pub(crate) fn validate_strict(&self) -> Result<(), OfdError> {
        let mut diagnostics = Vec::new();
        check_namespace(&self.doc, self.node, &mut diagnostics);
        check_attributes(&self.doc, self.node, &["ID"], &["ID"], &mut diagnostics);
        check_children_unordered(
            &self.doc,
            self.node,
            &["EncryptEntry"],
            &["EncryptEntry"],
            &["EncryptEntry"],
            &mut diagnostics,
        );
        for entry in self.doc.children_named(self.node, "EncryptEntry") {
            validate_encrypt_entry(&self.doc, entry, &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 单条明密文对应关系的视图。
#[derive(Debug, Clone)]
pub struct EncryptEntryView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl EncryptEntryView {
    /// 加密前包内文件路径。
    pub fn path(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Path")
    }

    /// 加密后密文路径。
    pub fn epath(&self) -> Option<&str> {
        self.doc.attribute(self.node, "EPath")
    }

    /// 该条目独有的密钥描述文件路径。
    pub fn decrypt_seed_loc(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "DecryptSeedLoc")
            .map(|node| self.doc.text(node).trim())
    }
}

fn validate_encrypt_entry(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["Path", "EPath"], &["Path"], diagnostics);
    check_children_unordered(doc, node, &["DecryptSeedLoc"], &[], &[], diagnostics);
    if let Some(seed) = doc.find_child(node, "DecryptSeedLoc") {
        validate_text_element(doc, seed, true, diagnostics);
    }
}
