use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Seek, Write};
use std::sync::Arc;

use zip::ZipArchive;

use crate::cache::ByteLruCache;
use crate::crypto::{CryptoBackend, FileKey, SignatureVerifiers, detect_signed_value_format};
use crate::error::OfdError;
use crate::model::annotation::{Annotations, PageAnnot};
use crate::model::attachment::Attachments;
use crate::model::custom_tags::CustomTags;
use crate::model::decrypt_seed::DecryptSeed;
use crate::model::document::{Document, PageEntry};
use crate::model::encryption::Encryptions;
use crate::model::entries_map::EncryptedEntries;
use crate::model::extensions::Extensions;
use crate::model::integrity::DocEntries;
use crate::model::ofd::OfdRoot;
use crate::model::page::Page;
use crate::model::res::Res;
use crate::model::signature::{Signature, Signatures};
use crate::model::version::Version;
use crate::options::{CacheOptions, ReadOptions};
use crate::package_write::{
    DecryptedWriteReport, ExistingEntry, PackageEdits, PackageWriteOptions, PlanItem, copy_raw,
    write_plain_entry,
};
use crate::schema::doc_entries::{
    DocEntries as SchemaDocEntries, File as DocEntryFile, FileList as DocEntryFileList,
};
use crate::sign::ReferenceDigest;
use crate::sign::{
    DEFAULT_SIGNATURE_METHOD, ProtectScope, SignedValueProducer, SigningContext, SigningReport,
    SigningRequest,
};
use crate::xml::parser::parse_xml;
use crate::xml::tree::XmlDocument;

/// OFD 包根条目的固定名称。
pub const ROOT_ENTRY: &str = "OFD.xml";

trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

/// 一条逻辑条目的解密信息。
struct DecryptionEntry {
    /// 物理密文路径。
    physical: String,
    /// 条目独有的密钥描述文件路径（0099 附录 C.4 `EncryptEntry/@DecryptSeedLoc`）。
    seed_loc: Option<String>,
}

struct DecryptionOperation {
    /// 所属加密操作，用于按条目解包包装密钥。
    info: crate::model::encryption::EncryptInfo,
    /// 该次加密操作的通用文件密钥。
    default_key: FileKey,
    /// 逻辑路径 -> 解密信息。
    entries: HashMap<String, DecryptionEntry>,
}

struct DecryptionContext {
    backend: Box<dyn CryptoBackend>,
    /// 解密顺序：最新一次加密在前。
    operations: Vec<DecryptionOperation>,
    /// 已解出的条目级密钥，按密钥描述文件路径缓存。
    entry_keys: HashMap<String, FileKey>,
}

/// `OFDEntries.xml` 校验结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntriesVerification {
    /// `FileList` 中的文件数量。
    pub files: usize,
    /// 已验签的 `SignedValueLoc`（包内绝对路径）；未提供签名时为 `None`。
    pub signed_value: Option<String>,
    /// 包内既未被文档引用、也未列入 `FileList` 的文件（夹带文件）。
    pub unlisted: Vec<String>,
    /// 列入了 `FileList`、但仍被文档引用的文件（列表与引用图不一致）。
    pub listed_but_referenced: Vec<String>,
}

impl EntriesVerification {
    /// 文件列表与引用图完全一致。
    pub fn is_consistent(&self) -> bool {
        self.unlisted.is_empty() && self.listed_but_referenced.is_empty()
    }
}

/// 生成 `OFDEntries.xml` 的参数。
#[derive(Debug, Clone)]
pub struct EntriesWriteOptions {
    /// `DocEntries/@ID`，必须是合法的 XML ID。
    pub id: String,
    /// `CreatorName` 属性。
    pub creator_name: Option<String>,
    /// `Version` 属性。
    pub version: Option<String>,
    /// `CreationDate` 属性。
    pub creation_date: Option<String>,
    /// 签名值文件路径，缺省 `Integrity/SignedValue.dat`。
    pub signed_value_entry: String,
    /// 签名时间，透传给签名值生产者。
    pub signed_at: String,
    /// 是否覆盖已有的 `OFDEntries.xml`。
    pub replace_existing: bool,
}

impl Default for EntriesWriteOptions {
    fn default() -> Self {
        Self {
            id: "ofdbox_entries".to_owned(),
            creator_name: None,
            version: None,
            creation_date: None,
            signed_value_entry: "Integrity/SignedValue.dat".to_owned(),
            signed_at: String::new(),
            replace_existing: false,
        }
    }
}

/// 生成 `OFDEntries.xml` 的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntriesWriteReport {
    /// 写入 `FileList` 的夹带文件数量。
    pub files: usize,
    /// `OFDEntries.xml` 的包内路径。
    pub entries_entry: String,
    /// 签名值文件的包内路径。
    pub signed_value_entry: String,
}

/// 缓存统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// 当前 entry 字节缓存占用。
    pub entry_bytes: usize,
    /// 当前 XML 语法树缓存占用。
    pub xml_bytes: usize,
    /// entry 缓存命中次数。
    pub entry_hits: u64,
    /// entry 缓存未命中次数。
    pub entry_misses: u64,
    /// XML 缓存命中次数。
    pub xml_hits: u64,
    /// XML 缓存未命中次数。
    pub xml_misses: u64,
    /// 解密后明文缓存占用。
    pub plain_bytes: usize,
    /// 明文缓存命中次数。
    pub plain_hits: u64,
    /// 明文缓存未命中次数。
    pub plain_misses: u64,
}

/// 只读 OFD 包。
pub struct OfdPackage {
    archive: ZipArchive<Box<dyn ReadSeek>>,
    /// 归一化路径（正斜杠、小写）到 ZIP 条目名的索引。
    entry_index: HashMap<String, String>,
    options: ReadOptions,
    entry_cache: ByteLruCache<String, Arc<[u8]>>,
    plain_cache: ByteLruCache<String, Arc<[u8]>>,
    xml_cache: ByteLruCache<(String, bool), Arc<XmlDocument>>,
    decryption: Option<DecryptionContext>,
    document_entry: Option<String>,
    signature_entry: Option<String>,
    annotations_entry: Option<String>,
    signatures_entry: Option<String>,
}

impl std::fmt::Debug for OfdPackage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OfdPackage")
            .field("entry_count", &self.entry_count())
            .field("options", &self.options)
            .field("decryption_enabled", &self.decryption.is_some())
            .finish()
    }
}

/// OFD 文档入口（DocBody）描述。
#[derive(Clone, Debug)]
pub struct DocumentEntry {
    /// DocBody 序号。
    pub index: usize,
    /// DocID。
    pub doc_id: Option<String>,
    /// DocRoot 路径。
    pub doc_root: Option<String>,
}

impl OfdPackage {
    /// 从内存字节打开包，使用宽松读取模式和默认缓存。
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, OfdError> {
        Self::from_bytes_with_options(bytes, ReadOptions::LENIENT, CacheOptions::default())
    }

