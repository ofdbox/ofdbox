//! 0099 明密文映射模型。

use super::definition::StLoc;

/// `Entries.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "EncryptedEntries")]
pub struct EncryptedEntries {
    /// 加密操作标识。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: String,
    /// 明密文条目。
    #[yaserde(rename = "EncryptEntry")]
    pub entries: Vec<EncryptEntry>,
}

/// 明密文条目。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "EncryptEntry")]
pub struct EncryptEntry {
    /// 加密前路径。
    #[yaserde(attribute = true, rename = "Path")]
    pub path: String,
    /// 加密后路径。
    #[yaserde(attribute = true, rename = "EPath")]
    pub epath: String,
    /// 条目独立密钥描述路径。
    #[yaserde(attribute = true, rename = "DecryptSeedLoc")]
    pub decrypt_seed_loc: Option<StLoc>,
}


impl EncryptedEntries {
    /// 明密文条目。
    pub fn entries(&self) -> impl Iterator<Item = &EncryptEntry> {
        self.entries.iter()
    }
}

impl EncryptEntry {
    /// 加密前路径。
    pub fn path(&self) -> Option<&str> {
        Some(self.path.as_str())
    }

    /// 加密后路径。
    pub fn epath(&self) -> Option<&str> {
        Some(self.epath.as_str())
    }

    /// 条目独立密钥描述路径。
    pub fn decrypt_seed_loc(&self) -> Option<&str> {
        self.decrypt_seed_loc.as_ref().map(|loc| loc.as_str())
    }
}
