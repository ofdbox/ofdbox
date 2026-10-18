use std::io::{Cursor, Read, Seek};
use std::sync::Arc;

use zip::ZipArchive;

use crate::cache::ByteLruCache;
use crate::error::OfdError;
use crate::model::annotation::{AnnotationsView, PageAnnotView};
use crate::model::attachment::AttachmentsView;
use crate::model::custom_tags::CustomTagsView;
use crate::model::decrypt_seed::DecryptSeedView;
use crate::model::document::{DocumentView, PageView};
use crate::model::encryption::EncryptionsView;
use crate::model::entries_map::EncryptedEntriesView;
use crate::model::extensions::ExtensionsView;
use crate::model::ofd::OfdRootView;
use crate::model::page::PageContentView;
use crate::model::res::ResView;
use crate::model::signature::{SignatureView, SignaturesView};
use crate::model::version::DocVersionView;
use crate::options::{CacheOptions, ReadOptions};
use crate::xml::parser::parse_xml;
use crate::xml::tree::XmlDocument;

/// OFD 包根条目的固定名称。
pub const ROOT_ENTRY: &str = "OFD.xml";

trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

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
}

/// 只读 OFD 包。
pub struct OfdPackage {
    archive: ZipArchive<Box<dyn ReadSeek>>,
    options: ReadOptions,
    entry_cache: ByteLruCache<String, Arc<[u8]>>,
    xml_cache: ByteLruCache<(String, bool), Arc<XmlDocument>>,
    document_entry: Option<String>,
    annotations_entry: Option<String>,
    signatures_entry: Option<String>,
}

impl std::fmt::Debug for OfdPackage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OfdPackage")
            .field("entry_count", &self.entry_count())
            .field("options", &self.options)
            .finish()
    }
}

