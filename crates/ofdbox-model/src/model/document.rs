//! `Document.xsd`：文档根节点、公共数据、页树、权限、视图偏好、大纲和书签。

use super::definition::{Actions, Dest, PageArea, StId, StLoc, StRefId};

/// `Document.xml` 根对象。
#[derive(Clone, Debug, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Document")]
pub struct Document {
    /// 公共数据。
    #[yaserde(rename = "CommonData")]
    pub common_data: CommonData,
    /// 页树索引。
    #[yaserde(rename = "Pages")]
    pub pages: Pages,
    /// 大纲。
    #[yaserde(rename = "Outlines")]
    pub outlines: Option<Outlines>,
    /// 权限声明。
    #[yaserde(rename = "Permissions")]
    pub permissions: Option<Permissions>,
    /// 文档动作。
    #[yaserde(rename = "Actions")]
    pub actions: Option<Actions>,
    /// 视图偏好。
    #[yaserde(rename = "VPreferences")]
    pub v_preferences: Option<VPreferences>,
    /// 书签。
    #[yaserde(rename = "Bookmarks")]
    pub bookmarks: Option<Bookmarks>,
    /// 注释索引路径。
    #[yaserde(rename = "Annotations")]
    pub annotations: Option<StLoc>,
    /// 自定义标引索引路径。
    #[yaserde(rename = "CustomTags")]
    pub custom_tags: Option<StLoc>,
    /// 附件索引路径。
    #[yaserde(rename = "Attachments")]
    pub attachments: Option<StLoc>,
    /// 扩展索引路径。
    #[yaserde(rename = "Extensions")]
    pub extensions: Option<StLoc>,
}

/// 文档公共数据。
#[derive(Clone, Debug, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "CommonData")]
pub struct CommonData {
    /// 文档内最大对象 ID。
    #[yaserde(rename = "MaxUnitID")]
    pub max_unit_id: StId,
    /// 默认页面区域。
    #[yaserde(rename = "PageArea", default = "default_page_area")]
    pub page_area: PageArea,
    /// 公共资源路径。
    #[yaserde(rename = "PublicRes")]
    pub public_res: Vec<StLoc>,
    /// 文档资源路径。
    #[yaserde(rename = "DocumentRes")]
    pub document_res: Vec<StLoc>,
    /// 模板页。
    #[yaserde(rename = "TemplatePage")]
    pub template_pages: Vec<TemplatePage>,
    /// 默认颜色空间。
    #[yaserde(rename = "DefaultCS")]
    pub default_cs: Option<StRefId>,
}

/// 模板页。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "TemplatePage")]
pub struct TemplatePage {
    /// 模板 ID。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: StId,
    /// 名称。
    #[yaserde(attribute = true, rename = "Name")]
    pub name: Option<String>,
    /// 显示层级。
    #[yaserde(attribute = true, rename = "ZOrder")]
    pub z_order: Option<PageZOrder>,
    /// 模板内容路径。
    #[yaserde(attribute = true, rename = "BaseLoc")]
    pub base_loc: StLoc,
}

/// 模板显示层级。
#[derive(Clone, Debug, Default, PartialEq, Eq, yaserde::DefaultYaSerde)]
pub enum PageZOrder {
    /// 背景。
    #[default]
    Background,
    /// 前景。
    Foreground,
    /// 未知值。
    Other(String),
}

crate::impl_xml_enum_codec!(PageZOrder, Other, {
    Background => "Background",
    Foreground => "Foreground",
});

/// 页树。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Pages")]
pub struct Pages {
    /// 页面索引。
    #[yaserde(rename = "Page")]
    pub pages: Vec<PageEntry>,
}

/// 页面索引条目。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Page")]
pub struct PageEntry {
    /// 页面 ID。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: StId,
    /// 页面内容路径。
    #[yaserde(attribute = true, rename = "BaseLoc")]
    pub base_loc: StLoc,
}

/// 大纲。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Outlines")]
pub struct Outlines {
    /// 大纲元素。
    #[yaserde(rename = "OutlineElem")]
    pub outline_elems: Vec<OutlineElem>,
}

/// 大纲元素。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "OutlineElem")]
pub struct OutlineElem {
    /// 标题。
    #[yaserde(attribute = true, rename = "Title")]
    pub title: String,
    /// 子项数量。
    #[yaserde(attribute = true, rename = "Count")]
    pub count: Option<i32>,
    /// 是否展开。
    #[yaserde(attribute = true, rename = "Expanded", default = "default_true")]
    pub expanded: bool,
    /// 动作。
    #[yaserde(rename = "Actions")]
    pub actions: Option<Actions>,
    /// 子元素。
    #[yaserde(rename = "OutlineElem")]
    pub children: Vec<OutlineElem>,
}

