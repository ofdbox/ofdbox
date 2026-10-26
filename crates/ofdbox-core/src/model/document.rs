use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::model::action::{Actions, validate_actions};
use crate::model::ofd::{
    check_attributes, check_children_order, check_namespace, validate_text_element,
};
use crate::model::outline::{Outlines, validate_outlines};
use crate::options::ReadOptions;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

const DOCUMENT_ORDER: &[&str] = &[
    "CommonData",
    "Pages",
    "Outlines",
    "Permissions",
    "Actions",
    "VPreferences",
    "Bookmarks",
    "Annotations",
    "CustomTags",
    "Attachments",
    "Extensions",
];
const COMMON_DATA_ORDER: &[&str] = &[
    "MaxUnitID",
    "PageArea",
    "PublicRes",
    "DocumentRes",
    "TemplatePage",
    "DefaultCS",
];
const PAGE_AREA_ORDER: &[&str] = &["PhysicalBox", "ApplicationBox", "ContentBox", "BleedBox"];
const PERMISSIONS_ORDER: &[&str] = &[
    "Edit",
    "Annot",
    "Export",
    "Signature",
    "Watermark",
    "PrintScreen",
    "Print",
    "ValidPeriod",
];
const VPREFERENCES_ORDER: &[&str] = &[
    "PageMode",
    "PageLayout",
    "TabDisplay",
    "HideToolbar",
    "HideMenubar",
    "HideWindowUI",
    "ZoomMode",
    "Zoom",
];

