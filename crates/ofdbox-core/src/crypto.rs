use std::io::Write;

use ofdbox_algo::AlgorithmRegistry;
use zeroize::Zeroizing;

use crate::error::OfdError;
use crate::model::decrypt_seed::DecryptSeedView;
use crate::model::encryption::EncryptInfoView;

/// 把摘要方法写法归一化为标准名称。
///
/// 真实文件的 `Reference/@CheckMethod` 至少有 OID、子弧、大小写等写法：
/// `1.2.156.10197.1.401`、`1.2.156.10197.1.401.1`、`1.2.156.10197.1.401.2`、
/// `SM3`、`sm3`。返回 `None` 表示不是已知方法，调用方应保留原值交给后端。
pub fn canonical_digest_method(method: &str) -> Option<&'static str> {
    let method = method.trim();
    if method.is_empty() {
        return None;
    }
    match method.to_ascii_uppercase().as_str() {
        "SM3" | "1.2.156.10197.1.401" | "1.2.156.10197.1.401.1" | "1.2.156.10197.1.401.2" => {
            Some("SM3")
        }
        // 真实样本里出现过少写一段的 OID 笔误（1.2.156.197.401），仍按 SM3 处理。
        "1.2.156.197.401" => Some("SM3"),
        "MD5" | "1.2.840.113549.2.5" => Some("MD5"),
        "SHA1" | "SHA-1" | "1.3.14.3.2.26" => Some("SHA1"),
        _ => None,
    }
}

/// 解密出的文件加密密钥、IV 和加密方案标识。
#[derive(Clone)]
pub struct FileKey {
    key: Zeroizing<Vec<u8>>,
    iv: Zeroizing<Vec<u8>>,
    scheme: Option<String>,
}

impl FileKey {
    /// 创建文件密钥。
    pub fn new(key: Vec<u8>, iv: Vec<u8>) -> Self {
        Self {
            key: Zeroizing::new(key),
            iv: Zeroizing::new(iv),
            scheme: None,
        }
    }

    /// 设置加密方案标识，供 `CryptoRegistry` 在解密时分发。
    pub fn with_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.scheme = Some(scheme.into());
        self
    }

    /// 文件加密密钥。
    pub fn key(&self) -> &[u8] {
        self.key.as_slice()
    }

    /// 初始化向量。
    pub fn iv(&self) -> &[u8] {
        self.iv.as_slice()
    }

    /// 加密方案标识。
    pub fn scheme(&self) -> Option<&str> {
        self.scheme.as_deref()
    }
}

/// 文档解密所需的密码后端。
///
/// 核心库只定义接口，不依赖具体国密实现；由 `ofdbox-crypto` 提供口令和证书实现，
/// 其他厂商可以实现远程 KMS、加密狗或私有算法。
pub trait CryptoBackend {
    /// 是否支持指定加密方案标识。
    fn supports(&self, _scheme_id: &str) -> bool {
        true
    }

    /// 从密钥描述文件中恢复文件加密密钥和 IV。
    fn unwrap_file_key(
        &self,
        operation: &EncryptInfoView,
        seed: &DecryptSeedView,
    ) -> Result<FileKey, OfdError>;

    /// 解密包内文件或明密文映射表。
    fn decrypt(&self, key: &FileKey, data: &[u8]) -> Result<Vec<u8>, OfdError>;

    /// 解密并写入输出 sink；默认实现先完整解密。
    fn decrypt_to(
        &self,
        key: &FileKey,
        data: &[u8],
        writer: &mut dyn Write,
    ) -> Result<(), OfdError> {
        let plaintext = self.decrypt(key, data)?;
        writer.write_all(&plaintext).map_err(OfdError::Io)
    }

    /// 计算摘要，用于校验签名 `References`。
    fn digest(&self, method: &str, _data: &[u8]) -> Result<Vec<u8>, OfdError> {
        Err(OfdError::UnsupportedDigest(method.to_owned()))
    }
}

/// 按方案标识分发到多个后端的注册表。
pub struct CryptoRegistry {
    backends: Vec<Box<dyn CryptoBackend>>,
}

