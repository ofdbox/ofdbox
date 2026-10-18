use std::collections::HashSet;
use std::io::{Cursor, Read, Write};
use std::sync::Arc;

use base64::Engine;
use ofdbox_core::OfdError;
use sm2::elliptic_curve::rand_core::TryCryptoRng;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::backend::{CERTIFICATE_CASE_ID, PASSWORD_CASE_ID};
use crate::certificate::encrypt_file_key_with_certificate;
use crate::cipher::encrypt_cbc;
use crate::password::FILE_KEY_LEN;
use crate::password::wrap_file_key;

/// 加密方案。
pub enum EncryptionScheme {
    /// 口令方案 1.1.1。
    Password(String),
    /// 证书方案 1.1.2，值为 X.509 证书 DER。
    Certificate(Vec<u8>),
}

/// 文件过滤函数。
pub type EncryptionFilter = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// 加密选项。
pub struct EncryptOptions {
    /// 可解密用户名称。
    pub username: String,
    /// 用户角色类型。
    pub user_type: Option<String>,
    /// 加密方案。
    pub scheme: EncryptionScheme,
    /// 文件过滤；返回 false 的文件保持明文。
    pub filter: Option<EncryptionFilter>,
}

impl EncryptOptions {
    /// 口令加密选项。
    pub fn password(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            user_type: None,
            scheme: EncryptionScheme::Password(password.into()),
            filter: None,
        }
    }

    /// 证书加密选项。
    pub fn certificate(username: impl Into<String>, certificate_der: Vec<u8>) -> Self {
        Self {
            username: username.into(),
            user_type: None,
            scheme: EncryptionScheme::Certificate(certificate_der),
            filter: None,
        }
    }

    /// 设置用户角色类型。
    pub fn with_user_type(mut self, user_type: impl Into<String>) -> Self {
        self.user_type = Some(user_type.into());
        self
    }

    /// 设置文件过滤。
    pub fn with_filter(mut self, filter: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
        self.filter = Some(Arc::new(filter));
        self
    }
}

/// 使用系统随机源加密一个未加密的 OFD 包。
#[cfg(not(target_arch = "wasm32"))]
pub fn encrypt_package(input: &[u8], options: &EncryptOptions) -> Result<Vec<u8>, OfdError> {
    encrypt_package_with_rng(input, options, &mut getrandom::SysRng)
}

/// 使用调用方提供的随机源加密一个未加密的 OFD 包。
pub fn encrypt_package_with_rng<R: TryCryptoRng>(
    input: &[u8],
    options: &EncryptOptions,
    rng: &mut R,
) -> Result<Vec<u8>, OfdError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(input))?;
    let mut files = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name()?.into_owned();
        if matches!(
            name.as_str(),
            "Encryptions.xml" | "decryptseed.dat" | "entriesmap.dat"
        ) {
            return Err(OfdError::Crypto("输入包已经包含加密描述".to_owned()));
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        files.push((name, data));
    }
    if files.is_empty() {
        return Err(OfdError::Crypto("输入包没有可加密的 entry".to_owned()));
    }

    let mut file_key = [0u8; FILE_KEY_LEN];
    let mut iv = [0u8; 16];
    rng.try_fill_bytes(&mut file_key)
        .map_err(|_| OfdError::Crypto("生成文件密钥失败".to_owned()))?;
    rng.try_fill_bytes(&mut iv)
        .map_err(|_| OfdError::Crypto("生成 IV 失败".to_owned()))?;

    let mut used_names: HashSet<String> = files.iter().map(|(name, _)| name.clone()).collect();
    let mut mappings = Vec::new();
    let mut output_entries: Vec<(String, Vec<u8>)> = Vec::new();
    for (name, data) in files {
        let should_encrypt = options
            .filter
            .as_ref()
            .map(|filter| filter(&name))
            .unwrap_or(true);
        if !should_encrypt {
            output_entries.push((name, data));
            continue;
        }
        let cipher_path = cipher_path(&name, &mut used_names);
        let ciphertext = encrypt_cbc(&file_key, &iv, &data)
            .map_err(|_| OfdError::Crypto("SM4 加密失败".to_owned()))?;
        mappings.push((format!("/{name}"), format!("/{cipher_path}")));
        output_entries.push((cipher_path, ciphertext));
    }
    if mappings.is_empty() {
        return Err(OfdError::Crypto("没有文件被加密".to_owned()));
    }

    let map_xml = build_entries_map(&mappings);
    let encrypted_map = encrypt_cbc(&file_key, &iv, map_xml.as_bytes())
        .map_err(|_| OfdError::Crypto("映射表加密失败".to_owned()))?;
    let (case_id, wrapped_key, user_cert) = match &options.scheme {
        EncryptionScheme::Password(password) => (
            PASSWORD_CASE_ID,
            wrap_file_key(password, &iv, &file_key)
                .map_err(|_| OfdError::Crypto("文件密钥包装失败".to_owned()))?,
            None,
        ),
        EncryptionScheme::Certificate(certificate) => (
            CERTIFICATE_CASE_ID,
            encrypt_file_key_with_certificate(certificate, &file_key, rng)
                .map_err(|_| OfdError::Crypto("证书加密失败".to_owned()))?,
            Some(certificate.clone()),
        ),
    };
    let seed_xml = build_decrypt_seed(
        case_id,
        &options.username,
        options.user_type.as_deref(),
        &iv,
        &wrapped_key,
        user_cert.as_deref(),
    );
    let encryptions_xml = build_encryptions();

    output_entries.push(("entriesmap.dat".to_owned(), encrypted_map));
    output_entries.push(("decryptseed.dat".to_owned(), seed_xml.into_bytes()));
    output_entries.push(("Encryptions.xml".to_owned(), encryptions_xml.into_bytes()));

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in output_entries {
        writer
            .start_file(name, SimpleFileOptions::default())
            .map_err(|_| OfdError::Crypto("写入加密 ZIP 失败".to_owned()))?;
        writer
            .write_all(&data)
            .map_err(|_| OfdError::Crypto("写入加密 entry 失败".to_owned()))?;
    }
    writer
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|_| OfdError::Crypto("完成加密 ZIP 失败".to_owned()))
}

