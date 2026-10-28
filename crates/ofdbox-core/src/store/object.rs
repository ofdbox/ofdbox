use std::cell::RefCell;
use std::rc::Rc;

use yaserde::{YaDeserialize, YaSerialize};

use crate::error::OfdError;
use crate::model::definition::StLoc;
use crate::model::document::Document;
use crate::model_xml;
use crate::store::ObjectContext;

/// 已绑定容器位置的 XML 对象。
///
/// `T` 仍是纯模型数据；`Stored<T>` 只额外保存共享上下文和 entry 路径。
pub struct Stored<T> {
    /// 纯模型数据。
    pub data: T,
    context: Rc<ObjectContext>,
    location: StLoc,
    document_cache: RefCell<Option<Rc<Stored<Document>>>>,
}

impl<T> Stored<T> {
    /// 创建一个尚未从容器读取、但已绑定位置的对象。
    pub fn new(data: T, context: Rc<ObjectContext>, location: StLoc) -> Self {
        Self {
            data,
            context,
            location,
            document_cache: RefCell::new(None),
        }
    }

    /// 当前对象的上下文。
    pub fn context(&self) -> &Rc<ObjectContext> {
        &self.context
    }

    /// 当前对象的 entry。
    pub fn location(&self) -> &StLoc {
        &self.location
    }

    pub(crate) fn cached_document(&self) -> Option<Rc<Stored<Document>>> {
        self.document_cache.borrow().as_ref().map(Rc::clone)
    }

    pub(crate) fn cache_document(&self, document: Rc<Stored<Document>>) {
        *self.document_cache.borrow_mut() = Some(document);
    }

    /// 取出纯数据。
    pub fn into_data(self) -> T {
        self.data
    }

    /// 暂存删除当前 entry。
    pub fn delete(&self) -> Result<(), OfdError> {
        self.context.delete_entry(&self.location)
    }
}

impl<T: YaDeserialize> Stored<T> {
    /// 从容器加载对象。
    pub fn load(context: Rc<ObjectContext>, location: StLoc) -> Result<Self, OfdError> {
        let bytes = context.read_entry(&location)?;
        let data = model_xml::from_bytes(&bytes)
            .map_err(|error| OfdError::InvalidStructure(error.to_string()))?;
        let object_context = ObjectContext::child(&context, location.clone())?;

        Ok(Self {
            data,
            context: object_context,
            location,
            document_cache: RefCell::new(None),
        })
    }
}

impl<T: YaSerialize> Stored<T> {
    /// 把当前对象写回容器。
    pub fn save(&self) -> Result<(), OfdError> {
        let bytes = model_xml::to_bytes(&self.data)
            .map_err(|error| OfdError::InvalidStructure(error.to_string()))?;
        self.context.write_entry(&self.location, bytes)
    }
}

impl<T> std::ops::Deref for Stored<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> std::ops::DerefMut for Stored<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Stored<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Stored")
            .field("data", &self.data)
            .field("location", &self.location)
            .finish()
    }
}

/// 从容器读取、写回和删除对象的统一接口。
pub trait StoredObject: Sized {
    /// 纯模型类型。
    type Data;

    /// 当前对象上下文。
    fn context(&self) -> &Rc<ObjectContext>;

    /// 当前 entry。
    fn location(&self) -> &StLoc;

    /// 读取模型数据。
    fn data(&self) -> &Self::Data;

    /// 修改模型数据。
    fn data_mut(&mut self) -> &mut Self::Data;

    /// 从容器加载。
    fn load(context: Rc<ObjectContext>, location: StLoc) -> Result<Self, OfdError>;

    /// 写回容器。
    fn save(&self) -> Result<(), OfdError>;

    /// 删除 entry。
    fn delete(&self) -> Result<(), OfdError>;
}

impl<T> StoredObject for Stored<T>
where
    T: YaSerialize + YaDeserialize,
{
    type Data = T;

    fn context(&self) -> &Rc<ObjectContext> {
        &self.context
    }

    fn location(&self) -> &StLoc {
        &self.location
    }

    fn data(&self) -> &Self::Data {
        &self.data
    }

    fn data_mut(&mut self) -> &mut Self::Data {
        &mut self.data
    }

    fn load(context: Rc<ObjectContext>, location: StLoc) -> Result<Self, OfdError> {
        let bytes = context.read_entry(&location)?;
        let data = model_xml::from_bytes(&bytes)
            .map_err(|error| OfdError::InvalidStructure(error.to_string()))?;
        let object_context = ObjectContext::child(&context, location.clone())?;
        Ok(Self {
            data,
            context: object_context,
            location,
            document_cache: RefCell::new(None),
        })
    }

    fn save(&self) -> Result<(), OfdError> {
        let bytes = model_xml::to_bytes(&self.data)
            .map_err(|error| OfdError::InvalidStructure(error.to_string()))?;
        self.context.write_entry(&self.location, bytes)
    }

    fn delete(&self) -> Result<(), OfdError> {
        self.context.delete_entry(&self.location)
    }
}
