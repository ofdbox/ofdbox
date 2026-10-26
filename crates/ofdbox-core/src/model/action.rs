use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode};
use crate::model::document::{Dest, validate_dest};
use crate::model::ofd::{check_attributes, check_children_order, check_namespace};
use crate::model::region::{Region, validate_region};
use crate::xml::tree::{NodeId, XmlDocument};

/// 动作集的类型化视图。
#[derive(Debug, Clone)]
pub struct Actions {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Actions {
    pub(crate) fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 遍历动作。
    pub fn actions(&self) -> impl Iterator<Item = Action> {
        self.doc
            .children_named(self.node, "Action")
            .map(|node| Action {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 单个动作的视图。
#[derive(Debug, Clone)]
pub struct Action {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Action {
    /// 触发事件。
    pub fn event(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Event")
    }

    /// 区域限定。
    pub fn region(&self) -> Option<Region> {
        self.doc
            .find_child(self.node, "Region")
            .map(|node| Region::new(Arc::clone(&self.doc), node))
    }

    /// 是否存在区域限定。
    pub fn has_region(&self) -> bool {
        self.region().is_some()
    }

    /// 跳转动作。
    pub fn goto(&self) -> Option<Goto> {
        self.child("Goto", |doc, node| Goto { doc, node })
    }

    /// URI 动作。
    pub fn uri(&self) -> Option<Uri> {
        self.child("URI", |doc, node| Uri { doc, node })
    }

    /// 附件动作。
    pub fn goto_a(&self) -> Option<GotoA> {
        self.child("GotoA", |doc, node| GotoA { doc, node })
    }

    /// 音频动作。
    pub fn sound(&self) -> Option<Sound> {
        self.child("Sound", |doc, node| Sound { doc, node })
    }

    /// 视频动作。
    pub fn movie(&self) -> Option<Movie> {
        self.child("Movie", |doc, node| Movie { doc, node })
    }

    fn child<T>(&self, name: &str, make: impl FnOnce(Arc<XmlDocument>, NodeId) -> T) -> Option<T> {
        self.doc
            .find_child(self.node, name)
            .map(|node| make(Arc::clone(&self.doc), node))
    }
}

/// 跳转动作的视图。
#[derive(Debug, Clone)]
pub struct Goto {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Goto {
    /// 跳转目标区域。
    pub fn dest(&self) -> Option<Dest> {
        self.doc
            .find_child(self.node, "Dest")
            .map(|node| Dest::new(Arc::clone(&self.doc), node))
    }

    /// 跳转目标书签名称。
    pub fn bookmark(&self) -> Option<&str> {
        self.doc
            .find_child(self.node, "Bookmark")
            .and_then(|node| self.doc.attribute(node, "Name"))
    }
}

/// URI 动作的视图。
#[derive(Debug, Clone)]
pub struct Uri {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Uri {
    /// URI 地址。
    pub fn uri(&self) -> Option<&str> {
        self.doc.attribute(self.node, "URI")
    }

    /// 基础 URI。
    pub fn base(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Base")
    }

    /// 目标窗口。
    pub fn target(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Target")
    }
}

/// 附件动作的视图。
#[derive(Debug, Clone)]
pub struct GotoA {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl GotoA {
    /// 附件标识。
    pub fn attach_id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "AttachID")
    }

    /// 是否新窗口打开。
    pub fn new_window(&self) -> Option<bool> {
        self.doc
            .attribute(self.node, "NewWindow")
            .and_then(parse_bool)
    }
}

/// 音频动作的视图。
#[derive(Debug, Clone)]
pub struct Sound {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Sound {
    /// 音频资源标识。
    pub fn resource_id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ResourceID")
    }

    /// 音量。
    pub fn volume(&self) -> Option<i32> {
        self.doc
            .attribute(self.node, "Volume")
            .and_then(|value| value.parse().ok())
    }

    /// 是否重复播放。
    pub fn repeat(&self) -> Option<bool> {
        self.doc.attribute(self.node, "Repeat").and_then(parse_bool)
    }

    /// 是否同步播放。
    pub fn synchronous(&self) -> Option<bool> {
        self.doc
            .attribute(self.node, "Synchronous")
            .and_then(parse_bool)
    }
}

/// 视频动作的视图。
#[derive(Debug, Clone)]
pub struct Movie {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Movie {
    /// 视频资源标识。
    pub fn resource_id(&self) -> Option<&str> {
        self.doc.attribute(self.node, "ResourceID")
    }

    /// 播放操作。
    pub fn operator(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Operator")
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

pub(crate) fn validate_actions(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(
        doc,
        node,
        &["Action"],
        &["Action"],
        &["Action"],
        diagnostics,
    );
    for action in doc.children_named(node, "Action") {
        validate_action(doc, action, diagnostics);
    }
}

fn validate_action(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["Event"], &["Event"], diagnostics);
    if let Some(event) = doc.attribute(node, "Event")
        && !matches!(event, "DO" | "PO" | "CLICK")
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Event", doc.path(node)),
            "Event 只能是 DO、PO 或 CLICK",
        ));
    }

    if let Some(region) = doc.find_child(node, "Region") {
        validate_region(doc, region, diagnostics);
    }

    let mut kinds = Vec::new();
    for child in doc.element_children(node) {
        match doc.name(child).local.as_str() {
            "Region" => {}
            "Goto" => {
                kinds.push("Goto");
                validate_goto(doc, child, diagnostics);
            }
            "URI" => {
                kinds.push("URI");
                validate_uri(doc, child, diagnostics);
            }
            "GotoA" => {
                kinds.push("GotoA");
                validate_goto_a(doc, child, diagnostics);
            }
            "Sound" => {
                kinds.push("Sound");
                validate_sound(doc, child, diagnostics);
            }
            "Movie" => {
                kinds.push("Movie");
                validate_movie(doc, child, diagnostics);
            }
            _ => diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "Action 子元素不在允许范围内",
            )),
        }
    }
    if kinds.is_empty() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            doc.path(node),
            "Action 缺少动作类型",
        ));
    } else if kinds.len() > 1 {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnexpectedElement,
            doc.path(node),
            "Action 只能包含一种动作类型",
        ));
    }
}

