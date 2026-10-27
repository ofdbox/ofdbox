//! 基于容器与上下文的模型对象存储。
//!
//! 模型本身只保存 XML 数据；本模块负责把模型绑定到具体 entry，
//! 并通过 [`ObjectContext`] 共享 [`PackageStore`]。

mod context;
mod entries;
mod object;
mod package_store;

pub use context::ObjectContext;
pub use entries::{OfdReader, PageEntryRef};
pub use object::{Stored, StoredObject};
pub use package_store::PackageStore;