/// `Document.xml` 根元素的类型化视图。
#[derive(Debug, Clone)]
pub struct Document {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Document {
    /// 从 XML 字节解析文档根视图。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "Document" {
            return Err(OfdError::InvalidStructure(
                "Document.xml 的根元素不是 Document".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 底层语法树。
    pub fn document(&self) -> &Arc<XmlDocument> {
        &self.doc
    }

    /// 根节点编号。
    pub fn node(&self) -> NodeId {
        self.node
    }

    /// 公共数据。
    pub fn common_data(&self) -> Option<CommonData> {
        self.view_child("CommonData", |doc, node| CommonData { doc, node })
    }

    /// 页树。
    pub fn pages(&self) -> Option<Pages> {
        self.view_child("Pages", |doc, node| Pages { doc, node })
    }

    /// 文档权限。
    pub fn permissions(&self) -> Option<Permissions> {
        self.view_child("Permissions", |doc, node| Permissions { doc, node })
    }

    /// 视图首选项。
    pub fn v_preferences(&self) -> Option<VPreferences> {
        self.view_child("VPreferences", |doc, node| VPreferences { doc, node })
    }

    /// 书签集。
    pub fn bookmarks(&self) -> Option<Bookmarks> {
        self.view_child("Bookmarks", |doc, node| Bookmarks { doc, node })
    }

    /// 注释列表路径。
    pub fn annotations(&self) -> Option<&str> {
        self.child_text("Annotations")
    }

    /// 自定义标引列表路径。
    pub fn custom_tags(&self) -> Option<&str> {
        self.child_text("CustomTags")
    }

    /// 附件列表路径。
    pub fn attachments(&self) -> Option<&str> {
        self.child_text("Attachments")
    }

    /// 扩展信息列表路径。
    pub fn extensions(&self) -> Option<&str> {
        self.child_text("Extensions")
    }

    /// 大纲。
    pub fn outlines(&self) -> Option<Outlines> {
        self.view_child("Outlines", Outlines::new)
    }

    /// 文档级动作。
    pub fn actions(&self) -> Option<Actions> {
        self.view_child("Actions", Actions::new)
    }

    /// 是否存在大纲。
    pub fn has_outlines(&self) -> bool {
        self.outlines().is_some()
    }

    /// 是否存在文档级动作。
    pub fn has_actions(&self) -> bool {
        self.actions().is_some()
    }

    /// 严格序列化为 `Document.xml` 文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::document::write_document(self)
    }

    /// 严格序列化为 UTF-8 字节。
    pub fn to_bytes(&self) -> Result<Vec<u8>, OfdError> {
        self.to_xml().map(String::into_bytes)
    }

    pub(crate) fn validate_strict(&self) -> Result<(), OfdError> {
        let mut diagnostics = Vec::new();
        check_namespace(&self.doc, self.node, &mut diagnostics);
        check_attributes(&self.doc, self.node, &[], &[], &mut diagnostics);
        check_children_order(
            &self.doc,
            self.node,
            DOCUMENT_ORDER,
            &["CommonData", "Pages"],
            &[],
            &mut diagnostics,
        );

        if let Some(common) = self.doc.find_child(self.node, "CommonData") {
            validate_common_data(&self.doc, common, &mut diagnostics);
        }
        if let Some(pages) = self.doc.find_child(self.node, "Pages") {
            validate_pages(&self.doc, pages, &mut diagnostics);
        }
        if let Some(outlines) = self.doc.find_child(self.node, "Outlines") {
            validate_outlines(&self.doc, outlines, &mut diagnostics);
        }
        if let Some(permissions) = self.doc.find_child(self.node, "Permissions") {
            validate_permissions(&self.doc, permissions, &mut diagnostics);
        }
        if let Some(actions) = self.doc.find_child(self.node, "Actions") {
            validate_actions(&self.doc, actions, &mut diagnostics);
        }
        if let Some(prefs) = self.doc.find_child(self.node, "VPreferences") {
            validate_v_preferences(&self.doc, prefs, &mut diagnostics);
        }
        if let Some(bookmarks) = self.doc.find_child(self.node, "Bookmarks") {
            validate_bookmarks(&self.doc, bookmarks, &mut diagnostics);
        }
        for name in ["Annotations", "CustomTags", "Attachments", "Extensions"] {
            if let Some(node) = self.doc.find_child(self.node, name) {
                validate_text_element(&self.doc, node, false, &mut diagnostics);
            }
        }

        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }

    fn view_child<T>(
        &self,
        name: &str,
        make: impl FnOnce(Arc<XmlDocument>, NodeId) -> T,
    ) -> Option<T> {
        self.doc
            .find_child(self.node, name)
            .map(|node| make(Arc::clone(&self.doc), node))
    }

    fn child_text(&self, name: &str) -> Option<&str> {
        self.doc
            .find_child(self.node, name)
            .map(|node| self.doc.text(node).trim())
    }
}

/// `CommonData` 的类型化视图。
#[derive(Debug, Clone)]
pub struct CommonData {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl CommonData {
    /// 当前文档最大对象标识。
    pub fn max_unit_id(&self) -> Option<&str> {
        child_text(&self.doc, self.node, "MaxUnitID")
    }

