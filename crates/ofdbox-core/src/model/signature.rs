use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::model::ofd::{
    check_attributes, check_children_order, check_children_unordered, check_namespace,
    validate_text_element,
};
use crate::options::ReadOptions;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

const SIGNATURE_TYPES: &[&str] = &["Seal", "Sign"];

/// 签名列表文件的类型化视图。
#[derive(Debug, Clone)]
pub struct SignaturesView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl SignaturesView {
    /// 从 XML 字节解析签名列表。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "Signatures" {
            return Err(OfdError::InvalidStructure(
                "签名列表的根元素不是 Signatures".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 安全标识的最大值。
    pub fn max_sign_id(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "MaxSignId")
            .map(|node| self.doc.text(node).trim())
    }

    /// 遍历签名注册信息。
    pub fn signatures(&self) -> impl Iterator<Item = SignatureEntryView> {
        self.doc
            .children_named(self.node, "Signature")
            .map(|node| SignatureEntryView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 严格序列化为签名列表文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::signature::write_signatures(self)
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
            &["MaxSignId", "Signature"],
            &[],
            &["Signature"],
            &mut diagnostics,
        );
        if let Some(max_sign_id) = self.doc.find_child(self.node, "MaxSignId") {
            validate_text_element(&self.doc, max_sign_id, true, &mut diagnostics);
        }
        for signature in self.doc.children_named(self.node, "Signature") {
            validate_signature_entry(&self.doc, signature, &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 签名列表中单条注册信息的视图。
#[derive(Debug, Clone)]
pub struct SignatureEntryView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl SignatureEntryView {
    /// 签名标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 签名类型。
    pub fn signature_type(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Type")
    }

    /// 本次签名基于的签名标识（0099 新增）。
    pub fn relative(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Relative")
    }

    /// 签名描述文件路径。
    pub fn base_loc(&self) -> Option<&str> {
        self.doc.attribute(self.node, "BaseLoc")
    }
}

/// 签名描述文件的类型化视图。
#[derive(Debug, Clone)]
pub struct SignatureView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl SignatureView {
    /// 从 XML 字节解析签名描述文件。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "Signature" {
            return Err(OfdError::InvalidStructure(
                "签名描述文件的根元素不是 Signature".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 签名保护信息。
    pub fn signed_info(&self) -> Option<SignedInfoView> {
        self.doc
            .find_child(self.node, "SignedInfo")
            .map(|node| SignedInfoView {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    /// 签名值文件路径。
    pub fn signed_value(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "SignedValue")
            .map(|node| self.doc.text(node).trim())
    }

    /// 严格序列化为签名描述文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::signature::write_signature(self)
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
            &["SignedInfo", "SignedValue"],
            &["SignedInfo", "SignedValue"],
            &[],
            &mut diagnostics,
        );
        if let Some(info) = self.doc.find_child(self.node, "SignedInfo") {
            validate_signed_info(&self.doc, info, &mut diagnostics);
        }
        if let Some(value) = self.doc.find_child(self.node, "SignedValue") {
            validate_text_element(&self.doc, value, true, &mut diagnostics);
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// `SignedInfo` 的类型化视图。
#[derive(Debug, Clone)]
pub struct SignedInfoView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl SignedInfoView {
    /// 签章组件提供者信息。
    pub fn provider(&self) -> Option<ProviderView> {
        self.doc
            .find_child(self.node, "Provider")
            .map(|node| ProviderView {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    /// 签名算法代码。
    pub fn signature_method(&self) -> Option<&str> {
        self.text_child("SignatureMethod")
    }

    /// 签名时间。
    pub fn signature_date_time(&self) -> Option<&str> {
        self.text_child("SignatureDateTime")
    }

    /// 受保护文件的摘要列表。
    pub fn references(&self) -> Option<ReferencesView> {
        self.doc
            .find_child(self.node, "References")
            .map(|node| ReferencesView {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    /// 签名外观。
    pub fn stamp_annots(&self) -> impl Iterator<Item = StampAnnotView> {
        self.doc
            .children_named(self.node, "StampAnnot")
            .map(|node| StampAnnotView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 签名扩展属性（0099 新增）。
    pub fn parameters(&self) -> impl Iterator<Item = SignatureParameterView> {
        let mut values = Vec::new();
        if let Some(parameters) = self.doc.find_child(self.node, "Parameters") {
            for node in self.doc.children_named(parameters, "Parameter") {
                values.push(SignatureParameterView {
                    doc: Arc::clone(&self.doc),
                    node,
                });
            }
        }
        values.into_iter()
    }

    /// 电子印章信息。
    pub fn seal(&self) -> Option<SealView> {
        self.doc.find_child(self.node, "Seal").map(|node| SealView {
            doc: Arc::clone(&self.doc),
            node,
        })
    }

    fn text_child(&self, name: &str) -> Option<&str> {
        self.doc
            .find_child(self.node, name)
            .map(|node| self.doc.text(node).trim())
    }
}

/// 签章组件提供者的视图。
#[derive(Debug, Clone)]
pub struct ProviderView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl ProviderView {
    /// 提供者名称。
    ///
    /// 标准写法是 `Provider@ProviderName`；真实样本也有写成同名子元素的，
    /// 属性缺失时回退到子元素。
    pub fn provider_name(&self) -> Option<&str> {
        self.attribute_or_child("ProviderName")
    }

    /// 提供者版本。
    pub fn version(&self) -> Option<&str> {
        self.attribute_or_child("Version")
    }

    /// 提供者制造商。
    pub fn company(&self) -> Option<&str> {
        self.attribute_or_child("Company")
    }

    /// 接口协议版本（0099 新增）。
    pub fn protocol_version(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ProtocolVer")
    }

    /// 组件扩展信息（0099 新增）。
    pub fn extend_data(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "ExtendData")
            .map(|node| self.doc.text(node).trim())
    }

    fn attribute_or_child(&self, name: &str) -> Option<&str> {
        self.doc.attribute(self.node, name).or_else(|| {
            self.doc
                .find_child(self.node, name)
                .map(|node| self.doc.text(node).trim())
                .filter(|value| !value.is_empty())
        })
    }
}

/// 签名扩展属性的视图。
#[derive(Debug, Clone)]
pub struct SignatureParameterView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl SignatureParameterView {
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

/// 摘要列表的视图。
#[derive(Debug, Clone)]
pub struct ReferencesView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl ReferencesView {
    /// 摘要方法。
    pub fn check_method(&self) -> Option<&str> {
        self.doc.attribute(self.node, "CheckMethod")
    }

    /// 遍历文件摘要。
    pub fn references(&self) -> impl Iterator<Item = ReferenceView> {
        self.doc
            .children_named(self.node, "Reference")
            .map(|node| ReferenceView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 单个文件摘要的视图。
#[derive(Debug, Clone)]
pub struct ReferenceView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl ReferenceView {
    /// 被保护文件路径。
    pub fn file_ref(&self) -> Option<&str> {
        self.doc.attribute(self.node, "FileRef")
    }

    /// Base64 编码的摘要值。
    pub fn check_value(&self) -> &str {
        self.doc
            .find_child(self.node, "CheckValue")
            .map(|node| self.doc.text(node).trim())
            .unwrap_or_default()
    }
}

/// 签名外观的视图。
#[derive(Debug, Clone)]
pub struct StampAnnotView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl StampAnnotView {
    /// 外观注释标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 外观注释所在页面。
    pub fn page_ref(&self) -> Option<&str> {
        self.doc.attribute(self.node, "PageRef")
    }

    /// 外观外边框。
    pub fn boundary(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Boundary")
    }

    /// 外观裁剪区域。
    pub fn clip(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Clip")
    }
}

/// 电子印章信息的视图。
#[derive(Debug, Clone)]
pub struct SealView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl SealView {
    /// 电子印章文件路径。
    pub fn base_loc(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "BaseLoc")
            .map(|node| self.doc.text(node).trim())
    }

    /// 印模图片位置（0099 新增）。
    pub fn image_loc(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "ImageLoc")
            .map(|node| self.doc.text(node).trim())
    }
}

fn validate_signature_entry(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["ID", "Type", "Relative", "BaseLoc"],
        &["ID", "BaseLoc"],
        diagnostics,
    );
    if let Some(signature_type) = doc.attribute(node, "Type")
        && !SIGNATURE_TYPES.contains(&signature_type)
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Type", doc.path(node)),
            "签名类型只能是 Seal 或 Sign",
        ));
    }
    check_children_order(doc, node, &[], &[], &[], diagnostics);
}

fn validate_signed_info(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_unordered(
        doc,
        node,
        &[
            "Provider",
            "SignatureMethod",
            "SignatureDateTime",
            "Parameters",
            "References",
            "StampAnnot",
            "Seal",
        ],
        &["Provider", "References"],
        &["StampAnnot"],
        diagnostics,
    );
    if let Some(provider) = doc.find_child(node, "Provider") {
        validate_provider(doc, provider, diagnostics);
    }
    for name in ["SignatureMethod", "SignatureDateTime"] {
        if let Some(child) = doc.find_child(node, name) {
            validate_text_element(doc, child, false, diagnostics);
        }
    }
    if let Some(parameters) = doc.find_child(node, "Parameters") {
        validate_parameters(doc, parameters, diagnostics);
    }
    if let Some(references) = doc.find_child(node, "References") {
        validate_references(doc, references, diagnostics);
    }
    for stamp_annot in doc.children_named(node, "StampAnnot") {
        validate_stamp_annot(doc, stamp_annot, diagnostics);
    }
    if let Some(seal) = doc.find_child(node, "Seal") {
        validate_seal(doc, seal, diagnostics);
    }
}

fn validate_provider(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["ProviderName", "Version", "Company", "ProtocolVer"],
        &["ProviderName"],
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

fn validate_references(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["CheckMethod"], &[], diagnostics);
    // GB/T 33190 表 68 把 CheckMethod 定义为 xs:string，真实文件里既有
    // `MD5`/`SM3` 也有 OID、大小写变体，这里只拒绝空值，方法是否可识别由
    // 验签时的 `canonical_digest_method` 判断。
    if let Some(check_method) = doc.attribute(node, "CheckMethod")
        && check_method.trim().is_empty()
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@CheckMethod", doc.path(node)),
            "摘要方法不能为空",
        ));
    }
    check_children_order(
        doc,
        node,
        &["Reference"],
        &["Reference"],
        &["Reference"],
        diagnostics,
    );
    for reference in doc.children_named(node, "Reference") {
        validate_reference(doc, reference, diagnostics);
    }
}

fn validate_reference(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["FileRef"], &["FileRef"], diagnostics);
    check_children_order(
        doc,
        node,
        &["CheckValue"],
        &["CheckValue"],
        &[],
        diagnostics,
    );
    if let Some(check_value) = doc.find_child(node, "CheckValue") {
        validate_text_element(doc, check_value, true, diagnostics);
    }
}

fn validate_stamp_annot(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["ID", "PageRef", "Boundary", "Clip"],
        &["ID", "PageRef", "Boundary"],
        diagnostics,
    );
    check_children_order(doc, node, &[], &[], &[], diagnostics);
}

fn validate_seal(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_unordered(doc, node, &["BaseLoc", "ImageLoc"], &[], &[], diagnostics);
    for name in ["BaseLoc", "ImageLoc"] {
        if let Some(child) = doc.find_child(node, name) {
            validate_text_element(doc, child, true, diagnostics);
        }
    }
}

/// 单个 `Reference` 的摘要校验结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceDigest {
    /// 被保护文件路径。
    pub file_ref: String,
    /// 摘要方法。
    pub check_method: String,
    /// XML 中记录的摘要值。
    pub expected: Vec<u8>,
    /// 实际计算出的摘要值。
    pub actual: Vec<u8>,
}

impl ReferenceDigest {
    /// 摘要是否一致。
    pub fn matches(&self) -> bool {
        self.expected == self.actual
    }
}
