//! `Res.xsd`：字体、多媒体、颜色空间、绘制参数和复合图元资源。

use super::definition::{StArray, StId, StLoc, StRefId};
use super::page::{Color, PageBlock};

/// 资源文件根对象。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Res")]
pub struct Res {
    /// 资源基准路径。
    #[yaserde(attribute = true, rename = "BaseLoc", default = "default_base_loc")]
    pub base_loc: StLoc,
    /// 颜色空间集合。
    #[yaserde(rename = "ColorSpaces")]
    pub color_spaces: Option<ColorSpaces>,
    /// 绘制参数集合。
    #[yaserde(rename = "DrawParams")]
    pub draw_params: Option<DrawParams>,
    /// 字体集合。
    #[yaserde(rename = "Fonts")]
    pub fonts: Option<Fonts>,
    /// 多媒体集合。
    #[yaserde(rename = "MultiMedias")]
    pub multi_medias: Option<MultiMedias>,
    /// 复合图元集合。
    #[yaserde(rename = "CompositeGraphicUnits")]
    pub composite_graphic_units: Option<CompositeGraphicUnits>,
}

/// 颜色空间集合。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "ColorSpaces")]
pub struct ColorSpaces {
    /// 颜色空间。
    #[yaserde(rename = "ColorSpace")]
    pub color_spaces: Vec<ColorSpace>,
}

/// 颜色空间。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "ColorSpace")]
pub struct ColorSpace {
    /// 资源 ID。
    #[yaserde(attribute = true, rename = "ID", default = "default_st_id")]
    pub id: StId,
    /// 颜色空间类型。
    #[yaserde(
        attribute = true,
        rename = "Type",
        default = "default_color_space_type"
    )]
    pub color_space_type: ColorSpaceType,
    /// 每分量位数。
    #[yaserde(
        attribute = true,
        rename = "BitsPerComponent",
        default = "default_bits"
    )]
    pub bits_per_component: i32,
    /// ICC 配置路径。
    #[yaserde(attribute = true, rename = "Profile")]
    pub profile: Option<StLoc>,
    /// 调色板。
    #[yaserde(rename = "Palette")]
    pub palette: Option<Palette>,
}

/// 颜色空间类型。
#[derive(Clone, Debug, Default, PartialEq, Eq, yaserde::DefaultYaSerde)]
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

crate::impl_xml_enum_codec!(ColorSpaceType, Other, {
    Gray => "GRAY",
    Rgb => "RGB",
    Cmyk => "CMYK",
});

/// 调色板。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Palette")]
pub struct Palette {
    /// 颜色值。
    #[yaserde(rename = "CV")]
    pub values: Vec<StArray>,
}

/// 绘制参数集合。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "DrawParams")]
pub struct DrawParams {
    /// 绘制参数。
    #[yaserde(rename = "DrawParam")]
    pub draw_params: Vec<DrawParam>,
}

/// 绘制参数。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "DrawParam")]
pub struct DrawParam {
    /// 资源 ID。
    #[yaserde(attribute = true, rename = "ID", default = "default_st_id")]
    pub id: StId,
    /// 相对参数引用。
    #[yaserde(attribute = true, rename = "Relative")]
    pub relative: Option<StRefId>,
    /// 填充色。
    #[yaserde(rename = "FillColor")]
    pub fill_color: Option<Color>,
    /// 描边色。
    #[yaserde(rename = "StrokeColor")]
    pub stroke_color: Option<Color>,
    /// 线宽。
    #[yaserde(attribute = true, rename = "LineWidth", default = "default_line_width")]
    pub line_width: f64,
    /// 连接样式。
    #[yaserde(attribute = true, rename = "Join", default = "default_join")]
    pub join: String,
    /// 线端样式。
    #[yaserde(attribute = true, rename = "Cap", default = "default_cap")]
    pub cap: String,
    /// 虚线偏移。
    #[yaserde(attribute = true, rename = "DashOffset", default = "default_zero")]
    pub dash_offset: f64,
    /// 虚线模式。
    #[yaserde(attribute = true, rename = "DashPattern")]
    pub dash_pattern: Option<StArray>,
    /// 斜接限制。
    #[yaserde(attribute = true, rename = "MiterLimit", default = "default_miter")]
    pub miter_limit: f64,
}

/// 字体集合。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Fonts")]
pub struct Fonts {
    /// 字体。
    #[yaserde(rename = "Font")]
    pub fonts: Vec<Font>,
}

