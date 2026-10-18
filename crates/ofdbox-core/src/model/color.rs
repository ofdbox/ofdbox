use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode};
use crate::model::ofd::{check_attributes, check_children_order, check_namespace};
use crate::model::page::{GraphicObjectView, validate_graphic_object};
use crate::xml::tree::{NodeId, XmlDocument};

/// 颜色或填充的形态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorKind {
    /// 纯色值。
    Solid,
    /// 底纹。
    Pattern,
    /// 轴向渐变。
    AxialShd,
    /// 径向渐变。
    RadialShd,
    /// 高洛德渐变。
    GouraudShd,
    /// 格状高洛德渐变。
    LaGouraudShd,
    /// 未知形态。
    Other(String),
}

/// `CT_Color` 的类型化视图。
#[derive(Debug, Clone)]
pub struct ColorView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl ColorView {
    pub(crate) fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 颜色值或数组。
    pub fn value(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Value")
    }

    /// 调色板索引。
    pub fn index(&self) -> Option<i32> {
        self.int_attr("Index")
    }

    /// 颜色空间引用。
    pub fn color_space(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ColorSpace")
    }

    /// 透明度。
    pub fn alpha(&self) -> Option<i32> {
        self.int_attr("Alpha")
    }

    /// 颜色形态。
    pub fn kind(&self) -> ColorKind {
        let Some(child) = self.shape_child() else {
            return ColorKind::Solid;
        };
        match self.doc.name(child).local.as_str() {
            "Pattern" => ColorKind::Pattern,
            "AxialShd" => ColorKind::AxialShd,
            "RadialShd" => ColorKind::RadialShd,
            "GouraudShd" => ColorKind::GouraudShd,
            "LaGourandShd" => ColorKind::LaGouraudShd,
            other => ColorKind::Other(other.to_owned()),
        }
    }

    /// 底纹。
    pub fn pattern(&self) -> Option<PatternView> {
        self.shape_view("Pattern", PatternView::new)
    }

    /// 轴向渐变。
    pub fn axial_shd(&self) -> Option<AxialShdView> {
        self.shape_view("AxialShd", AxialShdView::new)
    }

    /// 径向渐变。
    pub fn radial_shd(&self) -> Option<RadialShdView> {
        self.shape_view("RadialShd", RadialShdView::new)
    }

    /// 高洛德渐变。
    pub fn gouraud_shd(&self) -> Option<GouraudShdView> {
        self.shape_view("GouraudShd", GouraudShdView::new)
    }

    /// 格状高洛德渐变。
    pub fn la_gouraud_shd(&self) -> Option<LaGouraudShdView> {
        self.shape_view("LaGourandShd", LaGouraudShdView::new)
    }

    fn shape_child(&self) -> Option<NodeId> {
        self.doc
            .element_children(self.node)
            .find(|child| shape_name(self.doc.name(*child).local.as_str()))
    }

    fn shape_view<T>(
        &self,
        name: &str,
        make: impl FnOnce(Arc<XmlDocument>, NodeId) -> T,
    ) -> Option<T> {
        self.doc
            .find_child(self.node, name)
            .map(|node| make(Arc::clone(&self.doc), node))
    }

    fn int_attr(&self, name: &str) -> Option<i32> {
        self.doc
            .attribute(self.node, name)
            .and_then(|value| value.parse().ok())
    }
}

/// 底纹的视图。
#[derive(Debug, Clone)]
pub struct PatternView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl PatternView {
    fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 单元宽度。
    pub fn width(&self) -> Option<f64> {
        self.float_attr("Width")
    }

    /// 单元高度。
    pub fn height(&self) -> Option<f64> {
        self.float_attr("Height")
    }

    /// 横向步长。
    pub fn x_step(&self) -> Option<f64> {
        self.float_attr("XStep")
    }

    /// 纵向步长。
    pub fn y_step(&self) -> Option<f64> {
        self.float_attr("YStep")
    }

    /// 反射方式。
    pub fn reflect_method(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ReflectMethod")
    }

    /// 坐标基准。
    pub fn relative_to(&self) -> Option<&str> {
        self.doc.attribute(self.node, "RelativeTo")
    }

    /// 变换矩阵。
    pub fn ctm(&self) -> Option<&str> {
        self.doc.attribute(self.node, "CTM")
    }

    /// 缩略图引用。
    pub fn thumbnail(&self) -> Option<&str> {
        self.cell_content()
            .and_then(|cell| self.doc.attribute(cell, "Thumbnail"))
    }

    /// 单元内容中的图元。
    pub fn objects(&self) -> impl Iterator<Item = GraphicObjectView> {
        let mut values = Vec::new();
        if let Some(cell) = self.cell_content() {
            for node in self.doc.element_children(cell) {
                values.push(GraphicObjectView::new(Arc::clone(&self.doc), node));
            }
        }
        values.into_iter()
    }

