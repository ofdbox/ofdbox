/// 语法树节点编号。
pub type NodeId = usize;

/// XML 限定名。前缀和命名空间分开保存，便于宽松读取时按本地名匹配。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QName {
    /// 原始前缀，例如 `ofd`；没有前缀时为 `None`。
    pub prefix: Option<String>,
    /// 本地名。
    pub local: String,
    /// 解析后的命名空间 URI；宽松模式下无法确认时为 `None`。
    pub namespace: Option<String>,
}

/// XML 属性。OFD 的属性不带命名空间前缀。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlAttribute {
    /// 属性名。
    pub name: QName,
    /// 属性值，已完成实体反转义。
    pub value: String,
}

/// 源文档中的字节范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// 起始字节偏移。
    pub start: usize,
    /// 结束字节偏移。
    pub end: usize,
}

/// 一个元素节点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlNode {
    /// 元素名。
    pub name: QName,
    /// 属性列表。
    pub attributes: Vec<XmlAttribute>,
    /// 子元素节点编号。
    pub children: Vec<NodeId>,
    /// 父节点编号。
    pub parent: Option<NodeId>,
    /// 直接文本内容。
    pub text: String,
    /// 元素在源文档中的范围。
    pub span: Span,
}

/// 一个 XML 条目的 arena 语法树。
#[derive(Debug, Clone)]
pub struct XmlDocument {
    nodes: Vec<XmlNode>,
    root: NodeId,
    source_len: usize,
}

impl XmlDocument {
    pub(crate) fn new(nodes: Vec<XmlNode>, root: NodeId, source_len: usize) -> Self {
        Self {
            nodes,
            root,
            source_len,
        }
    }

    /// 根元素编号。
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// 按编号访问节点。
    pub fn node(&self, id: NodeId) -> &XmlNode {
        &self.nodes[id]
    }

    /// 所有节点。
    pub fn nodes(&self) -> &[XmlNode] {
        &self.nodes
    }

    /// 原始 XML 字节长度，用于缓存计费。
    pub fn source_len(&self) -> usize {
        self.source_len
    }

    /// 子元素迭代器。
    pub fn element_children(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes[id].children.iter().copied()
    }

    /// 按本地名查找子元素。
    pub fn find_child(&self, id: NodeId, local: &str) -> Option<NodeId> {
        self.nodes[id]
            .children
            .iter()
            .copied()
            .find(|child| self.nodes[*child].name.local == local)
    }

    /// 按本地名迭代子元素。
    pub fn children_named<'a>(
        &'a self,
        id: NodeId,
        local: &'a str,
    ) -> impl Iterator<Item = NodeId> + 'a {
        self.nodes[id]
            .children
            .iter()
            .copied()
            .filter(move |child| self.nodes[*child].name.local == local)
    }

    /// 读取属性值。
    pub fn attribute(&self, id: NodeId, local: &str) -> Option<&str> {
        self.nodes[id]
            .attributes
            .iter()
            .find(|attribute| attribute.name.local == local)
            .map(|attribute| attribute.value.as_str())
    }

    /// 读取节点文本。
    pub fn text(&self, id: NodeId) -> &str {
        self.nodes[id].text.as_str()
    }

    /// 读取节点名。
    pub fn name(&self, id: NodeId) -> &QName {
        &self.nodes[id].name
    }

    /// 读取节点源范围。
    pub fn span(&self, id: NodeId) -> Span {
        self.nodes[id].span
    }

    /// 构造从根到指定节点的简单路径。
    pub fn path(&self, id: NodeId) -> String {
        let mut parts = Vec::new();
        let mut current = Some(id);
        while let Some(node) = current {
            parts.push(self.nodes[node].name.local.as_str());
            current = self.nodes[node].parent;
        }
        parts.reverse();
        format!("/{}", parts.join("/"))
    }

    /// 缓存计费使用的估算大小。
    pub fn estimated_size(&self) -> usize {
        self.source_len + self.nodes.len() * 32
    }
}
