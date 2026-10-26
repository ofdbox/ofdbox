use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::model::color::{Color, validate_color};
use crate::model::ofd::{
    check_attributes, check_children_order, check_namespace, validate_text_element,
};
use crate::model::page::{GraphicObject, validate_graphic_object};
use crate::xml::tree::{NodeId, XmlDocument};

/// 资源文件的类型化视图。
#[derive(Debug, Clone)]
pub struct Res {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Res {
    /// 从 XML 字节解析资源文件。
    pub fn from_xml(bytes: &[u8], options: crate::options::ReadOptions) -> Result<Self, OfdError> {
        let doc = Arc::new(crate::xml::parser::parse_xml(bytes, options)?);
        Self::from_document(doc, options)
    }

    /// 基于已解析的语法树创建视图。
    pub fn from_document(
        doc: Arc<XmlDocument>,
        options: crate::options::ReadOptions,
    ) -> Result<Self, OfdError> {
        let node = doc.root();
        if doc.name(node).local != "Res" {
            return Err(OfdError::InvalidStructure(
                "资源文件的根元素不是 Res".to_owned(),
            ));
        }
        let view = Self { doc, node };
        if options.strict {
            view.validate_strict()?;
        }
        Ok(view)
    }

    /// 资源文件基路径。
    pub fn base_loc(&self) -> Option<&str> {
        self.doc.attribute(self.node, "BaseLoc")
    }

    /// 颜色空间资源。
    pub fn color_spaces(&self) -> impl Iterator<Item = ColorSpace> {
        self.entries("ColorSpaces", "ColorSpace", ColorSpace::new)
    }

    /// 绘制参数资源。
    pub fn draw_params(&self) -> impl Iterator<Item = DrawParam> {
        self.entries("DrawParams", "DrawParam", DrawParam::new)
    }

    /// 字型资源。
    pub fn fonts(&self) -> impl Iterator<Item = Font> {
        self.entries("Fonts", "Font", Font::new)
    }

    /// 多媒体资源。
    pub fn multi_medias(&self) -> impl Iterator<Item = MultiMedia> {
        self.entries("MultiMedias", "MultiMedia", MultiMedia::new)
    }

    /// 矢量图像资源。
    pub fn composite_graphic_units(&self) -> impl Iterator<Item = CompositeGraphicUnit> {
        self.entries(
            "CompositeGraphicUnits",
            "CompositeGraphicUnit",
            CompositeGraphicUnit::new,
        )
    }

    /// 严格序列化为资源文件文本。
    pub fn to_xml(&self) -> Result<String, OfdError> {
        self.validate_strict()?;
        crate::write::res::write_res(self)
    }

    /// 严格序列化为 UTF-8 字节。
    pub fn to_bytes(&self) -> Result<Vec<u8>, OfdError> {
        self.to_xml().map(String::into_bytes)
    }

    pub(crate) fn validate_strict(&self) -> Result<(), OfdError> {
        let mut diagnostics = Vec::new();
        check_namespace(&self.doc, self.node, &mut diagnostics);
        check_attributes(
            &self.doc,
            self.node,
            &["BaseLoc"],
            &["BaseLoc"],
            &mut diagnostics,
        );
        for child in self.doc.element_children(self.node) {
            match self.doc.name(child).local.as_str() {
                "ColorSpaces" => validate_container(
                    &self.doc,
                    child,
                    "ColorSpace",
                    validate_color_space,
                    &mut diagnostics,
                ),
                "DrawParams" => validate_container(
                    &self.doc,
                    child,
                    "DrawParam",
                    validate_draw_param,
                    &mut diagnostics,
                ),
                "Fonts" => {
                    validate_container(&self.doc, child, "Font", validate_font, &mut diagnostics)
                }
                "MultiMedias" => validate_container(
                    &self.doc,
                    child,
                    "MultiMedia",
                    validate_multi_media,
                    &mut diagnostics,
                ),
                "CompositeGraphicUnits" => validate_container(
                    &self.doc,
                    child,
                    "CompositeGraphicUnit",
                    validate_composite_graphic_unit,
                    &mut diagnostics,
                ),
                _ => diagnostics.push(Diagnostic::new(
                    DiagnosticCode::UnexpectedElement,
                    self.doc.path(child),
                    "资源文件容器不在允许范围内",
                )),
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(OfdError::Conformance(diagnostics))
        }
    }

    fn entries<T>(
        &self,
        container: &str,
        entry: &str,
        make: impl Fn(Arc<XmlDocument>, NodeId) -> T,
    ) -> impl Iterator<Item = T> {
        let mut values = Vec::new();
        for container_node in self.doc.children_named(self.node, container) {
            for node in self.doc.children_named(container_node, entry) {
                values.push(make(Arc::clone(&self.doc), node));
            }
        }
        values.into_iter()
    }
}

/// 颜色空间资源。
#[derive(Debug, Clone)]
pub struct ColorSpace {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl ColorSpace {
    fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 资源标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 颜色空间类型。
    pub fn color_space_type(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Type")
    }

