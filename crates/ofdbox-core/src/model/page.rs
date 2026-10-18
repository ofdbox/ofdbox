use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::model::action::{ActionsView, validate_actions};
use crate::model::color::{ColorView, validate_color};
use crate::model::document::{PageAreaView, validate_page_area};
use crate::model::ofd::{
    check_attributes, check_children_order, check_namespace, validate_text_element,
};
use crate::options::ReadOptions;
use crate::xml::parser::parse_xml;
use crate::xml::tree::{NodeId, XmlDocument};

const PAGE_ORDER: &[&str] = &["Template", "PageRes", "Area", "Content", "Actions"];
const COMMON_GRAPHIC_ATTRS: &[&str] = &[
    "ID",
    "Boundary",
    "Name",
    "Visible",
    "CTM",
    "DrawParam",
    "LineWidth",
    "Cap",
    "Join",
    "MiterLimit",
    "DashOffset",
    "DashPattern",
    "Alpha",
];

/// 图元对象类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphicObjectKind {
    /// 文字对象。
    Text,
    /// 图形对象。
    Path,
    /// 图像对象。
    Image,
    /// 复合对象。
    Composite,
    /// 页面块。
    PageBlock,
    /// 其他未知类型。
    Other(String),
}

/// `Content.xml` 根元素的类型化视图。
#[derive(Debug, Clone)]
pub struct PageContentView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl PageContentView {
    /// 从 XML 字节解析页面内容。
    pub fn from_xml(bytes: &[u8], options: ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(doc: Arc<XmlDocument>, options: ReadOptions) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "Page" {
            return Err(OfdError::InvalidStructure(
                "Content.xml 的根元素不是 Page".to_owned(),
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

    /// 模板引用。
    pub fn templates(&self) -> impl Iterator<Item = TemplateView> {
        self.doc
            .children_named(self.node, "Template")
            .map(|node| TemplateView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 页面资源路径列表。
    pub fn page_res(&self) -> impl Iterator<Item = &str> + '_ {
        child_texts(&self.doc, self.node, "PageRes").into_iter()
    }

    /// 页面区域。
    pub fn area(&self) -> Option<PageAreaView> {
        self.doc
            .find_child(self.node, "Area")
            .map(|node| PageAreaView::new(Arc::clone(&self.doc), node))
    }

    /// 页面内容。
    pub fn content(&self) -> Option<ContentView> {
        self.doc
            .find_child(self.node, "Content")
            .map(|node| ContentView {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    /// 页面级动作。
    pub fn actions(&self) -> Option<ActionsView> {
        self.doc
            .find_child(self.node, "Actions")
            .map(|node| ActionsView::new(Arc::clone(&self.doc), node))
    }

    /// 严格序列化为 `Content.xml` 文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::page::write_page(self)
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
            PAGE_ORDER,
            &[],
            &["Template", "PageRes"],
            &mut diagnostics,
        );

        for template in self.doc.children_named(self.node, "Template") {
            validate_template(&self.doc, template, &mut diagnostics);
        }
        for page_res in self.doc.children_named(self.node, "PageRes") {
            validate_text_element(&self.doc, page_res, true, &mut diagnostics);
        }
        if let Some(area) = self.doc.find_child(self.node, "Area") {
            validate_page_area(&self.doc, area, &mut diagnostics);
        }
        if let Some(content) = self.doc.find_child(self.node, "Content") {
            validate_content(&self.doc, content, &mut diagnostics);
        }
        if let Some(actions) = self.doc.find_child(self.node, "Actions") {
            validate_actions(&self.doc, actions, &mut diagnostics);
        }

        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }
}

/// 页面模板引用的视图。
#[derive(Debug, Clone)]
pub struct TemplateView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl TemplateView {
    /// 模板标识。
    pub fn template_id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "TemplateID")
    }

    /// 模板层级。
    pub fn z_order(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ZOrder")
    }
}

/// `Content` 的类型化视图。
#[derive(Debug, Clone)]
pub struct ContentView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl ContentView {
    /// 遍历图层。
    pub fn layers(&self) -> impl Iterator<Item = LayerView> {
        self.doc
            .children_named(self.node, "Layer")
            .map(|node| LayerView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 图层的视图。
#[derive(Debug, Clone)]
pub struct LayerView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl LayerView {
    /// 图层标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 图层类型。
    pub fn layer_type(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Type")
    }

    /// 图层绘制参数。
    pub fn draw_param(&self) -> Option<&str> {
        self.doc.attribute(self.node, "DrawParam")
    }

    /// 遍历图元对象。
    pub fn objects(&self) -> impl Iterator<Item = GraphicObjectView> {
        self.doc
            .element_children(self.node)
            .map(|node| GraphicObjectView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 图元对象的视图。
#[derive(Debug, Clone)]
pub struct GraphicObjectView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl GraphicObjectView {
    pub(crate) fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 图元类型。
    pub fn kind(&self) -> GraphicObjectKind {
        match self.doc.name(self.node).local.as_str() {
            "TextObject" => GraphicObjectKind::Text,
            "PathObject" => GraphicObjectKind::Path,
            "ImageObject" => GraphicObjectKind::Image,
            "CompositeObject" => GraphicObjectKind::Composite,
            "PageBlock" => GraphicObjectKind::PageBlock,
            other => GraphicObjectKind::Other(other.to_owned()),
        }
    }

    /// 图元标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 图元边界。
    pub fn boundary(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Boundary")
    }

    /// 读取任意属性值。
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.doc.attribute(self.node, name)
    }

    /// 填充颜色。
    pub fn fill_color(&self) -> Option<ColorView> {
        self.color_child("FillColor")
    }

    /// 描边颜色。
    pub fn stroke_color(&self) -> Option<ColorView> {
        self.color_child("StrokeColor")
    }

    fn color_child(&self, name: &str) -> Option<ColorView> {
        self.doc
            .find_child(self.node, name)
            .map(|node| ColorView::new(Arc::clone(&self.doc), node))
    }

    /// 图元的载荷视图。
    pub fn payload(&self) -> GraphicPayloadView {
        GraphicPayloadView::new(Arc::clone(&self.doc), self.node)
    }

    /// 图元关联的动作。
    pub fn actions(&self) -> Option<ActionsView> {
        self.payload().actions()
    }

    /// 图元裁剪区域。
    pub fn clips(&self) -> Option<ClipsView> {
        self.payload().clips()
    }

    /// 文字编码段。
    pub fn text_codes(&self) -> impl Iterator<Item = TextCodeView> {
        self.doc
            .children_named(self.node, "TextCode")
            .map(|node| TextCodeView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 字形变换。
    pub fn cg_transforms(&self) -> impl Iterator<Item = CGTransformView> {
        self.doc
            .children_named(self.node, "CGTransform")
            .map(|node| CGTransformView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 路径的缩写数据。
    pub fn abbreviated_data(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "AbbreviatedData")
            .map(|node| self.doc.text(node).trim())
    }

    /// 图像边框。
    pub fn border(&self) -> Option<BorderView> {
        self.payload().border()
    }

    /// `PageBlock` 的子图元。
    pub fn children(&self) -> impl Iterator<Item = GraphicObjectView> {
        self.doc
            .element_children(self.node)
            .map(|node| GraphicObjectView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    pub(crate) fn document(&self) -> &Arc<XmlDocument> {
        &self.doc
    }

    pub(crate) fn node(&self) -> NodeId {
        self.node
    }
}

fn child_texts<'a>(doc: &'a XmlDocument, parent: NodeId, local: &str) -> Vec<&'a str> {
    doc.children_named(parent, local)
        .map(|node| doc.text(node).trim())
        .collect()
}

fn validate_template(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["TemplateID", "ZOrder"],
        &["TemplateID"],
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
            "Template 不应包含子元素",
        ));
    }
}

fn validate_content(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(doc, node, &["Layer"], &["Layer"], &["Layer"], diagnostics);
    for layer in doc.children_named(node, "Layer") {
        validate_layer(doc, layer, diagnostics);
    }
}

fn validate_layer(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["ID", "Type", "DrawParam"],
        &["ID"],
        diagnostics,
    );
    if let Some(layer_type) = doc.attribute(node, "Type")
        && !matches!(layer_type, "Body" | "Background" | "Foreground" | "Custom")
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Type", doc.path(node)),
            "Layer Type 不在允许范围内",
        ));
    }
    for object in doc.element_children(node) {
        validate_graphic_object(doc, object, diagnostics);
    }
}

pub(crate) fn validate_graphic_object(
    doc: &XmlDocument,
    node: NodeId,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_namespace(doc, node, diagnostics);
    match doc.name(node).local.as_str() {
        "TextObject" => {
            let allowed = [
                COMMON_GRAPHIC_ATTRS,
                &[
                    "Font",
                    "Size",
                    "Stroke",
                    "Fill",
                    "HScale",
                    "ReadDirection",
                    "CharDirection",
                    "Weight",
                    "Italic",
                ],
            ]
            .concat();
            check_attributes(
                doc,
                node,
                &allowed,
                &["ID", "Boundary", "Font", "Size"],
                diagnostics,
            );
            validate_text_payload(doc, node, diagnostics);
        }
        "PathObject" => {
            let allowed = [COMMON_GRAPHIC_ATTRS, &["Stroke", "Fill", "Rule"]].concat();
            check_attributes(doc, node, &allowed, &["ID", "Boundary"], diagnostics);
            validate_path_payload(doc, node, diagnostics);
        }
        "ImageObject" => {
            let allowed = [
                COMMON_GRAPHIC_ATTRS,
                &["ResourceID", "Substitution", "ImageMask"],
            ]
            .concat();
            check_attributes(
                doc,
                node,
                &allowed,
                &["ID", "Boundary", "ResourceID"],
                diagnostics,
            );
            validate_image_payload(doc, node, diagnostics);
        }
        "CompositeObject" => {
            let allowed = [COMMON_GRAPHIC_ATTRS, &["ResourceID"]].concat();
            check_attributes(
                doc,
                node,
                &allowed,
                &["ID", "Boundary", "ResourceID"],
                diagnostics,
            );
            validate_composite_payload(doc, node, diagnostics);
        }
        "PageBlock" => {
            check_attributes(doc, node, &["ID"], &["ID"], diagnostics);
            for child in doc.element_children(node) {
                validate_graphic_object(doc, child, diagnostics);
            }
        }
        other => diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnexpectedElement,
            doc.path(node),
            format!("未知图元类型: {other}"),
        )),
    }
}

fn validate_color_children(
    doc: &XmlDocument,
    node: NodeId,
    names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for child in doc.element_children(node) {
        if names.contains(&doc.name(child).local.as_str()) {
            validate_color(doc, child, diagnostics);
        }
    }
    for name in names {
        if doc.children_named(node, name).count() > 1 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DuplicateElement,
                format!("{}/{name}", doc.path(node)),
                "该颜色元素重复出现",
            ));
        }
    }
}

