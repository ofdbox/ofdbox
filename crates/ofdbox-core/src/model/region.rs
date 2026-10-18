use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode};
use crate::model::ofd::{check_attributes, check_children_order, check_namespace};
use crate::xml::tree::{NodeId, XmlDocument};

/// 区域命令类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionCommandKind {
    /// 移动。
    Move,
    /// 直线。
    Line,
    /// 二次贝塞尔曲线。
    QuadraticBezier,
    /// 三次贝塞尔曲线。
    CubicBezier,
    /// 圆弧。
    Arc,
    /// 闭合。
    Close,
    /// 未知命令。
    Other(String),
}

/// 动作区域的视图。
#[derive(Debug, Clone)]
pub struct RegionView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl RegionView {
    pub(crate) fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 区域中的子路径。
    pub fn areas(&self) -> impl Iterator<Item = RegionAreaView> {
        self.doc
            .children_named(self.node, "Area")
            .map(|node| RegionAreaView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 区域子路径的视图。
#[derive(Debug, Clone)]
pub struct RegionAreaView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl RegionAreaView {
    /// 起始点。
    pub fn start(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Start")
    }

    /// 子路径的绘制命令。
    pub fn commands(&self) -> impl Iterator<Item = RegionCommandView> {
        self.doc
            .element_children(self.node)
            .map(|node| RegionCommandView {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 单条区域绘制命令的视图。
#[derive(Debug, Clone)]
pub struct RegionCommandView {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl RegionCommandView {
    /// 命令类型。
    pub fn kind(&self) -> RegionCommandKind {
        match self.doc.name(self.node).local.as_str() {
            "Move" => RegionCommandKind::Move,
            "Line" => RegionCommandKind::Line,
            "QuadraticBezier" => RegionCommandKind::QuadraticBezier,
            "CubicBezier" => RegionCommandKind::CubicBezier,
            "Arc" => RegionCommandKind::Arc,
            "Close" => RegionCommandKind::Close,
            other => RegionCommandKind::Other(other.to_owned()),
        }
    }

    /// 第一个控制点。
    pub fn point1(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Point1")
    }

    /// 第二个控制点。
    pub fn point2(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Point2")
    }

    /// 第三个控制点。
    pub fn point3(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Point3")
    }

    /// 圆弧方向。
    pub fn sweep_direction(&self) -> Option<bool> {
        self.doc
            .attribute(self.node, "SweepDirection")
            .and_then(parse_bool)
    }

    /// 是否大弧。
    pub fn large_arc(&self) -> Option<bool> {
        self.doc
            .attribute(self.node, "LargeArc")
            .and_then(parse_bool)
    }

    /// 旋转角度。
    pub fn rotation_angle(&self) -> Option<f64> {
        self.doc
            .attribute(self.node, "RotationAngle")
            .and_then(|value| value.parse().ok())
    }

    /// 椭圆尺寸。
    pub fn ellipse_size(&self) -> Option<&str> {
        self.doc.attribute(self.node, "EllipseSize")
    }

    /// 圆弧终点。
    pub fn end_point(&self) -> Option<&str> {
        self.doc.attribute(self.node, "EndPoint")
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

pub(crate) fn validate_region(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(doc, node, &["Area"], &["Area"], &["Area"], diagnostics);
    for area in doc.children_named(node, "Area") {
        validate_area(doc, area, diagnostics);
    }
}

fn validate_area(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["Start"], &["Start"], diagnostics);
    let mut commands = 0usize;
    for child in doc.element_children(node) {
        commands += 1;
        check_namespace(doc, child, diagnostics);
        match doc.name(child).local.as_str() {
            "Move" | "Line" => {
                check_attributes(doc, child, &["Point1"], &["Point1"], diagnostics);
                check_children_order(doc, child, &[], &[], &[], diagnostics);
            }
            "QuadraticBezier" => {
                check_attributes(
                    doc,
                    child,
                    &["Point1", "Point2"],
                    &["Point1", "Point2"],
                    diagnostics,
                );
                check_children_order(doc, child, &[], &[], &[], diagnostics);
            }
            "CubicBezier" => {
                check_attributes(
                    doc,
                    child,
                    &["Point1", "Point2", "Point3"],
                    &["Point3"],
                    diagnostics,
                );
                check_children_order(doc, child, &[], &[], &[], diagnostics);
            }
            "Arc" => {
                check_attributes(
                    doc,
                    child,
                    &[
                        "SweepDirection",
                        "LargeArc",
                        "RotationAngle",
                        "EllipseSize",
                        "EndPoint",
                    ],
                    &[
                        "SweepDirection",
                        "LargeArc",
                        "RotationAngle",
                        "EllipseSize",
                        "EndPoint",
                    ],
                    diagnostics,
                );
                for name in ["SweepDirection", "LargeArc"] {
                    if let Some(value) = doc.attribute(child, name)
                        && parse_bool(value).is_none()
                    {
                        diagnostics.push(Diagnostic::new(
                            DiagnosticCode::InvalidValue,
                            format!("{}/@{name}", doc.path(child)),
                            "该属性必须是布尔值",
                        ));
                    }
                }
                if let Some(value) = doc.attribute(child, "RotationAngle")
                    && value.parse::<f64>().is_err()
                {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::InvalidValue,
                        format!("{}/@RotationAngle", doc.path(child)),
                        "RotationAngle 必须是浮点数",
                    ));
                }
                check_children_order(doc, child, &[], &[], &[], diagnostics);
            }
            "Close" => {
                check_attributes(doc, child, &[], &[], diagnostics);
                check_children_order(doc, child, &[], &[], &[], diagnostics);
            }
            other => diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                format!("区域命令不在允许范围内: {other}"),
            )),
        }
    }
    if commands == 0 {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            doc.path(node),
            "区域 Area 至少需要一条绘制命令",
        ));
    }
}
