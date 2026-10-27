use std::rc::Rc;

use crate::error::OfdError;
use crate::schema::definition::StLoc;
use crate::schema::document::{Document, PageEntry};
use crate::schema::ofd::{DocBody, Ofd};
use crate::schema::page::Page;
use crate::store::{ObjectContext, Stored};

impl Stored<Ofd> {
    /// 按 DocBody 序号加载文档。
    pub fn document(&self, index: usize) -> Result<Stored<Document>, OfdError> {
        let body = self
            .data
            .doc_bodies
            .get(index)
            .ok_or_else(|| OfdError::InvalidStructure(format!("DocBody 不存在: {index}")))?;

        body.load(self)
    }
}

impl DocBody {
    /// 根据 OFD 上下文加载 Document。
    pub fn load(&self, ofd: &Stored<Ofd>) -> Result<Stored<Document>, OfdError> {
        let location = ofd.context().resolve(&self.doc_root)?;
        Stored::<Document>::load(Rc::clone(ofd.context()), location)
    }
}

impl Stored<Document> {
    /// 按页面 ID 加载页面。
    pub fn page(&self, id: &str) -> Result<Stored<Page>, OfdError> {
        let entry = self
            .data
            .pages
            .pages
            .iter()
            .find(|entry| entry.id.to_string() == id)
            .ok_or_else(|| OfdError::InvalidStructure(format!("页面不存在: {id}")))?;

        self.page_entry(entry)
    }

    /// 按 PageEntry 加载页面。
    pub fn page_entry(&self, entry: &PageEntry) -> Result<Stored<Page>, OfdError> {
        let location = self.context().resolve(&entry.base_loc)?;
        Stored::<Page>::load(Rc::clone(self.context()), location)
    }

    /// 页面索引迭代器。
    pub fn pages(&self) -> impl Iterator<Item = PageEntryRef<'_>> {
        self.data.pages.pages.iter().map(|entry| PageEntryRef {
            entry,
            document: self,
        })
    }
}

impl PageEntry {
    /// 从所属 Document 直接加载页面。
    pub fn load(&self, document: &Stored<Document>) -> Result<Stored<Page>, OfdError> {
        document.page_entry(self)
    }
}

/// 带文档上下文的页面索引引用。
pub struct PageEntryRef<'a> {
    entry: &'a PageEntry,
    document: &'a Stored<Document>,
}

impl<'a> PageEntryRef<'a> {
    /// 原始 PageEntry。
    pub fn entry(&self) -> &'a PageEntry {
        self.entry
    }

    /// 页面 ID。
    pub fn id(&self) -> &crate::schema::definition::StId {
        &self.entry.id
    }

    /// 页面内容路径。
    pub fn base_loc(&self) -> &StLoc {
        &self.entry.base_loc
    }

    /// 加载页面。
    pub fn load(&self) -> Result<Stored<Page>, OfdError> {
        self.entry.load(self.document)
    }
}

/// 一个便于从根上下文加载 OFD 的入口。
pub struct OfdReader {
    store: Rc<crate::store::PackageStore>,
    root: Rc<ObjectContext>,
}

impl OfdReader {
    /// 打开 OFD 文件。
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, OfdError> {
        let store = Rc::new(crate::store::PackageStore::open(path)?);
        let root = ObjectContext::root(Rc::clone(&store));
        Ok(Self { store, root })
    }

    /// 从内存字节打开。
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, OfdError> {
        let store = Rc::new(crate::store::PackageStore::from_bytes(bytes)?);
        let root = ObjectContext::root(Rc::clone(&store));
        Ok(Self { store, root })
    }

    /// 加载 OFD.xml。
    pub fn ofd(&self) -> Result<Stored<Ofd>, OfdError> {
        Stored::<Ofd>::load(Rc::clone(&self.root), StLoc::new("OFD.xml"))
    }

    /// 写回文件。
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), OfdError> {
        self.store.save(path)
    }
}