fn validate_border(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    let Some(border) = doc.find_child(node, "Border") else {
        return;
    };
    check_namespace(doc, border, diagnostics);
    check_attributes(
        doc,
        border,
        &[
            "LineWidth",
            "HorizonalCornerRadius",
            "VerticalCornerRadius",
            "DashOffset",
            "DashPattern",
        ],
        &[],
        diagnostics,
    );
    for name in [
        "LineWidth",
        "HorizonalCornerRadius",
        "VerticalCornerRadius",
        "DashOffset",
    ] {
        if let Some(value) = doc.attribute(border, name)
            && value.parse::<f64>().is_err()
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidValue,
                format!("{}/@{name}", doc.path(border)),
                "该属性必须是浮点数",
            ));
        }
    }
    check_children_order(doc, border, &["BorderColor"], &[], &[], diagnostics);
    if let Some(color) = doc.find_child(border, "BorderColor") {
        validate_color(doc, color, diagnostics);
    }
}

/// 图元载荷的视图，可用于页面图元或裁剪区域中的 Path/Text。
#[derive(Debug, Clone)]
pub struct GraphicPayloadView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl GraphicPayloadView {
    pub(crate) fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 载荷类型。
    pub fn kind(&self) -> GraphicObjectKind {
        match self.doc.name(self.node).local.as_str() {
            "TextObject" | "Text" => GraphicObjectKind::Text,
            "PathObject" | "Path" => GraphicObjectKind::Path,
            "ImageObject" => GraphicObjectKind::Image,
            "CompositeObject" => GraphicObjectKind::Composite,
            "PageBlock" => GraphicObjectKind::PageBlock,
            other => GraphicObjectKind::Other(other.to_owned()),
        }
    }