    fn cell_content(&self) -> Option<NodeId> {
        self.doc.find_child(self.node, "CellContent")
    }

    fn float_attr(&self, name: &str) -> Option<f64> {
        self.doc
            .attribute(self.node, name)
            .and_then(|value| value.parse().ok())
    }
}

/// 渐变段的视图。
#[derive(Debug, Clone)]
pub struct SegmentView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl SegmentView {
    fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 段位置。
    pub fn position(&self) -> Option<f64> {
        self.doc
            .attribute(self.node, "Position")
            .and_then(|value| value.parse().ok())
    }

    /// 段颜色。
    pub fn color(&self) -> Option<ColorView> {
        self.doc
            .find_child(self.node, "Color")
            .map(|node| ColorView::new(Arc::clone(&self.doc), node))
    }
}

/// 轴向渐变的视图。
#[derive(Debug, Clone)]
pub struct AxialShdView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl AxialShdView {
    fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 映射方式。
    pub fn map_type(&self) -> Option<&str> {
        self.doc.attribute(self.node, "MapType")
    }

    /// 映射单位。
    pub fn map_unit(&self) -> Option<f64> {
        self.float_attr("MapUnit")
    }

    /// 延伸方式。
    pub fn extend(&self) -> Option<i32> {
        self.int_attr("Extend")
    }

    /// 起始点。
    pub fn start_point(&self) -> Option<&str> {
        self.doc.attribute(self.node, "StartPoint")
    }

    /// 结束点。
    pub fn end_point(&self) -> Option<&str> {
        self.doc.attribute(self.node, "EndPoint")
    }

    /// 渐变段。
    pub fn segments(&self) -> impl Iterator<Item = SegmentView> {
        segments(&self.doc, self.node)
    }

    fn float_attr(&self, name: &str) -> Option<f64> {
        self.doc
            .attribute(self.node, name)
            .and_then(|value| value.parse().ok())
    }

    fn int_attr(&self, name: &str) -> Option<i32> {
        self.doc
            .attribute(self.node, name)
            .and_then(|value| value.parse().ok())
    }
}

/// 径向渐变的视图。
#[derive(Debug, Clone)]
pub struct RadialShdView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl RadialShdView {
    fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 映射方式。
    pub fn map_type(&self) -> Option<&str> {
        self.doc.attribute(self.node, "MapType")
    }

    /// 映射单位。
    pub fn map_unit(&self) -> Option<f64> {
        self.float_attr("MapUnit")
    }

    /// 离心率。
    pub fn eccentricity(&self) -> Option<f64> {
        self.float_attr("Eccentricity")
    }

    /// 旋转角度。
    pub fn angle(&self) -> Option<f64> {
        self.float_attr("Angle")
    }

    /// 起始点。
    pub fn start_point(&self) -> Option<&str> {
        self.doc.attribute(self.node, "StartPoint")
    }

    /// 起始半径。
    pub fn start_radius(&self) -> Option<f64> {
        self.float_attr("StartRadius")
    }

    /// 结束点。
    pub fn end_point(&self) -> Option<&str> {
        self.doc.attribute(self.node, "EndPoint")
    }

    /// 结束半径。
    pub fn end_radius(&self) -> Option<f64> {
        self.float_attr("EndRadius")
    }

    /// 延伸方式。
    pub fn extend(&self) -> Option<i32> {
        self.doc
            .attribute(self.node, "Extend")
            .and_then(|value| value.parse().ok())
    }

    /// 渐变段。
    pub fn segments(&self) -> impl Iterator<Item = SegmentView> {
        segments(&self.doc, self.node)
    }

    fn float_attr(&self, name: &str) -> Option<f64> {
        self.doc
            .attribute(self.node, name)
            .and_then(|value| value.parse().ok())
    }
}

/// 高洛德渐变点的视图。
#[derive(Debug, Clone)]
pub struct PointView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl PointView {
    fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 横坐标。
    pub fn x(&self) -> Option<f64> {
        self.float_attr("X")
    }

    /// 纵坐标。
    pub fn y(&self) -> Option<f64> {
        self.float_attr("Y")
    }

    /// 边缘标志。
    pub fn edge_flag(&self) -> Option<i32> {
        self.doc
            .attribute(self.node, "EdgeFlag")
            .and_then(|value| value.parse().ok())
    }

    /// 点颜色。
    pub fn color(&self) -> Option<ColorView> {
        self.doc
            .find_child(self.node, "Color")
            .map(|node| ColorView::new(Arc::clone(&self.doc), node))
    }

    fn float_attr(&self, name: &str) -> Option<f64> {
        self.doc
            .attribute(self.node, name)
            .and_then(|value| value.parse().ok())
    }
}