    /// 探测字节是否为 OFD 包。
    ///
    /// 只读取 ZIP 索引并查找 `OFD.xml`，不解析 XML；与正常读取共用同一套
    /// ZIP 容错，因此能接受前置字节、空 EOCD 等非标准封装。
    pub fn probe(bytes: &[u8]) -> Result<bool, OfdError> {
        let archive = ZipArchive::new(std::io::Cursor::new(bytes))?;
        for name in archive.file_names() {
            if name?.eq_ignore_ascii_case("OFD.xml") {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// 从内存字节打开包，并指定读取模式和缓存配置。
    pub fn from_bytes_with_options(
        bytes: Vec<u8>,
        options: ReadOptions,
        cache_options: CacheOptions,
    ) -> Result<Self, OfdError> {
        Self::from_reader_with_options(Cursor::new(bytes), options, cache_options)
    }

    /// 从任意 `Read + Seek` 打开包，使用宽松读取模式和默认缓存。
    pub fn from_reader(reader: impl Read + Seek + 'static) -> Result<Self, OfdError> {
        Self::from_reader_with_options(reader, ReadOptions::LENIENT, CacheOptions::default())
    }

    /// 从任意 `Read + Seek` 打开包。
    pub fn from_reader_with_options(
        reader: impl Read + Seek + 'static,
        options: ReadOptions,
        cache_options: CacheOptions,
    ) -> Result<Self, OfdError> {
        let archive = ZipArchive::new(Box::new(reader) as Box<dyn ReadSeek>)?;
        let mut entry_index = HashMap::new();
        for name in archive.file_names().flatten() {
            let name = name.into_owned();
            entry_index.entry(entry_key(&name)).or_insert(name);
        }
        Ok(Self {
            archive,
            entry_index,
            options,
            entry_cache: ByteLruCache::new(cache_options.max_bytes),
            plain_cache: ByteLruCache::new(cache_options.max_bytes),
            xml_cache: ByteLruCache::new(cache_options.max_bytes),
            decryption: None,
            document_entry: None,
            signature_entry: None,
            annotations_entry: None,
            signatures_entry: None,
        })
    }

    /// 从文件系统打开包。
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, OfdError> {
        Self::open_with_options(path, ReadOptions::LENIENT, CacheOptions::default())
    }

    /// 从文件系统打开包，并指定读取模式和缓存配置。
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_with_options(
        path: impl AsRef<std::path::Path>,
        options: ReadOptions,
        cache_options: CacheOptions,
    ) -> Result<Self, OfdError> {
        let file = std::fs::File::open(path)?;
        Self::from_reader_with_options(file, options, cache_options)
    }

    /// 当前包的读取模式。
    pub fn options(&self) -> ReadOptions {
        self.options
    }

    /// 是否包含指定 entry。
    ///
    /// 启用解密后，映射中的逻辑路径也视为存在。
    pub fn contains(&self, name: &str) -> bool {
        let name = name.trim_start_matches('/');
        if let Some(context) = &self.decryption
            && context
                .operations
                .iter()
                .any(|operation| operation.entries.contains_key(name))
        {
            return true;
        }
        self.archive.index_for_name(name).is_some()
            || self.entry_index.contains_key(&entry_key(name))
    }

    /// entry 数量。
    pub fn entry_count(&self) -> usize {
        self.archive.len()
    }

    /// 读取全部 entry 名称。
    pub fn entry_names(&self) -> Result<Vec<String>, OfdError> {
        let mut names = Vec::new();
        for name in self.archive.file_names() {
            names.push(name?.replace('\\', "/"));
        }
        if let Some(context) = &self.decryption {
            for operation in &context.operations {
                for name in operation.entries.keys() {
                    if !names.iter().any(|existing| existing == name) {
                        names.push(name.clone());
                    }
                }
            }
        }
        Ok(names)
    }

    /// 按需读取一个 entry。
    ///
    /// 未启用解密时返回 ZIP 原始字节；启用后，映射中的逻辑路径会透明解密。
    /// 条目声明了独立密钥描述文件时按条目取密钥，解不开只影响该条目。
    pub fn read_entry(&mut self, name: &str) -> Result<Arc<[u8]>, OfdError> {
        let name = normalize_entry_name(name)?;
        if let Some(bytes) = self.plain_cache.get(&name) {
            return Ok(bytes);
        }
        let Some(start) = self.decryption_layer(&name) else {
            return self.read_raw_entry(&name);
        };
        let mut plaintext = self.decrypt_layer(start, &name)?;
        let total = self.decryption_operation_count();
        for index in start + 1..total {
            if !self.layer_contains(index, &name) {
                break;
            }
            let key = self.layer_key(index, &name)?;
            plaintext = self
                .decryption
                .as_ref()
                .expect("decryption context")
                .backend
                .decrypt(&key, &plaintext)?;
        }
        let plaintext: Arc<[u8]> = Arc::from(plaintext);
        self.plain_cache
            .insert(name, Arc::clone(&plaintext), plaintext.len());
        Ok(plaintext)
    }

    /// 读取 entry 并将最终明文写入 sink，不写入明文缓存。
    ///
    /// 单层加密时 SM4 按块解密后写出；多层加密仍会先合并各层明文。
    pub fn read_entry_to(&mut self, name: &str, writer: &mut impl Write) -> Result<(), OfdError> {
        let name = normalize_entry_name(name)?;
        if let Some(bytes) = self.plain_cache.get(&name) {
            writer.write_all(&bytes)?;
            return Ok(());
        }
        let Some(start) = self.decryption_layer(&name) else {
            let bytes = self.read_raw_entry(&name)?;
            writer.write_all(&bytes)?;
            return Ok(());
        };
        let total = self.decryption_operation_count();
        if start + 1 == total {
            let physical = self.layer_physical(start, &name)?;
            let ciphertext = self.read_raw_entry(&physical)?;
            let key = self.layer_key(start, &name)?;
            return self
                .decryption
                .as_ref()
                .expect("decryption context")
                .backend
                .decrypt_to(&key, &ciphertext, writer);
        }
        let mut plaintext = self.decrypt_layer(start, &name)?;
        for index in start + 1..total {
            if !self.layer_contains(index, &name) {
                break;
            }
            let key = self.layer_key(index, &name)?;
            plaintext = self
                .decryption
                .as_ref()
                .expect("decryption context")
                .backend
                .decrypt(&key, &plaintext)?;
        }
        writer.write_all(&plaintext)?;
        Ok(())
    }

    fn decryption_operation_count(&self) -> usize {
        self.decryption
            .as_ref()
            .map(|context| context.operations.len())
            .unwrap_or(0)
    }

    fn decryption_layer(&self, name: &str) -> Option<usize> {
        self.decryption.as_ref().and_then(|context| {
            context
                .operations
                .iter()
                .position(|operation| operation.entries.contains_key(name))
        })
    }

    fn layer_contains(&self, index: usize, name: &str) -> bool {
        self.decryption
            .as_ref()
            .and_then(|context| context.operations.get(index))
            .is_some_and(|operation| operation.entries.contains_key(name))
    }

    fn layer_physical(&self, index: usize, name: &str) -> Result<String, OfdError> {
        self.decryption
            .as_ref()
            .and_then(|context| context.operations.get(index))
            .and_then(|operation| operation.entries.get(name))
            .map(|entry| entry.physical.clone())
            .ok_or_else(|| OfdError::InvalidStructure(format!("解密层未映射 entry: {name}")))
    }

    /// 取某一层解密该条目所用的文件密钥。
    ///
    /// 条目声明了独立密钥描述文件时按需解包并缓存，否则用该次加密操作的通用密钥。
    fn layer_key(&mut self, index: usize, name: &str) -> Result<FileKey, OfdError> {
        let (seed_loc, info) =
            {
                let context = self.decryption.as_ref().expect("decryption context");
                let operation = context.operations.get(index).ok_or_else(|| {
                    OfdError::InvalidStructure(format!("解密层索引越界: {index}"))
                })?;
                let entry = operation.entries.get(name).ok_or_else(|| {
                    OfdError::InvalidStructure(format!("解密层未映射 entry: {name}"))
                })?;
                (entry.seed_loc.clone(), operation.info.clone())
            };
        let Some(seed_loc) = seed_loc else {
            return Ok(self
                .decryption
                .as_ref()
                .expect("decryption context")
                .operations[index]
                .default_key
                .clone());
        };
        if let Some(key) = self
            .decryption
            .as_ref()
            .expect("decryption context")
            .entry_keys
            .get(&seed_loc)
        {
            return Ok(key.clone());
        }
        let seed = self.parse_decrypt_seed(&seed_loc)?;
        let key = self
            .decryption
            .as_ref()
            .expect("decryption context")
            .backend
            .unwrap_file_key(&info, &seed)?;
        self.decryption
            .as_mut()
            .expect("decryption context")
            .entry_keys
            .insert(seed_loc, key.clone());
        Ok(key)
    }

    /// 读取某一层密文并用该层密钥解密。
    fn decrypt_layer(&mut self, index: usize, name: &str) -> Result<Vec<u8>, OfdError> {
        let physical = self.layer_physical(index, name)?;
        let ciphertext = self.read_raw_entry(&physical)?;
        let key = self.layer_key(index, name)?;
        self.decryption
            .as_ref()
            .expect("decryption context")
            .backend
            .decrypt(&key, &ciphertext)
    }

    fn resolve_zip_name(&self, name: &str) -> String {
        if self.archive.index_for_name(name).is_some() {
            return name.to_owned();
        }
        self.entry_index
            .get(&entry_key(name))
            .cloned()
            .unwrap_or_else(|| name.to_owned())
    }

    fn read_raw_entry(&mut self, name: &str) -> Result<Arc<[u8]>, OfdError> {
        if let Some(bytes) = self.entry_cache.get(&name.to_owned()) {
            return Ok(bytes);
        }

        let actual = self.resolve_zip_name(name);
        let mut entry = match self.archive.by_name(&actual) {
            Ok(entry) => entry,
            Err(zip::result::ZipError::FileNotFound) => {
                return Err(OfdError::MissingEntry(name.to_owned()));
            }
            Err(error) => return Err(error.into()),
        };
        let stored = entry.compression() == zip::CompressionMethod::Stored && !entry.encrypted();
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        if let Err(error) = entry.read_to_end(&mut bytes) {
            if !stored {
                return Err(error.into());
            }
            drop(entry);
            // 真实文件可能只在 ZIP 头里写了过期 CRC；Stored 条目的原始字节就是文件内容。
            let index = self
                .archive
                .index_for_name(&actual)
                .ok_or(OfdError::MissingEntry(name.to_owned()))?;
            let mut raw = self.archive.by_index_raw(index)?;
            let mut fallback = Vec::with_capacity(raw.size() as usize);
            raw.read_to_end(&mut fallback)?;
            bytes = fallback;
        }
        let bytes: Arc<[u8]> = Arc::from(bytes);
        self.entry_cache
            .insert(name.to_owned(), Arc::clone(&bytes), bytes.len());
        Ok(bytes)
    }

    /// 读取并解析 `OFD.xml`。
    pub fn parse_root(&mut self) -> Result<OfdRoot, OfdError> {
        let entry = self.root_entry_name()?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return OfdRoot::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        OfdRoot::from_document(doc, self.options)
    }

    /// 读取并解析 `OFD.xml` 中 `DocRoot` 指向的 `Document.xml`。
    pub fn parse_document(&mut self) -> Result<Document, OfdError> {
        let doc_root = {
            let root = self.parse_root()?;
            root.doc_bodies()
                .next()
                .and_then(|body| body.doc_root().map(str::to_owned))
        }
        .ok_or_else(|| OfdError::InvalidStructure("OFD.xml 缺少 DocRoot".to_owned()))?;
        let entry = resolve_loc("", &doc_root)?;
        self.document_entry = Some(entry.clone());
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return Document::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        Document::from_document(doc, self.options)
    }

    /// 读取并解析页面内容文件，路径相对于包根。
    pub fn parse_page(&mut self, loc: &str) -> Result<Page, OfdError> {
        let entry = resolve_loc("", loc)?;
        self.parse_page_entry(entry)
    }

    /// 枚举包内所有 DocBody。
    pub fn documents(&mut self) -> Result<Vec<DocumentEntry>, OfdError> {
        let root = self.parse_root()?;
        let bodies: Vec<_> = root.doc_bodies().collect();
        let entries = bodies
            .iter()
            .enumerate()
            .map(|(index, body)| DocumentEntry {
                index,
                doc_id: body
                    .doc_info()
                    .and_then(|info| info.doc_id().map(str::to_owned)),
                doc_root: body.doc_root().map(str::to_owned),
            })
            .collect();
        Ok(entries)
    }

    /// 按 DocBody 序号解析 `Document.xml`。
    pub fn parse_document_at(&mut self, index: usize) -> Result<Document, OfdError> {
        let doc_root = {
            let root = self.parse_root()?;
            root.doc_bodies()
                .nth(index)
                .and_then(|body| body.doc_root().map(str::to_owned))
        }
        .ok_or_else(|| OfdError::InvalidStructure(format!("DocBody 不存在: {index}")))?;
        let entry = resolve_loc("", &doc_root)?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return Document::from_document(doc, self.options);
        }
        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        Document::from_document(doc, self.options)
    }

    /// 解析指定 DocBody 的页面对象。
    pub fn parse_page_for_document(
        &mut self,
        index: usize,
        page: &PageEntry,
    ) -> Result<Page, OfdError> {
        let doc_root = {
            let root = self.parse_root()?;
            root.doc_bodies()
                .nth(index)
                .and_then(|body| body.doc_root().map(str::to_owned))
        }
        .ok_or_else(|| OfdError::InvalidStructure(format!("DocBody 不存在: {index}")))?;
        let doc_entry = resolve_loc("", &doc_root)?;
        let base = parent_dir(&doc_entry)
            .ok_or_else(|| OfdError::InvalidStructure("无法解析 Document.xml 目录".to_owned()))?;
        let loc = page
            .base_loc()
            .ok_or_else(|| OfdError::InvalidStructure("Page 缺少 BaseLoc".to_owned()))?;
        let entry = resolve_loc(base, loc)?;
        self.parse_page_entry(entry)
    }

    /// 解析指定 DocBody 的资源文件。
    pub fn parse_res_for_document(&mut self, index: usize, loc: &str) -> Result<Res, OfdError> {
        let doc_root = {
            let root = self.parse_root()?;
            root.doc_bodies()
                .nth(index)
                .and_then(|body| body.doc_root().map(str::to_owned))
        }
        .ok_or_else(|| OfdError::InvalidStructure(format!("DocBody 不存在: {index}")))?;
        let doc_entry = resolve_loc("", &doc_root)?;
        let base = parent_dir(&doc_entry)
            .ok_or_else(|| OfdError::InvalidStructure("无法解析 Document.xml 目录".to_owned()))?;
        let entry = resolve_loc(base, loc)?;
        self.parse_res_entry(entry)
    }

    /// 解析 `Document.xml` 中某个页面指向的内容文件。
    ///
    /// `Page.BaseLoc` 相对于 `Document.xml` 所在目录解析。
    pub fn parse_page_for(&mut self, page: &PageEntry) -> Result<Page, OfdError> {
        if self.document_entry.is_none() {
            self.parse_document()?;
        }
        let loc = page
            .base_loc()
            .ok_or_else(|| OfdError::InvalidStructure("Page 缺少 BaseLoc".to_owned()))?;
        let base = self
            .document_entry
            .as_deref()
            .and_then(parent_dir)
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位 Document.xml".to_owned()))?;
        let entry = resolve_loc(base, loc)?;
        self.parse_page_entry(entry)
    }

    fn parse_page_entry(&mut self, entry: String) -> Result<Page, OfdError> {
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return Page::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        Page::from_document(doc, self.options)
    }

    /// 读取并解析资源文件，路径相对于包根。
    pub fn parse_res(&mut self, loc: &str) -> Result<Res, OfdError> {
        let entry = resolve_loc("", loc)?;
        self.parse_res_entry(entry)
    }

    /// 解析 `Document.xml` 中引用的资源文件。
    ///
    /// `PublicRes` / `DocumentRes` 相对于 `Document.xml` 所在目录解析。
    pub fn parse_res_from_document(&mut self, loc: &str) -> Result<Res, OfdError> {
        if self.document_entry.is_none() {
            self.parse_document()?;
        }
        let base = self
            .document_entry
            .as_deref()
            .and_then(parent_dir)
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位 Document.xml".to_owned()))?;
        let entry = resolve_loc(base, loc)?;
        self.parse_res_entry(entry)
    }

    fn parse_res_entry(&mut self, entry: String) -> Result<Res, OfdError> {
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return Res::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        Res::from_document(doc, self.options)
    }

    /// 解析 `Document.xml` 中声明的注释索引文件。
    ///
    /// `Annotations` 相对于 `Document.xml` 所在目录解析。
    pub fn parse_annotations(&mut self) -> Result<Annotations, OfdError> {
        let document = self.parse_document()?;
        let loc = document.annotations().map(str::to_owned).ok_or_else(|| {
            OfdError::InvalidStructure("Document.xml 未声明 Annotations".to_owned())
        })?;
        let base = self
            .document_entry
            .as_deref()
            .and_then(parent_dir)
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位 Document.xml".to_owned()))?;
        let entry = self.resolve_existing_loc(base, &loc)?;
        let view = self.parse_annotations_entry(entry.clone())?;
        self.annotations_entry = Some(entry);
        Ok(view)
    }

    /// 解析 `Document.xml` 中声明的附件列表文件。
    ///
    /// `Attachments` 相对于 `Document.xml` 所在目录解析。
    pub fn parse_attachments(&mut self) -> Result<Attachments, OfdError> {
        let document = self.parse_document()?;
        let loc = document.attachments().map(str::to_owned).ok_or_else(|| {
            OfdError::InvalidStructure("Document.xml 未声明 Attachments".to_owned())
        })?;
        let base = self
            .document_entry
            .as_deref()
            .and_then(parent_dir)
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位 Document.xml".to_owned()))?;
        let entry = resolve_loc(base, &loc)?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return Attachments::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        Attachments::from_document(doc, self.options)
    }

    /// 解析 `Document.xml` 中声明的自定义标引列表文件。
    ///
    /// `CustomTags` 相对于 `Document.xml` 所在目录解析。
    pub fn parse_custom_tags(&mut self) -> Result<CustomTags, OfdError> {
        let document = self.parse_document()?;
        let loc = document.custom_tags().map(str::to_owned).ok_or_else(|| {
            OfdError::InvalidStructure("Document.xml 未声明 CustomTags".to_owned())
        })?;
        let base = self
            .document_entry
            .as_deref()
            .and_then(parent_dir)
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位 Document.xml".to_owned()))?;
        let entry = resolve_loc(base, &loc)?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return CustomTags::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        CustomTags::from_document(doc, self.options)
    }

    /// 解析 `Document.xml` 中声明的扩展信息列表文件。
    ///
    /// `Extensions` 相对于 `Document.xml` 所在目录解析。
    pub fn parse_extensions(&mut self) -> Result<Extensions, OfdError> {
        let document = self.parse_document()?;
        let loc = document.extensions().map(str::to_owned).ok_or_else(|| {
            OfdError::InvalidStructure("Document.xml 未声明 Extensions".to_owned())
        })?;
        let base = self
            .document_entry
            .as_deref()
            .and_then(parent_dir)
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位 Document.xml".to_owned()))?;
        let entry = resolve_loc(base, &loc)?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return Extensions::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        Extensions::from_document(doc, self.options)
    }

    /// 解析注释索引中 `FileLoc` 指向的分页注释文件。
    ///
    /// 路径相对于注释索引文件所在目录解析；索引尚未解析时会先读取 `Document.xml`。
    pub fn parse_page_annot(&mut self, file_loc: &str) -> Result<PageAnnot, OfdError> {
        if self.annotations_entry.is_none() {
            self.parse_annotations()?;
        }
        let base = self
            .annotations_entry
            .as_deref()
            .and_then(parent_dir)
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位注释索引文件".to_owned()))?;
        let entry = self.resolve_existing_loc(base, file_loc)?;
        self.parse_page_annot_entry(entry)
    }

    fn parse_annotations_entry(&mut self, entry: String) -> Result<Annotations, OfdError> {
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return Annotations::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        Annotations::from_document(doc, self.options)
    }

    fn parse_page_annot_entry(&mut self, entry: String) -> Result<PageAnnot, OfdError> {
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return PageAnnot::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        PageAnnot::from_document(doc, self.options)
    }

    /// 解析 `OFD.xml` 中第一个 `DocBody` 声明的签名列表文件。
    ///
    /// `Signatures` 相对于 `OFD.xml` 所在目录解析。
    pub fn parse_signatures(&mut self) -> Result<Signatures, OfdError> {
        let loc = {
            let root = self.parse_root()?;
            root.doc_bodies()
                .next()
                .and_then(|body| body.signatures().map(str::to_owned))
        }
        .ok_or_else(|| OfdError::InvalidStructure("OFD.xml 未声明 Signatures".to_owned()))?;
        let entry = resolve_loc("", &loc)?;
        let view = self.parse_signatures_entry(entry.clone())?;
        self.signatures_entry = Some(entry);
        Ok(view)
    }

    /// 返回已定位的 `Document.xml` entry。
    pub fn document_entry_name(&self) -> Option<&str> {
        self.document_entry.as_deref()
    }

    /// 返回页面 `BaseLoc` 对应的包内 entry。
    pub fn page_entry_name(&mut self, page: &PageEntry) -> Result<String, OfdError> {
        if self.document_entry.is_none() {
            self.parse_document()?;
        }
        let loc = page
            .base_loc()
            .ok_or_else(|| OfdError::InvalidStructure("Page 缺少 BaseLoc".to_owned()))?;
        let base = self
            .document_entry
            .as_deref()
            .and_then(parent_dir)
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位 Document.xml".to_owned()))?;
        resolve_loc(base, loc)
    }

    fn resolve_existing_loc(&self, base: &str, loc: &str) -> Result<String, OfdError> {
        let entry = resolve_loc(base, loc)?;
        if self.contains(&entry) {
            return Ok(entry);
        }
        // 真实文件里同一字段既可能相对于文档目录，也可能已经是根相对路径。
        let root = resolve_loc("", loc)?;
        if self.contains(&root) {
            Ok(root)
        } else {
            Ok(entry)
        }
    }

    /// 按包内路径规则解析相对引用。
    pub fn resolve_entry(&self, base: &str, loc: &str) -> Result<String, OfdError> {
        resolve_loc(base, loc)
    }

    /// 按包根路径直接解析签名描述文件。
    pub fn parse_signature_file(&mut self, loc: &str) -> Result<Signature, OfdError> {
        let entry = resolve_loc("", loc)?;
        self.parse_signature_entry(entry)
    }

    /// 解析签名列表中 `BaseLoc` 指向的签名描述文件。
    ///
    /// 路径相对于签名列表文件所在目录解析；列表尚未解析时会先读取 `OFD.xml`。
    pub fn parse_signature(&mut self, base_loc: &str) -> Result<Signature, OfdError> {
        if self.signatures_entry.is_none() {
            self.parse_signatures()?;
        }
        let base = self
            .signatures_entry
            .as_deref()
            .and_then(parent_dir)
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位签名列表文件".to_owned()))?;
        let relative = resolve_loc(base, base_loc)?;
        // 部分厂商把 BaseLoc 写成包内全路径（没有前导 `/`），先按相对解析，
        // 找不到时再按包根解析。
        let entry = if self.contains(&relative) {
            relative
        } else {
            let absolute = resolve_loc("", base_loc)?;
            if self.contains(&absolute) {
                absolute
            } else {
                relative
            }
        };
        self.parse_signature_entry(entry)
    }

    /// 按需读取签名描述引用的电子印章文件。
    ///
    /// 只有需要显示或验证印章时才应调用；普通内容解析不会读取该文件。
    /// `Seal/BaseLoc` 相对于签名描述文件所在目录解析。
    pub fn read_seal_bytes(&mut self, signature: &Signature) -> Result<Arc<[u8]>, OfdError> {
        let base_loc = signature
            .signed_info()
            .and_then(|info| info.seal())
            .and_then(|seal| seal.base_loc().map(str::to_owned))
            .ok_or_else(|| OfdError::InvalidStructure("签名描述未声明印章文件".to_owned()))?;
        let entry = self
            .signature_entry
            .clone()
            .ok_or_else(|| OfdError::InvalidStructure("尚未解析 Signature.xml".to_owned()))?;
        let base = parent_dir(&entry)
            .ok_or_else(|| OfdError::InvalidStructure("无法解析印章文件路径".to_owned()))?;
        self.read_entry(&resolve_loc(base, &base_loc)?)
    }

    fn parse_signatures_entry(&mut self, entry: String) -> Result<Signatures, OfdError> {
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return Signatures::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        Signatures::from_document(doc, self.options)
    }

    /// 解析版本列表 `BaseLoc` 指向的版本描述文件，路径相对于包根。
    pub fn parse_doc_version(&mut self, base_loc: &str) -> Result<Version, OfdError> {
        let entry = resolve_loc("", base_loc)?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return Version::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        Version::from_document(doc, self.options)
    }

    /// 解析包根目录下的解密入口 `Encryptions.xml`。
    pub fn parse_encryptions(&mut self) -> Result<Encryptions, OfdError> {
        let entry = resolve_loc("", "Encryptions.xml")?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return Encryptions::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        Encryptions::from_document(doc, self.options)
    }

    /// 解析密钥描述文件，路径相对于包根。
    pub fn parse_decrypt_seed(&mut self, loc: &str) -> Result<DecryptSeed, OfdError> {
        let entry = resolve_loc("", loc)?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return DecryptSeed::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        DecryptSeed::from_document(doc, self.options)
    }

    /// 解析未加密的明密文映射表，路径相对于包根。
    ///
    /// `entriesmap.dat` 可能是加密二进制，需先解密后再调用本方法。
    pub fn parse_entries_map(&mut self, loc: &str) -> Result<EncryptedEntries, OfdError> {
        let entry = resolve_loc("", loc)?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return EncryptedEntries::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        EncryptedEntries::from_document(doc, self.options)
    }

    /// 启用文档解密。
    ///
    /// 只解出各加密操作的文件密钥和明密文映射表；页面和资源在读取时按需解密。
    /// 条目声明了独立密钥描述文件时，在首次读取该条目时按需解包。
    pub fn enable_decryption(&mut self, backend: Box<dyn CryptoBackend>) -> Result<(), OfdError> {
        if self.decryption.is_some() {
            return Ok(());
        }
        let encryptions = self.parse_encryptions()?;
        let infos: Vec<_> = encryptions.encrypt_infos().collect();
        if infos.is_empty() {
            return Err(OfdError::Crypto(
                "Encryptions.xml 中没有 EncryptInfo".to_owned(),
            ));
        }
        let mut specs = Vec::with_capacity(infos.len());
        for info in &infos {
            let id = info
                .id()
                .ok_or_else(|| OfdError::InvalidStructure("EncryptInfo 缺少 ID".to_owned()))?;
            let seed_loc = info
                .decrypt_seed_loc()
                .ok_or_else(|| {
                    OfdError::InvalidStructure("EncryptInfo 缺少 DecryptSeedLoc".to_owned())
                })?
                .to_owned();
            let map_loc = info
                .entries_map_loc()
                .ok_or_else(|| {
                    OfdError::InvalidStructure("EncryptInfo 缺少 EntriesMapLoc".to_owned())
                })?
                .to_owned();
            specs.push((
                id.to_owned(),
                info.relative().map(str::to_owned),
                seed_loc,
                map_loc,
            ));
        }
        let order = decryption_order(&specs)?;

        let mut operations = Vec::with_capacity(order.len());
        for index in order {
            let (_, _, seed_loc, map_loc) = &specs[index];
            let info = &infos[index];
            let seed = self.parse_decrypt_seed(seed_loc)?;
            let file_key = backend.unwrap_file_key(info, &seed)?;

            let map_entry = normalize_entry_name(&resolve_loc("", map_loc)?)?;
            let map_raw = self.read_raw_entry(&map_entry)?;
            let map_bytes = if looks_like_xml(&map_raw) {
                map_raw.to_vec()
            } else {
                backend
                    .decrypt(&file_key, &map_raw)
                    .map_err(|_| OfdError::Crypto("明密文映射表解密失败".to_owned()))?
            };
            let map = EncryptedEntries::from_xml(&map_bytes, self.options)?;
            let mut entries = HashMap::new();
            for entry in map.entries() {
                let path = entry.path().ok_or_else(|| {
                    OfdError::InvalidStructure("EncryptEntry 缺少 Path".to_owned())
                })?;
                let epath = entry.epath().unwrap_or(path);
                let seed_loc = match entry.decrypt_seed_loc() {
                    Some(loc) => Some(normalize_entry_name(&resolve_loc("", loc)?)?),
                    None => None,
                };
                entries.insert(
                    normalize_entry_name(path)?,
                    DecryptionEntry {
                        physical: normalize_entry_name(epath)?,
                        seed_loc,
                    },
                );
            }
            operations.push(DecryptionOperation {
                info: info.clone(),
                default_key: file_key,
                entries,
            });
        }

        self.decryption = Some(DecryptionContext {
            backend,
            operations,
            entry_keys: HashMap::new(),
        });
        self.plain_cache.clear();
        Ok(())
    }

    /// 使用调用方提供的验签器验证签名值。
    ///
    /// 先按签名值内容嗅探容器格式（`SignedData` / `SES_Signature` / 其他），
    /// 再交给 [`SignatureVerifiers`] 中匹配的实现；未命中返回
    /// [`OfdError::UnsupportedSignature`]。`Type` 缺失或与内容不一致时以内容为准。
    pub fn verify_signature(
        &mut self,
        signature: &Signature,
        verifiers: &SignatureVerifiers,
    ) -> Result<(), OfdError> {
        let entry = self
            .signature_entry
            .clone()
            .ok_or_else(|| OfdError::InvalidStructure("尚未解析 Signature.xml".to_owned()))?;
        let method = signature
            .signed_info()
            .and_then(|info| info.signature_method().map(str::to_owned))
            .ok_or_else(|| OfdError::InvalidStructure("签名缺少 SignatureMethod".to_owned()))?;
        let content = self.read_entry(&entry)?;
        let signed_value_loc = signature
            .signed_value()
            .ok_or_else(|| OfdError::InvalidStructure("签名缺少 SignedValue".to_owned()))?;
        let relative = resolve_loc(parent_dir(&entry).unwrap_or(""), signed_value_loc)?;
        let signed_entry = if self.contains(&relative) {
            relative
        } else {
            let absolute = resolve_loc("", signed_value_loc)?;
            if self.contains(&absolute) {
                absolute
            } else {
                relative
            }
        };
        let signed_value = self.read_entry(&signed_entry)?;
        let format = crate::crypto::detect_signed_value_format(&signed_value);
        verifiers.verify(format, &method, &content, &signed_value)
    }

    /// 读取签名描述引用的 `SignedValue` 文件。
    ///
    /// `SignedValue` 相对于签名描述文件所在目录解析。
    pub fn read_signature_signed_value(
        &mut self,
        signature: &Signature,
    ) -> Result<std::sync::Arc<[u8]>, OfdError> {
        let entry = self
            .signature_entry
            .clone()
            .ok_or_else(|| OfdError::InvalidStructure("尚未解析 Signature.xml".to_owned()))?;
        let value = signature
            .signed_value()
            .ok_or_else(|| OfdError::InvalidStructure("签名缺少 SignedValue".to_owned()))?;
        let base = parent_dir(&entry)
            .ok_or_else(|| OfdError::InvalidStructure("无法解析签名值路径".to_owned()))?;
        let path = self.resolve_existing_loc(base, value)?;
        self.read_entry(&path)
    }

    /// 校验一个签名描述中的 `References` 文件摘要。
    ///
    /// 只校验文件摘要，不验证 `SignedValue` 的密码学签名。
    pub fn verify_references(
        &mut self,
        signature: &Signature,
        backend: &dyn CryptoBackend,
    ) -> Result<Vec<ReferenceDigest>, OfdError> {
        use base64::Engine;

        let info = signature
            .signed_info()
            .ok_or_else(|| OfdError::InvalidStructure("签名缺少 SignedInfo".to_owned()))?;
        let references = info
            .references()
            .ok_or_else(|| OfdError::InvalidStructure("签名缺少 References".to_owned()))?;
        let method = references.check_method().unwrap_or("MD5");
        // 真实文件的 CheckMethod 有 OID、子弧、大小写等写法，统一成标准名称再交给后端。
        let method = crate::crypto::canonical_digest_method(method).unwrap_or(method);
        let mut checks = Vec::new();
        for reference in references.references() {
            let file_ref = reference
                .file_ref()
                .ok_or_else(|| OfdError::InvalidStructure("Reference 缺少 FileRef".to_owned()))?;
            // 厂商文件里存在把目录写进 References 的情况，目录没有文件摘要，跳过。
            if file_ref.ends_with('/') {
                continue;
            }
            let entry = resolve_loc("", file_ref)?;
            let bytes = self.read_entry(&entry)?;
            let actual = backend.digest(method, &bytes)?;
            let expected = base64::engine::general_purpose::STANDARD
                .decode(reference.check_value().trim())
                .map_err(|_| OfdError::InvalidStructure("CheckValue 不是合法 Base64".to_owned()))?;
            checks.push(ReferenceDigest {
                file_ref: file_ref.to_owned(),
                check_method: method.to_owned(),
                expected,
                actual,
            });
        }
        Ok(checks)
    }

    /// 解析包根目录下的完整性保护描述 `OFDEntries.xml`。
    pub fn parse_ofd_entries(&mut self) -> Result<DocEntries, OfdError> {
        let entry = resolve_loc("", "OFDEntries.xml")?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return DocEntries::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        DocEntries::from_document(doc, self.options)
    }

    /// 校验 `OFDEntries.xml`：列出文件必须存在、不得重复；`SignedValueLoc`
    /// 指向的签名值按 GB/T 35275 验证其对该文件字节的签名。
    pub fn verify_ofd_entries(
        &mut self,
        verifiers: &SignatureVerifiers,
    ) -> Result<EntriesVerification, OfdError> {
        self.verify_ofd_entries_with(verifiers, &crate::refs::ReferenceGraphOptions::default())
    }

    /// 同 [`Self::verify_ofd_entries`]，但允许补充厂商自定义的引用字段。
    pub fn verify_ofd_entries_with(
        &mut self,
        verifiers: &SignatureVerifiers,
        graph: &crate::refs::ReferenceGraphOptions,
    ) -> Result<EntriesVerification, OfdError> {
        let entries_entry = resolve_loc("", "OFDEntries.xml")?;
        let entries_bytes = self.read_entry(&entries_entry)?;
        let view = self.parse_ofd_entries()?;

        let mut seen = HashSet::new();
        let mut files = 0usize;
        for file in view.files() {
            let file_loc = file
                .file_loc()
                .ok_or_else(|| OfdError::InvalidStructure("File 缺少 FileLoc 属性".to_owned()))?;
            let path = resolve_loc("", file_loc)?;
            if !seen.insert(entry_key(&path)) {
                return Err(OfdError::InvalidStructure(format!(
                    "OFDEntries.xml 重复列出文件: {file_loc}"
                )));
            }
            if !self.contains(&path) {
                return Err(OfdError::InvalidStructure(format!(
                    "OFDEntries.xml 列出的文件不存在: {file_loc}"
                )));
            }
            files += 1;
        }

        let signed_value = match view.signed_value_loc() {
            Some(loc) => {
                let path = resolve_loc("", loc)?;
                let value = self.read_entry(&path)?;
                let format = detect_signed_value_format(&value);
                verifiers.verify(format, DEFAULT_SIGNATURE_METHOD, &entries_bytes, &value)?;
                Some(format!("/{path}"))
            }
            None => None,
        };

        let referenced = crate::refs::referenced_entries_with(self, graph)?;
        let mut all = HashMap::new();
        for name in self.entry_names()? {
            if !name.ends_with('/') {
                all.insert(entry_key(&name), name);
            }
        }
        let mut listed = HashMap::new();
        for file in view.files() {
            if let Some(loc) = file.file_loc() {
                let path = resolve_loc("", loc)?;
                listed.insert(entry_key(&path), format!("/{path}"));
            }
        }
        let mut excluded: HashSet<String> = [
            "OFDEntries.xml",
            "Encryptions.xml",
            "decryptseed.dat",
            "entriesmap.dat",
        ]
        .into_iter()
        .map(entry_key)
        .collect();
        if let Some(path) = signed_value.as_deref() {
            excluded.insert(entry_key(path));
        }

        let mut unlisted = Vec::new();
        for (key, name) in &all {
            if referenced.contains(key) || listed.contains_key(key) || excluded.contains(key) {
                continue;
            }
            unlisted.push(format!("/{}", name.replace('\\', "/")));
        }
        unlisted.sort();

        let mut listed_but_referenced = Vec::new();
        for (key, path) in &listed {
            if referenced.contains(key) {
                listed_but_referenced.push(path.clone());
            }
        }
        listed_but_referenced.sort();

        Ok(EntriesVerification {
            files,
            signed_value,
            unlisted,
            listed_but_referenced,
        })
    }

    /// 生成并签署 `OFDEntries.xml`：列出未被文档引用的夹带文件，签名值
    /// 按 GB/T 35275 覆盖 `OFDEntries.xml` 字节。
    pub fn write_ofd_entries<W: Write + Seek>(
        &mut self,
        writer: W,
        producer: &dyn SignedValueProducer,
        options: &EntriesWriteOptions,
    ) -> Result<EntriesWriteReport, OfdError> {
        self.write_ofd_entries_with(
            writer,
            producer,
            options,
            &crate::refs::ReferenceGraphOptions::default(),
        )
    }

    /// 同 [`Self::write_ofd_entries`]，但允许补充厂商自定义的引用字段。
    pub fn write_ofd_entries_with<W: Write + Seek>(
        &mut self,
        mut writer: W,
        producer: &dyn SignedValueProducer,
        options: &EntriesWriteOptions,
        graph: &crate::refs::ReferenceGraphOptions,
    ) -> Result<EntriesWriteReport, OfdError> {
        let entries_entry = resolve_loc("", "OFDEntries.xml")?;
        if self.contains(&entries_entry) && !options.replace_existing {
            return Err(OfdError::InvalidStructure(
                "OFDEntries.xml 已存在；需要覆盖时设置 replace_existing".to_owned(),
            ));
        }
        let signed_entry = resolve_loc("", &options.signed_value_entry)?;

        let referenced = crate::refs::referenced_entries_with(self, graph)?;
        let mut all = HashMap::new();
        for name in self.entry_names()? {
            if !name.ends_with('/') {
                all.insert(entry_key(&name), name);
            }
        }
        let mut excluded: HashSet<String> = [
            "OFDEntries.xml",
            "Encryptions.xml",
            "decryptseed.dat",
            "entriesmap.dat",
        ]
        .into_iter()
        .map(entry_key)
        .collect();
        excluded.insert(entry_key(&signed_entry));

        let mut files: Vec<String> = all
            .iter()
            .filter(|(key, _)| !referenced.contains(*key) && !excluded.contains(*key))
            .map(|(_, name)| format!("/{}", name.replace('\\', "/")))
            .collect();
        files.sort();

        let xml = build_entries_xml(options, &files, &format!("/{signed_entry}"))?;
        let context = SigningContext {
            content: &xml,
            signed_entry: "/OFDEntries.xml",
            signature_method: DEFAULT_SIGNATURE_METHOD,
            signed_at: &options.signed_at,
            references: &[],
        };
        let signed_value = producer.produce(&context)?;

        let mut edits = PackageEdits::new();
        if self.contains(&entries_entry) {
            edits.replace(&entries_entry, xml);
        } else {
            edits.add(&entries_entry, xml);
        }
        if self.contains(&signed_entry) {
            edits.replace(&signed_entry, signed_value);
        } else {
            edits.add(&signed_entry, signed_value);
        }
        self.write_with(&mut writer, &edits, &PackageWriteOptions::default())?;

        Ok(EntriesWriteReport {
            files: files.len(),
            entries_entry: format!("/{entries_entry}"),
            signed_value_entry: format!("/{signed_entry}"),
        })
    }

    fn parse_signature_entry(&mut self, entry: String) -> Result<Signature, OfdError> {
        self.signature_entry = Some(entry.clone());
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return Signature::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        Signature::from_document(doc, self.options)
    }

    /// 清空缓存；已经取出的 `Arc` 视图仍然有效。
    pub fn clear_cache(&mut self) {
        self.entry_cache.clear();
        self.plain_cache.clear();
        self.xml_cache.clear();
    }

    /// 关闭解密会话并清空明文缓存。
    pub fn close_decryption(&mut self) {
        self.decryption = None;
        self.plain_cache.clear();
    }

    /// 是否已启用解密。
    pub fn decryption_enabled(&self) -> bool {
        self.decryption.is_some()
    }

    /// 当前缓存统计。
    pub fn cache_stats(&self) -> CacheStats {
        let entry = self.entry_cache.stats();
        let xml = self.xml_cache.stats();
        let plain = self.plain_cache.stats();
        CacheStats {
            entry_bytes: entry.bytes,
            xml_bytes: xml.bytes,
            entry_hits: entry.hits,
            entry_misses: entry.misses,
            xml_hits: xml.hits,
            xml_misses: xml.misses,
            plain_bytes: plain.bytes,
            plain_hits: plain.hits,
            plain_misses: plain.misses,
        }
    }

    /// 原样重打包当前包。
    ///
    /// 未修改的 entry 直接复制压缩数据与 ZIP 元数据（压缩方式、时间、权限、
    /// extra field、文件注释），包注释同样保留；条目顺序不变。
    pub fn write_to(&mut self, writer: &mut (impl Write + Seek)) -> Result<(), OfdError> {
        self.write_with(
            writer,
            &PackageEdits::new(),
            &PackageWriteOptions::default(),
        )
    }

    /// 带编辑重打包。
    ///
    /// 编辑只作用于指定 entry，其余条目原样复制。加密包不支持直接编辑：
    /// 替换后的明文与 `Encryptions.xml` 的密钥、映射表不一致，请先
    /// [`Self::write_decrypted_to`] 另存为未加密包。
    pub fn write_with(
        &mut self,
        writer: &mut (impl Write + Seek),
        edits: &PackageEdits,
        options: &PackageWriteOptions,
    ) -> Result<(), OfdError> {
        if !edits.is_empty() && self.contains("Encryptions.xml") {
            return Err(OfdError::Crypto(
                "加密包不支持直接替换 entry，请先 write_decrypted_to 另存为未加密包".to_owned(),
            ));
        }
        let existing = self.existing_entries()?;
        let normalized = edits.normalized()?;
        let plan = crate::package_write::plan(&existing, &normalized, options)?;
        self.execute_plan(writer, plan, options)
    }

    /// 解密后另存为未加密包。
    ///
    /// 写出全部逻辑条目（已解密），并移除 `Encryptions.xml`、密钥描述文件、
    /// 明密文映射表与中间密文条目。被移除条目仍被签名 `References` 或
    /// `OFDEntries.xml` 引用时，只记入 [`DecryptedWriteReport::warnings`]，
    /// 不阻断写出。
    pub fn write_decrypted_to(
        &mut self,
        writer: &mut (impl Write + Seek),
    ) -> Result<DecryptedWriteReport, OfdError> {
        if self.decryption.is_none() {
            return Err(OfdError::EncryptionRequired);
        }

        let mut skip: HashSet<String> = HashSet::new();
        skip.insert(entry_key("Encryptions.xml"));
        let encryptions = self.parse_encryptions()?;
        for info in encryptions.encrypt_infos() {
            for loc in [info.decrypt_seed_loc(), info.entries_map_loc()]
                .into_iter()
                .flatten()
            {
                skip.insert(entry_key(&normalize_entry_name(&resolve_loc("", loc)?)?));
            }
        }

        let mut logical_by_physical: HashMap<String, String> = HashMap::new();
        {
            let context = self.decryption.as_ref().expect("decryption context");
            for operation in &context.operations {
                for (logical, entry) in &operation.entries {
                    logical_by_physical
                        .entry(entry_key(&entry.physical))
                        .or_insert_with(|| logical.clone());
                    if let Some(seed) = &entry.seed_loc {
                        skip.insert(entry_key(seed));
                    }
                }
            }
        }

        let options = PackageWriteOptions::default();
        let comment = self.archive.comment().to_vec();
        let mut zip = zip::ZipWriter::new(writer);
        if !comment.is_empty() {
            zip.set_raw_comment(comment.into())?;
        }

        let mut written: HashSet<String> = HashSet::new();
        let mut removed = Vec::new();
        let mut entries = 0usize;
        for index in 0..self.archive.len() {
            let (name, is_dir, modified) = {
                let file = self.archive.by_index_raw(index)?;
                (
                    file.name()?.into_owned(),
                    file.is_dir(),
                    file.last_modified(),
                )
            };
            let key = entry_key(&name);
            if skip.contains(&key) {
                removed.push(name);
                continue;
            }
            if is_dir {
                if !written.insert(key) {
                    continue;
                }
                copy_raw(&mut zip, self.archive.by_index_raw(index)?, &name)?;
                entries += 1;
                continue;
            }
            if let Some(logical) = logical_by_physical.get(&key).cloned() {
                if !written.insert(entry_key(&logical)) {
                    continue;
                }
                let bytes = self.read_entry(&logical)?;
                write_plain_entry(
                    &mut zip,
                    &logical,
                    &bytes,
                    &options,
                    modified.unwrap_or_default(),
                )?;
                entries += 1;
                continue;
            }
            if !written.insert(key) {
                continue;
            }
            copy_raw(&mut zip, self.archive.by_index_raw(index)?, &name)?;
            entries += 1;
        }
        zip.finish()?;

        let references = self.referenced_entries();
        let mut warnings = Vec::new();
        for name in &removed {
            if references.contains(&entry_key(name)) {
                warnings.push(format!(
                    "条目 {name} 被签名或完整性保护引用，解密另存后对应摘要将失效"
                ));
            }
        }
        for physical in logical_by_physical.keys() {
            if references.contains(physical) {
                warnings.push(format!(
                    "密文条目 {physical} 被签名或完整性保护引用，解密另存后对应摘要将失效"
                ));
            }
        }

        Ok(DecryptedWriteReport {
            entries,
            removed,
            warnings,
        })
    }

    /// 生成一个数字签名（`Type=Sign`）。
    ///
    /// 结构层完成：选取保护文件、计算 `References`、组装 `Signature.xml` 与
    /// `Signatures.xml`、必要时给 `OFD.xml` 补 `Signatures` 链接，最后通过
    /// `PackageEdits` 写包。签名值由 `producer` 产出（SM2 → GB/T 35275）。
    ///
    /// 加密包请先 [`Self::write_decrypted_to`] 另存为未加密包。
    pub fn sign_package(
        &mut self,
        request: &SigningRequest<'_>,
        producer: &dyn SignedValueProducer,
        writer: &mut (impl Write + Seek),
    ) -> Result<SigningReport, OfdError> {
        if self.contains("Encryptions.xml") {
            return Err(OfdError::Crypto(
                "加密包不支持直接签名，请先 write_decrypted_to 另存为未加密包".to_owned(),
            ));
        }
        if !producer.supports(request.signature_method) {
            return Err(OfdError::UnsupportedSignature(
                request.signature_method.to_owned(),
            ));
        }
        if request.signed_at.trim().is_empty() {
            return Err(OfdError::InvalidStructure("缺少签名时间".to_owned()));
        }

        self.parse_document()?;
        let doc_entry = self
            .document_entry
            .clone()
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位 Document.xml".to_owned()))?;
        let doc_dir = parent_dir(&doc_entry).unwrap_or("").to_owned();

        let stamp_files = if request.stamps.is_empty() {
            None
        } else {
            Some(crate::stamp::prepare(self, &request.stamps, &doc_entry)?)
        };

        let root = self.parse_root()?;
        let link = root
            .doc_bodies()
            .next()
            .and_then(|body| body.signatures().map(str::to_owned));
        let (signatures_entry, root_edit) = match link {
            Some(loc) => (resolve_loc("", &loc)?, None),
            None => {
                let entry = format!("{doc_dir}/Signs/Signatures.xml");
                let updated =
                    crate::sign::insert_signatures_link(&self.read_entry(ROOT_ENTRY)?, &entry)?;
                (entry, Some(updated))
            }
        };

        let existing = self.signature_entries();
        let id = next_signature_id(&existing);
        let mut index = existing.len();
        loop {
            let dir = format!("{doc_dir}/Signs/Sign_{index}");
            let used = existing
                .iter()
                .any(|(_, base)| base.starts_with(&format!("Sign_{index}/")));
            if !used && !self.contains(&format!("{dir}/Signature.xml")) {
                break;
            }
            index += 1;
        }
        let sign_dir = format!("{doc_dir}/Signs/Sign_{index}");
        let signature_entry = format!("{sign_dir}/Signature.xml");
        let signed_value_entry = format!("{sign_dir}/{}", producer.value_name());
        let base_loc = format!("Sign_{index}/Signature.xml");

        let mut protected: Vec<String> = match &request.scope {
            ProtectScope::AllContent => self
                .entry_names()?
                .into_iter()
                .filter(|name| !name.ends_with('/'))
                .filter_map(|name| normalize_entry_name(&name).ok())
                .collect(),
            ProtectScope::Custom(paths) => paths
                .iter()
                .filter_map(|path| normalize_entry_name(path).ok())
                .collect(),
        };
        let signatures_key = entry_key(&signatures_entry);
        let sign_dir_key = entry_key(&sign_dir);
        protected.retain(|path| {
            let key = entry_key(path);
            if matches!(
                key.as_str(),
                "encryptions.xml" | "decryptseed.dat" | "entriesmap.dat"
            ) {
                return false;
            }
            if request.allow_append && key == signatures_key {
                return false;
            }
            !key.starts_with(&format!("{sign_dir_key}/"))
        });
        if let Some(files) = &stamp_files {
            protected.push(doc_entry.clone());
            for (name, _) in &files.entries {
                protected.push(name.clone());
            }
        }
        protected.sort();
        protected.dedup_by(|left, right| entry_key(left) == entry_key(right));
        if let ProtectScope::Custom(paths) = &request.scope {
            for path in paths {
                let normalized = normalize_entry_name(path)?;
                if !self.contains(&normalized) {
                    return Err(OfdError::InvalidStructure(format!(
                        "保护文件不存在: {path}"
                    )));
                }
            }
        }

        let mut references = Vec::with_capacity(protected.len());
        for path in &protected {
            let staged: Option<&[u8]> = stamp_files.as_ref().and_then(|files| {
                if entry_key(path) == entry_key(&doc_entry) {
                    Some(files.document_xml.as_slice())
                } else {
                    files
                        .entries
                        .iter()
                        .find(|(name, _)| entry_key(name) == entry_key(path))
                        .map(|(_, bytes)| bytes.as_slice())
                }
            });
            let bytes: Arc<[u8]> = if let Some(bytes) = staged {
                Arc::from(bytes)
            } else if entry_key(path) == entry_key(ROOT_ENTRY)
                && let Some(updated) = root_edit.as_deref()
            {
                Arc::from(updated)
            } else {
                self.read_entry(path)?
            };
            let digest = producer.digest(&bytes)?;
            references.push(ReferenceDigest {
                file_ref: format!("/{path}"),
                check_method: crate::sign::DEFAULT_CHECK_METHOD.to_owned(),
                expected: digest.clone(),
                actual: digest,
            });
        }

        let signed_value_path = format!("/{signed_value_entry}");
        let signed_entry_path = format!("/{signature_entry}");
        let signature_xml = crate::sign::write_signature_xml(
            request,
            &references,
            &signed_value_path,
            stamp_files
                .as_ref()
                .map_or(&[][..], |files| files.annots.as_slice()),
        )?;
        let context = SigningContext {
            content: signature_xml.as_bytes(),
            signed_entry: &signed_entry_path,
            signature_method: request.signature_method,
            signed_at: &request.signed_at,
            references: &references,
        };
        let signed_value = producer.produce(&context)?;

        let signature_type = producer.signature_type();
        let signatures_xml = match self.read_entry(&signatures_entry) {
            Ok(existing) => {
                crate::sign::append_signature(&existing, &id, &base_loc, &id, signature_type)?
            }
            Err(_) => crate::sign::new_signatures_xml(&id, &base_loc, signature_type)?.into_bytes(),
        };

        let mut edits = PackageEdits::new();
        if let Some(updated) = root_edit {
            edits.replace(ROOT_ENTRY, updated);
        }
        if self.contains(&signatures_entry) {
            edits.replace(&signatures_entry, signatures_xml);
        } else {
            edits.add(&signatures_entry, signatures_xml);
        }
        edits.add(&signature_entry, signature_xml.into_bytes());
        edits.add(&signed_value_entry, signed_value);
        if let Some(files) = &stamp_files {
            edits.replace(&doc_entry, files.document_xml.clone());
            for (name, bytes) in &files.entries {
                if self.contains(name) {
                    edits.replace(name, bytes.clone());
                } else {
                    edits.add(name, bytes.clone());
                }
            }
        }
        self.write_with(writer, &edits, &PackageWriteOptions::default())?;

        Ok(SigningReport {
            signature_id: id,
            signature_entry: format!("/{signature_entry}"),
            signed_value_entry: signed_value_path,
            signatures_entry: format!("/{signatures_entry}"),
            protected: protected.iter().map(|path| format!("/{path}")).collect(),
        })
    }

    /// 读取已有签名记录的 `(ID, BaseLoc)`，失败时视为空。
    fn signature_entries(&mut self) -> Vec<(String, String)> {
        let Ok(view) = self.parse_signatures() else {
            return Vec::new();
        };
        view.signatures()
            .filter_map(|entry| Some((entry.id()?.to_owned(), entry.base_loc()?.to_owned())))
            .collect()
    }

    /// 便利：重打包到内存。
    pub fn to_bytes(&mut self) -> Result<Vec<u8>, OfdError> {
        let mut cursor = Cursor::new(Vec::new());
        self.write_to(&mut cursor)?;
        Ok(cursor.into_inner())
    }

    /// 便利：重打包到文件（非 wasm）。
    ///
    /// 直接覆盖目标文件；需要原子替换时由调用方先写临时文件再 `rename`。
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&mut self, path: impl AsRef<std::path::Path>) -> Result<(), OfdError> {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        self.write_to(&mut writer)?;
        writer.flush()?;
        Ok(())
    }

    fn existing_entries(&mut self) -> Result<Vec<ExistingEntry>, OfdError> {
        let mut entries = Vec::with_capacity(self.archive.len());
        for index in 0..self.archive.len() {
            let file = self.archive.by_index_raw(index)?;
            entries.push(ExistingEntry {
                index,
                name: file.name()?.into_owned(),
                is_dir: file.is_dir(),
                modified: file.last_modified(),
            });
        }
        Ok(entries)
    }

    fn execute_plan(
        &mut self,
        writer: &mut (impl Write + Seek),
        plan: Vec<PlanItem>,
        options: &PackageWriteOptions,
    ) -> Result<(), OfdError> {
        let comment = self.archive.comment().to_vec();
        let mut zip = zip::ZipWriter::new(writer);
        if !comment.is_empty() {
            zip.set_raw_comment(comment.into())?;
        }
        for item in plan {
            match item {
                PlanItem::CopyRaw { index, name } => {
                    copy_raw(&mut zip, self.archive.by_index_raw(index)?, &name)?;
                }
                PlanItem::CopyDecoded { index, name, time } => {
                    let mut data = Vec::new();
                    self.archive.by_index(index)?.read_to_end(&mut data)?;
                    write_plain_entry(&mut zip, &name, &data, options, time)?;
                }
                PlanItem::Replace { name, bytes, time } => {
                    write_plain_entry(&mut zip, &name, &bytes, options, time)?
                }
                PlanItem::Add { name, bytes, time } => {
                    write_plain_entry(&mut zip, &name, &bytes, options, time)?
                }
            }
        }
        zip.finish()?;
        Ok(())
    }

    /// 收集签名 `References` 与完整性保护文件列表引用到的包内路径。
    ///
    /// 只用于解密另存时的失效提示，任何解析失败都静默忽略。
    fn referenced_entries(&mut self) -> Vec<String> {
        let mut paths = Vec::new();
        if let Ok(signatures) = self.parse_signatures() {
            let base = self
                .signatures_entry
                .as_deref()
                .and_then(parent_dir)
                .map(str::to_owned);
            if let Some(base) = base {
                let locs: Vec<String> = signatures
                    .signatures()
                    .filter_map(|entry| entry.base_loc().map(str::to_owned))
                    .collect();
                for loc in locs {
                    let Ok(entry) = resolve_loc(&base, &loc) else {
                        continue;
                    };
                    let Ok(bytes) = self.read_entry(&entry) else {
                        continue;
                    };
                    let Ok(doc) = parse_xml(&bytes, self.options) else {
                        continue;
                    };
                    let Ok(signature) = Signature::from_document(Arc::new(doc), self.options)
                    else {
                        continue;
                    };
                    let Some(info) = signature.signed_info() else {
                        continue;
                    };
                    let Some(references) = info.references() else {
                        continue;
                    };
                    for reference in references.references() {
                        if let Some(file_ref) = reference.file_ref()
                            && let Ok(normalized) = normalize_entry_name(file_ref)
                        {
                            paths.push(entry_key(&normalized));
                        }
                    }
                }
            }
        }
        if let Ok(entries) = self.parse_ofd_entries() {
            for file in entries.files() {
                if let Some(loc) = file.file_loc()
                    && let Ok(normalized) = normalize_entry_name(loc)
                {
                    paths.push(entry_key(&normalized));
                }
            }
        }
        paths
    }

    fn root_entry_name(&self) -> Result<String, OfdError> {
        if self.contains(ROOT_ENTRY) {
            return Ok(ROOT_ENTRY.to_owned());
        }
        if self.archive.index_for_name("Encryptions.xml").is_some() {
            return Err(OfdError::EncryptionRequired);
        }
        if self.options.strict {
            return Err(OfdError::MissingEntry(ROOT_ENTRY.to_owned()));
        }
        self.entry_names()?
            .into_iter()
            .find(|name| name.eq_ignore_ascii_case(ROOT_ENTRY))
            .ok_or_else(|| OfdError::MissingEntry(ROOT_ENTRY.to_owned()))
    }
}

fn next_signature_id(existing: &[(String, String)]) -> String {
    let used: HashSet<&str> = existing.iter().map(|(id, _)| id.as_str()).collect();
    for number in 1..=9999 {
        let candidate = format!("s{number:03}");
        if !used.contains(candidate.as_str()) {
            return candidate;
        }
    }
    format!("s{}", existing.len() + 1)
}

pub(crate) fn resolve_loc(base: &str, loc: &str) -> Result<String, OfdError> {
    let loc = loc.replace('\\', "/");
    let mut parts: Vec<&str> = if loc.starts_with('/') {
        Vec::new()
    } else {
        base.split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .collect()
    };
    for part in loc.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts
                    .pop()
                    .ok_or_else(|| OfdError::InvalidStructure(format!("路径越过包根: {loc}")))?;
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Err(OfdError::InvalidStructure("路径为空".to_owned()));
    }
    Ok(parts.join("/"))
}

