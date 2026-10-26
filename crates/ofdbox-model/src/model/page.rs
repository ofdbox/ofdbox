//! `Page.xsd`：页面内容、图层、图元、颜色、裁剪和底纹。

use super::definition::{Action, Actions, PageArea, StArray, StBox, StId, StLoc, StPos, StRefId};

/// `Content.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    /// 模板引用。
    pub templates: Vec<Template>,
    /// 页面资源路径。
    pub page_res: Vec<StLoc>,
    /// 页面区域。
    pub area: Option<PageArea>,
    /// 页面内容。
    pub content: Option<PageContent>,
    /// 页面动作。
    pub actions: Option<Actions>,
}

/// 模板引用。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Template {
    /// 模板 ID。
    pub template_id: StRefId,
    /// 显示层级。
    pub z_order: TemplateZOrder,
}

/// 模板显示层级。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TemplateZOrder {
    /// 背景。
    #[default]
    Background,
    /// 前景。
    Foreground,
    /// 未知值。
    Other(String),
}

/// 页面内容。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageContent {
    /// 图层列表。
    pub layers: Vec<Layer>,
}

/// 图层。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layer {
    /// 图层 ID。
    pub id: StId,
    /// 图层类型。
    pub layer_type: LayerType,
    /// 绘制参数引用。
    pub draw_param: Option<StRefId>,
    /// 图层中的图元。
    pub objects: Vec<GraphicObject>,
}

/// 图层类型。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LayerType {
    /// 正文层。
    #[default]
    Body,
    /// 背景层。
    Background,
    /// 前景层。
    Foreground,
    /// 自定义层。
    Custom,
    /// 未知值。
    Other(String),
}

/// 图元对象。
#[derive(Clone, Debug, PartialEq)]
pub enum GraphicObject {
    /// 文字。
    Text(TextObject),
    /// 路径。
    Path(PathObject),
    /// 图像。
    Image(ImageObject),
    /// 复合对象。
    Composite(CompositeObject),
    /// 页块。
    PageBlock(PageBlockObject),
}

/// 图元公共属性。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GraphicUnit {
    /// 图元 ID。
    pub id: StId,
    /// 边界区域。
    pub boundary: StBox,
    /// 名称。
    pub name: Option<String>,
    /// 是否可见。
    pub visible: bool,
    /// 变换矩阵。
    pub ctm: Option<StArray>,
    /// 绘制参数引用。
    pub draw_param: Option<StRefId>,
    /// 线宽。
    pub line_width: f64,
    /// 线端样式。
    pub cap: Cap,
    /// 线连接样式。
    pub join: Join,
    /// 斜接限制。
    pub miter_limit: f64,
    /// 虚线偏移。
    pub dash_offset: f64,
    /// 虚线模式。
    pub dash_pattern: Option<StArray>,
    /// 透明度。
    pub alpha: i32,
    /// 图元动作。
    pub actions: Option<Actions>,
    /// 裁剪区域。
    pub clips: Option<Vec<Clip>>,
}

/// 线端样式。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Cap {
    /// 平头。
    #[default]
    Butt,
    /// 圆头。
    Round,
    /// 方头。
    Square,
    /// 未知值。
    Other(String),
}

/// 线连接样式。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Join {
    /// 斜接。
    #[default]
    Miter,
    /// 圆角。
    Round,
    /// 斜角。
    Bevel,
    /// 未知值。
    Other(String),
}

/// 文字图元。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextObject {
    /// 公共图元属性。
    pub common: GraphicUnit,
    /// 字体资源 ID。
    pub font: StRefId,
    /// 字号。
    pub size: f64,
    /// 是否描边。
    pub stroke: bool,
    /// 是否填充。
    pub fill: bool,
    /// 水平缩放。
    pub h_scale: f64,
    /// 阅读方向。
    pub read_direction: i32,
    /// 字符方向。
    pub char_direction: i32,
    /// 字重。
    pub weight: i32,
    /// 是否斜体。
    pub italic: bool,
    /// 填充色。
    pub fill_color: Option<Color>,
    /// 描边色。
    pub stroke_color: Option<Color>,
    /// 字形变换。
    pub cg_transforms: Vec<CgTransform>,
    /// 文字片段。
    pub text_codes: Vec<TextCode>,
}