    /// 每分量位数。
    pub fn bits_per_component(&self) -> Option<i32> {
        self.doc
            .attribute(self.node, "BitsPerComponent")
            .and_then(|value| value.parse().ok())
    }

    /// 颜色配置文件路径。
    pub fn profile(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Profile")
    }

    /// 调色板颜色值。
    pub fn palette(&self) -> impl Iterator<Item = &str> + '_ {
        let mut values = Vec::new();
        if let Some(palette) = self.doc.find_child(self.node, "Palette") {
            for cv in self.doc.children_named(palette, "CV") {
                values.push(self.doc.text(cv).trim());
            }
        }
        values.into_iter()
    }
}

/// 绘制参数资源。
#[derive(Debug, Clone)]
pub struct DrawParam {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl DrawParam {
    fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 资源标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 基础绘制参数。
    pub fn relative(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Relative")
    }

    /// 线宽。
    pub fn line_width(&self) -> Option<f64> {
        self.float_attr("LineWidth")
    }

    /// 线条连接样式。
    pub fn join(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Join")
    }

    /// 线条端点样式。
    pub fn cap(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Cap")
    }

    /// 虚线偏移。
    pub fn dash_offset(&self) -> Option<f64> {
        self.float_attr("DashOffset")
    }

    /// 虚线样式。
    pub fn dash_pattern(&self) -> Option<&str> {
        self.doc.attribute(self.node, "DashPattern")
    }

    /// 结合点限值。
    pub fn miter_limit(&self) -> Option<f64> {
        self.float_attr("MiterLimit")
    }

    /// 填充颜色。
    pub fn fill_color(&self) -> Option<Color> {
        self.color_child("FillColor")
    }

    /// 描边颜色。
    pub fn stroke_color(&self) -> Option<Color> {
        self.color_child("StrokeColor")
    }

    fn color_child(&self, name: &str) -> Option<Color> {
        self.doc
            .find_child(self.node, name)
            .map(|node| Color::new(Arc::clone(&self.doc), node))
    }

    fn float_attr(&self, name: &str) -> Option<f64> {
        self.doc
            .attribute(self.node, name)
            .and_then(|value| value.parse().ok())
    }
}

/// 字型资源。
#[derive(Debug, Clone)]
pub struct Font {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Font {
    fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 资源标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 字型名称。
    pub fn font_name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "FontName")
    }

    /// 字族名称。
    pub fn family_name(&self) -> Option<&str> {
        self.doc.attribute(self.node, "FamilyName")
    }

    /// 字符集。宽松兼容真实文件中的 `CharSet` 拼写。
    pub fn charset(&self) -> Option<&str> {
        self.doc
            .attribute(self.node, "Charset")
            .or_else(|| self.doc.attribute(self.node, "CharSet"))
    }

    /// 是否斜体。
    pub fn italic(&self) -> Option<bool> {
        self.bool_attr("Italic")
    }

    /// 是否粗体。
    pub fn bold(&self) -> Option<bool> {
        self.bool_attr("Bold")
    }

