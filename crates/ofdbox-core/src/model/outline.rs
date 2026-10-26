use std::sync::Arc;

use crate::error::{Diagnostic, DiagnosticCode};
use crate::model::action::{Actions, validate_actions};
use crate::model::ofd::{check_attributes, check_children_order, check_namespace};
use crate::xml::tree::{NodeId, XmlDocument};

/// 大纲的类型化视图。
#[derive(Debug, Clone)]
pub struct Outlines {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl Outlines {
    pub(crate) fn new(doc: Arc<XmlDocument>, node: NodeId) -> Self {
        Self { doc, node }
    }

    /// 遍历顶层大纲节点。
    pub fn outline_elems(&self) -> impl Iterator<Item = OutlineElem> {
        self.doc
            .children_named(self.node, "OutlineElem")
            .map(|node| OutlineElem {
                doc: Arc::clone(&self.doc),
                node,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// 单个大纲节点的视图。
#[derive(Debug, Clone)]
pub struct OutlineElem {
    doc: Arc<XmlDocument>,
    node: NodeId,
}

impl OutlineElem {
    /// 大纲标题。
    pub fn title(&self) -> Option<&str> {
        self.doc.attribute(self.node, "Title")
    }

    /// 子节点数量。
    pub fn count(&self) -> Option<i32> {
        self.doc
            .attribute(self.node, "Count")
            .and_then(|value| value.parse().ok())
    }

    /// 是否展开。
    pub fn expanded(&self) -> Option<bool> {
        self.doc
            .attribute(self.node, "Expanded")
            .and_then(parse_bool)
    }

    /// 该节点关联的动作。
    pub fn actions(&self) -> Option<Actions> {
        self.doc
            .find_child(self.node, "Actions")
            .map(|node| Actions::new(Arc::clone(&self.doc), node))
    }

    /// 子大纲节点。
    pub fn children(&self) -> impl Iterator<Item = OutlineElem> {
        self.doc
            .children_named(self.node, "OutlineElem")
            .map(|node| OutlineElem {
                doc: Arc::clone(&self.doc),
                node,
            })
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

pub(crate) fn validate_outlines(
    doc: &XmlDocument,
    node: NodeId,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_namespace(doc, node, diagnostics);
    check_attributes(doc, node, &[], &[], diagnostics);
    check_children_order(
        doc,
        node,
        &["OutlineElem"],
        &["OutlineElem"],
        &["OutlineElem"],
        diagnostics,
    );
    for child in doc.children_named(node, "OutlineElem") {
        validate_outline_elem(doc, child, diagnostics);
    }
}

fn validate_outline_elem(doc: &XmlDocument, node: NodeId, diagnostics: &mut Vec<Diagnostic>) {
    check_namespace(doc, node, diagnostics);
    check_attributes(
        doc,
        node,
        &["Title", "Count", "Expanded"],
        &["Title"],
        diagnostics,
    );
    if let Some(count) = doc.attribute(node, "Count")
        && count.parse::<i32>().is_err()
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Count", doc.path(node)),
            "Count 必须是整数",
        ));
    }
    if let Some(expanded) = doc.attribute(node, "Expanded")
        && parse_bool(expanded).is_none()
    {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidValue,
            format!("{}/@Expanded", doc.path(node)),
            "Expanded 必须是布尔值",
        ));
    }

    check_children_order(
        doc,
        node,
        &["Actions", "OutlineElem"],
        &[],
        &["OutlineElem"],
        diagnostics,
    );
    if let Some(actions) = doc.find_child(node, "Actions") {
        validate_actions(doc, actions, diagnostics);
    }
    for child in doc.children_named(node, "OutlineElem") {
        validate_outline_elem(doc, child, diagnostics);
    }
}
