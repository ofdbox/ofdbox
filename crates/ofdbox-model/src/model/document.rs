//! `Document.xsd`：文档根节点、公共数据、页树、权限、视图偏好、大纲和书签。

use super::definition::{Actions, PageArea, StId, StLoc, StRefId};

/// `Document.xml` 根对象。
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    /// 公共数据。
    pub common_data: CommonData,
    /// 页树索引。
    pub pages: Pages,
    /// 大纲。
    pub outlines: Option<Outlines>,
    /// 权限声明。
    pub permissions: Option<Permissions>,
    /// 文档动作。
    pub actions: Option<Actions>,
    /// 视图偏好。
    pub v_preferences: Option<VPreferences>,
    /// 书签。
    pub bookmarks: Option<Bookmarks>,
    /// 注释索引路径。
    pub annotations: Option<StLoc>,
    /// 自定义标引索引路径。
    pub custom_tags: Option<StLoc>,
    /// 附件索引路径。
    pub attachments: Option<StLoc>,
    /// 扩展索引路径。
    pub extensions: Option<StLoc>,
}

/// 文档公共数据。
#[derive(Clone, Debug, PartialEq)]
pub struct CommonData {
    /// 文档内最大对象 ID。
    pub max_unit_id: StId,
    /// 默认页面区域。
    pub page_area: PageArea,
    /// 公共资源路径。
    pub public_res: Vec<StLoc>,
    /// 文档资源路径。
    pub document_res: Vec<StLoc>,
    /// 模板页。
    pub template_pages: Vec<TemplatePage>,
    /// 默认颜色空间。
    pub default_cs: Option<StRefId>,
}

/// 模板页。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TemplatePage {
    /// 模板 ID。
    pub id: StId,
    /// 名称。
    pub name: Option<String>,
    /// 显示层级。
    pub z_order: Option<PageZOrder>,
    /// 模板内容路径。
    pub base_loc: StLoc,
}

/// 模板显示层级。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PageZOrder {
    /// 背景。
    #[default]
    Background,
    /// 前景。
    Foreground,
    /// 未知值。
    Other(String),
}

/// 页树。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pages {
    /// 页面索引。
    pub pages: Vec<PageEntry>,
}

/// 页面索引条目。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageEntry {
    /// 页面 ID。
    pub id: StId,
    /// 页面内容路径。
    pub base_loc: StLoc,
}

/// 大纲。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outlines {
    /// 大纲元素。
    pub outline_elems: Vec<OutlineElem>,
}

/// 大纲元素。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutlineElem {
    /// 标题。
    pub title: String,
    /// 子项数量。
    pub count: Option<i32>,
    /// 是否展开。
    pub expanded: bool,
    /// 动作。
    pub actions: Option<Actions>,
    /// 子元素。
    pub children: Vec<OutlineElem>,
}

/// 权限声明。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Permissions {
    /// 是否允许编辑。
    pub edit: Option<bool>,
    /// 是否允许注释。
    pub annot: Option<bool>,
    /// 是否允许导出。
    pub export: Option<bool>,
    /// 是否允许签名。
    pub signature: Option<bool>,
    /// 是否允许水印。
    pub watermark: Option<bool>,
    /// 是否允许截屏。
    pub print_screen: Option<bool>,
    /// 打印权限。
    pub print: Option<PrintPermission>,
    /// 有效期。
    pub valid_period: Option<ValidPeriod>,
}

/// 打印权限。
#[derive(Clone, Debug, PartialEq)]
pub struct PrintPermission {
    /// 是否可打印。
    pub printable: bool,
    /// 打印份数。
    pub copies: i32,
}

/// 有效期。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValidPeriod {
    /// 开始时间。
    pub start_date: Option<String>,
    /// 结束时间。
    pub end_date: Option<String>,
}

/// 视图偏好。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VPreferences {
    /// 页面模式。
    pub page_mode: Option<PageMode>,
    /// 页面布局。
    pub page_layout: Option<PageLayout>,
    /// 标签显示。
    pub tab_display: Option<TabDisplay>,
    /// 隐藏工具栏。
    pub hide_toolbar: Option<bool>,
    /// 隐藏菜单栏。
    pub hide_menubar: Option<bool>,
    /// 隐藏窗口 UI。
    pub hide_window_ui: Option<bool>,
    /// 缩放设置。
    pub zoom: Option<ZoomSetting>,
}

/// 页面模式。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PageMode {
    /// 无。
    #[default]
    None,
    /// 全屏。
    FullScreen,
    /// 使用大纲。
    UseOutlines,
    /// 使用缩略图。
    UseThumbs,
    /// 使用自定义标引。
    UseCustomTags,
    /// 使用图层。
    UseLayers,
    /// 使用附件。
    UseAttatchs,
    /// 使用书签。
    UseBookmarks,
    /// 未知值。
    Other(String),
}

/// 页面布局。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PageLayout {
    /// 单页。
    OnePage,
    /// 单列。
    #[default]
    OneColumn,
    /// 双页左。
    TwoPageL,
    /// 双列左。
    TwoColumnL,
    /// 双页右。
    TwoPageR,
    /// 双列右。
    TwoColumnR,
    /// 未知值。
    Other(String),
}

/// 标签显示方式。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TabDisplay {
    /// 文档标题。
    #[default]
    DocTitle,
    /// 文件名。
    FileName,
    /// 未知值。
    Other(String),
}

/// 缩放模式。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ZoomMode {
    /// 默认。
    #[default]
    Default,
    /// 适合高度。
    FitHeight,
    /// 适合宽度。
    FitWidth,
    /// 适合区域。
    FitRect,
    /// 未知值。
    Other(String),
}

/// 缩放设置。
#[derive(Clone, Debug, PartialEq)]
pub enum ZoomSetting {
    /// 缩放模式。
    Mode(ZoomMode),
    /// 固定比例。
    Value(f64),
}

/// 书签集合。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bookmarks {
    /// 书签。
    pub bookmarks: Vec<Bookmark>,
}

/// 书签。
#[derive(Clone, Debug, PartialEq)]
pub struct Bookmark {
    /// 名称。
    pub name: String,
    /// 目标位置。
    pub dest: super::definition::Dest,
}