/// 文字片段。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextCode {
    /// X 坐标。
    pub x: Option<f64>,
    /// Y 坐标。
    pub y: Option<f64>,
    /// X 增量。
    pub delta_x: Option<StArray>,
    /// Y 增量。
    pub delta_y: Option<StArray>,
    /// 文本。
    pub text: String,
}

/// 字形变换。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CgTransform {
    /// 字符起点。
    pub code_position: i32,
    /// 字符数量。
    pub code_count: i32,
    /// 字形数量。
    pub glyph_count: i32,
    /// 字形索引。
    pub glyphs: Option<StArray>,
}

/// 路径图元。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathObject {
    /// 公共图元属性。
    pub common: GraphicUnit,
    /// 描边色。
    pub stroke_color: Option<Color>,
    /// 填充色。
    pub fill_color: Option<Color>,
    /// 缩写路径数据。
    pub abbreviated_data: String,
    /// 是否描边。
    pub stroke: bool,
    /// 是否填充。
    pub fill: bool,
    /// 填充规则。
    pub rule: PathRule,
}

/// 路径填充规则。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PathRule {
    /// 非零环绕。
    #[default]
    NonZero,
    /// 奇偶环绕。
    EvenOdd,
    /// 未知值。
    Other(String),
}

/// 图像图元。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageObject {
    /// 公共图元属性。
    pub common: GraphicUnit,
    /// 图像资源 ID。
    pub resource_id: StRefId,
    /// 替换资源 ID。
    pub substitution: Option<StRefId>,
    /// 图像蒙版 ID。
    pub image_mask: Option<StRefId>,
    /// 边框。
    pub border: Option<Border>,
}

/// 图像边框。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Border {
    /// 边框颜色。
    pub color: Option<Color>,
    /// 线宽。
    pub line_width: f64,
    /// 水平圆角半径。
    pub horizontal_corner_radius: f64,
    /// 垂直圆角半径。
    pub vertical_corner_radius: f64,
    /// 虚线偏移。
    pub dash_offset: f64,
    /// 虚线模式。
    pub dash_pattern: Option<StArray>,
}

/// 复合图元。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompositeObject {
    /// 公共图元属性。
    pub common: GraphicUnit,
    /// 复合资源 ID。
    pub resource_id: StRefId,
}

/// 页块内容。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageBlock {
    /// 图元。
    pub objects: Vec<GraphicObject>,
}

/// 页块对象。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageBlockObject {
    /// 页块 ID。
    pub id: StId,
    /// 页块内容。
    pub block: PageBlock,
}

/// 裁剪区域。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clip {
    /// 裁剪区域。
    pub areas: Vec<ClipArea>,
}

/// 单个裁剪区域。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipArea {
    /// 绘制参数引用。
    pub draw_param: Option<StRefId>,
    /// 变换矩阵。
    pub ctm: Option<StArray>,
    /// 裁剪内容。
    pub content: ClipContent,
}

/// 裁剪内容。
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ClipContent {
    /// 路径。
    #[default]
    Empty,
    /// 路径图元。
    Path(PathObject),
    /// 文字图元。
    Text(TextObject),
}

/// 颜色。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Color {
    /// 颜色值。
    pub value: Option<StArray>,
    /// 颜色索引。
    pub index: Option<i32>,
    /// 颜色空间引用。
    pub color_space: Option<StRefId>,
    /// 透明度。
    pub alpha: i32,
    /// 颜色内容。
    pub kind: Option<ColorKind>,
}

