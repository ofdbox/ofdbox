//! 0099 密钥描述模型。

/// `DecryptSeed.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "DecryptSeed")]
pub struct DecryptSeed {
    /// 加密操作标识。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: String,
    /// 加密保护方案标识。
    #[yaserde(attribute = true, rename = "EncryptCaseId")]
    pub encrypt_case_id: String,
    /// 可解密用户。
    #[yaserde(rename = "UserInfo")]
    pub users: Vec<DecryptUser>,
    /// 扩展参数。
    #[yaserde(rename = "ExtendParams")]
    pub extend_params: Option<ExtendParams>,
}

/// 可解密用户。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "UserInfo")]
pub struct DecryptUser {
    /// 用户名称。
    #[yaserde(attribute = true, rename = "UserName")]
    pub user_name: String,
    /// 用户类型。
    #[yaserde(attribute = true, rename = "UserType")]
    pub user_type: Option<String>,
    /// 用户证书。
    #[yaserde(rename = "UserCert")]
    pub user_cert: Option<String>,
    /// 文件对称加密的包装密钥。
    #[yaserde(rename = "EncryptedWK")]
    pub encrypted_work_key: Option<String>,
    /// 初始化向量。
    #[yaserde(rename = "IVValue")]
    pub iv_value: Option<String>,
}

/// 扩展参数。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "ExtendParams")]
pub struct ExtendParams {
    /// 参数。
    #[yaserde(rename = "Parameter")]
    pub parameters: Vec<ExtendParameter>,
}

/// 扩展参数项。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Parameter")]
pub struct ExtendParameter {
    /// 参数名称。
    #[yaserde(attribute = true, rename = "Name")]
    pub name: String,
    /// `Value` 属性。
    #[yaserde(attribute = true, rename = "Value")]
    pub value_attr: Option<String>,
    /// 元素文本。
    #[yaserde(text = true)]
    pub text: String,
}


impl DecryptSeed {
    /// 加密保护方案标识。
    pub fn encrypt_case_id(&self) -> Option<&str> {
        Some(self.encrypt_case_id.as_str())
    }

    /// 可解密用户。
    pub fn users(&self) -> impl Iterator<Item = &DecryptUser> {
        self.users.iter()
    }
}

impl DecryptUser {
    /// 用户名称。
    pub fn user_name(&self) -> Option<&str> {
        Some(self.user_name.as_str())
    }

    /// 用户类型。
    pub fn user_type(&self) -> Option<&str> {
        self.user_type.as_deref()
    }

    /// 用户证书。
    pub fn user_cert(&self) -> Option<&str> {
        self.user_cert.as_deref()
    }

    /// 包装密钥。
    pub fn encrypted_work_key(&self) -> Option<&str> {
        self.encrypted_work_key.as_deref()
    }

    /// 初始化向量。
    pub fn iv_value(&self) -> Option<&str> {
        self.iv_value.as_deref()
    }
}
