use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::model::ofd::{
    check_attributes, check_children_unordered, check_namespace, validate_text_element,
};
use crate::options::ReadOptions;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

/// 密钥描述文件 `DecryptSeed` 的类型化视图。
#[derive(Debug, Clone)]
pub struct DecryptSeed {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl DecryptSeed {
    /// 从 XML 字节解析密钥描述文件。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if !matches!(doc.name(node).local.as_str(), "DecryptSeed" | "DecyptSeed") {
            return Err(OfdError::InvalidStructure(
                "密钥描述文件的根元素不是 DecryptSeed".to_owned(),
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

    /// 加密保护方案标识。
    pub fn encrypt_case_id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "EncryptCaseId")
    }

    /// 可解密用户列表。
    pub fn users(&self) -> impl Iterator<Item = DecryptUser> {
        self.doc
            .children_named(self.node, "UserInfo")
            .map(|node| DecryptUser {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 扩展参数。
    pub fn extend_params(&self) -> Option<ExtendParams> {
        self.doc
            .find_child(self.node, "ExtendParams")
            .map(|node| ExtendParams {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    /// 严格序列化为密钥描述文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::decrypt_seed::write_decrypt_seed(self)
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
            &["ID", "EncryptCaseId"],
            &["ID", "EncryptCaseId"],
            &mut diagnostics,
        );
        check_children_unordered(
            &self.doc,
            self.node,
            &["UserInfo", "ExtendParams"],
            &["UserInfo", "ExtendParams"],
            &["UserInfo"],
            &mut diagnostics,
        );
        for user in self.doc.children_named(self.node, "UserInfo") {
            validate_user(&self.doc, user, &mut diagnostics);
        }
        if let Some(params) = self.doc.find_child(self.node, "ExtendParams") {
            validate_extend_params(&self.doc, params, &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 可解密用户的视图。
#[derive(Debug, Clone)]
pub struct DecryptUser {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl DecryptUser {
    /// 用户名称。
    pub fn user_name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "UserName")
    }

    /// 用户角色类型，默认 `User`。
    pub fn user_type(&self) -> Option<&str> {
        self.doc.attribute(self.node, "UserType")
    }

    /// 用户公钥证书。
    pub fn user_cert(&self) -> Option<&str> {
        self.text_child("UserCert")
    }

    /// 文件对称加密的包装密钥。
    pub fn encrypted_work_key(&self) -> Option<&str> {
        self.text_child("EncryptedWK")
            .or_else(|| self.text_child("EncrypteWK"))
    }

    /// 初始化向量值。
    pub fn iv_value(&self) -> Option<&str> {
        self.text_child("IVValue")
    }

    fn text_child(&self, name: &str) -> Option<&str> {
        self.doc
            .find_child(self.node, name)
            .map(|node| self.doc.text(node).trim())
    }
}

/// 扩展参数的视图。
#[derive(Debug, Clone)]
pub struct ExtendParams {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl ExtendParams {
    /// 遍历扩展参数。
    pub fn parameters(&self) -> impl Iterator<Item = ExtendParameter> {
        self.doc
            .children_named(self.node, "Parameter")
            .map(|node| ExtendParameter {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// `ExtendParams/Parameter` 的视图。
#[derive(Debug, Clone)]
pub struct ExtendParameter {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl ExtendParameter {
    /// 参数名称。
    pub fn name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Name")
    }

    /// `Value` 属性。
    pub fn value_attr(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Value")
    }

    /// 元素文本内容。
    pub fn text(&self) -> &str {
        self.doc.text(self.node).trim()
    }

    /// 语义值：优先文本，文本为空时取 `Value` 属性。
    pub fn value(&self) -> &str {
        let text = self.text();
        if text.is_empty() {
            self.value_attr().unwrap_or_default()
        } else {
            text
        }
    }
}

fn validate_user(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["UserName", "UserType"],
        &["UserName"],
        diagnostics,
    );
    check_children_unordered(
        doc,
        node,
        &["UserCert", "EncryptedWK", "EncrypteWK", "IVValue"],
        &[],
        &[],
        diagnostics,
    );
    for name in ["UserCert", "EncryptedWK", "EncrypteWK", "IVValue"] {
        if let Some(child) = doc.find_child(node, name) {
            validate_text_element(doc, child, true, diagnostics);
        }
    }
    let keys = doc.children_named(node, "EncryptedWK").count()
        + doc.children_named(node, "EncrypteWK").count();
    if keys == 0 {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            format!("{}/EncryptedWK", doc.path(node)),
            "缺少包装密钥 EncryptedWK",
        ));
    } else if keys > 1 {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::DuplicateElement,
            format!("{}/EncryptedWK", doc.path(node)),
            "包装密钥重复出现",
        ));
    }
}

fn validate_extend_params(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_unordered(doc, node, &["Parameter"], &[], &["Parameter"], diagnostics);
    for parameter in doc.children_named(node, "Parameter") {
        check_namespace(doc, parameter, diagnostics);
        check_attributes(doc, parameter, &["Name", "Value"], &["Name"], diagnostics);
        validate_text_element(doc, parameter, false, diagnostics);
    }
}
