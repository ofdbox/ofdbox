//! `Annotation.xsd`：单页注释内容。

use super::definition::{StBox, StId};
use super::page::PageBlock;

/// `PageAnnot` 根对象。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageAnnot {
    /// 注释。
    pub annots: Vec<Annot>,
}

/// 注释。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Annot {
    /// 注释 ID。
    pub id: StId,
    /// 注释类型。
    pub annot_type: AnnotationType,
    /// 创建者。
    pub creator: String,
    /// 最后修改日期。
    pub last_mod_date: String,
    /// 是否可见。
    pub visible: bool,
    /// 子类型。
    pub subtype: Option<String>,
    /// 是否打印。
    pub print: bool,
    /// 是否随缩放。
    pub no_zoom: bool,
    /// 是否随旋转。
    pub no_rotate: bool,
    /// 是否只读。
    pub read_only: bool,
    /// 备注。
    pub remark: Option<String>,
    /// 参数。
    pub parameters: Option<Parameters>,
    /// 外观。
    pub appearance: Appearance,
}

/// 注释类型。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AnnotationType {
    /// 链接。
    #[default]
    Link,
    /// 路径。
    Path,
    /// 高亮。
    Highlight,
    /// 印章。
    Stamp,
    /// 水印。
    Watermark,
    /// 未知值。
    Other(String),
}

/// 注释参数集合。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parameters {
    /// 参数。
    pub parameters: Vec<Parameter>,
}

/// 注释参数。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parameter {
    /// 参数名。
    pub name: String,
    /// 参数值。
    pub value: String,
}

/// 注释外观。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Appearance {
    /// 边界。
    pub boundary: Option<StBox>,
    /// 页块内容。
    pub page_block: PageBlock,
}