/// 颜色内容。
#[derive(Clone, Debug, PartialEq)]
pub enum ColorKind {
    /// 底纹。
    Pattern(Pattern),
    /// 轴向渐变。
    AxialShd(AxialShd),
    /// 径向渐变。
    RadialShd(RadialShd),
    /// 高洛德渐变。
    GouraudShd(GouraudShd),
    /// 格点高洛德渐变。
    LaGouraudShd(LaGouraudShd),
}

/// 底纹。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pattern {
    /// 单元格内容。
    pub cell_content: PageBlock,
    /// 缩略图。
    pub thumbnail: Option<StRefId>,
    /// 单元格宽。
    pub width: f64,
    /// 单元格高。
    pub height: f64,
    /// X 步长。
    pub x_step: Option<f64>,
    /// Y 步长。
    pub y_step: Option<f64>,
    /// 反射方式。
    pub reflect_method: PatternReflect,
    /// 相对对象。
    pub relative_to: PatternRelativeTo,
    /// 变换矩阵。
    pub ctm: Option<StArray>,
}

/// 底纹反射方式。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PatternReflect {
    /// 正常。
    #[default]
    Normal,
    /// 行反射。
    Row,
    /// 列反射。
    Column,
    /// 行列反射。
    RowAndColumn,
    /// 未知值。
    Other(String),
}

/// 底纹相对对象。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PatternRelativeTo {
    /// 页面。
    Page,
    /// 对象。
    #[default]
    Object,
    /// 未知值。
    Other(String),
}

/// 渐变映射方式。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MapType {
    /// 直接。
    #[default]
    Direct,
    /// 重复。
    Repeat,
    /// 反射。
    Reflect,
    /// 未知值。
    Other(String),
}

/// 渐变色标。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShdSegment {
    /// 位置。
    pub position: Option<f64>,
    /// 颜色。
    pub color: Color,
}

/// 轴向渐变。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AxialShd {
    /// 色标。
    pub segments: Vec<ShdSegment>,
    /// 映射方式。
    pub map_type: MapType,
    /// 映射单位。
    pub map_unit: Option<f64>,
    /// 扩展方式。
    pub extend: i32,
    /// 起点。
    pub start_point: StPos,
    /// 终点。
    pub end_point: StPos,
}

/// 径向渐变。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RadialShd {
    /// 色标。
    pub segments: Vec<ShdSegment>,
    /// 映射方式。
    pub map_type: MapType,
    /// 映射单位。
    pub map_unit: Option<f64>,
    /// 离心率。
    pub eccentricity: f64,
    /// 角度。
    pub angle: f64,
    /// 起点。
    pub start_point: StPos,
    /// 起始半径。
    pub start_radius: f64,
    /// 终点。
    pub end_point: StPos,
    /// 结束半径。
    pub end_radius: f64,
    /// 扩展方式。
    pub extend: i32,
}

/// 高洛德渐变。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GouraudShd {
    /// 点。
    pub points: Vec<GouraudPoint>,
    /// 背景色。
    pub back_color: Option<Box<Color>>,
    /// 扩展方式。
    pub extend: Option<i32>,
}

/// 高洛德渐变点。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GouraudPoint {
    /// X 坐标。
    pub x: f64,
    /// Y 坐标。
    pub y: f64,
    /// 边缘标志。
    pub edge_flag: Option<i32>,
    /// 颜色。
    pub color: Color,
}

/// 格点高洛德渐变。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaGouraudShd {
    /// 点。
    pub points: Vec<LaGouraudPoint>,
    /// 背景色。
    pub back_color: Option<Box<Color>>,
    /// 每行顶点数。
    pub vertices_per_row: i32,
    /// 扩展方式。
    pub extend: Option<i32>,
}

/// 格点高洛德渐变点。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaGouraudPoint {
    /// X 坐标。
    pub x: Option<f64>,
    /// Y 坐标。
    pub y: Option<f64>,
    /// 颜色。
    pub color: Color,
}

/// 动作集合的便捷类型。
pub type PageActions = Actions;

/// 单个动作的便捷类型。
pub type PageAction = Action;