impl CryptoRegistry {
    /// 创建空注册表。
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
        }
    }

    /// 注册一个后端，返回自身以便链式调用。
    pub fn with(mut self, backend: Box<dyn CryptoBackend>) -> Self {
        self.backends.push(backend);
        self
    }

    /// 追加一个后端。
    pub fn add(&mut self, backend: Box<dyn CryptoBackend>) {
        self.backends.push(backend);
    }

    fn backend_for(&self, scheme: &str) -> Option<&dyn CryptoBackend> {
        self.backends
            .iter()
            .map(Box::as_ref)
            .find(|backend| backend.supports(scheme))
    }
}

impl Default for CryptoRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CryptoBackend for CryptoRegistry {
    fn supports(&self, scheme_id: &str) -> bool {
        self.backend_for(scheme_id).is_some()
    }

    fn unwrap_file_key(
        &self,
        operation: &EncryptInfoView,
        seed: &DecryptSeedView,
    ) -> Result<FileKey, OfdError> {
        let scheme = seed.encrypt_case_id().unwrap_or_default();
        for backend in &self.backends {
            if !backend.supports(scheme) {
                continue;
            }
            if let Ok(file_key) = backend.unwrap_file_key(operation, seed) {
                return Ok(file_key);
            }
        }
        Err(OfdError::UnsupportedScheme(scheme.to_owned()))
    }

    fn decrypt(&self, key: &FileKey, data: &[u8]) -> Result<Vec<u8>, OfdError> {
        let Some(scheme) = key.scheme() else {
            return Err(OfdError::Crypto("文件密钥缺少加密方案标识".to_owned()));
        };
        let backend = self
            .backend_for(scheme)
            .ok_or_else(|| OfdError::UnsupportedScheme(scheme.to_owned()))?;
        backend.decrypt(key, data)
    }
}

/// 签名值容器格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignedValueFormat {
    /// GB/T 35275（沿用 GM/T 0010）或 PKCS#7 的 `SignedData`。
    SignedData,
    /// GB/T 38540 的 `SES_Signature`。
    SesSignature,
    /// GM/T 0031 的 `SES_sign`。
    GmT0031Signature,
    /// 其他或无法识别的厂商自定义结构。
    Other,
}

/// GB/T 35275 的 `signedData` OID 内容（1.2.156.10197.6.1.4.2.2）。
const OID_SIGNED_DATA_GMT: &[u8] = &[0x2A, 0x81, 0x1C, 0xCF, 0x55, 0x06, 0x01, 0x04, 0x02, 0x02];
/// PKCS#7 的 `signedData` OID 内容（1.2.840.113549.1.7.2）。
const OID_SIGNED_DATA_PKCS7: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x02];

/// 读取一个 DER TLV，返回 `(tag, 内容, 总长度)`。
fn read_tlv(bytes: &[u8]) -> Option<(u8, &[u8], usize)> {
    let tag = *bytes.first()?;
    let first = *bytes.get(1)?;
    let (length, header) = if first < 0x80 {
        (first as usize, 2)
    } else {
        let count = (first & 0x7F) as usize;
        if count == 0 || count > 4 || bytes.len() < 2 + count {
            return None;
        }
        let mut length = 0usize;
        for byte in &bytes[2..2 + count] {
            length = (length << 8) | *byte as usize;
        }
        (length, 2 + count)
    };
    let end = header.checked_add(length)?;
    if end > bytes.len() {
        return None;
    }
    Some((tag, &bytes[header..end], end))
}