    /// 读取任意属性值。
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.doc.attribute(self.node, name)
    }

    /// 填充颜色。
    pub fn fill_color(&self) -> Option<ColorView> {
        self.color_child("FillColor")
    }

    /// 描边颜色。
    pub fn stroke_color(&self) -> Option<ColorView> {
        self.color_child("StrokeColor")
    }

    /// 关联动作。
    pub fn actions(&self) -> Option<ActionsView> {
        self.doc
            .find_child(self.node, "Actions")
            .map(|node| ActionsView::new(Arc::clone(&self.doc), node))
    }

    /// 裁剪区域。
    pub fn clips(&self) -> Option<ClipsView> {
        self.doc
            .find_child(self.node, "Clips")
            .map(|node| ClipsView {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    /// 文字编码段。
    pub fn text_codes(&self) -> impl Iterator<Item = TextCodeView> {
        self.doc
            .children_named(self.node, "TextCode")
            .map(|node| TextCodeView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 字形变换。
    pub fn cg_transforms(&self) -> impl Iterator<Item = CGTransformView> {
        self.doc
            .children_named(self.node, "CGTransform")
            .map(|node| CGTransformView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 路径的缩写数据。
    pub fn abbreviated_data(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "AbbreviatedData")
            .map(|node| self.doc.text(node).trim())
    }

    /// 图像边框。
    pub fn border(&self) -> Option<BorderView> {
        self.doc
            .find_child(self.node, "Border")
            .map(|node| BorderView {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    fn color_child(&self, name: &str) -> Option<ColorView> {
        self.doc
            .find_child(self.node, name)
            .map(|node| ColorView::new(Arc::clone(&self.doc), node))
    }
}

/// 文字编码段的视图。
#[derive(Debug, Clone)]
pub struct TextCodeView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl TextCodeView {
    /// 文字内容。
    pub fn text(&self) -> &str {
        self.doc.text(self.node).trim()
    }

    /// 横坐标。
    pub fn x(&self) -> Option<f64> {
        self.float_attr("X")
    }

    /// 纵坐标。
    pub fn y(&self) -> Option<f64> {
        self.float_attr("Y")
    }

    /// 横向位移数组。
    pub fn delta_x(&self) -> Option<&str> {
        self.doc.attribute(self.node, "DeltaX")
    }

    /// 纵向位移数组。
    pub fn delta_y(&self) -> Option<&str> {
        self.doc.attribute(self.node, "DeltaY")
    }

    fn float_attr(&self, name: &str) -> Option<f64> {
        self.doc
            .attribute(self.node, name)
            .and_then(|value| value.parse().ok())
    }
}

/// 字形变换的视图。
#[derive(Debug, Clone)]
pub struct CGTransformView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl CGTransformView {
    /// 编码起始位置。
    pub fn code_position(&self) -> Option<i32> {
        self.int_attr("CodePosition")
    }

    /// 编码长度。
    pub fn code_count(&self) -> Option<i32> {
        self.int_attr("CodeCount")
    }

    /// 字形长度。
    pub fn glyph_count(&self) -> Option<i32> {
        self.int_attr("GlyphCount")
    }

    /// 字形索引数组。
    pub fn glyphs(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "Glyphs")
            .map(|node| self.doc.text(node).trim())
    }

    fn int_attr(&self, name: &str) -> Option<i32> {
        self.doc
            .attribute(self.node, name)
            .and_then(|value| value.parse().ok())
    }
}

/// 图像边框的视图。
#[derive(Debug, Clone)]
pub struct BorderView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl BorderView {
    /// 线宽。
    pub fn line_width(&self) -> Option<f64> {
        self.float_attr("LineWidth")
    }

    /// 横向圆角半径。
    pub fn horizontal_corner_radius(&self) -> Option<f64> {
        self.float_attr("HorizonalCornerRadius")
    }

    /// 纵向圆角半径。
    pub fn vertical_corner_radius(&self) -> Option<f64> {
        self.float_attr("VerticalCornerRadius")
    }

    /// 虚线偏移。
    pub fn dash_offset(&self) -> Option<f64> {
        self.float_attr("DashOffset")
    }

    /// 虚线样式。
    pub fn dash_pattern(&self) -> Option<&str> {
        self.doc.attribute(self.node, "DashPattern")
    }

    /// 边框颜色。
    pub fn border_color(&self) -> Option<ColorView> {
        self.doc
            .find_child(self.node, "BorderColor")
            .map(|node| ColorView::new(Arc::clone(&self.doc), node))
    }

    fn float_attr(&self, name: &str) -> Option<f64> {
        self.doc
            .attribute(self.node, name)
            .and_then(|value| value.parse().ok())
    }
}

/// 裁剪区域集合的视图。
#[derive(Debug, Clone)]
pub struct ClipsView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl ClipsView {
    /// 遍历裁剪区域。
    pub fn clips(&self) -> impl Iterator<Item = ClipView> {
        self.doc
            .children_named(self.node, "Clip")
            .map(|node| ClipView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 单个裁剪区域的视图。
#[derive(Debug, Clone)]
pub struct ClipView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl ClipView {
    /// 遍历裁剪形状。
    pub fn areas(&self) -> impl Iterator<Item = ClipAreaView> {
        self.doc
            .children_named(self.node, "Area")
            .map(|node| ClipAreaView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 裁剪形状类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipAreaKind {
    /// 路径。
    Path,
    /// 文字。
    Text,
    /// 未知类型。
    Other(String),
}

/// 单个裁剪形状的视图。
#[derive(Debug, Clone)]
pub struct ClipAreaView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl ClipAreaView {
    /// 裁剪形状类型。
    pub fn kind(&self) -> ClipAreaKind {
        match self
            .doc
            .element_children(self.node)
            .next()
            .map(|node| self.doc.name(node).local.as_str())
        {
            Some("Path") => ClipAreaKind::Path,
            Some("Text") => ClipAreaKind::Text,
            Some(other) => ClipAreaKind::Other(other.to_owned()),
            None => ClipAreaKind::Other(String::new()),
        }
    }

    /// 绘制参数引用。
    pub fn draw_param(&self) -> Option<&str> {
        self.doc.attribute(self.node, "DrawParam")
    }

    /// 变换矩阵。
    pub fn ctm(&self) -> Option<&str> {
        self.doc.attribute(self.node, "CTM")
    }

    /// 裁剪形状的载荷。
    pub fn content(&self) -> Option<GraphicPayloadView> {
        self.doc
            .element_children(self.node)
            .next()
            .map(|node| GraphicPayloadView::new(Arc::clone(&self.doc), node))
    }
}

fn validate_graphic_base(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    for child in doc.element_children(node) {
        match doc.name(child).local.as_str() {
            "Actions" => validate_actions(doc, child, diagnostics),
            "Clips" => validate_clips(doc, child, diagnostics),
            _ => {}
        }
    }
}

fn validate_text_payload(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    validate_graphic_base(doc, node, diagnostics);
    validate_color_children(doc, node, &["FillColor", "StrokeColor"], diagnostics);
    let mut text_codes = 0usize;
    for child in doc.element_children(node) {
        match doc.name(child).local.as_str() {
            "Actions" | "Clips" | "FillColor" | "StrokeColor" => {}
            "CGTransform" => validate_cg_transform(doc, child, diagnostics),
            "TextCode" => {
                text_codes += 1;
                validate_text_code(doc, child, diagnostics);
            }
            other => diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                format!("文字图元不支持子元素: {other}"),
            )),
        }
    }
    if text_codes == 0 {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            doc.path(node),
            "文字图元至少需要一个 TextCode",
        ));
    }
}

fn validate_path_payload(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    validate_graphic_base(doc, node, diagnostics);
    validate_color_children(doc, node, &["StrokeColor", "FillColor"], diagnostics);
    let mut data = 0usize;
    for child in doc.element_children(node) {
        match doc.name(child).local.as_str() {
            "Actions" | "Clips" | "StrokeColor" | "FillColor" => {}
            "AbbreviatedData" => {
                data += 1;
                validate_text_element(doc, child, true, diagnostics);
            }
            other => diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                format!("路径图元不支持子元素: {other}"),
            )),
        }
    }
    if data == 0 {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            doc.path(node),
            "路径图元缺少 AbbreviatedData",
        ));
    }
}

fn validate_image_payload(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    validate_graphic_base(doc, node, diagnostics);
    for child in doc.element_children(node) {
        match doc.name(child).local.as_str() {
            "Actions" | "Clips" => {}
            "Border" => validate_border(doc, node, diagnostics),
            other => diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                format!("图像图元不支持子元素: {other}"),
            )),
        }
    }
}

fn validate_composite_payload(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    validate_graphic_base(doc, node, diagnostics);
    for child in doc.element_children(node) {
        if !matches!(doc.name(child).local.as_str(), "Actions" | "Clips") {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "复合图元不支持该子元素",
            ));
        }
    }
}

fn validate_cg_transform(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["CodePosition", "CodeCount", "GlyphCount"],
        &["CodePosition"],
        diagnostics,
    );
    for name in ["CodePosition", "CodeCount", "GlyphCount"] {
        if let Some(value) = doc.attribute(node, name)
            && value.parse::<i32>().is_err()
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidValue,
                format!("{}/@{name}", doc.path(node)),
                "该属性必须是整数",
            ));
        }
    }
    check_children_order(doc, node, &["Glyphs"], &[], &[], diagnostics);
}

