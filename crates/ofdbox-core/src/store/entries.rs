use std::rc::Rc;

use crate::error::OfdError;
use crate::schema::annotation::PageAnnot;
use crate::schema::annotations::Annotations;
use crate::schema::attachments::Attachments;
use crate::schema::custom_tags::CustomTags;
use crate::schema::definition::StLoc;
use crate::schema::document::{Document, PageEntry};
use crate::schema::extensions::Extensions;
use crate::schema::ofd::{DocBody, Ofd};
use crate::schema::page::Page;
use crate::schema::res::Res;
use crate::store::{ObjectContext, Stored};
use yaserde::YaDeserialize;

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
            .find(|entry| entry.id == id)
            .ok_or_else(|| OfdError::InvalidStructure(format!("页面不存在: {id}")))?;

        self.page_entry(entry)
    }

    /// 按 PageEntry 加载页面。
    pub fn page_entry(&self, entry: &PageEntry) -> Result<Stored<Page>, OfdError> {
        let location = self.context().resolve(&entry.base_loc)?;
        Stored::<Page>::load(Rc::clone(self.context()), location)
    }

    /// 加载公共资源。
    pub fn public_res(&self) -> Result<Vec<Stored<Res>>, OfdError> {
        load_all(self.context(), &self.data.common_data.public_res)
    }

    /// 加载文档资源。
    pub fn document_res(&self) -> Result<Vec<Stored<Res>>, OfdError> {
        load_all(self.context(), &self.data.common_data.document_res)
    }

    /// 加载注释索引。
    pub fn annotations(&self) -> Result<Option<Stored<Annotations>>, OfdError> {
        load_optional(self.context(), self.data.annotations.as_ref())
    }

    /// 加载附件索引。
    pub fn attachments(&self) -> Result<Option<Stored<Attachments>>, OfdError> {
        load_optional(self.context(), self.data.attachments.as_ref())
    }

    /// 加载自定义标引索引。
    pub fn custom_tags(&self) -> Result<Option<Stored<CustomTags>>, OfdError> {
        load_optional(self.context(), self.data.custom_tags.as_ref())
    }

    /// 加载扩展索引。
    pub fn extensions(&self) -> Result<Option<Stored<Extensions>>, OfdError> {
        load_optional(self.context(), self.data.extensions.as_ref())
    }

    /// 新建页面并立即写入存储。
    pub fn add_page(&mut self, page: Page) -> Result<Stored<Page>, OfdError> {
        let next_id = self
            .data
            .pages
            .pages
            .iter()
            .map(|entry| entry.id.parse::<u32>().unwrap_or(0))
            .max()
            .unwrap_or(0)
            + 1;
        let id = next_id.to_string();
        let base_loc = StLoc::new(format!("Pages/Page_{next_id}/Content.xml"));
        let location = self.context().resolve(&base_loc)?;
        let page_context = ObjectContext::child(self.context(), location.clone())?;
        let stored = Stored::new(page, page_context, location);
        stored.save()?;

        self.data.pages.pages.push(PageEntry { id, base_loc });
        self.save()?;
        Ok(stored)
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

impl Stored<Page> {
    /// 加载页面资源。
    pub fn page_res(&self) -> Result<Vec<Stored<Res>>, OfdError> {
        load_all(self.context(), &self.data.page_res)
    }

    /// 加载所属文档（按页面缓存）。
    pub fn document(&self) -> Result<Rc<Stored<Document>>, OfdError> {
        if let Some(document) = self.cached_document() {
            return Ok(document);
        }
        let parent = self
            .context()
            .parent()
            .ok_or_else(|| OfdError::InvalidStructure("页面缺少文档上下文".to_owned()))?;
        let document = Rc::new(Stored::<Document>::load(
            Rc::clone(parent),
            parent.current().clone(),
        )?);
        self.cache_document(Rc::clone(&document));
        Ok(document)
    }

    /// 加载所属文档的文档资源。
    pub fn document_res(&self) -> Result<Vec<Stored<Res>>, OfdError> {
        self.document()?.document_res()
    }
}

impl Stored<Annotations> {
    /// 按页面 ID 加载页面注释。
    pub fn page(&self, page_id: &str) -> Result<Stored<PageAnnot>, OfdError> {
        let page = self
            .data
            .pages
            .iter()
            .find(|page| page.page_id == page_id)
            .ok_or_else(|| OfdError::InvalidStructure(format!("页面注释不存在: {page_id}")))?;

        let location = self.context().resolve(&page.file_loc)?;
        Stored::<PageAnnot>::load(Rc::clone(self.context()), location)
    }
}

fn load_all<T: YaDeserialize>(
    context: &Rc<ObjectContext>,
    locations: &[StLoc],
) -> Result<Vec<Stored<T>>, OfdError> {
    locations
        .iter()
        .map(|location| {
            let resolved = context.resolve(location)?;
            Stored::<T>::load(Rc::clone(context), resolved)
        })
        .collect()
}

fn load_optional<T: YaDeserialize>(
    context: &Rc<ObjectContext>,
    location: Option<&StLoc>,
) -> Result<Option<Stored<T>>, OfdError> {
    let Some(location) = location else {
        return Ok(None);
    };
    let resolved = context.resolve(location)?;
    Ok(Some(Stored::<T>::load(Rc::clone(context), resolved)?))
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
    pub fn id(&self) -> &String {
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