/// 按内容嗅探签名值格式。
///
/// `Signature/@Type` 在真实文件里可能缺失，因此不能只靠类型字段分派。
pub fn detect_signed_value_format(value: &[u8]) -> SignedValueFormat {
    let Some((tag, content, _)) = read_tlv(value) else {
        return SignedValueFormat::Other;
    };
    if tag != 0x30 {
        return SignedValueFormat::Other;
    }
    let Some((first_tag, first, first_len)) = read_tlv(content) else {
        return SignedValueFormat::Other;
    };
    if first_tag == 0x06 {
        if first == OID_SIGNED_DATA_GMT || first == OID_SIGNED_DATA_PKCS7 {
            return SignedValueFormat::SignedData;
        }
        return SignedValueFormat::Other;
    }
    if first_tag == 0x30 {
        // GM/T 0031 `SES_sign ::= SEQUENCE { SES_SignInfo, BIT STRING }`
        let mut rest = &content[first_len..];
        if let Some((second_tag, _, _)) = read_tlv(rest)
            && second_tag == 0x03
            && is_gmt0031_sign_info(first)
        {
            return SignedValueFormat::GmT0031Signature;
        }

        // SES_Signature ::= SEQUENCE { toSign, cert OCTET STRING, alg OID, signature BIT STRING, ... }
        let mut tags = [0u8; 3];
        for slot in &mut tags {
            let Some((tag, _, len)) = read_tlv(rest) else {
                return SignedValueFormat::Other;
            };
            *slot = tag;
            rest = &rest[len..];
        }
        if tags == [0x04, 0x06, 0x03] {
            return SignedValueFormat::SesSignature;
        }
    }
    SignedValueFormat::Other
}

/// `SES_SignInfo` 的字段形状：INTEGER 版本 + SEQUENCE 印章。
fn is_gmt0031_sign_info(bytes: &[u8]) -> bool {
    let Some((tag, _, len)) = read_tlv(bytes) else {
        return false;
    };
    if tag != 0x02 {
        return false;
    }
    let Some((next_tag, _, _)) = read_tlv(&bytes[len..]) else {
        return false;
    };
    next_tag == 0x30
}

/// 签名值验签后端。
///
/// 标准 GB/T 35275、GB/T 38540 和厂商自定义 SignedValue 都可以通过实现该 trait 接入。
pub trait SignatureVerifier {
    /// 该实现处理的容器格式；默认按 `SignedData` 处理。
    fn format(&self) -> SignedValueFormat {
        SignedValueFormat::SignedData
    }

    /// 是否认领这段签名值字节。
    ///
    /// 同一格式下存在多个厂商变体时用它进一步区分；默认全部认领。
    fn accepts(&self, _signed_value: &[u8]) -> bool {
        true
    }

    /// 验证 `signed_value` 对 `content` 的签名。
    ///
    /// `content` 是 `Signature.xml` 原始字节，`signed_value` 是 `SignedValue.dat` 原始字节。
    /// 算法实现由 `algorithms` 按 OID 解析，容器本身不绑定具体算法。
    fn verify(
        &self,
        signature_method: &str,
        content: &[u8],
        signed_value: &[u8],
        algorithms: &AlgorithmRegistry,
    ) -> Result<(), OfdError>;
}

/// 按「格式 → 字节认领」分发的验签器集合。
pub struct SignatureVerifiers {
    verifiers: Vec<Box<dyn SignatureVerifier>>,
    algorithms: AlgorithmRegistry,
}

impl Default for SignatureVerifiers {
    fn default() -> Self {
        Self {
            verifiers: Vec::new(),
            algorithms: AlgorithmRegistry::gm_default(),
        }
    }
}

impl SignatureVerifiers {
    /// 创建空集合，使用默认国密算法注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 指定算法注册表。
    pub fn with_algorithms(mut self, algorithms: AlgorithmRegistry) -> Self {
        self.algorithms = algorithms;
        self
    }

    /// 追加一个验签器，返回自身以便链式调用。
    pub fn with(mut self, verifier: Box<dyn SignatureVerifier>) -> Self {
        self.verifiers.push(verifier);
        self
    }

    /// 追加一个验签器。
    pub fn add(&mut self, verifier: Box<dyn SignatureVerifier>) {
        self.verifiers.push(verifier);
    }

    /// 是否没有注册任何验签器。
    pub fn is_empty(&self) -> bool {
        self.verifiers.is_empty()
    }

    /// 已注册的验签器数量。
    pub fn len(&self) -> usize {
        self.verifiers.len()
    }