/// 权限声明。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Permissions")]
pub struct Permissions {
    /// 是否允许编辑。
    #[yaserde(rename = "Edit")]
    pub edit: Option<bool>,
    /// 是否允许注释。
    #[yaserde(rename = "Annot")]
    pub annot: Option<bool>,
    /// 是否允许导出。
    #[yaserde(rename = "Export")]
    pub export: Option<bool>,
    /// 是否允许签名。
    #[yaserde(rename = "Signature")]
    pub signature: Option<bool>,
    /// 是否允许水印。
    #[yaserde(rename = "Watermark")]
    pub watermark: Option<bool>,
    /// 是否允许截屏。
    #[yaserde(rename = "PrintScreen")]
    pub print_screen: Option<bool>,
    /// 打印权限。
    #[yaserde(rename = "Print")]
    pub print: Option<PrintPermission>,
    /// 有效期。
    #[yaserde(rename = "ValidPeriod")]
    pub valid_period: Option<ValidPeriod>,
}

/// 打印权限。
#[derive(Clone, Debug, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Print")]
pub struct PrintPermission {
    /// 是否可打印。
    #[yaserde(attribute = true, rename = "Printable")]
    pub printable: bool,
    /// 打印份数。
    #[yaserde(attribute = true, rename = "Copies", default = "default_copies")]
    pub copies: i32,
}

/// 有效期。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "ValidPeriod")]
pub struct ValidPeriod {
    /// 开始时间。
    #[yaserde(attribute = true, rename = "StartDate")]
    pub start_date: Option<String>,
    /// 结束时间。
    #[yaserde(attribute = true, rename = "EndDate")]
    pub end_date: Option<String>,
}

/// 视图偏好。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "VPreferences")]
pub struct VPreferences {
    /// 页面模式。
    #[yaserde(rename = "PageMode")]
    pub page_mode: Option<PageMode>,
    /// 页面布局。
    #[yaserde(rename = "PageLayout")]
    pub page_layout: Option<PageLayout>,
    /// 标签显示。
    #[yaserde(rename = "TabDisplay")]
    pub tab_display: Option<TabDisplay>,
    /// 隐藏工具栏。
    #[yaserde(rename = "HideToolbar")]
    pub hide_toolbar: Option<bool>,
    /// 隐藏菜单栏。
    #[yaserde(rename = "HideMenubar")]
    pub hide_menubar: Option<bool>,
    /// 隐藏窗口 UI。
    #[yaserde(rename = "HideWindowUI")]
    pub hide_window_ui: Option<bool>,
    /// 缩放模式。
    #[yaserde(rename = "ZoomMode")]
    pub zoom_mode: Option<ZoomMode>,
    /// 固定缩放比例。
    #[yaserde(rename = "Zoom")]
    pub zoom: Option<f64>,
}

/// 页面模式。
#[derive(Clone, Debug, Default, PartialEq, Eq, yaserde::DefaultYaSerde)]
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

crate::impl_xml_enum_codec!(PageMode, Other, {
    None => "None",
    FullScreen => "FullScreen",
    UseOutlines => "UseOutlines",
    UseThumbs => "UseThumbs",
    UseCustomTags => "UseCustomTags",
    UseLayers => "UseLayers",
    UseAttatchs => "UseAttatchs",
    UseBookmarks => "UseBookmarks",
});

/// 页面布局。
#[derive(Clone, Debug, Default, PartialEq, Eq, yaserde::DefaultYaSerde)]
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

crate::impl_xml_enum_codec!(PageLayout, Other, {
    OnePage => "OnePage",
    OneColumn => "OneColumn",
    TwoPageL => "TwoPageL",
    TwoColumnL => "TwoColumnL",
    TwoPageR => "TwoPageR",
    TwoColumnR => "TwoColumnR",
});

/// 标签显示方式。
#[derive(Clone, Debug, Default, PartialEq, Eq, yaserde::DefaultYaSerde)]
pub enum TabDisplay {
    /// 文档标题。
    #[default]
    DocTitle,
    /// 文件名。
    FileName,
    /// 未知值。
    Other(String),
}

crate::impl_xml_enum_codec!(TabDisplay, Other, {
    DocTitle => "DocTitle",
    FileName => "FileName",
});

/// 缩放模式。
#[derive(Clone, Debug, Default, PartialEq, Eq, yaserde::DefaultYaSerde)]
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

crate::impl_xml_enum_codec!(ZoomMode, Other, {
    Default => "Default",
    FitHeight => "FitHeight",
    FitWidth => "FitWidth",
    FitRect => "FitRect",
});

/// 书签集合。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Bookmarks")]
pub struct Bookmarks {
    /// 书签。
    #[yaserde(rename = "Bookmark")]
    pub bookmarks: Vec<Bookmark>,
}

/// 书签。
#[derive(Clone, Debug, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Bookmark")]
pub struct Bookmark {
    /// 名称。
    #[yaserde(attribute = true, rename = "Name")]
    pub name: String,
    /// 目标位置。
    #[yaserde(rename = "Dest")]
    pub dest: Dest,
}

fn default_true() -> bool {
    true
}

fn default_page_area() -> PageArea {
    PageArea::default()
}

fn default_copies() -> i32 {
    -1
}