    /// 是否衬线。
    pub fn serif(&self) -> Option<bool> {
        self.bool_attr("Serif")
    }

    /// 是否等宽。宽松兼容真实文件中的 `FixedWith` 拼写。
    pub fn fixed_width(&self) -> Option<bool> {
        self.doc
            .attribute(self.node, "FixedWidth")
            .or_else(|| self.doc.attribute(self.node, "FixedWith"))
            .and_then(parse_bool)
    }

    /// 字型文件路径。
    pub fn font_file(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "FontFile")
            .map(|node| self.doc.text(node).trim())
    }

    fn bool_attr(&self, name: &str) -> Option<bool> {
        self.doc.attribute(self.node, name).and_then(parse_bool)
    }
}

/// 多媒体资源。
#[derive(Debug, Clone)]
pub struct MultiMedia {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl MultiMedia {
    fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 资源标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 多媒体类型。
    pub fn media_type(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Type")
    }

    /// 多媒体格式。
    pub fn format(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Format")
    }

    /// 多媒体文件路径。
    pub fn media_file(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "MediaFile")
            .map(|node| self.doc.text(node).trim())
    }
}

/// 矢量图像资源。
#[derive(Debug, Clone)]
pub struct CompositeGraphicUnit {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl CompositeGraphicUnit {
    fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 资源标识。
    pub fn id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ID")
    }

    /// 宽度。
    pub fn width(&self) -> Option<f64> {
        self.float_attr("Width")
    }

    /// 高度。
    pub fn height(&self) -> Option<f64> {
        self.float_attr("Height")
    }

    /// 缩略图引用。
    pub fn thumbnail(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "Thumbnail")
            .map(|node| self.doc.text(node).trim())
    }

    /// 替换图像引用。
    pub fn substitution(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "Substitution")
            .map(|node| self.doc.text(node).trim())
    }

    /// 矢量图像内容。
    pub fn content(&self) -> Option<CompositePageContent> {
        self.doc
            .find_child(self.node, "Content")
            .map(|node| CompositePageContent {
                doc: Arc::clone(&self.doc),
                node,
            })
    }

    fn float_attr(&self, name: &str) -> Option<f64> {
        self.doc
            .attribute(self.node, name)
            .and_then(|value| value.parse().ok())
    }
}

/// 矢量图像内容的视图。
#[derive(Debug, Clone)]
pub struct CompositePageContent {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl CompositePageContent {
    /// 内容中的图元。
    pub fn objects(&self) -> impl Iterator<Item = GraphicObject> {
        self.doc
            .element_children(self.node)
            .map(|node| GraphicObject::new(Arc::clone(&self.doc), node))
            .collect::<Vec<_>>()
            .into_iter()
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn validate_container(
    doc: &XmlDocument,
    node: NodeId,
    entry: &str,
    validate: fn(&XmlDocument, NodeId, &mut Vec<Diagnostic>),
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(doc, node, &[entry], &[entry], &[entry], diagnostics);
    for child in doc.children_named(node, entry) {
        validate(doc, child, diagnostics);
    }
}

fn validate_color_space(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["ID", "Type", "BitsPerComponent", "Profile"],
        &["ID", "Type"],
        diagnostics,
    );
    if let Some(color_type) = doc.attribute(node, "Type")
        && !matches!(color_type, "GRAY" | "RGB" | "CMYK")
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Type", doc.path(node)),
            "颜色空间类型不在允许范围内",
        ));
    }
    if let Some(bits) = doc.attribute(node, "BitsPerComponent")
        && bits.parse::<i32>().is_err()
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@BitsPerComponent", doc.path(node)),
            "BitsPerComponent 必须是整数",
        ));
    }
    for child in doc.element_children(node) {
        if doc.name(child).local != "Palette" {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "ColorSpace 只允许 Palette",
            ));
            continue;
        }
        check_namespace(doc, child, diagnostics);
        check_attributes(doc, child, &[], &[], diagnostics);
        check_children_order(doc, child, &["CV"], &["CV"], &["CV"], diagnostics);
        for cv in doc.children_named(child, "CV") {
            validate_text_element(doc, cv, true, diagnostics);
        }
    }
}