impl OfdPackage {
    /// 从内存字节打开包，使用宽松读取模式和默认缓存。
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, OfdError> {
        Self::from_bytes_with_options(bytes, ReadOptions::LENIENT, CacheOptions::default())
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
        Ok(Self {
            archive,
            options,
            entry_cache: ByteLruCache::new(cache_options.max_bytes),
            xml_cache: ByteLruCache::new(cache_options.max_bytes),
            document_entry: None,
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
    pub fn contains(&self, name: &str) -> bool {
        self.archive.index_for_name(name).is_some()
    }

    /// entry 数量。
    pub fn entry_count(&self) -> usize {
        self.archive.len()
    }

    /// 读取全部 entry 名称。
    pub fn entry_names(&self) -> Result<Vec<String>, OfdError> {
        let mut names = Vec::new();
        for name in self.archive.file_names() {
            names.push(name?.into_owned());
        }
        Ok(names)
    }

    /// 按需读取并解压一个 entry。
    pub fn read_entry(&mut self, name: &str) -> Result<Arc<[u8]>, OfdError> {
        let name = normalize_entry_name(name)?;
        if let Some(bytes) = self.entry_cache.get(&name) {
            return Ok(bytes);
        }

        let mut entry = match self.archive.by_name(&name) {
            Ok(entry) => entry,
            Err(zip::result::ZipError::FileNotFound) => {
                return Err(OfdError::MissingEntry(name));
            }
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        let bytes: Arc<[u8]> = Arc::from(bytes);
        self.entry_cache
            .insert(name, Arc::clone(&bytes), bytes.len());
        Ok(bytes)
    }

    /// 读取并解析 `OFD.xml`。
    pub fn parse_root(&mut self) -> Result<OfdRootView, OfdError> {
        let entry = self.root_entry_name()?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return OfdRootView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        OfdRootView::from_document(doc, self.options)
    }

    /// 读取并解析 `OFD.xml` 中 `DocRoot` 指向的 `Document.xml`。
    pub fn parse_document(&mut self) -> Result<DocumentView, OfdError> {
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
            return DocumentView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        DocumentView::from_document(doc, self.options)
    }

    /// 读取并解析页面内容文件，路径相对于包根。
    pub fn parse_page(&mut self, loc: &str) -> Result<PageContentView, OfdError> {
        let entry = resolve_loc("", loc)?;
        self.parse_page_entry(entry)
    }

    /// 解析 `Document.xml` 中某个页面指向的内容文件。
    ///
    /// `Page.BaseLoc` 相对于 `Document.xml` 所在目录解析。
    pub fn parse_page_for(&mut self, page: &PageView) -> Result<PageContentView, OfdError> {
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

    fn parse_page_entry(&mut self, entry: String) -> Result<PageContentView, OfdError> {
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return PageContentView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        PageContentView::from_document(doc, self.options)
    }

    /// 读取并解析资源文件，路径相对于包根。
    pub fn parse_res(&mut self, loc: &str) -> Result<ResView, OfdError> {
        let entry = resolve_loc("", loc)?;
        self.parse_res_entry(entry)
    }

    /// 解析 `Document.xml` 中引用的资源文件。
    ///
    /// `PublicRes` / `DocumentRes` 相对于 `Document.xml` 所在目录解析。
    pub fn parse_res_from_document(&mut self, loc: &str) -> Result<ResView, OfdError> {
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

    fn parse_res_entry(&mut self, entry: String) -> Result<ResView, OfdError> {
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return ResView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        ResView::from_document(doc, self.options)
    }

    /// 解析 `Document.xml` 中声明的注释索引文件。
    ///
    /// `Annotations` 相对于 `Document.xml` 所在目录解析。
    pub fn parse_annotations(&mut self) -> Result<AnnotationsView, OfdError> {
        let document = self.parse_document()?;
        let loc = document.annotations().map(str::to_owned).ok_or_else(|| {
            OfdError::InvalidStructure("Document.xml 未声明 Annotations".to_owned())
        })?;
        let base = self
            .document_entry
            .as_deref()
            .and_then(parent_dir)
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位 Document.xml".to_owned()))?;
        let entry = resolve_loc(base, &loc)?;
        let view = self.parse_annotations_entry(entry.clone())?;
        self.annotations_entry = Some(entry);
        Ok(view)
    }

    /// 解析 `Document.xml` 中声明的附件列表文件。
    ///
    /// `Attachments` 相对于 `Document.xml` 所在目录解析。
    pub fn parse_attachments(&mut self) -> Result<AttachmentsView, OfdError> {
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
            return AttachmentsView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        AttachmentsView::from_document(doc, self.options)
    }

    /// 解析 `Document.xml` 中声明的自定义标引列表文件。
    ///
    /// `CustomTags` 相对于 `Document.xml` 所在目录解析。
    pub fn parse_custom_tags(&mut self) -> Result<CustomTagsView, OfdError> {
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
            return CustomTagsView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        CustomTagsView::from_document(doc, self.options)
    }

    /// 解析 `Document.xml` 中声明的扩展信息列表文件。
    ///
    /// `Extensions` 相对于 `Document.xml` 所在目录解析。
    pub fn parse_extensions(&mut self) -> Result<ExtensionsView, OfdError> {
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
            return ExtensionsView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        ExtensionsView::from_document(doc, self.options)
    }

    /// 解析注释索引中 `FileLoc` 指向的分页注释文件。
    ///
    /// 路径相对于注释索引文件所在目录解析；索引尚未解析时会先读取 `Document.xml`。
    pub fn parse_page_annot(&mut self, file_loc: &str) -> Result<PageAnnotView, OfdError> {
        if self.annotations_entry.is_none() {
            self.parse_annotations()?;
        }
        let base = self
            .annotations_entry
            .as_deref()
            .and_then(parent_dir)
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位注释索引文件".to_owned()))?;
        let entry = resolve_loc(base, file_loc)?;
        self.parse_page_annot_entry(entry)
    }

    fn parse_annotations_entry(&mut self, entry: String) -> Result<AnnotationsView, OfdError> {
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return AnnotationsView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        AnnotationsView::from_document(doc, self.options)
    }

    fn parse_page_annot_entry(&mut self, entry: String) -> Result<PageAnnotView, OfdError> {
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return PageAnnotView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        PageAnnotView::from_document(doc, self.options)
    }

    /// 解析 `OFD.xml` 中第一个 `DocBody` 声明的签名列表文件。
    ///
    /// `Signatures` 相对于 `OFD.xml` 所在目录解析。
    pub fn parse_signatures(&mut self) -> Result<SignaturesView, OfdError> {
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

    /// 解析签名列表中 `BaseLoc` 指向的签名描述文件。
    ///
    /// 路径相对于签名列表文件所在目录解析；列表尚未解析时会先读取 `OFD.xml`。
    pub fn parse_signature(&mut self, base_loc: &str) -> Result<SignatureView, OfdError> {
        if self.signatures_entry.is_none() {
            self.parse_signatures()?;
        }
        let base = self
            .signatures_entry
            .as_deref()
            .and_then(parent_dir)
            .ok_or_else(|| OfdError::InvalidStructure("尚未定位签名列表文件".to_owned()))?;
        let entry = resolve_loc(base, base_loc)?;
        self.parse_signature_entry(entry)
    }

    fn parse_signatures_entry(&mut self, entry: String) -> Result<SignaturesView, OfdError> {
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return SignaturesView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        SignaturesView::from_document(doc, self.options)
    }

    /// 解析版本列表 `BaseLoc` 指向的版本描述文件，路径相对于包根。
    pub fn parse_doc_version(&mut self, base_loc: &str) -> Result<DocVersionView, OfdError> {
        let entry = resolve_loc("", base_loc)?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return DocVersionView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        DocVersionView::from_document(doc, self.options)
    }

    /// 解析包根目录下的解密入口 `Encryptions.xml`。
    pub fn parse_encryptions(&mut self) -> Result<EncryptionsView, OfdError> {
        let entry = resolve_loc("", "Encryptions.xml")?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return EncryptionsView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        EncryptionsView::from_document(doc, self.options)
    }

    /// 解析密钥描述文件，路径相对于包根。
    pub fn parse_decrypt_seed(&mut self, loc: &str) -> Result<DecryptSeedView, OfdError> {
        let entry = resolve_loc("", loc)?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return DecryptSeedView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        DecryptSeedView::from_document(doc, self.options)
    }

    /// 解析未加密的明密文映射表，路径相对于包根。
    ///
    /// `entriesmap.dat` 可能是加密二进制，需先解密后再调用本方法。
    pub fn parse_entries_map(&mut self, loc: &str) -> Result<EncryptedEntriesView, OfdError> {
        let entry = resolve_loc("", loc)?;
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return EncryptedEntriesView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        EncryptedEntriesView::from_document(doc, self.options)
    }

    fn parse_signature_entry(&mut self, entry: String) -> Result<SignatureView, OfdError> {
        let key = (entry.clone(), self.options.strict);
        if let Some(doc) = self.xml_cache.get(&key) {
            return SignatureView::from_document(doc, self.options);
        }

        let bytes = self.read_entry(&entry)?;
        let doc = Arc::new(parse_xml(&bytes, self.options)?);
        self.xml_cache
            .insert(key, Arc::clone(&doc), doc.estimated_size());
        SignatureView::from_document(doc, self.options)
    }

    /// 清空缓存；已经取出的 `Arc` 视图仍然有效。
    pub fn clear_cache(&mut self) {
        self.entry_cache.clear();
        self.xml_cache.clear();
    }

    /// 当前缓存统计。
    pub fn cache_stats(&self) -> CacheStats {
        let entry = self.entry_cache.stats();
        let xml = self.xml_cache.stats();
        CacheStats {
            entry_bytes: entry.bytes,
            xml_bytes: xml.bytes,
            entry_hits: entry.hits,
            entry_misses: entry.misses,
            xml_hits: xml.hits,
            xml_misses: xml.misses,
        }
    }

    fn root_entry_name(&self) -> Result<String, OfdError> {
        if self.contains(ROOT_ENTRY) {
            return Ok(ROOT_ENTRY.to_owned());
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

fn resolve_loc(base: &str, loc: &str) -> Result<String, OfdError> {
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

fn parent_dir(path: &str) -> Option<&str> {
    Some(path.rfind('/').map(|index| &path[..index]).unwrap_or(""))
}

fn normalize_entry_name(name: &str) -> Result<String, OfdError> {
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
