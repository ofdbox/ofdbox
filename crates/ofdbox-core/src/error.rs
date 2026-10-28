use std::io;

use thiserror::Error;

/// 严格模式下的诊断代码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCode {
    /// 命名空间 URI 是已知变体，但不是标准 URI。
    NamespaceVariant,
    /// 元素没有命名空间，或前缀未绑定。
    MissingNamespace,
    /// 元素或属性不在允许范围内。
    UnexpectedElement,
    /// 属性不在允许范围内。
    UnexpectedAttribute,
    /// 属性值不合法。
    InvalidAttribute,
    /// 同一父元素下出现重复的单值元素。
    DuplicateElement,
    /// 子元素顺序不符合 XSD 的 sequence。
    OutOfOrder,
    /// 缺少必选元素或属性。
    MissingRequired,
    /// 元素或属性值不合法。
    InvalidValue,
}

/// 严格模式下的一条结构化诊断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// 诊断代码。
    pub code: DiagnosticCode,
    /// 元素路径，例如 `/OFD/DocBody/DocInfo/DocID`。
    pub path: String,
    /// 面向开发者的中文说明。
    pub message: String,
}

/// `ofdbox-core` 的统一错误类型。
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OfdError {
    /// 文件或流读取失败。
    #[error("I/O 错误: {0}")]
    Io(#[from] io::Error),

    /// ZIP 容器无法读取。
    #[error("ZIP 容器错误: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// XML 不是合法 XML。
    #[error("XML 错误: {0}")]
    Xml(#[from] quick_xml::Error),

    /// 缺少 OFD 包中的必要条目。
    #[error("缺少 OFD 条目: {0}")]
    MissingEntry(String),

    /// 结构无法继续处理。
    #[error("OFD 结构无效: {0}")]
    InvalidStructure(String),

    /// 当前包以只读方式打开。
    #[error("当前 OFD 以只读方式打开")]
    ReadOnly,

    /// 严格模式下发现一处或多处不符合标准。
    #[error("OFD 不符合标准: {0:?}")]
    Conformance(Vec<Diagnostic>),

    /// 文档已加密，但尚未启用解密。
    #[error("文档已加密，需要先启用解密")]
    EncryptionRequired,

    /// 密码运算失败。
    #[error("密码运算失败: {0}")]
    Crypto(String),

    /// 没有注册对应加密方案的后端。
    #[error("不支持的加密方案: {0}")]
    UnsupportedScheme(String),

    /// 不支持的摘要算法。
    #[error("不支持的摘要算法: {0}")]
    UnsupportedDigest(String),

    /// 不支持的签名方案。
    #[error("不支持的签名方案: {0}")]
    UnsupportedSignature(String),
}
