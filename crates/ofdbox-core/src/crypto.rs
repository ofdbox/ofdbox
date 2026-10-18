use crate::error::OfdError;
use crate::model::decrypt_seed::DecryptSeedView;
use crate::model::encryption::EncryptInfoView;

/// 解密出的文件加密密钥、IV 和加密方案标识。
#[derive(Clone)]
pub struct FileKey {
    key: Vec<u8>,
    iv: Vec<u8>,
    scheme: Option<String>,
}

impl FileKey {
    /// 创建文件密钥。
    pub fn new(key: Vec<u8>, iv: Vec<u8>) -> Self {
        Self {
            key,
            iv,
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
        &self.key
    }

    /// 初始化向量。
    pub fn iv(&self) -> &[u8] {
        &self.iv
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

/// 签名值验签后端。
///
/// 标准 GB/T 35275 和厂商自定义 SignedValue 都可以通过实现该 trait 接入。
pub trait SignatureVerifier {
    /// 是否支持指定签名方案标识。
    fn supports(&self, signature_method: &str) -> bool;

    /// 验证 `signed_value` 对 `content` 的签名。
    ///
    /// `content` 是 `Signature.xml` 原始字节，`signed_value` 是 `SignedValue.dat` 原始字节。
    fn verify(
        &self,
        signature_method: &str,
        content: &[u8],
        signed_value: &[u8],
    ) -> Result<(), OfdError>;
}