    /// 页面区域。
    pub fn page_area(&self) -> Option<PageArea> {
        self.doc
            .find_child(self.node, "PageArea")
            .map(|node| PageArea {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    /// 公共资源路径列表。
    pub fn public_res(&self) -> impl Iterator<Item = &str> + '_ {
        child_texts(&self.doc, self.node, "PublicRes").into_iter()
    }

    /// 文档资源路径列表。
    pub fn document_res(&self) -> impl Iterator<Item = &str> + '_ {
        child_texts(&self.doc, self.node, "DocumentRes").into_iter()
    }

    /// 模板页列表。
    pub fn template_pages(&self) -> impl Iterator<Item = TemplatePageEntry> {
        self.doc
            .children_named(self.node, "TemplatePage")
            .map(|node| TemplatePageEntry {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 默认颜色空间标识。
    pub fn default_cs(&self) -> Option<&str> {
        child_text(&self.doc, self.node, "DefaultCS")
    }
}

/// `PageArea` 的类型化视图。
#[derive(Debug, Clone)]
pub struct PageArea {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl PageArea {
    pub(crate) fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 物理区域。
    pub fn physical_box(&self) -> Option<&str> {
        child_text(&self.doc, self.node, "PhysicalBox")
    }

    /// 显示区域。
    pub fn application_box(&self) -> Option<&str> {
        child_text(&self.doc, self.node, "ApplicationBox")
    }

    /// 版心区域。
    pub fn content_box(&self) -> Option<&str> {
        child_text(&self.doc, self.node, "ContentBox")
    }

    /// 出血区域。
    pub fn bleed_box(&self) -> Option<&str> {
        child_text(&self.doc, self.node, "BleedBox")
    }
}

/// 模板页的类型化视图。
#[derive(Debug, Clone)]
pub struct TemplatePageEntry {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl TemplatePageEntry {
    /// 模板页标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 模板页名称。
    pub fn name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Name")
    }

    /// 模板页层级。
    pub fn z_order(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ZOrder")
    }

    /// 模板页内容路径。
    pub fn base_loc(&self) -> Option<&str> {
        self.doc.attribute(self.node, "BaseLoc")
    }
}

/// `Pages` 的类型化视图。
#[derive(Debug, Clone)]
pub struct Pages {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Pages {
    /// 遍历页面。
    pub fn pages(&self) -> impl Iterator<Item = PageEntry> {
        self.doc
            .children_named(self.node, "Page")
            .map(|node| PageEntry {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 单个页面的视图。
#[derive(Debug, Clone)]
pub struct PageEntry {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl PageEntry {
    /// 页面标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 页面内容路径。
    pub fn base_loc(&self) -> Option<&str> {
        self.doc.attribute(self.node, "BaseLoc")
    }
}

/// 文档权限的视图。
#[derive(Debug, Clone)]
pub struct Permissions {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Permissions {
    fn bool_value(&self, name: &str) -> Option<bool> {
        child_text(&self.doc, self.node, name).and_then(parse_bool)
    }

    /// 是否允许编辑。
    pub fn edit(&self) -> Option<bool> {
        self.bool_value("Edit")
    }

    /// 是否允许注释。
    pub fn annot(&self) -> Option<bool> {
        self.bool_value("Annot")
    }

    /// 是否允许导出。
    pub fn export(&self) -> Option<bool> {
        self.bool_value("Export")
    }

    /// 是否允许签名。
    pub fn signature(&self) -> Option<bool> {
        self.bool_value("Signature")
    }

    /// 是否允许水印。
    pub fn watermark(&self) -> Option<bool> {
        self.bool_value("Watermark")
    }

    /// 是否允许截屏。
    pub fn print_screen(&self) -> Option<bool> {
        self.bool_value("PrintScreen")
    }

    /// 打印权限。
    pub fn print(&self) -> Option<Print> {
        self.doc.find_child(self.node, "Print").map(|node| Print {
            doc: Arc::clone(&self.doc),
            node,
        })
    }

    /// 有效期。
    pub fn valid_period(&self) -> Option<ValidPeriod> {
        self.doc
            .find_child(self.node, "ValidPeriod")
            .map(|node| ValidPeriod {
                doc: Arc::clone(&self.doc),
                node,
            })
    }
}

/// 打印权限的视图。
#[derive(Debug, Clone)]
pub struct Print {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Print {
    /// 是否允许打印。
    pub fn printable(&self) -> Option<bool> {
        self.doc
            .attribute(self.node, "Printable")
            .and_then(parse_bool)
    }

    /// 允许打印份数。
    pub fn copies(&self) -> Option<i32> {
        self.doc
            .attribute(self.node, "Copies")
            .and_then(|value| value.parse().ok())
    }
}

/// 有效期的视图。
#[derive(Debug, Clone)]
pub struct ValidPeriod {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl ValidPeriod {
    /// 开始日期。
    pub fn start_date(&self) -> Option<&str> {
        self.doc.attribute(self.node, "StartDate")
    }

    /// 结束日期。
    pub fn end_date(&self) -> Option<&str> {
        self.doc.attribute(self.node, "EndDate")
    }
}

/// 视图首选项的视图。
#[derive(Debug, Clone)]
pub struct VPreferences {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl VPreferences {
    fn text_value(&self, name: &str) -> Option<&str> {
        child_text(&self.doc, self.node, name)
    }

    fn bool_value(&self, name: &str) -> Option<bool> {
        self.text_value(name).and_then(parse_bool)
    }

    /// 页面模式。
    pub fn page_mode(&self) -> Option<&str> {
        self.text_value("PageMode")
    }

    /// 页面布局。
    pub fn page_layout(&self) -> Option<&str> {
        self.text_value("PageLayout")
    }

    /// 标签显示方式。
    pub fn tab_display(&self) -> Option<&str> {
        self.text_value("TabDisplay")
    }

    /// 是否隐藏工具栏。
    pub fn hide_toolbar(&self) -> Option<bool> {
        self.bool_value("HideToolbar")
    }

    /// 是否隐藏菜单栏。
    pub fn hide_menubar(&self) -> Option<bool> {
        self.bool_value("HideMenubar")
    }

    /// 是否隐藏窗口 UI。
    pub fn hide_window_ui(&self) -> Option<bool> {
        self.bool_value("HideWindowUI")
    }

    /// 缩放模式。
    pub fn zoom_mode(&self) -> Option<&str> {
        self.text_value("ZoomMode")
    }

    /// 缩放比例。
    pub fn zoom(&self) -> Option<f64> {
        self.text_value("Zoom").and_then(|value| value.parse().ok())
    }
}

/// 书签集的视图。
#[derive(Debug, Clone)]
pub struct Bookmarks {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Bookmarks {
    /// 遍历书签。
    pub fn bookmarks(&self) -> impl Iterator<Item = Bookmark> {
        self.doc
            .children_named(self.node, "Bookmark")
            .map(|node| Bookmark {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 单个书签的视图。
#[derive(Debug, Clone)]
pub struct Bookmark {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Bookmark {
    /// 书签名称。
    pub fn name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Name")
    }

    /// 跳转目标。
    pub fn dest(&self) -> Option<Dest> {
        self.doc.find_child(self.node, "Dest").map(|node| Dest {
            doc: Arc::clone(&self.doc),
            node,
        })
    }
}

/// 跳转目标的视图。
#[derive(Debug, Clone)]
pub struct Dest {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Dest {
    pub(crate) fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 目标类型。
    pub fn dest_type(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Type")
    }

    /// 目标页面标识。
    pub fn page_id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "PageID")
    }

    /// 左坐标。
    pub fn left(&self) -> Option<f64> {
        self.float_attr("Left")
    }

    /// 上坐标。
    pub fn top(&self) -> Option<f64> {
        self.float_attr("Top")
    }

    /// 右坐标。
    pub fn right(&self) -> Option<f64> {
        self.float_attr("Right")
    }

    /// 下坐标。
    pub fn bottom(&self) -> Option<f64> {
        self.float_attr("Bottom")
    }

    /// 缩放比例。
    pub fn zoom(&self) -> Option<f64> {
        self.float_attr("Zoom")
    }

    fn float_attr(&self, name: &str) -> Option<f64> {
        self.doc
            .attribute(self.node, name)
            .and_then(|value| value.parse().ok())
    }
}

fn child_text<'a>(doc: &'a XmlDocument, parent: NodeId, local: &str) -> Option<&'a str> {
    doc.find_child(parent, local)
        .map(|node| doc.text(node).trim())
}

fn child_texts<'a>(doc: &'a XmlDocument, parent: NodeId, local: &str) -> Vec<&'a str> {
    doc.children_named(parent, local)
        .map(|node| doc.text(node).trim())
        .collect()
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn validate_common_data(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(
        doc,
        node,
        COMMON_DATA_ORDER,
        &["MaxUnitID", "PageArea"],
        &["PublicRes", "DocumentRes", "TemplatePage"],
        diagnostics,
    );

    if let Some(max_id) = doc.find_child(node, "MaxUnitID") {
        validate_text_element(doc, max_id, true, diagnostics);
        if doc.text(max_id).trim().parse::<u64>().is_err() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidValue,
                doc.path(max_id),
                "MaxUnitID 必须是无符号整数",
            ));
        }
    }
    if let Some(area) = doc.find_child(node, "PageArea") {
        validate_page_area(doc, area, diagnostics);
    }
    for child in doc.children_named(node, "PublicRes") {
        validate_text_element(doc, child, true, diagnostics);
    }
    for child in doc.children_named(node, "DocumentRes") {
        validate_text_element(doc, child, true, diagnostics);
    }
    for child in doc.children_named(node, "TemplatePage") {
        validate_template_page(doc, child, diagnostics);
    }
    if let Some(default_cs) = doc.find_child(node, "DefaultCS") {
        validate_text_element(doc, default_cs, true, diagnostics);
        if doc.text(default_cs).trim().parse::<u64>().is_err() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidValue,
                doc.path(default_cs),
                "DefaultCS 必须是无符号整数",
            ));
        }
    }
}

pub(crate) fn validate_page_area(
    doc: &XmlDocument,
    node: NodeId,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(
        doc,
        node,
        PAGE_AREA_ORDER,
        &["PhysicalBox"],
        &[],
        diagnostics,
    );
    for child in doc.element_children(node) {
        validate_text_element(doc, child, true, diagnostics);
    }
}

fn validate_template_page(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["ID", "Name", "ZOrder", "BaseLoc"],
        &["ID", "BaseLoc"],
        diagnostics,
    );
    if let Some(z_order) = doc.attribute(node, "ZOrder")
        && !matches!(z_order, "Background" | "Foreground")
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@ZOrder", doc.path(node)),
            "ZOrder 只能是 Background 或 Foreground",
        ));
    }
    if doc.element_children(node).next().is_some() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnexpectedElement,
            doc.path(node),
            "TemplatePage 不应包含子元素",
        ));
    }
}

fn validate_pages(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(doc, node, &["Page"], &["Page"], &["Page"], diagnostics);
    for page in doc.children_named(node, "Page") {
        check_namespace(doc, page, diagnostics);
        check_attributes(
            doc,
            page,
            &["ID", "BaseLoc"],
            &["ID", "BaseLoc"],
            diagnostics,
        );
        if doc.element_children(page).next().is_some() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(page),
                "Page 不应包含子元素",
            ));
        }
    }
}

fn validate_permissions(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(doc, node, PERMISSIONS_ORDER, &[], &[], diagnostics);

    for name in [
        "Edit",
        "Annot",
        "Export",
        "Signature",
        "Watermark",
        "PrintScreen",
    ] {
        if let Some(child) = doc.find_child(node, name) {
            validate_text_element(doc, child, true, diagnostics);
            if parse_bool(doc.text(child).trim()).is_none() {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::InvalidValue,
                    doc.path(child),
                    "权限值必须是布尔值",
                ));
            }
        }
    }
    if let Some(print) = doc.find_child(node, "Print") {
        validate_print(doc, print, diagnostics);
    }
    if let Some(period) = doc.find_child(node, "ValidPeriod") {
        validate_valid_period(doc, period, diagnostics);
    }
}

fn validate_print(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["Printable", "Copies"],
        &["Printable"],
        diagnostics,
    );
    if let Some(printable) = doc.attribute(node, "Printable")
        && parse_bool(printable).is_none()
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Printable", doc.path(node)),
            "Printable 必须是布尔值",
        ));
    }
    if let Some(copies) = doc.attribute(node, "Copies")
        && copies.parse::<i32>().is_err()
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Copies", doc.path(node)),
            "Copies 必须是整数",
        ));
    }
    if doc.element_children(node).next().is_some() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnexpectedElement,
            doc.path(node),
            "Print 不应包含子元素",
        ));
    }
}