fn validate_draw_param(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &[
            "ID",
            "Relative",
            "LineWidth",
            "Join",
            "Cap",
            "DashOffset",
            "DashPattern",
            "MiterLimit",
        ],
        &["ID"],
        diagnostics,
    );
    if let Some(join) = doc.attribute(node, "Join")
        && !matches!(join, "Miter" | "Round" | "Bevel")
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Join", doc.path(node)),
            "Join 不在允许范围内",
        ));
    }
    if let Some(cap) = doc.attribute(node, "Cap")
        && !matches!(cap, "Butt" | "Round" | "Square")
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Cap", doc.path(node)),
            "Cap 不在允许范围内",
        ));
    }
    for name in ["LineWidth", "DashOffset", "MiterLimit"] {
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
    for child in doc.element_children(node) {
        if !matches!(doc.name(child).local.as_str(), "FillColor" | "StrokeColor") {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "DrawParam 只允许 FillColor 或 StrokeColor",
            ));
            continue;
        }
        validate_color(doc, child, diagnostics);
    }
    for name in ["FillColor", "StrokeColor"] {
        if doc.children_named(node, name).count() > 1 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DuplicateElement,
                format!("{}/{name}", doc.path(node)),
                "该颜色元素重复出现",
            ));
        }
    }
}

fn validate_font(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &[
            "ID",
            "FontName",
            "FamilyName",
            "Charset",
            "Italic",
            "Bold",
            "Serif",
            "FixedWidth",
        ],
        &["ID", "FontName"],
        diagnostics,
    );
    if let Some(charset) = doc.attribute(node, "Charset")
        && !matches!(
            charset,
            "symbol" | "prc" | "big5" | "shift-jis" | "wansung" | "johab" | "unicode"
        )
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Charset", doc.path(node)),
            "Charset 不在允许范围内",
        ));
    }
    for name in ["Italic", "Bold", "Serif", "FixedWidth"] {
        if let Some(value) = doc.attribute(node, name)
            && parse_bool(value).is_none()
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidValue,
                format!("{}/@{name}", doc.path(node)),
                "该属性必须是布尔值",
            ));
        }
    }
    for child in doc.element_children(node) {
        if doc.name(child).local != "FontFile" {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "Font 只允许 FontFile",
            ));
            continue;
        }
        validate_text_element(doc, child, true, diagnostics);
    }
}

fn validate_multi_media(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["ID", "Type", "Format"],
        &["ID", "Type"],
        diagnostics,
    );
    if let Some(media_type) = doc.attribute(node, "Type")
        && !matches!(media_type, "Image" | "Audio" | "Video")
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Type", doc.path(node)),
            "多媒体类型不在允许范围内",
        ));
    }
    check_children_order(doc, node, &["MediaFile"], &["MediaFile"], &[], diagnostics);
    if let Some(media_file) = doc.find_child(node, "MediaFile") {
        validate_text_element(doc, media_file, true, diagnostics);
    }
}

fn validate_composite_graphic_unit(
    doc: &XmlDocument,
    node: NodeId,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["ID", "Width", "Height"],
        &["ID", "Width", "Height"],
        diagnostics,
    );
    for name in ["Width", "Height"] {
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
    check_children_order(
        doc,
        node,
        &["Thumbnail", "Substitution", "Content"],
        &["Content"],
        &[],
        diagnostics,
    );
    for name in ["Thumbnail", "Substitution"] {
        if let Some(child) = doc.find_child(node, name) {
            validate_text_element(doc, child, true, diagnostics);
        }
    }
    if let Some(content) = doc.find_child(node, "Content") {
        check_namespace(doc, content, diagnostics);
        check_attributes(doc, content, &[], &[], diagnostics);
        for child in doc.element_children(content) {
            validate_graphic_object(doc, child, diagnostics);
        }
    }
}