/// 字体资源。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Font")]
pub struct Font {
    /// 资源 ID。
    #[yaserde(attribute = true, rename = "ID", default = "default_st_id")]
    pub id: StId,
    /// 字体名。
    #[yaserde(attribute = true, rename = "FontName", default = "default_string")]
    pub font_name: String,
    /// 字体族名。
    #[yaserde(attribute = true, rename = "FamilyName")]
    pub family_name: Option<String>,
    /// 字符集。
    #[yaserde(attribute = true, rename = "Charset", default = "default_charset")]
    pub charset: Charset,
    /// 是否斜体。
    #[yaserde(attribute = true, rename = "Italic", default = "default_false")]
    pub italic: bool,
    /// 是否粗体。
    #[yaserde(attribute = true, rename = "Bold", default = "default_false")]
    pub bold: bool,
    /// 是否衬线。
    #[yaserde(attribute = true, rename = "Serif", default = "default_false")]
    pub serif: bool,
    /// 是否等宽。
    #[yaserde(attribute = true, rename = "FixedWidth", default = "default_false")]
    pub fixed_width: bool,
    /// 字体文件路径。
    #[yaserde(rename = "FontFile")]
    pub font_file: Option<StLoc>,
}

/// 字体字符集。
#[derive(Clone, Debug, Default, PartialEq, Eq, yaserde::DefaultYaSerde)]
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

crate::impl_xml_enum_codec!(Charset, Other, {
    Symbol => "symbol",
    Prc => "prc",
    Big5 => "big5",
    ShiftJis => "shift-jis",
    Wansung => "wansung",
    Johab => "johab",
    Unicode => "unicode",
});

/// 多媒体集合。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "MultiMedias")]
pub struct MultiMedias {
    /// 多媒体资源。
    #[yaserde(rename = "MultiMedia")]
    pub multi_medias: Vec<MultiMedia>,
}

/// 多媒体资源。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "MultiMedia")]
pub struct MultiMedia {
    /// 资源 ID。
    #[yaserde(attribute = true, rename = "ID", default = "default_st_id")]
    pub id: StId,
    /// 资源类型。
    #[yaserde(attribute = true, rename = "Type", default = "default_media_type")]
    pub media_type: MultiMediaType,
    /// 格式。
    #[yaserde(attribute = true, rename = "Format")]
    pub format: Option<String>,
    /// 媒体文件路径。
    #[yaserde(rename = "MediaFile", default = "default_st_loc")]
    pub media_file: StLoc,
}

/// 多媒体类型。
#[derive(Clone, Debug, Default, PartialEq, Eq, yaserde::DefaultYaSerde)]
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

crate::impl_xml_enum_codec!(MultiMediaType, Other, {
    Image => "Image",
    Audio => "Audio",
    Video => "Video",
});

/// 复合图元集合。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "CompositeGraphicUnits")]
pub struct CompositeGraphicUnits {
    /// 复合图元。
    #[yaserde(rename = "CompositeGraphicUnit")]
    pub units: Vec<CompositeGraphicUnit>,
}

/// 复合图元。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "CompositeGraphicUnit")]
pub struct CompositeGraphicUnit {
    /// 资源 ID。
    #[yaserde(attribute = true, rename = "ID", default = "default_st_id")]
    pub id: StId,
    /// 缩略图引用。
    #[yaserde(rename = "Thumbnail")]
    pub thumbnail: Option<StRefId>,
    /// 替换资源引用。
    #[yaserde(rename = "Substitution")]
    pub substitution: Option<StRefId>,
    /// 图元内容。
    #[yaserde(rename = "Content")]
    pub content: PageBlock,
    /// 宽。
    #[yaserde(attribute = true, rename = "Width")]
    pub width: f64,
    /// 高。
    #[yaserde(attribute = true, rename = "Height")]
    pub height: f64,
}

fn default_charset() -> Charset {
    Charset::Unicode
}

fn default_false() -> bool {
    false
}

fn default_base_loc() -> StLoc {
    StLoc::new(".")
}

fn default_st_id() -> StId {
    StId::default()
}

fn default_bits() -> i32 {
    8
}

fn default_media_type() -> MultiMediaType {
    MultiMediaType::Image
}

fn default_line_width() -> f64 {
    0.353
}

fn default_join() -> String {
    "Miter".to_owned()
}

fn default_cap() -> String {
    "Butt".to_owned()
}

fn default_zero() -> f64 {
    0.0
}

fn default_miter() -> f64 {
    4.234
}

fn default_st_loc() -> StLoc {
    StLoc::default()
}

fn default_color_space_type() -> ColorSpaceType {
    ColorSpaceType::Rgb
}

fn default_string() -> String {
    String::new()
}