pub(crate) fn parent_dir(path: &str) -> Option<&str> {
    Some(path.rfind('/').map(|index| &path[..index]).unwrap_or(""))
}

fn looks_like_xml(bytes: &[u8]) -> bool {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    bytes
        .iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| *byte == b'<')
}

pub(crate) fn entry_key(name: &str) -> String {
    name.replace('\\', "/")
        .trim_start_matches('/')
        .to_ascii_lowercase()
}

pub(crate) fn normalize_entry_name(name: &str) -> Result<String, OfdError> {
    let name = name.replace('\\', "/");
    let mut parts = Vec::new();
    for part in name.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                return Err(OfdError::InvalidStructure(format!(
                    "entry 路径不允许包含 ..: {name}"
                )));
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Err(OfdError::InvalidStructure("entry 名为空".to_owned()));
    }
    Ok(parts.join("/"))
}

fn decryption_order(
    specs: &[(String, Option<String>, String, String)],
) -> Result<Vec<usize>, OfdError> {
    if specs.len() == 1 {
        return Ok(vec![0]);
    }
    let mut by_id = HashMap::new();
    for (index, (id, _, _, _)) in specs.iter().enumerate() {
        if by_id.insert(id.as_str(), index).is_some() {
            return Err(OfdError::InvalidStructure(format!(
                "EncryptInfo ID 重复: {id}"
            )));
        }
    }
    let referenced: HashSet<&str> = specs
        .iter()
        .filter_map(|(_, relative, _, _)| relative.as_deref())
        .collect();
    let heads: Vec<usize> = specs
        .iter()
        .enumerate()
        .filter(|(_, (id, _, _, _))| !referenced.contains(id.as_str()))
        .map(|(index, _)| index)
        .collect();
    if heads.len() != 1 {
        return Err(OfdError::Crypto(
            "多重加密缺少唯一的最新 EncryptInfo 或 Relative 链".to_owned(),
        ));
    }

    let mut order = Vec::with_capacity(specs.len());
    let mut seen = HashSet::new();
    let mut current = heads[0];
    loop {
        if !seen.insert(current) {
            return Err(OfdError::Crypto("多重加密 Relative 链存在环".to_owned()));
        }
        order.push(current);
        let Some(relative) = specs[current].1.as_deref() else {
            break;
        };
        current = *by_id.get(relative).ok_or_else(|| {
            OfdError::InvalidStructure(format!("Relative 指向不存在的 EncryptInfo: {relative}"))
        })?;
    }
    if order.len() != specs.len() {
        return Err(OfdError::Crypto("多重加密 Relative 链不完整".to_owned()));
    }
    Ok(order)
}

