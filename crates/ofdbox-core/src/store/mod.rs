//! 基于容器与上下文的模型对象存储。
//!
//! 模型本身只保存 XML 数据；本模块负责把模型绑定到具体 entry，
//! 并通过 [`ObjectContext`] 共享 [`PackageStore`]。

mod context;
mod entries;
mod file;
mod object;
mod package_store;
mod storage;

pub use context::ObjectContext;
pub use entries::PageEntryRef;
pub use file::OfdFile;
pub use object::{Stored, StoredObject};
pub use package_store::PackageStore;
pub use storage::{CreateOptions, OpenOptions, OpenStorage, Storage};
