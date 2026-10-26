//! `Res.xsd`：字体、多媒体、颜色空间、绘制参数和复合图元资源。

use super::definition::{StArray, StId, StLoc, StRefId};
use super::page::{Color, PageBlock};

/// 资源文件根对象。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Res {
    /// 资源基准路径。
    pub base_loc: StLoc,
    /// 颜色空间集合。
    pub color_spaces: Option<ColorSpaces>,
    /// 绘制参数集合。
    pub draw_params: Option<DrawParams>,
    /// 字体集合。
    pub fonts: Option<Fonts>,
    /// 多媒体集合。
    pub multi_medias: Option<MultiMedias>,
    /// 复合图元集合。
    pub composite_graphic_units: Option<CompositeGraphicUnits>,
}

/// 颜色空间集合。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorSpaces {
    /// 颜色空间。
    pub color_spaces: Vec<ColorSpace>,
}

/// 颜色空间。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorSpace {
    /// 资源 ID。
    pub id: StId,
    /// 颜色空间类型。
    pub color_space_type: ColorSpaceType,
    /// 每分量位数。
    pub bits_per_component: i32,
    /// ICC 配置路径。
    pub profile: Option<StLoc>,
    /// 调色板。
    pub palette: Option<Palette>,
}

/// 颜色空间类型。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ColorSpaceType {
    /// 灰度。
    Gray,
    /// RGB。
    #[default]
    Rgb,
    /// CMYK。
    Cmyk,
    /// 未知值。
    Other(String),
}

/// 调色板。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Palette {
    /// 颜色值。
    pub values: Vec<StArray>,
}

/// 绘制参数集合。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawParams {
    /// 绘制参数。
    pub draw_params: Vec<DrawParam>,
}

/// 绘制参数。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawParam {
    /// 资源 ID。
    pub id: StId,
    /// 相对参数引用。
    pub relative: Option<StRefId>,
    /// 填充色。
    pub fill_color: Option<Color>,
    /// 描边色。
    pub stroke_color: Option<Color>,
    /// 线宽。
    pub line_width: f64,
    /// 连接样式。
    pub join: String,
    /// 线端样式。
    pub cap: String,
    /// 虚线偏移。
    pub dash_offset: f64,
    /// 虚线模式。
    pub dash_pattern: Option<StArray>,
    /// 斜接限制。
    pub miter_limit: f64,
}

/// 字体集合。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fonts {
    /// 字体。
    pub fonts: Vec<Font>,
}

/// 字体资源。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Font {
    /// 资源 ID。
    pub id: StId,
    /// 字体名。
    pub font_name: String,
    /// 字体族名。
    pub family_name: Option<String>,
    /// 字符集。
    pub charset: Charset,
    /// 是否斜体。
    pub italic: bool,
    /// 是否粗体。
    pub bold: bool,
    /// 是否衬线。
    pub serif: bool,
    /// 是否等宽。
    pub fixed_width: bool,
    /// 字体文件路径。
    pub font_file: Option<StLoc>,
}

/// 字体字符集。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Charset {
    /// 符号。
    Symbol,
    /// 简体中文。
    Prc,
    /// 繁体中文。
    Big5,
    /// Shift-JIS。
    ShiftJis,
    /// Wansung。
    Wansung,
    /// Johab。
    Johab,
    /// Unicode。
    #[default]
    Unicode,
    /// 未知值。
    Other(String),
}

/// 多媒体集合。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MultiMedias {
    /// 多媒体资源。
    pub multi_medias: Vec<MultiMedia>,
}

/// 多媒体资源。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MultiMedia {
    /// 资源 ID。
    pub id: StId,
    /// 资源类型。
    pub media_type: MultiMediaType,
    /// 格式。
    pub format: Option<String>,
    /// 媒体文件路径。
    pub media_file: StLoc,
}

/// 多媒体类型。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MultiMediaType {
    /// 图像。
    #[default]
    Image,
    /// 音频。
    Audio,
    /// 视频。
    Video,
    /// 未知值。
    Other(String),
}

/// 复合图元集合。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompositeGraphicUnits {
    /// 复合图元。
    pub units: Vec<CompositeGraphicUnit>,
}

/// 复合图元。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompositeGraphicUnit {
    /// 资源 ID。
    pub id: StId,
    /// 缩略图引用。
    pub thumbnail: Option<StRefId>,
    /// 替换资源引用。
    pub substitution: Option<StRefId>,
    /// 图元内容。
    pub content: PageBlock,
    /// 宽。
    pub width: f64,
    /// 高。
    pub height: f64,
}