fn validate_valid_period(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["StartDate", "EndDate"], &[], diagnostics);
    if doc.element_children(node).next().is_some() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnexpectedElement,
            doc.path(node),
            "ValidPeriod 不应包含子元素",
        ));
    }
}

fn validate_v_preferences(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(doc, node, VPREFERENCES_ORDER, &[], &[], diagnostics);

    for (name, allowed) in [
        (
            "PageMode",
            &[
                "None",
                "FullScreen",
                "UseOutlines",
                "UseThumbs",
                "UseCustomTags",
                "UseLayers",
                "UseAttatchs",
                "UseBookmarks",
            ][..],
        ),
        (
            "PageLayout",
            &[
                "OnePage",
                "OneColumn",
                "TwoPageL",
                "TwoColumnL",
                "TwoPageR",
                "TwoColumnR",
            ][..],
        ),
        ("TabDisplay", &["DocTitle", "FileName"][..]),
        (
            "ZoomMode",
            &["Default", "FitHeight", "FitWidth", "FitRect"][..],
        ),
    ] {
        if let Some(child) = doc.find_child(node, name) {
            validate_text_element(doc, child, true, diagnostics);
            if !allowed.contains(&doc.text(child).trim()) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::InvalidValue,
                    doc.path(child),
                    "枚举值不在允许范围内",
                ));
            }
        }
    }
    for name in ["HideToolbar", "HideMenubar", "HideWindowUI"] {
        if let Some(child) = doc.find_child(node, name) {
            validate_text_element(doc, child, true, diagnostics);
            if parse_bool(doc.text(child).trim()).is_none() {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::InvalidValue,
                    doc.path(child),
                    "该值必须是布尔值",
                ));
            }
        }
    }
    if let Some(zoom) = doc.find_child(node, "Zoom") {
        validate_text_element(doc, zoom, true, diagnostics);
        if doc.text(zoom).trim().parse::<f64>().is_err() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidValue,
                doc.path(zoom),
                "Zoom 必须是浮点数",
            ));
        }
    }
    if doc.find_child(node, "ZoomMode").is_some() && doc.find_child(node, "Zoom").is_some() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnexpectedElement,
            doc.path(node),
            "ZoomMode 与 Zoom 只能出现一个",
        ));
    }
}

