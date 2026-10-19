use std::sync::Arc;

use crate::asset::{FontAsset, ImageAsset};
use crate::geometry::{Matrix, Size};
use crate::paint::Paint;
use crate::path::{FillRule, Path, StrokeStyle};

/// 页面的计算后场景。
#[derive(Clone, Debug)]
pub struct Scene {
    /// 页面尺寸，毫米。
    pub size: Size,
    /// 场景根节点。
    pub root: Group,
    /// 构建过程中跳过的特性。
    pub diagnostics: Vec<RenderDiagnostic>,
}

/// 带变换和裁剪的元素组。
#[derive(Clone, Debug)]
pub struct Group {
    /// 组变换，子元素从本组局部空间映射到父空间。
    pub transform: Matrix,
    /// 裁剪路径，位于本组局部空间。
    pub clip: Option<Clip>,
    /// 组透明度。
    pub opacity: f32,
    /// 子元素。
    pub children: Vec<Element>,
}

impl Group {
    /// 创建空组。
    pub fn new(transform: Matrix) -> Self {
        Self {
            transform,
            clip: None,
            opacity: 1.0,
            children: Vec::new(),
        }
    }
}

impl Default for Group {
    fn default() -> Self {
        Self::new(Matrix::IDENTITY)
    }
}

/// 裁剪路径。
#[derive(Clone, Debug)]
pub struct Clip {
    /// 裁剪轮廓。
    pub path: Arc<Path>,
    /// 填充规则。
    pub rule: FillRule,
}

/// 场景元素。
#[derive(Clone, Debug)]
pub enum Element {
    /// 子组。
    Group(Group),
    /// 路径。
    Path(PathElement),
    /// 文字。
    Text(TextElement),
    /// 图片。
    Image(ImageElement),
}

/// 路径绘制。
#[derive(Clone, Debug)]
pub struct PathElement {
    /// 轮廓。
    pub path: Arc<Path>,
    /// 填充画笔。
    pub fill: Option<Paint>,
    /// 描边。
    pub stroke: Option<StrokePaint>,
    /// 填充规则。
    pub fill_rule: FillRule,
}

/// 文字绘制。
#[derive(Clone, Debug)]
pub struct TextElement {
    /// 字体资源。
    pub font: Arc<FontAsset>,
    /// 字号，毫米。
    pub size: f64,
    /// 文字局部空间到父空间的变换。
    pub transform: Matrix,
    /// 字形及位置。
    pub glyphs: Vec<PositionedGlyph>,
    /// 填充画笔。
    pub fill: Option<Paint>,
    /// 描边。
    pub stroke: Option<StrokePaint>,
}

/// 已定位字形。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionedGlyph {
    /// 字体内的字形编号。
    pub id: u32,
    /// 文字局部空间横坐标。
    pub x: f32,
    /// 文字局部空间纵坐标。
    pub y: f32,
}

/// 描边画笔。
#[derive(Clone, Debug)]
pub struct StrokePaint {
    /// 画笔。
    pub paint: Paint,
    /// 描边样式。
    pub style: StrokeStyle,
}

/// 图片绘制。
#[derive(Clone, Debug)]
pub struct ImageElement {
    /// 图片资源。
    pub image: Arc<ImageAsset>,
    /// 图片像素空间到父空间的变换。
    pub transform: Matrix,
    /// 额外透明度。
    pub alpha: f32,
}

/// 场景构建诊断。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderDiagnostic {
    /// 诊断类别。
    pub code: DiagnosticCode,
    /// 相关对象标识。
    pub object_id: Option<String>,
    /// 说明。
    pub message: String,
}

/// 诊断类别。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagnosticCode {
    /// 图元类型暂未实现。
    UnsupportedObject,
    /// 字体缺失或无法解析。
    Font,
    /// 图片缺失或无法解码。
    Image,
    /// 颜色或画笔暂未实现。
    Paint,
    /// 几何数据无效。
    Geometry,
}

impl DiagnosticCode {
    /// 稳定的报告名称。
    pub fn name(self) -> &'static str {
        match self {
            Self::UnsupportedObject => "UnsupportedObject",
            Self::Font => "Font",
            Self::Image => "Image",
            Self::Paint => "Paint",
            Self::Geometry => "Geometry",
        }
    }
}

impl Scene {
    /// 粗略估计场景占用的字节数，用于资源池预算。
    pub fn estimated_bytes(&self) -> usize {
        256 + self
            .diagnostics
            .iter()
            .map(|item| item.message.len() + 32)
            .sum::<usize>()
            + group_bytes(&self.root)
    }
}

fn group_bytes(group: &Group) -> usize {
    let clip = group.clip.as_ref().map_or(0, |clip| path_bytes(&clip.path));
    clip + group
        .children
        .iter()
        .map(|element| match element {
            Element::Group(group) => 32 + group_bytes(group),
            Element::Path(path) => 64 + path_bytes(&path.path),
            Element::Text(text) => 96 + text.glyphs.len() * std::mem::size_of::<PositionedGlyph>(),
            Element::Image(_) => 64,
        })
        .sum::<usize>()
}

fn path_bytes(path: &Path) -> usize {
    path.segments.len() * std::mem::size_of::<crate::path::PathSegment>()
}
