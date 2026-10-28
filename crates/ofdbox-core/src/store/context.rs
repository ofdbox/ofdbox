use std::rc::Rc;

use crate::error::OfdError;
use crate::package::{parent_dir, resolve_loc};
use crate::model::definition::StLoc;
use crate::store::PackageStore;

/// 模型对象的共享上下文。
///
/// 包含容器、当前 XML entry 路径、所在目录，以及父上下文。
/// 对象之间只共享 Context，不建立父子对象树。
pub struct ObjectContext {
    store: Rc<PackageStore>,
    current: StLoc,
    base: StLoc,
    parent: Option<Rc<ObjectContext>>,
}

impl ObjectContext {
    /// 创建根上下文。
    pub fn root(store: Rc<PackageStore>) -> Rc<Self> {
        Rc::new(Self {
            store,
            current: StLoc::new(""),
            base: StLoc::new(""),
            parent: None,
        })
    }

    /// 创建某个 entry 的子上下文。
    pub fn child(parent: &Rc<Self>, location: StLoc) -> Result<Rc<Self>, OfdError> {
        let base = parent_dir(location.as_str()).unwrap_or("").to_owned();
        Ok(Rc::new(Self {
            store: Rc::clone(&parent.store),
            current: location,
            base: StLoc::new(base),
            parent: Some(Rc::clone(parent)),
        }))
    }

    /// 解析相对路径。
    pub fn resolve(&self, loc: &StLoc) -> Result<StLoc, OfdError> {
        Ok(StLoc::new(resolve_loc(self.base.as_str(), loc.as_str())?))
    }

    /// 当前 entry。
    pub fn current(&self) -> &StLoc {
        &self.current
    }

    /// 当前 entry 所在目录。
    pub fn base(&self) -> &StLoc {
        &self.base
    }

    /// 父上下文。
    pub fn parent(&self) -> Option<&Rc<ObjectContext>> {
        self.parent.as_ref()
    }

    /// 共享存储。
    pub fn store(&self) -> &Rc<PackageStore> {
        &self.store
    }

    /// 读取 entry。
    pub fn read_entry(&self, entry: &StLoc) -> Result<Vec<u8>, OfdError> {
        self.store.read_entry(entry.as_str())
    }

    /// 暂存写入 entry。
    pub fn write_entry(&self, entry: &StLoc, bytes: Vec<u8>) -> Result<(), OfdError> {
        self.store.write_entry(entry.as_str(), bytes)
    }

    /// 暂存删除 entry。
    pub fn delete_entry(&self, entry: &StLoc) -> Result<(), OfdError> {
        self.store.delete_entry(entry.as_str())
    }
}

impl Clone for ObjectContext {
    fn clone(&self) -> Self {
        Self {
            store: Rc::clone(&self.store),
            current: self.current.clone(),
            base: self.base.clone(),
            parent: self.parent.clone(),
        }
    }
}

impl std::fmt::Debug for ObjectContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ObjectContext")
            .field("current", &self.current)
            .field("base", &self.base)
            .field("has_parent", &self.parent.is_some())
            .finish()
    }
}