    /// 依次匹配格式与字节认领，第一个命中的实现完成验签。
    pub fn verify(
        &self,
        format: SignedValueFormat,
        signature_method: &str,
        content: &[u8],
        signed_value: &[u8],
    ) -> Result<(), OfdError> {
        for verifier in &self.verifiers {
            if verifier.format() != format || !verifier.accepts(signed_value) {
                continue;
            }
            return verifier.verify(signature_method, content, signed_value, &self.algorithms);
        }
        Err(OfdError::UnsupportedSignature(format!(
            "{signature_method}（签名值格式 {format:?}）"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CryptoBackend, FileKey, OfdError, SignedValueFormat, canonical_digest_method,
        detect_signed_value_format,
    };

    #[test]
    fn detects_signed_value_formats() {
        // GB/T 35275 signedData
        assert_eq!(
            detect_signed_value_format(&[
                0x30, 0x0C, 0x06, 0x0A, 0x2A, 0x81, 0x1C, 0xCF, 0x55, 0x06, 0x01, 0x04, 0x02, 0x02,
            ]),
            SignedValueFormat::SignedData
        );
        // PKCS#7 signedData
        assert_eq!(
            detect_signed_value_format(&[
                0x30, 0x0B, 0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x02,
            ]),
            SignedValueFormat::SignedData
        );
        // SES_Signature 形状：SEQUENCE { SEQUENCE, OCTET STRING, OID, BIT STRING }
        assert_eq!(
            detect_signed_value_format(&[
                0x30, 0x0D, 0x30, 0x02, 0x05, 0x00, 0x04, 0x01, 0x00, 0x06, 0x01, 0x2A, 0x03, 0x01,
                0x00,
            ]),
            SignedValueFormat::SesSignature
        );
        // GM/T 0031 SES_sign：SEQUENCE { SEQUENCE { INTEGER, SEQUENCE }, BIT STRING }
        assert_eq!(
            detect_signed_value_format(&[
                0x30, 0x0A, 0x30, 0x05, 0x02, 0x01, 0x01, 0x30, 0x00, 0x03, 0x01, 0x00,
            ]),
            SignedValueFormat::GmT0031Signature
        );
        // 厂商自定义 / 非 DER
        assert_eq!(
            detect_signed_value_format(&[0x30, 0x03, 0x02, 0x01, 0x01]),
            SignedValueFormat::Other
        );
        assert_eq!(
            detect_signed_value_format(b"signature"),
            SignedValueFormat::Other
        );
    }

    #[test]
    fn normalizes_digest_method_aliases() {
        for method in [
            "SM3",
            "sm3",
            "1.2.156.10197.1.401",
            "1.2.156.10197.1.401.1",
            "1.2.156.10197.1.401.2",
            "1.2.156.197.401",
        ] {
            assert_eq!(canonical_digest_method(method), Some("SM3"), "{method}");
        }
        assert_eq!(canonical_digest_method("MD5"), Some("MD5"));
        assert_eq!(canonical_digest_method("1.2.840.113549.2.5"), Some("MD5"));
        assert_eq!(canonical_digest_method("SHA1"), Some("SHA1"));
        assert_eq!(canonical_digest_method("sha-1"), Some("SHA1"));
        assert_eq!(canonical_digest_method("1.3.14.3.2.26"), Some("SHA1"));
        assert_eq!(canonical_digest_method("SHA256"), None);
        assert_eq!(canonical_digest_method(""), None);
    }

    struct EmptyBackend;

    impl CryptoBackend for EmptyBackend {
        fn unwrap_file_key(
            &self,
            _operation: &crate::model::encryption::EncryptInfoView,
            _seed: &crate::model::decrypt_seed::DecryptSeedView,
        ) -> Result<FileKey, OfdError> {
            Err(OfdError::Crypto("unused".to_owned()))
        }

        fn decrypt(&self, _key: &FileKey, _data: &[u8]) -> Result<Vec<u8>, OfdError> {
            Err(OfdError::Crypto("unused".to_owned()))
        }
    }

    #[test]
    fn empty_backend_rejects_digest_by_default() {
        assert!(matches!(
            EmptyBackend.digest("SM3", b"x"),
            Err(OfdError::UnsupportedDigest(_))
        ));
    }
}
