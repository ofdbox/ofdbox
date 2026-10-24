use std::collections::HashSet;
use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::model::ofd::{
    check_attributes, check_children_unordered, check_namespace, validate_text_element,
};
use crate::options::ReadOptions;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

/// 解密入口文件 `Encryptions.xml` 的类型化视图。
#[derive(Debug, Clone)]
pub struct EncryptionsView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl EncryptionsView {
    /// 从 XML 字节解析解密入口。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if !matches!(doc.name(node).local.as_str(), "Encryptions" | "EncryptList") {
            return Err(OfdError::InvalidStructure(
                "解密入口的根元素不是 Encryptions".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 遍历加密操作记录。
    pub fn encrypt_infos(&self) -> impl Iterator<Item = EncryptInfoView> {
        self.doc
            .children_named(self.node, "EncryptInfo")
            .map(|node| EncryptInfoView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 严格序列化为解密入口文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::encryption::write_encryptions(self)
    }

    /// 严格序列化为 UTF-8 字节。
    pub fn to_bytes(&self) -> Result<Vec<u8>, OfdError> {
        self.to_xml().map(String::into_bytes)
    }

    pub(crate) fn validate_strict(&self) -> Result<(), OfdError> {
        let mut diagnostics = Vec::new();
        check_namespace(&self.doc, self.node, &mut diagnostics);
        check_attributes(&self.doc, self.node, &[], &[], &mut diagnostics);
        check_children_unordered(
            &self.doc,
            self.node,
            &["EncryptInfo"],
            &["EncryptInfo"],
            &["EncryptInfo"],
            &mut diagnostics,
        );
        let mut ids = HashSet::new();
        for info in self.doc.children_named(self.node, "EncryptInfo") {
            if let Some(id) = self.doc.attribute(info, "ID")
                && !ids.insert(id.to_owned())
            {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::DuplicateElement,
                    format!("{}/@ID", self.doc.path(info)),
                    "EncryptInfo 的 ID 重复",
                ));
            }
            validate_encrypt_info(&self.doc, info, &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 单次加密操作的视图。
#[derive(Debug, Clone)]
pub struct EncryptInfoView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl EncryptInfoView {
    /// 加密操作标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 上一次加密操作标识。
    pub fn relative(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Relative")
    }

    /// 加解密组件信息。
    pub fn provider(&self) -> Option<EncryptionProviderView> {
        self.doc
            .find_child(self.node, "Provider")
            .map(|node| EncryptionProviderView {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    /// 加密范围描述。
    pub fn encrypt_scope(&self) -> Option<&str> {
        self.text_child("EncryptScope")
    }

    /// 加密时间。
    pub fn encrypt_date(&self) -> Option<&str> {
        self.text_child("EncryptDate")
    }

    /// 加密操作附加参数。
    pub fn parameters(&self) -> impl Iterator<Item = EncryptionParameterView> {
        let mut values = Vec::new();
        if let Some(parameters) = self.doc.find_child(self.node, "Parameters") {
            for node in self.doc.children_named(parameters, "Parameter") {
                values.push(EncryptionParameterView {
                    doc: Arc::clone(&self.doc),
                    node,
                });
            }
        }
        values.into_iter()
    }

    /// 密钥描述文件路径。
    pub fn decrypt_seed_loc(&self) -> Option<&str> {
        self.text_child("DecryptSeedLoc")
    }

    /// 明密文映射表路径。
    pub fn entries_map_loc(&self) -> Option<&str> {
        self.text_child("EntriesMapLoc")
    }

    fn text_child(&self, name: &str) -> Option<&str> {
        self.doc
            .find_child(self.node, name)
            .map(|node| self.doc.text(node).trim())
    }
}

/// 加解密组件的视图。
#[derive(Debug, Clone)]
pub struct EncryptionProviderView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl EncryptionProviderView {
    /// 组件名称。
    pub fn name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Name")
    }

    /// 组件公司名称。
    pub fn company(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Company")
    }

    /// 组件版本。
    pub fn version(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Version")
    }

    /// 接口协议版本。
    pub fn protocol_version(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ProtocolVer")
    }

    /// 组件扩展信息。
    pub fn extend_data(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "ExtendData")
            .map(|node| self.doc.text(node).trim())
    }
}

/// 加密操作附加参数的视图。
#[derive(Debug, Clone)]
pub struct EncryptionParameterView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl EncryptionParameterView {
    /// 参数名称。
    pub fn name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Name")
    }

    /// 参数值类型。
    pub fn value_type(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Type")
    }

    /// 参数值。
    pub fn value(&self) -> &str {
        self.doc.text(self.node).trim()
    }
}

fn validate_encrypt_info(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["ID", "Relative"], &["ID"], diagnostics);
    check_children_unordered(
        doc,
        node,
        &[
            "Provider",
            "EncryptScope",
            "Parameters",
            "EncryptDate",
            "DecryptSeedLoc",
            "EntriesMapLoc",
        ],
        &[
            "Provider",
            "EncryptScope",
            "DecryptSeedLoc",
            "EntriesMapLoc",
        ],
        &[],
        diagnostics,
    );
    if let Some(provider) = doc.find_child(node, "Provider") {
        validate_provider(doc, provider, diagnostics);
    }
    for name in ["EncryptScope", "EncryptDate"] {
        if let Some(child) = doc.find_child(node, name) {
            validate_text_element(doc, child, true, diagnostics);
        }
    }
    if let Some(parameters) = doc.find_child(node, "Parameters") {
        validate_parameters(doc, parameters, diagnostics);
    }
    for name in ["DecryptSeedLoc", "EntriesMapLoc"] {
        if let Some(child) = doc.find_child(node, name) {
            validate_text_element(doc, child, true, diagnostics);
        }
    }
}

fn validate_provider(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["Name", "Company", "Version", "ProtocolVer"],
        &["Name"],
        diagnostics,
    );
    check_children_unordered(doc, node, &["ExtendData"], &[], &[], diagnostics);
    if let Some(extend_data) = doc.find_child(node, "ExtendData") {
        validate_text_element(doc, extend_data, false, diagnostics);
    }
}

fn validate_parameters(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_unordered(
        doc,
        node,
        &["Parameter"],
        &["Parameter"],
        &["Parameter"],
        diagnostics,
    );
    for parameter in doc.children_named(node, "Parameter") {
        check_namespace(doc, parameter, diagnostics);
        check_attributes(doc, parameter, &["Name", "Type"], &["Name"], diagnostics);
        validate_text_element(doc, parameter, false, diagnostics);
    }
}