fn validate_text_code(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["X", "Y", "DeltaX", "DeltaY"], &[], diagnostics);
    for name in ["X", "Y"] {
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
    validate_text_element(doc, node, false, diagnostics);
}

fn validate_clips(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(doc, node, &["Clip"], &["Clip"], &["Clip"], diagnostics);
    for clip in doc.children_named(node, "Clip") {
        check_namespace(doc, clip, diagnostics);
        check_attributes(doc, clip, &[], &[], diagnostics);
        check_children_order(doc, clip, &["Area"], &["Area"], &["Area"], diagnostics);
        for area in doc.children_named(clip, "Area") {
            validate_clip_area(doc, area, diagnostics);
        }
    }
}

fn validate_clip_area(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["DrawParam", "CTM"], &[], diagnostics);
    let children: Vec<NodeId> = doc.element_children(node).collect();
    if children.len() != 1 {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            doc.path(node),
            "裁剪 Area 必须且只能包含一个 Path 或 Text",
        ));
    }
    for child in children {
        check_namespace(doc, child, diagnostics);
        match doc.name(child).local.as_str() {
            "Path" => validate_clip_path(doc, child, diagnostics),
            "Text" => validate_clip_text(doc, child, diagnostics),
            other => diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                format!("裁剪 Area 不支持子元素: {other}"),
            )),
        }
    }
}

fn validate_clip_path(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    let allowed = [COMMON_GRAPHIC_ATTRS, &["Stroke", "Fill", "Rule"]].concat();
    check_attributes(doc, node, &allowed, &["Boundary"], diagnostics);
    validate_path_payload(doc, node, diagnostics);
}

fn validate_clip_text(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    let allowed = [
        COMMON_GRAPHIC_ATTRS,
        &[
            "Font",
            "Size",
            "Stroke",
            "Fill",
            "HScale",
            "ReadDirection",
            "CharDirection",
            "Weight",
            "Italic",
        ],
    ]
    .concat();
    check_attributes(
        doc,
        node,
        &allowed,
        &["Boundary", "Font", "Size"],
        diagnostics,
    );
    validate_text_payload(doc, node, diagnostics);
}