fn validate_goto(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    let mut count = 0;
    for child in doc.element_children(node) {
        match doc.name(child).local.as_str() {
            "Dest" => {
                count += 1;
                validate_dest(doc, child, diagnostics);
            }
            "Bookmark" => {
                count += 1;
                check_namespace(doc, child, diagnostics);
                check_attributes(doc, child, &["Name"], &["Name"], diagnostics);
                if doc.element_children(child).next().is_some() {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::UnexpectedElement,
                        doc.path(child),
                        "Bookmark 不应包含子元素",
                    ));
                }
            }
            _ => diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnexpectedElement,
                doc.path(child),
                "Goto 只允许 Dest 或 Bookmark",
            )),
        }
    }
    if count != 1 {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingRequired,
            doc.path(node),
            "Goto 必须且只能包含一个 Dest 或 Bookmark",
        ));
    }
}

fn validate_uri(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &["URI", "Base", "Target"], &["URI"], diagnostics);
    validate_no_children(doc, node, diagnostics);
}

fn validate_goto_a(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["AttachID", "NewWindow"],
        &["AttachID"],
        diagnostics,
    );
    validate_bool_attr(doc, node, "NewWindow", diagnostics);
    validate_no_children(doc, node, diagnostics);
}

fn validate_sound(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["ResourceID", "Volume", "Repeat", "Synchronous"],
        &["ResourceID"],
        diagnostics,
    );
    if let Some(volume) = doc.attribute(node, "Volume")
        && volume.parse::<i32>().is_err()
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Volume", doc.path(node)),
            "Volume 必须是整数",
        ));
    }
    validate_bool_attr(doc, node, "Repeat", diagnostics);
    validate_bool_attr(doc, node, "Synchronous", diagnostics);
    validate_no_children(doc, node, diagnostics);
}

fn validate_movie(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["ResourceID", "Operator"],
        &["ResourceID"],
        diagnostics,
    );
    if let Some(operator) = doc.attribute(node, "Operator")
        && !matches!(operator, "Play" | "Stop" | "Pause" | "Resume")
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Operator", doc.path(node)),
            "Operator 不在允许范围内",
        ));
    }
    validate_no_children(doc, node, diagnostics);
}

fn validate_bool_attr(
    doc: &XmlDocument,
    node: NodeId,
    name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
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

fn validate_no_children(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    if doc.element_children(node).next().is_some() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnexpectedElement,
            doc.path(node),
            "该元素不应包含子元素",
        ));
    }
}