fn build_entries_xml(
    options: &EntriesWriteOptions,
    files: &[String],
    signed_value: &str,
) -> Result<Vec<u8>, OfdError> {
    let entries = SchemaDocEntries {
        id: options.id.clone(),
        creator_name: options.creator_name.clone(),
        version: options.version.clone(),
        creation_date: options.creation_date.clone(),
        file_list: DocEntryFileList {
            files: files
                .iter()
                .enumerate()
                .map(|(index, path)| DocEntryFile {
                    id: format!("f{}", index + 1),
                    file_loc: crate::schema::definition::StLoc::new(path),
                })
                .collect(),
        },
        signed_value_loc: crate::schema::definition::StLoc::new(signed_value),
    };
    crate::schema_xml::to_bytes(&entries)
        .map_err(|error| OfdError::InvalidStructure(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::OfdPackage;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;

    fn zip_with_ofd_xml() -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("OFD.xml", SimpleFileOptions::default())
            .expect("start file");
        writer.write_all(b"<ofd:OFD/>").expect("write entry");
        writer.finish().expect("finish zip").into_inner()
    }

    #[test]
    fn probe_detects_ofd_package() {
        assert!(OfdPackage::probe(&zip_with_ofd_xml()).expect("probe"));
    }

    #[test]
    fn probe_rejects_empty_zip() {
        let empty = [
            0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert!(!OfdPackage::probe(&empty).expect("probe"));
    }

    #[test]
    fn probe_rejects_non_zip() {
        assert!(OfdPackage::probe(b"not a zip").is_err());
    }
}
