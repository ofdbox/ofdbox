use std::rc::Rc;

use crate::error::OfdError;
use crate::model::definition::{StId, StLoc};
use crate::model::document::{CommonData, Document, Pages};
use crate::model::ofd::{DocBody, DocInfo, Ofd};
use crate::store::storage::{CreateOptions, OpenOptions, OpenStorage, Storage};
use crate::store::{ObjectContext, PackageStore, Stored};

/// OFD 会话。
///
/// 同时支持打开已有文件和创建新文件；存储后端由 options 决定。
pub struct OfdFile {
    store: Rc<PackageStore>,
    root: Rc<ObjectContext>,
}

impl OfdFile {
    /// 创建内存中的新 OFD。
    pub fn create() -> Result<Self, OfdError> {
        Self::create_with(CreateOptions::default())
    }

    /// 按选项创建新 OFD。
    pub fn create_with(options: CreateOptions) -> Result<Self, OfdError> {
        let store = Rc::new(PackageStore::create(options.storage)?);
        let root = ObjectContext::root(Rc::clone(&store));

        let doc_id = options.doc_id.unwrap_or_else(|| "ofdbox".to_owned());
        let ofd = Ofd {
            version: "1.0".to_owned(),
            doc_type: "OFD".to_owned(),
            doc_bodies: vec![DocBody {
                doc_info: DocInfo {
                    doc_id,
                    ..DocInfo::default()
                },
                doc_root: StLoc::new("Doc_0/Document.xml"),
                versions: None,
                signatures: None,
            }],
        };
        Stored::new(ofd, Rc::clone(&root), StLoc::new("OFD.xml")).save()?;

        let document_location = StLoc::new("Doc_0/Document.xml");
        let document_context = ObjectContext::child(&root, document_location.clone())?;
        let document = Document {
            common_data: CommonData {
                max_unit_id: StId::default(),
                page_area: options.page_area.unwrap_or_default(),
                public_res: Vec::new(),
                document_res: Vec::new(),
                template_pages: Vec::new(),
                default_cs: None,
            },
            pages: Pages::default(),
            outlines: None,
            permissions: None,
            actions: None,
            v_preferences: None,
            bookmarks: None,
            annotations: None,
            custom_tags: None,
            attachments: None,
            extensions: None,
        };
        Stored::new(document, document_context, document_location).save()?;

        Ok(Self { store, root })
    }

    /// 以只读 ZIP 打开已有 OFD。
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, OfdError> {
        Self::open_with(path, OpenOptions::default())
    }

    /// 按选项打开已有 OFD。
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_with(
        path: impl AsRef<std::path::Path>,
        options: OpenOptions,
    ) -> Result<Self, OfdError> {
        let store = match options.storage {
            OpenStorage::ZipReadOnly => PackageStore::open_zip(path)?,
            OpenStorage::Memory => PackageStore::extract_from_zip(path, Storage::Memory)?,
            OpenStorage::Directory(directory) => {
                PackageStore::extract_from_zip(path, Storage::Directory(directory))?
            }
        };
        let store = Rc::new(store);
        let root = ObjectContext::root(Rc::clone(&store));
        Ok(Self { store, root })
    }

    /// 从字节以只读 ZIP 打开。
    pub fn open_bytes(bytes: Vec<u8>) -> Result<Self, OfdError> {
        Self::open_bytes_with(bytes, OpenOptions::default())
    }

    /// 从字节按选项打开。
    pub fn open_bytes_with(bytes: Vec<u8>, options: OpenOptions) -> Result<Self, OfdError> {
        let store = match options.storage {
            OpenStorage::ZipReadOnly => PackageStore::open_zip_bytes(bytes)?,
            OpenStorage::Memory => PackageStore::extract_from_bytes(bytes, Storage::Memory)?,
            OpenStorage::Directory(directory) => {
                PackageStore::extract_from_bytes(bytes, Storage::Directory(directory))?
            }
        };
        let store = Rc::new(store);
        let root = ObjectContext::root(Rc::clone(&store));
        Ok(Self { store, root })
    }

    /// 加载 OFD.xml。
    pub fn ofd(&self) -> Result<Stored<Ofd>, OfdError> {
        Stored::<Ofd>::load(Rc::clone(&self.root), StLoc::new("OFD.xml"))
    }

    /// 加载文档。
    pub fn document(&self, index: usize) -> Result<Stored<Document>, OfdError> {
        self.ofd()?.document(index)
    }

    /// 生成完整 OFD 字节。
    pub fn to_bytes(&self) -> Result<Vec<u8>, OfdError> {
        self.store.to_bytes()
    }

    /// 打包并写回 OFD 文件。
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), OfdError> {
        self.store.save(path)
    }
}

impl std::fmt::Debug for OfdFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("OfdFile").finish_non_exhaustive()
    }
}