fn cipher_path(name: &str, used: &mut HashSet<String>) -> String {
    let (directory, file_name) = match name.rsplit_once('/') {
        Some((directory, file_name)) => (format!("{directory}/"), file_name),
        None => (String::new(), name),
    };
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name)
        .to_lowercase();
    let mut candidate = format!("{directory}{stem}.dat");
    let mut suffix = 1;
    while used.contains(&candidate) {
        candidate = format!("{directory}{stem}_{suffix}.dat");
        suffix += 1;
    }
    used.insert(candidate.clone());
    candidate
}

fn build_entries_map(mappings: &[(String, String)]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <ofd:EncryptedEntries xmlns:ofd=\"http://www.ofdspec.org/2016\" ID=\"1\">",
    );
    for (path, epath) in mappings {
        xml.push_str(&format!(
            "<ofd:EncryptEntry Path=\"{}\" EPath=\"{}\"/>",
            escape_attr(path),
            escape_attr(epath)
        ));
    }
    xml.push_str("</ofd:EncryptedEntries>");
    xml
}

fn build_decrypt_seed(
    case_id: &str,
    username: &str,
    user_type: Option<&str>,
    iv: &[u8],
    wrapped_key: &[u8],
    user_cert: Option<&[u8]>,
) -> String {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <ofd:DecryptSeed xmlns:ofd=\"http://www.ofdspec.org/2016\" ID=\"1\" EncryptCaseId=\"{}\">",
        escape_attr(case_id)
    );
    xml.push_str(&format!(
        "<ofd:UserInfo UserName=\"{}\"",
        escape_attr(username)
    ));
    if let Some(user_type) = user_type {
        xml.push_str(&format!(" UserType=\"{}\"", escape_attr(user_type)));
    }
    xml.push('>');
    if let Some(cert) = user_cert {
        xml.push_str(&format!("<ofd:UserCert>{}</ofd:UserCert>", encode(cert)));
    }
    xml.push_str(&format!(
        "<ofd:EncryptedWK>{}</ofd:EncryptedWK>",
        encode(wrapped_key)
    ));
    xml.push_str(&format!("<ofd:IVValue>{}</ofd:IVValue>", encode(iv)));
    xml.push_str("</ofd:UserInfo><ofd:ExtendParams/></ofd:DecryptSeed>");
    xml
}

fn build_encryptions() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <ofd:Encryptions xmlns:ofd=\"http://www.ofdspec.org/2016\">\
         <ofd:EncryptInfo ID=\"1\">\
         <ofd:Provider Name=\"OFDBox\" Version=\"{}\"/>\
         <ofd:EncryptScope>All</ofd:EncryptScope>\
         <ofd:DecryptSeedLoc>/decryptseed.dat</ofd:DecryptSeedLoc>\
         <ofd:EntriesMapLoc>/entriesmap.dat</ofd:EntriesMapLoc>\
         </ofd:EncryptInfo></ofd:Encryptions>",
        escape_attr(env!("CARGO_PKG_VERSION"))
    )
}

fn encode(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