/// 高洛德渐变的视图。
#[derive(Debug, Clone)]
pub struct GouraudShdView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl GouraudShdView {
    fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 延伸方式。
    pub fn extend(&self) -> Option<i32> {
        self.doc
            .attribute(self.node, "Extend")
            .and_then(|value| value.parse().ok())
    }

    /// 渐变点。
    pub fn points(&self) -> impl Iterator<Item = PointView> {
        points(&self.doc, self.node)
    }

    /// 背景色。
    pub fn back_color(&self) -> Option<ColorView> {
        self.doc
            .find_child(self.node, "BackColor")
            .map(|node| ColorView::new(Arc::clone(&self.doc), node))
    }
}

/// 格状高洛德渐变的视图。
#[derive(Debug, Clone)]
pub struct LaGouraudShdView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl LaGouraudShdView {
    fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 每行顶点数。
    pub fn vertices_per_row(&self) -> Option<i32> {
        self.doc
            .attribute(self.node, "VerticesPerRow")
            .and_then(|value| value.parse().ok())
    }

    /// 延伸方式。
    pub fn extend(&self) -> Option<i32> {
        self.doc
            .attribute(self.node, "Extend")
            .and_then(|value| value.parse().ok())
    }

    /// 渐变点。
    pub fn points(&self) -> impl Iterator<Item = PointView> {
        points(&self.doc, self.node)
    }

    /// 背景色。
    pub fn back_color(&self) -> Option<ColorView> {
        self.doc
            .find_child(self.node, "BackColor")
            .map(|node| ColorView::new(Arc::clone(&self.doc), node))
    }
}

fn shape_name(name: &str) -> bool {
    matches!(
        name,
        "Pattern" | "AxialShd" | "RadialShd" | "GouraudShd" | "LaGourandShd"
    )
}

fn segments(doc: &Arc<XmlDocument>, node: NodeId) -> impl Iterator<Item = SegmentView> {
    doc.children_named(node, "Segment")
        .map(|node| SegmentView::new(Arc::clone(doc), node))
        .collect::<Vec<_>>()
        .into_iter()
}

fn points(doc: &Arc<XmlDocument>, node: NodeId) -> impl Iterator<Item = PointView> {
    doc.children_named(node, "Point")
        .map(|node| PointView::new(Arc::clone(doc), node))
        .collect::<Vec<_>>()
        .into_iter()
}

/// 严格校验一个 `CT_Color` 元素。
pub(crate) fn validate_color(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["Value", "Index", "ColorSpace", "Alpha"],
        &[],
        diagnostics,
    );
    for name in ["Index", "Alpha"] {
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

    let mut shape = None;
    for child in doc.element_children(node) {
        let name = doc.name(child).local.as_str();
        if !shape_name(name) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "颜色只允许 Pattern 或渐变子元素",
            ));
            continue;
        }
        if shape.is_some() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DuplicateElement,
                doc.path(child),
                "颜色只能包含一种形态",
            ));
            continue;
        }
        shape = Some(name);
        match name {
            "Pattern" => validate_pattern(doc, child, diagnostics),
            "AxialShd" => validate_axial_shd(doc, child, diagnostics),
            "RadialShd" => validate_radial_shd(doc, child, diagnostics),
            "GouraudShd" => validate_gouraud_shd(doc, child, diagnostics),
            "LaGourandShd" => validate_la_gouraud_shd(doc, child, diagnostics),
            _ => {}
        }
    }
}

fn validate_pattern(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &[
            "Width",
            "Height",
            "XStep",
            "YStep",
            "ReflectMethod",
            "RelativeTo",
            "CTM",
        ],
        &["Width", "Height"],
        diagnostics,
    );
    for name in ["Width", "Height", "XStep", "YStep"] {
        validate_float(doc, node, name, diagnostics);
    }
    if let Some(method) = doc.attribute(node, "ReflectMethod")
        && !matches!(method, "Normal" | "Row" | "Column" | "RowAndColumn")
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@ReflectMethod", doc.path(node)),
            "ReflectMethod 不在允许范围内",
        ));
    }
    if let Some(relative_to) = doc.attribute(node, "RelativeTo")
        && !matches!(relative_to, "Page" | "Object")
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@RelativeTo", doc.path(node)),
            "RelativeTo 只能是 Page 或 Object",
        ));
    }
    check_children_order(
        doc,
        node,
        &["CellContent"],
        &["CellContent"],
        &[],
        diagnostics,
    );
    if let Some(cell) = doc.find_child(node, "CellContent") {
        check_namespace(doc, cell, diagnostics);
        check_attributes(doc, cell, &["Thumbnail"], &[], diagnostics);
        for child in doc.element_children(cell) {
            validate_graphic_object(doc, child, diagnostics);
        }
    }
}

