//! 0099 加密入口模型。

use super::definition::StLoc;

/// `Encryptions.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Encryptions")]
pub struct Encryptions {
    /// 加密操作。
    #[yaserde(rename = "EncryptInfo")]
    pub encrypt_infos: Vec<EncryptInfo>,
}

/// 单次加密操作。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "EncryptInfo")]
pub struct EncryptInfo {
    /// 加密操作标识。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: String,
    /// 上一次加密操作标识。
    #[yaserde(attribute = true, rename = "Relative")]
    pub relative: Option<String>,
    /// 加解密组件。
    #[yaserde(rename = "Provider")]
    pub provider: EncryptionProvider,
    /// 加密范围。
    #[yaserde(rename = "EncryptScope")]
    pub encrypt_scope: Option<String>,
    /// 附加参数。
    #[yaserde(rename = "Parameters")]
    pub parameters: Option<EncryptionParameters>,
    /// 加密时间。
    #[yaserde(rename = "EncryptDate")]
    pub encrypt_date: Option<String>,
    /// 密钥描述文件路径。
    #[yaserde(rename = "DecryptSeedLoc")]
    pub decrypt_seed_loc: Option<StLoc>,
    /// 明密文映射表路径。
    #[yaserde(rename = "EntriesMapLoc")]
    pub entries_map_loc: Option<StLoc>,
}

/// 加解密组件。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Provider")]
pub struct EncryptionProvider {
    /// 组件名称。
    #[yaserde(attribute = true, rename = "Name")]
    pub name: String,
    /// 公司。
    #[yaserde(attribute = true, rename = "Company")]
    pub company: Option<String>,
    /// 版本。
    #[yaserde(attribute = true, rename = "Version")]
    pub version: Option<String>,
    /// 协议版本。
    #[yaserde(attribute = true, rename = "ProtocolVer")]
    pub protocol_version: Option<String>,
    /// 扩展信息。
    #[yaserde(rename = "ExtendData")]
    pub extend_data: Option<String>,
}

/// 加密参数集合。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Parameters")]
pub struct EncryptionParameters {
    /// 参数。
    #[yaserde(rename = "Parameter")]
    pub parameters: Vec<EncryptionParameter>,
}

/// 加密参数。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Parameter")]
pub struct EncryptionParameter {
    /// 参数名。
    #[yaserde(attribute = true, rename = "Name")]
    pub name: String,
    /// 值类型。
    #[yaserde(attribute = true, rename = "Type")]
    pub value_type: Option<String>,
    /// 值。
    #[yaserde(text = true)]
    pub value: String,
}


impl Encryptions {
    /// 遍历加密操作。
    pub fn encrypt_infos(&self) -> impl Iterator<Item = &EncryptInfo> {
        self.encrypt_infos.iter()
    }
}

impl EncryptInfo {
    /// 加密操作标识。
    pub fn id(&self) -> Option<&str> {
        Some(self.id.as_str())
    }

    /// 上一次加密操作标识。
    pub fn relative(&self) -> Option<&str> {
        self.relative.as_deref()
    }

    /// 密钥描述文件路径。
    pub fn decrypt_seed_loc(&self) -> Option<&str> {
        self.decrypt_seed_loc.as_ref().map(|loc| loc.as_str())
    }

    /// 明密文映射表路径。
    pub fn entries_map_loc(&self) -> Option<&str> {
        self.entries_map_loc.as_ref().map(|loc| loc.as_str())
    }
}