fn validate_bookmarks(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(
        doc,
        node,
        &["Bookmark"],
        &["Bookmark"],
        &["Bookmark"],
        diagnostics,
    );
    for bookmark in doc.children_named(node, "Bookmark") {
        check_namespace(doc, bookmark, diagnostics);
        check_attributes(doc, bookmark, &["Name"], &["Name"], diagnostics);
        if doc.find_child(bookmark, "Dest").is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MissingRequired,
                format!("{}/Dest", doc.path(bookmark)),
                "Bookmark 缺少 Dest",
            ));
        }
        for child in doc.element_children(bookmark) {
            if doc.name(child).local != "Dest" {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::UnexpectedElement,
                    doc.path(child),
                    "Bookmark 只允许 Dest",
                ));
            }
        }
        if let Some(dest) = doc.find_child(bookmark, "Dest") {
            validate_dest(doc, dest, diagnostics);
        }
    }
}

pub(crate) fn validate_dest(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["Type", "PageID", "Left", "Top", "Right", "Bottom", "Zoom"],
        &["Type", "PageID"],
        diagnostics,
    );
    if let Some(dest_type) = doc.attribute(node, "Type")
        && !matches!(dest_type, "XYZ" | "Fit" | "FitH" | "FitV" | "FitR")
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Type", doc.path(node)),
            "Dest Type 不在允许范围内",
        ));
    }
    for name in ["Left", "Top", "Right", "Bottom", "Zoom"] {
        if let Some(value) = doc.attribute(node, name)
            && value.parse::<f64>().is_err()
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidValue,
                format!("{}/@{name}", doc.path(node)),
                "该属性必须是浮点数",
            ));
        }
    }
    if doc.element_children(node).next().is_some() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnexpectedElement,
            doc.path(node),
            "Dest 不应包含子元素",
        ));
    }
}