fn validate_axial_shd(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["MapType", "MapUnit", "Extend", "StartPoint", "EndPoint"],
        &["StartPoint", "EndPoint"],
        diagnostics,
    );
    validate_gradient_common(doc, node, diagnostics);
    check_children_order(
        doc,
        node,
        &["Segment"],
        &["Segment"],
        &["Segment"],
        diagnostics,
    );
    validate_segments(doc, node, diagnostics);
}

fn validate_radial_shd(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &[
            "MapType",
            "MapUnit",
            "Eccentricity",
            "Angle",
            "StartPoint",
            "StartRadius",
            "EndPoint",
            "EndRadius",
            "Extend",
        ],
        &["StartPoint", "EndPoint", "EndRadius"],
        diagnostics,
    );
    validate_gradient_common(doc, node, diagnostics);
    for name in ["Eccentricity", "Angle", "StartRadius", "EndRadius"] {
        validate_float(doc, node, name, diagnostics);
    }
    check_children_order(
        doc,
        node,
        &["Segment"],
        &["Segment"],
        &["Segment"],
        diagnostics,
    );
    validate_segments(doc, node, diagnostics);
}

fn validate_gradient_common(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(map_type) = doc.attribute(node, "MapType")
        && !matches!(map_type, "Direct" | "Repeat" | "Reflect")
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@MapType", doc.path(node)),
            "MapType 不在允许范围内",
        ));
    }
    validate_float(doc, node, "MapUnit", diagnostics);
    if let Some(extend) = doc.attribute(node, "Extend") {
        match extend.parse::<i32>() {
            Ok(0..=3) => {}
            _ => diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidValue,
                format!("{}/@Extend", doc.path(node)),
                "Extend 必须是 0 到 3 的整数",
            )),
        }
    }
}

fn validate_segments(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    let segments: Vec<NodeId> = doc.children_named(node, "Segment").collect();
    if segments.len() < 2 {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            format!("{}/Segment", doc.path(node)),
            "渐变至少需要两个 Segment",
        ));
    }
    for segment in segments {
        check_namespace(doc, segment, diagnostics);
        check_attributes(doc, segment, &["Position"], &[], diagnostics);
        validate_float(doc, segment, "Position", diagnostics);
        check_children_order(doc, segment, &["Color"], &["Color"], &[], diagnostics);
        if let Some(color) = doc.find_child(segment, "Color") {
            validate_color(doc, color, diagnostics);
        }
    }
}

fn validate_gouraud_shd(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["Extend"], &[], diagnostics);
    if let Some(extend) = doc.attribute(node, "Extend")
        && extend.parse::<i32>().is_err()
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Extend", doc.path(node)),
            "Extend 必须是整数",
        ));
    }
    check_children_order(
        doc,
        node,
        &["Point", "BackColor"],
        &["Point"],
        &["Point"],
        diagnostics,
    );
    validate_points(doc, node, 3, diagnostics);
    if let Some(back) = doc.find_child(node, "BackColor") {
        validate_color(doc, back, diagnostics);
    }
}

fn validate_la_gouraud_shd(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["VerticesPerRow", "Extend"],
        &["VerticesPerRow"],
        diagnostics,
    );
    for name in ["VerticesPerRow", "Extend"] {
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
    check_children_order(
        doc,
        node,
        &["Point", "BackColor"],
        &["Point"],
        &["Point"],
        diagnostics,
    );
    validate_points(doc, node, 4, diagnostics);
    if let Some(back) = doc.find_child(node, "BackColor") {
        validate_color(doc, back, diagnostics);
    }
}

fn validate_points(
    doc: &XmlDocument,
    node: NodeId,
    minimum: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let points: Vec<NodeId> = doc.children_named(node, "Point").collect();
    if points.len() < minimum {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            format!("{}/Point", doc.path(node)),
            format!("渐变至少需要 {minimum} 个 Point"),
        ));
    }
    for point in points {
        check_namespace(doc, point, diagnostics);
        check_attributes(doc, point, &["X", "Y", "EdgeFlag"], &[], diagnostics);
        for name in ["X", "Y"] {
            validate_float(doc, point, name, diagnostics);
        }
        if let Some(edge_flag) = doc.attribute(point, "EdgeFlag") {
            match edge_flag.parse::<i32>() {
                Ok(0..=2) => {}
                _ => diagnostics.push(Diagnostic::new(
                    DiagnosticCode::InvalidValue,
                    format!("{}/@EdgeFlag", doc.path(point)),
                    "EdgeFlag 必须是 0 到 2 的整数",
                )),
            }
        }
        check_children_order(doc, point, &["Color"], &["Color"], &[], diagnostics);
        if let Some(color) = doc.find_child(point, "Color") {
            validate_color(doc, color, diagnostics);
        }
    }
}

fn validate_float(doc: &XmlDocument, node: NodeId, name: &str, diagnostics: &mut Vec<Diagnostic>) {
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
