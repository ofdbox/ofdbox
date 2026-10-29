//! `Page.xsd`：页面内容、图层、图元、颜色、裁剪和底纹。

use std::io::{Read, Write};

use ofdbox_yaserde::de::Deserializer;
use ofdbox_yaserde::ser::Serializer;
use ofdbox_yaserde::{YaDeserialize, YaSerialize};

use super::definition::{
    Action, Actions, ColorArray, DeltaArray, PageArea, StArray, StBox, StId, StLoc, StPos, StRefId,
};

/// `Content.xml` 根对象。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(
  namespace = "http://www.ofdspec.org/2016",
  xmlns = {
    "ofd" = "http://www.ofdspec.org/2016"
  },
  namespace_aliases = {
    "http://www.ofdspec.org" = "http://www.ofdspec.org/2016"
  }
)]
#[yaserde(rename = "Page")]
pub struct Page {
    /// 模板引用。
    #[yaserde(rename = "Template")]
    pub templates: Vec<Template>,
    /// 页面资源路径。
    #[yaserde(rename = "PageRes")]
    pub page_res: Vec<StLoc>,
    /// 页面区域。
    #[yaserde(rename = "Area")]
    pub area: Option<PageArea>,
    /// 页面内容。
    #[yaserde(rename = "Content")]
    pub content: Option<PageContent>,
    /// 页面动作。
    #[yaserde(rename = "Actions")]
    pub actions: Option<Actions>,
}

/// 模板引用。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Template")]
pub struct Template {
    /// 模板 ID。
    #[yaserde(attribute = true, rename = "TemplateID")]
    pub template_id: StRefId,
    /// 显示层级。
    #[yaserde(
        attribute = true,
        rename = "ZOrder",
        default = "default_template_z_order"
    )]
    pub z_order: TemplateZOrder,
}

/// 模板显示层级。
#[derive(Clone, Debug, Default, PartialEq, Eq, ofdbox_yaserde::DefaultYaSerde)]
pub enum TemplateZOrder {
    /// 背景。
    #[default]
    Background,
    /// 前景。
    Foreground,
    /// 未知值。
    Other(String),
}

crate::impl_xml_enum_codec!(TemplateZOrder, Other, {
    Background => "Background",
    Foreground => "Foreground",
});

/// 页面内容。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Content")]
pub struct PageContent {
    /// 图层列表。
    #[yaserde(rename = "Layer")]
    pub layers: Vec<Layer>,
}

impl Default for GraphicUnit {
    fn default() -> Self {
        Self {
            id: StId::default(),
            boundary: StBox::default(),
            name: String::new(),
            visible: true,
            ctm: None,
            draw_param: None,
            line_width: 0.353,
            cap: Cap::Butt,
            join: Join::Miter,
            miter_limit: 4.234,
            dash_offset: 0.0,
            dash_pattern: None,
            alpha: 255,
            actions: None,
            clips: None,
        }
    }
}

impl Default for TextObject {
    fn default() -> Self {
        Self {
            common: GraphicUnit::default(),
            font: StRefId::default(),
            size: 0.0,
            stroke: false,
            fill: true,
            h_scale: 1.0,
            read_direction: 0,
            char_direction: 0,
            weight: 400,
            italic: false,
            fill_color: None,
            stroke_color: None,
            cg_transforms: Vec::new(),
            text_codes: Vec::new(),
        }
    }
}

impl Default for PathObject {
    fn default() -> Self {
        Self {
            common: GraphicUnit::default(),
            stroke_color: None,
            fill_color: None,
            abbreviated_data: String::new(),
            stroke: true,
            fill: false,
            rule: PathRule::NonZero,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self {
            value: None,
            index: None,
            color_space: None,
            alpha: 255,
            kind: ColorKind::None,
        }
    }
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

impl YaSerialize for Layer {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let id = self.id.to_string();
        let layer_type = self.layer_type.to_string();
        let draw_param = self.draw_param.map(|value| value.to_string());
        let element_name = crate::model_xml::ofd_name(writer, "Layer");
        let mut event =
            ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str())
                .attr("ID", id.as_str());
        if self.layer_type != LayerType::Body {
            event = event.attr("Type", layer_type.as_str());
        }
        if let Some(draw_param) = &draw_param {
            event = event.attr("DrawParam", draw_param.as_str());
        }
        writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
        for object in &self.objects {
            writer.set_start_event_name(None);
            object.serialize(writer)?;
        }
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaDeserialize for Layer {
    fn deserialize<R: Read>(reader: &mut Deserializer<R>) -> ofdbox_yaserde::Result<Self> {
        let event = reader.next_event()?;
        let ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { attributes, .. } = event else {
            return Err(ofdbox_yaserde::Error::custom("Layer 缺少起始元素"));
        };
        let value = |key: &str| {
            attributes
                .iter()
                .find(|attribute| attribute.name.local_name == key)
                .map(|attribute| attribute.value.clone())
        };
        let id = value("ID")
            .ok_or_else(|| ofdbox_yaserde::Error::custom("Layer 缺少 ID"))?
            .parse()
            .map_err(ofdbox_yaserde::Error::custom)?;
        let layer_type = value("Type")
            .map(|value| value.parse().map_err(ofdbox_yaserde::Error::custom))
            .transpose()?
            .unwrap_or_default();
        let draw_param = value("DrawParam")
            .map(|value| value.parse().map_err(ofdbox_yaserde::Error::custom))
            .transpose()?;
        let objects = deserialize_objects(reader, "Layer")?;
        Ok(Self {
            id,
            layer_type,
            draw_param,
            objects,
        })
    }
}

/// 图层类型。
#[derive(Clone, Debug, Default, PartialEq, Eq, ofdbox_yaserde::DefaultYaSerde)]
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

crate::impl_xml_enum_codec!(LayerType, Other, {
    Body => "Body",
    Background => "Background",
    Foreground => "Foreground",
    Custom => "Custom",
});

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

impl YaSerialize for GraphicObject {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let (name, result) = match self {
            Self::Text(value) => {
                writer.set_start_event_name(Some("TextObject".to_owned()));
                writer.set_skip_start_end(false);
                ("TextObject", value.serialize(writer))
            }
            Self::Path(value) => {
                writer.set_start_event_name(Some("PathObject".to_owned()));
                writer.set_skip_start_end(false);
                ("PathObject", value.serialize(writer))
            }
            Self::Image(value) => {
                writer.set_start_event_name(Some("ImageObject".to_owned()));
                writer.set_skip_start_end(false);
                ("ImageObject", value.serialize(writer))
            }
            Self::Composite(value) => {
                writer.set_start_event_name(Some("CompositeObject".to_owned()));
                writer.set_skip_start_end(false);
                ("CompositeObject", value.serialize(writer))
            }
            Self::PageBlock(value) => {
                writer.set_start_event_name(Some("PageBlock".to_owned()));
                writer.set_skip_start_end(false);
                ("PageBlock", value.serialize(writer))
            }
        };
        let _ = name;
        result
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaDeserialize for GraphicObject {
    fn deserialize<R: Read>(reader: &mut Deserializer<R>) -> ofdbox_yaserde::Result<Self> {
        let event = reader.peek()?.to_owned();
        let ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { name, .. } = event else {
            return Err(ofdbox_yaserde::Error::custom("GraphicObject 缺少起始元素"));
        };
        match name.local_name.as_str() {
            "TextObject" => Ok(Self::Text(TextObject::deserialize(reader)?)),
            "PathObject" => Ok(Self::Path(PathObject::deserialize(reader)?)),
            "ImageObject" => Ok(Self::Image(ImageObject::deserialize(reader)?)),
            "CompositeObject" => Ok(Self::Composite(CompositeObject::deserialize(reader)?)),
            "PageBlock" => Ok(Self::PageBlock(PageBlockObject::deserialize(reader)?)),
            other => Err(ofdbox_yaserde::Error::custom(format!(
                "未知图元类型: {other}"
            ))),
        }
    }
}

/// 图元公共属性。
#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
pub struct GraphicUnit {
    /// 图元 ID。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: StId,
    /// 边界区域。
    #[yaserde(attribute = true, rename = "Boundary")]
    pub boundary: StBox,
    /// 名称。
    #[yaserde(attribute = true, rename = "Name", default = "default_string")]
    pub name: String,
    /// 是否可见。
    #[yaserde(attribute = true, rename = "Visible", default = "default_true")]
    pub visible: bool,
    /// 变换矩阵。
    #[yaserde(attribute = true, rename = "CTM")]
    pub ctm: Option<StArray>,
    /// 绘制参数引用。
    #[yaserde(attribute = true, rename = "DrawParam")]
    pub draw_param: Option<StRefId>,
    /// 线宽。
    #[yaserde(attribute = true, rename = "LineWidth", default = "default_line_width")]
    pub line_width: f64,
    /// 线端样式。
    #[yaserde(attribute = true, rename = "Cap", default = "default_cap")]
    pub cap: Cap,
    /// 线连接样式。
    #[yaserde(attribute = true, rename = "Join", default = "default_join")]
    pub join: Join,
    /// 斜接限制。
    #[yaserde(
        attribute = true,
        rename = "MiterLimit",
        default = "default_miter_limit"
    )]
    pub miter_limit: f64,
    /// 虚线偏移。
    #[yaserde(attribute = true, rename = "DashOffset", default = "default_zero_f64")]
    pub dash_offset: f64,
    /// 虚线模式。
    #[yaserde(attribute = true, rename = "DashPattern")]
    pub dash_pattern: Option<StArray>,
    /// 透明度。
    #[yaserde(attribute = true, rename = "Alpha", default = "default_alpha")]
    pub alpha: i32,
    /// 图元动作。
    #[yaserde(rename = "Actions")]
    pub actions: Option<Actions>,
    /// 裁剪区域。
    #[yaserde(rename = "Clips")]
    pub clips: Option<Clips>,
}

/// 线端样式。
#[derive(Clone, Debug, Default, PartialEq, Eq, ofdbox_yaserde::DefaultYaSerde)]
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

crate::impl_xml_enum_codec!(Cap, Other, {
    Butt => "Butt",
    Round => "Round",
    Square => "Square",
});

/// 线连接样式。
#[derive(Clone, Debug, Default, PartialEq, Eq, ofdbox_yaserde::DefaultYaSerde)]
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

crate::impl_xml_enum_codec!(Join, Other, {
    Miter => "Miter",
    Round => "Round",
    Bevel => "Bevel",
});

/// 文字图元。
#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "TextObject")]
pub struct TextObject {
    /// 公共图元属性。
    #[yaserde(flatten = true)]
    pub common: GraphicUnit,
    /// 字体资源 ID。
    #[yaserde(attribute = true, rename = "Font")]
    pub font: StRefId,
    /// 字号。
    #[yaserde(attribute = true, rename = "Size")]
    pub size: f64,
    /// 是否描边。
    #[yaserde(attribute = true, rename = "Stroke", default = "default_false")]
    pub stroke: bool,
    /// 是否填充。
    #[yaserde(attribute = true, rename = "Fill", default = "default_true")]
    pub fill: bool,
    /// 水平缩放。
    #[yaserde(attribute = true, rename = "HScale", default = "default_hscale")]
    pub h_scale: f64,
    /// 阅读方向。
    #[yaserde(
        attribute = true,
        rename = "ReadDirection",
        default = "default_i32_zero"
    )]
    pub read_direction: i32,
    /// 字符方向。
    #[yaserde(
        attribute = true,
        rename = "CharDirection",
        default = "default_i32_zero"
    )]
    pub char_direction: i32,
    /// 字重。
    #[yaserde(attribute = true, rename = "Weight", default = "default_weight")]
    pub weight: i32,
    /// 是否斜体。
    #[yaserde(attribute = true, rename = "Italic", default = "default_false")]
    pub italic: bool,
    /// 填充色。
    #[yaserde(rename = "FillColor")]
    pub fill_color: Option<Color>,
    /// 描边色。
    #[yaserde(rename = "StrokeColor")]
    pub stroke_color: Option<Color>,
    /// 字形变换。
    #[yaserde(rename = "CGTransform")]
    pub cg_transforms: Vec<CgTransform>,
    /// 文字片段。
    #[yaserde(rename = "TextCode")]
    pub text_codes: Vec<TextCode>,
}

/// 文字片段。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "TextCode")]
pub struct TextCode {
    /// X 坐标。
    #[yaserde(attribute = true, rename = "X")]
    pub x: Option<f64>,
    /// Y 坐标。
    #[yaserde(attribute = true, rename = "Y")]
    pub y: Option<f64>,
    /// X 增量。
    #[yaserde(attribute = true, rename = "DeltaX")]
    pub delta_x: Option<DeltaArray>,
    /// Y 增量。
    #[yaserde(attribute = true, rename = "DeltaY")]
    pub delta_y: Option<DeltaArray>,
    /// 文本。
    #[yaserde(text = true, default = "default_string")]
    pub text: String,
}

/// 字形变换。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "CGTransform")]
pub struct CgTransform {
    /// 字符起点。
    #[yaserde(attribute = true, rename = "CodePosition")]
    pub code_position: i32,
    /// 字符数量。
    #[yaserde(attribute = true, rename = "CodeCount", default = "default_one_i32")]
    pub code_count: i32,
    /// 字形数量。
    #[yaserde(attribute = true, rename = "GlyphCount", default = "default_one_i32")]
    pub glyph_count: i32,
    /// 字形索引。
    #[yaserde(rename = "Glyphs")]
    pub glyphs: Option<String>,
}

impl YaSerialize for CgTransform {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        // `CodePosition`/`CodeCount`/`GlyphCount` 在 XSD 中是必选属性，即使取值等于
        // 默认值也必须写出；derive 的 `default` 会跳过等于默认值的属性，因此手写。
        let code_position = self.code_position.to_string();
        let code_count = self.code_count.to_string();
        let glyph_count = self.glyph_count.to_string();
        let element_name = crate::model_xml::ofd_name(writer, "CGTransform");
        writer
            .write(
                ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str())
                    .attr("CodePosition", code_position.as_str())
                    .attr("CodeCount", code_count.as_str())
                    .attr("GlyphCount", glyph_count.as_str()),
            )
            .map_err(ofdbox_yaserde::Error::custom)?;
        if let Some(glyphs) = &self.glyphs {
            let glyphs_name = crate::model_xml::ofd_name(writer, "Glyphs");
            writer
                .write(ofdbox_yaserde::__xml::writer::XmlEvent::start_element(
                    glyphs_name.as_str(),
                ))
                .map_err(ofdbox_yaserde::Error::custom)?;
            writer
                .write(ofdbox_yaserde::__xml::writer::XmlEvent::characters(
                    glyphs.as_str(),
                ))
                .map_err(ofdbox_yaserde::Error::custom)?;
            writer
                .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
                .map_err(ofdbox_yaserde::Error::custom)?;
        }
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

/// 路径图元。
#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "PathObject")]
pub struct PathObject {
    /// 公共图元属性。
    #[yaserde(flatten = true)]
    pub common: GraphicUnit,
    /// 描边色。
    #[yaserde(rename = "StrokeColor")]
    pub stroke_color: Option<Color>,
    /// 填充色。
    #[yaserde(rename = "FillColor")]
    pub fill_color: Option<Color>,
    /// 缩写路径数据。
    #[yaserde(rename = "AbbreviatedData")]
    pub abbreviated_data: String,
    /// 是否描边。
    #[yaserde(attribute = true, rename = "Stroke", default = "default_true")]
    pub stroke: bool,
    /// 是否填充。
    #[yaserde(attribute = true, rename = "Fill", default = "default_false")]
    pub fill: bool,
    /// 填充规则。
    #[yaserde(attribute = true, rename = "Rule", default = "default_rule")]
    pub rule: PathRule,
}

/// 路径填充规则。
#[derive(Clone, Debug, Default, PartialEq, Eq, ofdbox_yaserde::DefaultYaSerde)]
pub enum PathRule {
    /// 非零环绕。
    #[default]
    NonZero,
    /// 奇偶环绕。
    EvenOdd,
    /// 未知值。
    Other(String),
}

crate::impl_xml_enum_codec!(PathRule, Other, {
    NonZero => "NonZero",
    EvenOdd => "Even-Odd",
});

/// 图像图元。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "ImageObject")]
pub struct ImageObject {
    /// 公共图元属性。
    #[yaserde(flatten = true)]
    pub common: GraphicUnit,
    /// 图像资源 ID。
    #[yaserde(attribute = true, rename = "ResourceID")]
    pub resource_id: StRefId,
    /// 替换资源 ID。
    #[yaserde(attribute = true, rename = "Substitution")]
    pub substitution: Option<StRefId>,
    /// 图像蒙版 ID。
    #[yaserde(attribute = true, rename = "ImageMask")]
    pub image_mask: Option<StRefId>,
    /// 边框。
    #[yaserde(rename = "Border")]
    pub border: Option<Border>,
}

/// 图像边框。
#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "Border")]
pub struct Border {
    /// 边框颜色。
    #[yaserde(rename = "BorderColor")]
    pub color: Option<Color>,
    /// 线宽。
    #[yaserde(attribute = true, rename = "LineWidth", default = "default_line_width")]
    pub line_width: f64,
    /// 水平圆角半径。
    #[yaserde(
        attribute = true,
        rename = "HorizonalCornerRadius",
        default = "default_zero_f64"
    )]
    pub horizontal_corner_radius: f64,
    /// 垂直圆角半径。
    #[yaserde(
        attribute = true,
        rename = "VerticalCornerRadius",
        default = "default_zero_f64"
    )]
    pub vertical_corner_radius: f64,
    /// 虚线偏移。
    #[yaserde(attribute = true, rename = "DashOffset", default = "default_zero_f64")]
    pub dash_offset: f64,
    /// 虚线模式。
    #[yaserde(attribute = true, rename = "DashPattern")]
    pub dash_pattern: Option<StArray>,
}

/// 复合图元。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "CompositeObject")]
pub struct CompositeObject {
    /// 公共图元属性。
    #[yaserde(flatten = true)]
    pub common: GraphicUnit,
    /// 复合资源 ID。
    #[yaserde(attribute = true, rename = "ResourceID")]
    pub resource_id: StRefId,
}

/// 页块内容。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageBlock {
    /// 图元。
    pub objects: Vec<GraphicObject>,
}

impl YaSerialize for PageBlock {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let name = writer
            .get_start_event_name()
            .unwrap_or_else(|| "PageBlock".to_owned());
        let element_name = if name.contains(':') {
            name
        } else {
            crate::model_xml::ofd_name(writer, name.as_str())
        };
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::start_element(
                element_name.as_str(),
            ))
            .map_err(ofdbox_yaserde::Error::custom)?;
        for object in &self.objects {
            writer.set_start_event_name(None);
            object.serialize(writer)?;
        }
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaDeserialize for PageBlock {
    fn deserialize<R: Read>(reader: &mut Deserializer<R>) -> ofdbox_yaserde::Result<Self> {
        let event = reader.next_event()?;
        let ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { name, .. } = event else {
            return Err(ofdbox_yaserde::Error::custom("PageBlock 缺少起始元素"));
        };
        let objects = deserialize_objects(reader, &name.local_name)?;
        Ok(Self { objects })
    }
}

/// 页块对象。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageBlockObject {
    /// 页块 ID。
    pub id: StId,
    /// 页块内容。
    pub block: PageBlock,
}

impl YaSerialize for PageBlockObject {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let id = self.id.to_string();
        writer
            .write(
                ofdbox_yaserde::__xml::writer::XmlEvent::start_element(
                    crate::model_xml::ofd_name(writer, "PageBlock").as_str(),
                )
                .attr("ID", id.as_str()),
            )
            .map_err(ofdbox_yaserde::Error::custom)?;
        for object in &self.block.objects {
            object.serialize(writer)?;
        }
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaDeserialize for PageBlockObject {
    fn deserialize<R: Read>(reader: &mut Deserializer<R>) -> ofdbox_yaserde::Result<Self> {
        let event = reader.next_event()?;
        let ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { attributes, .. } = event else {
            return Err(ofdbox_yaserde::Error::custom("PageBlock 缺少起始元素"));
        };
        let id = attributes
            .iter()
            .find(|attribute| attribute.name.local_name == "ID")
            .ok_or_else(|| ofdbox_yaserde::Error::custom("PageBlock 缺少 ID"))?
            .value
            .parse()
            .map_err(ofdbox_yaserde::Error::custom)?;
        let objects = deserialize_objects(reader, "PageBlock")?;
        Ok(Self {
            id,
            block: PageBlock { objects },
        })
    }
}

fn skip_current_element<R: Read>(reader: &mut Deserializer<R>) -> ofdbox_yaserde::Result<()> {
    let depth = reader.depth();
    let _ = reader.next_event()?;
    while reader.depth() > depth {
        let _ = reader.next_event()?;
    }
    Ok(())
}

fn deserialize_objects<R: Read>(
    reader: &mut Deserializer<R>,
    end_name: &str,
) -> ofdbox_yaserde::Result<Vec<GraphicObject>> {
    let mut objects = Vec::new();
    loop {
        let event = reader.peek()?.to_owned();
        match event {
            ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { .. } => {
                match GraphicObject::deserialize(reader) {
                    Ok(object) => objects.push(object),
                    Err(error) if error.to_string().contains("未知图元类型") => {
                        skip_current_element(reader)?;
                    }
                    Err(error) => return Err(error),
                }
            }
            ofdbox_yaserde::__xml::reader::XmlEvent::EndElement { name } => {
                if name.local_name == end_name {
                    break;
                }
                let _ = reader.next_event()?;
            }
            ofdbox_yaserde::__xml::reader::XmlEvent::EndDocument => break,
            _ => {
                let _ = reader.next_event()?;
            }
        }
    }
    Ok(objects)
}

/// 裁剪区域集合。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Clips")]
pub struct Clips {
    /// 裁剪区域。
    #[yaserde(rename = "Clip")]
    pub clips: Vec<Clip>,
}

/// 裁剪区域。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Clip")]
pub struct Clip {
    /// 裁剪区域。
    #[yaserde(rename = "Area")]
    pub areas: Vec<ClipArea>,
}

/// 单个裁剪区域。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Area")]
pub struct ClipArea {
    /// 绘制参数引用。
    #[yaserde(attribute = true, rename = "DrawParam")]
    pub draw_param: Option<StRefId>,
    /// 变换矩阵。
    #[yaserde(attribute = true, rename = "CTM")]
    pub ctm: Option<StArray>,
    /// 裁剪内容。
    #[yaserde(flatten = true)]
    pub content: ClipContent,
}

/// 裁剪内容。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
pub enum ClipContent {
    /// 空内容。
    #[default]
    Empty,
    /// 路径图元。
    #[yaserde(rename = "Path")]
    Path(PathObject),
    /// 文字图元。
    #[yaserde(rename = "Text")]
    Text(TextObject),
}

/// 颜色。
#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
#[yaserde(
  namespace = "http://www.ofdspec.org/2016",
  xmlns = {
    "ofd" = "http://www.ofdspec.org/2016"
  },
  namespace_aliases = {
    "http://www.ofdspec.org" = "http://www.ofdspec.org/2016"
  }
)]
#[yaserde(rename = "Color")]
pub struct Color {
    /// 颜色值。
    #[yaserde(attribute = true, rename = "Value")]
    pub value: Option<ColorArray>,
    /// 颜色索引。
    #[yaserde(attribute = true, rename = "Index")]
    pub index: Option<i32>,
    /// 颜色空间引用。
    #[yaserde(attribute = true, rename = "ColorSpace")]
    pub color_space: Option<StRefId>,
    /// 透明度。
    #[yaserde(attribute = true, rename = "Alpha", default = "default_alpha")]
    pub alpha: i32,
    /// 颜色内容。
    #[yaserde(flatten = true)]
    pub kind: ColorKind,
}

/// 颜色内容。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaDeserialize)]
pub enum ColorKind {
    /// 底纹。
    #[yaserde(rename = "Pattern")]
    Pattern(Pattern),
    /// 轴向渐变。
    #[yaserde(rename = "AxialShd")]
    AxialShd(AxialShd),
    /// 径向渐变。
    #[yaserde(rename = "RadialShd")]
    RadialShd(RadialShd),
    /// 高洛德渐变。
    #[yaserde(rename = "GouraudShd")]
    GouraudShd(GouraudShd),
    /// 格点高洛德渐变。
    #[yaserde(rename = "LaGourandShd")]
    LaGouraudShd(LaGouraudShd),
    /// 无颜色扩展（纯色）。
    #[default]
    None,
}

impl YaSerialize for ColorKind {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        match self {
            Self::Pattern(value) => value.serialize(writer),
            Self::AxialShd(value) => {
                writer.set_start_event_name(Some("AxialShd".to_owned()));
                value.serialize(writer)
            }
            Self::RadialShd(value) => {
                writer.set_start_event_name(Some("RadialShd".to_owned()));
                value.serialize(writer)
            }
            Self::GouraudShd(value) => {
                writer.set_start_event_name(Some("GouraudShd".to_owned()));
                value.serialize(writer)
            }
            Self::LaGouraudShd(value) => {
                writer.set_start_event_name(Some("LaGourandShd".to_owned()));
                value.serialize(writer)
            }
            Self::None => Ok(()),
        }
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

/// 底纹。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pattern {
    /// 单元格内容。
    pub cell_content: CellContent,
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

/// 底纹单元格内容。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CellContent {
    /// 缩略图引用。
    pub thumbnail: Option<StRefId>,
    /// 单元格图元。
    pub objects: Vec<GraphicObject>,
}

impl YaSerialize for CellContent {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let thumbnail = self.thumbnail.map(|value| value.to_string());
        let element_name = crate::model_xml::ofd_name(writer, "CellContent");
        let mut event =
            ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str());
        if let Some(thumbnail) = &thumbnail {
            event = event.attr("Thumbnail", thumbnail.as_str());
        }
        writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
        for object in &self.objects {
            writer.set_start_event_name(None);
            object.serialize(writer)?;
        }
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaDeserialize for CellContent {
    fn deserialize<R: Read>(reader: &mut Deserializer<R>) -> ofdbox_yaserde::Result<Self> {
        let event = reader.next_event()?;
        let ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { attributes, .. } = event else {
            return Err(ofdbox_yaserde::Error::custom("CellContent 缺少起始元素"));
        };
        let thumbnail = attributes
            .iter()
            .find(|attribute| attribute.name.local_name == "Thumbnail")
            .map(|attribute| {
                attribute
                    .value
                    .parse()
                    .map_err(ofdbox_yaserde::Error::custom)
            })
            .transpose()?;
        let objects = deserialize_objects(reader, "CellContent")?;
        Ok(Self { thumbnail, objects })
    }
}

impl YaSerialize for Pattern {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let width = self.width.to_string();
        let height = self.height.to_string();
        let x_step = self.x_step.map(|value| value.to_string());
        let y_step = self.y_step.map(|value| value.to_string());
        let reflect = self.reflect_method.to_string();
        let relative = self.relative_to.to_string();
        let ctm = self.ctm.as_ref().map(ToString::to_string);
        let element_name = crate::model_xml::ofd_name(writer, "Pattern");
        let mut event =
            ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str())
                .attr("Width", width.as_str())
                .attr("Height", height.as_str());
        if self.reflect_method != PatternReflect::Normal {
            event = event.attr("ReflectMethod", reflect.as_str());
        }
        if self.relative_to != PatternRelativeTo::Object {
            event = event.attr("RelativeTo", relative.as_str());
        }
        if let Some(value) = &x_step {
            event = event.attr("XStep", value.as_str());
        }
        if let Some(value) = &y_step {
            event = event.attr("YStep", value.as_str());
        }
        if let Some(value) = &ctm {
            event = event.attr("CTM", value.as_str());
        }
        writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
        self.cell_content.serialize(writer)?;
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaDeserialize for Pattern {
    fn deserialize<R: Read>(reader: &mut Deserializer<R>) -> ofdbox_yaserde::Result<Self> {
        let event = reader.next_event()?;
        let ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { attributes, .. } = event else {
            return Err(ofdbox_yaserde::Error::custom("Pattern 缺少起始元素"));
        };
        let value = |key: &str| {
            attributes
                .iter()
                .find(|attribute| attribute.name.local_name == key)
                .map(|attribute| attribute.value.clone())
        };
        let number = |key: &str| -> ofdbox_yaserde::Result<Option<f64>> {
            value(key)
                .map(|value| value.parse().map_err(ofdbox_yaserde::Error::custom))
                .transpose()
        };
        let width =
            number("Width")?.ok_or_else(|| ofdbox_yaserde::Error::custom("Pattern 缺少 Width"))?;
        let height = number("Height")?
            .ok_or_else(|| ofdbox_yaserde::Error::custom("Pattern 缺少 Height"))?;
        let cell_content = CellContent::deserialize(reader)?;
        let _ = reader.next_event()?;
        Ok(Self {
            cell_content,
            width,
            height,
            x_step: number("XStep")?,
            y_step: number("YStep")?,
            reflect_method: value("ReflectMethod")
                .map(|value| value.parse().map_err(ofdbox_yaserde::Error::custom))
                .transpose()?
                .unwrap_or_default(),
            relative_to: value("RelativeTo")
                .map(|value| value.parse().map_err(ofdbox_yaserde::Error::custom))
                .transpose()?
                .unwrap_or_default(),
            ctm: value("CTM")
                .map(|value| value.parse().map_err(ofdbox_yaserde::Error::custom))
                .transpose()?,
        })
    }
}

/// 底纹反射方式。
#[derive(Clone, Debug, Default, PartialEq, Eq, ofdbox_yaserde::DefaultYaSerde)]
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

crate::impl_xml_enum_codec!(PatternReflect, Other, {
    Normal => "Normal",
    Row => "Row",
    Column => "Column",
    RowAndColumn => "RowAndColumn",
});

/// 底纹相对对象。
#[derive(Clone, Debug, Default, PartialEq, Eq, ofdbox_yaserde::DefaultYaSerde)]
pub enum PatternRelativeTo {
    /// 页面。
    Page,
    /// 对象。
    #[default]
    Object,
    /// 未知值。
    Other(String),
}

crate::impl_xml_enum_codec!(PatternRelativeTo, Other, {
    Page => "Page",
    Object => "Object",
});

/// 渐变映射方式。
#[derive(Clone, Debug, Default, PartialEq, Eq, ofdbox_yaserde::DefaultYaSerde)]
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

crate::impl_xml_enum_codec!(MapType, Other, {
    Direct => "Direct",
    Repeat => "Repeat",
    Reflect => "Reflect",
});

/// 渐变色标。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "Segment")]
pub struct ShdSegment {
    /// 位置。
    #[yaserde(attribute = true, rename = "Position")]
    pub position: Option<f64>,
    /// 颜色。
    #[yaserde(rename = "Color")]
    pub color: Color,
}

/// 轴向渐变。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "AxialShd")]
pub struct AxialShd {
    /// 色标。
    #[yaserde(rename = "Segment")]
    pub segments: Vec<ShdSegment>,
    /// 映射方式。
    #[yaserde(attribute = true, rename = "MapType", default = "default_map_type")]
    pub map_type: MapType,
    /// 映射单位。
    #[yaserde(attribute = true, rename = "MapUnit")]
    pub map_unit: Option<f64>,
    /// 扩展方式。
    #[yaserde(attribute = true, rename = "Extend", default = "default_i32_zero")]
    pub extend: i32,
    /// 起点。
    #[yaserde(attribute = true, rename = "StartPoint")]
    pub start_point: StPos,
    /// 终点。
    #[yaserde(attribute = true, rename = "EndPoint")]
    pub end_point: StPos,
}

/// 径向渐变。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "RadialShd")]
pub struct RadialShd {
    /// 色标。
    #[yaserde(rename = "Segment")]
    pub segments: Vec<ShdSegment>,
    /// 映射方式。
    #[yaserde(attribute = true, rename = "MapType", default = "default_map_type")]
    pub map_type: MapType,
    /// 映射单位。
    #[yaserde(attribute = true, rename = "MapUnit")]
    pub map_unit: Option<f64>,
    /// 离心率。
    #[yaserde(
        attribute = true,
        rename = "Eccentricity",
        default = "default_zero_f64"
    )]
    pub eccentricity: f64,
    /// 角度。
    #[yaserde(attribute = true, rename = "Angle", default = "default_zero_f64")]
    pub angle: f64,
    /// 起点。
    #[yaserde(attribute = true, rename = "StartPoint")]
    pub start_point: StPos,
    /// 起始半径。
    #[yaserde(attribute = true, rename = "StartRadius", default = "default_zero_f64")]
    pub start_radius: f64,
    /// 终点。
    #[yaserde(attribute = true, rename = "EndPoint")]
    pub end_point: StPos,
    /// 结束半径。
    #[yaserde(attribute = true, rename = "EndRadius")]
    pub end_radius: f64,
    /// 扩展方式。
    #[yaserde(attribute = true, rename = "Extend", default = "default_i32_zero")]
    pub extend: i32,
}

/// 高洛德渐变。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "GouraudShd")]
pub struct GouraudShd {
    /// 点。
    #[yaserde(rename = "Point")]
    pub points: Vec<GouraudPoint>,
    /// 背景色。
    #[yaserde(rename = "BackColor")]
    pub back_color: Option<Box<Color>>,
    /// 扩展方式。
    #[yaserde(attribute = true, rename = "Extend")]
    pub extend: Option<i32>,
}

/// 高洛德渐变点。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "Point")]
pub struct GouraudPoint {
    /// X 坐标。
    #[yaserde(attribute = true, rename = "X")]
    pub x: f64,
    /// Y 坐标。
    #[yaserde(attribute = true, rename = "Y")]
    pub y: f64,
    /// 边缘标志。
    #[yaserde(attribute = true, rename = "EdgeFlag")]
    pub edge_flag: Option<i32>,
    /// 颜色。
    #[yaserde(rename = "Color")]
    pub color: Color,
}

/// 格点高洛德渐变。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "LaGourandShd")]
pub struct LaGouraudShd {
    /// 点。
    #[yaserde(rename = "Point")]
    pub points: Vec<LaGouraudPoint>,
    /// 背景色。
    #[yaserde(rename = "BackColor")]
    pub back_color: Option<Box<Color>>,
    /// 每行顶点数。
    #[yaserde(attribute = true, rename = "VerticesPerRow")]
    pub vertices_per_row: i32,
    /// 扩展方式。
    #[yaserde(attribute = true, rename = "Extend")]
    pub extend: Option<i32>,
}

/// 格点高洛德渐变点。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "Point")]
pub struct LaGouraudPoint {
    /// X 坐标。
    #[yaserde(attribute = true, rename = "X")]
    pub x: Option<f64>,
    /// Y 坐标。
    #[yaserde(attribute = true, rename = "Y")]
    pub y: Option<f64>,
    /// 颜色。
    #[yaserde(rename = "Color")]
    pub color: Color,
}

impl YaSerialize for ShdSegment {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let position = self.position.map(|value| value.to_string());
        let element_name = crate::model_xml::ofd_name(writer, "Segment");
        let mut event =
            ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str());
        if let Some(position) = &position {
            event = event.attr("Position", position.as_str());
        }
        writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
        write_color(writer, &self.color)?;
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaSerialize for AxialShd {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let map_type = self.map_type.to_string();
        let start = self.start_point.to_string();
        let end = self.end_point.to_string();
        let map_unit = self.map_unit.map(|value| value.to_string());
        let extend = (self.extend != 0).then(|| self.extend.to_string());
        let element_name = crate::model_xml::ofd_name(writer, "AxialShd");
        let mut event =
            ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str())
                .attr("MapType", map_type.as_str())
                .attr("StartPoint", start.as_str())
                .attr("EndPoint", end.as_str());
        if let Some(value) = &extend {
            event = event.attr("Extend", value.as_str());
        }
        if let Some(value) = &map_unit {
            event = event.attr("MapUnit", value.as_str());
        }
        writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
        for segment in &self.segments {
            segment.serialize(writer)?;
        }
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaSerialize for RadialShd {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let map_type = self.map_type.to_string();
        let start = self.start_point.to_string();
        let end = self.end_point.to_string();
        let end_radius = self.end_radius.to_string();
        let map_unit = self.map_unit.map(|value| value.to_string());
        let extend = (self.extend != 0).then(|| self.extend.to_string());
        let eccentricity = (self.eccentricity != 0.0).then(|| self.eccentricity.to_string());
        let angle = (self.angle != 0.0).then(|| self.angle.to_string());
        let start_radius = (self.start_radius != 0.0).then(|| self.start_radius.to_string());
        let element_name = crate::model_xml::ofd_name(writer, "RadialShd");
        let mut event =
            ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str())
                .attr("MapType", map_type.as_str())
                .attr("StartPoint", start.as_str())
                .attr("EndPoint", end.as_str())
                .attr("EndRadius", end_radius.as_str());
        if let Some(value) = &extend {
            event = event.attr("Extend", value.as_str());
        }
        if let Some(value) = &eccentricity {
            event = event.attr("Eccentricity", value.as_str());
        }
        if let Some(value) = &angle {
            event = event.attr("Angle", value.as_str());
        }
        if let Some(value) = &start_radius {
            event = event.attr("StartRadius", value.as_str());
        }
        if let Some(value) = &map_unit {
            event = event.attr("MapUnit", value.as_str());
        }
        writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
        for segment in &self.segments {
            segment.serialize(writer)?;
        }
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaSerialize for GouraudPoint {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let x = self.x.to_string();
        let y = self.y.to_string();
        let edge = self.edge_flag.map(|value| value.to_string());
        let element_name = crate::model_xml::ofd_name(writer, "Point");
        let mut event =
            ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str())
                .attr("X", x.as_str())
                .attr("Y", y.as_str());
        if let Some(value) = &edge {
            event = event.attr("EdgeFlag", value.as_str());
        }
        writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
        write_color(writer, &self.color)?;
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaSerialize for GouraudShd {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let extend = self.extend.map(|value| value.to_string());
        let element_name = crate::model_xml::ofd_name(writer, "GouraudShd");
        let mut event =
            ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str());
        if let Some(value) = &extend {
            event = event.attr("Extend", value.as_str());
        }
        writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
        for point in &self.points {
            point.serialize(writer)?;
        }
        if let Some(color) = &self.back_color {
            write_color_named(writer, "BackColor", color)?;
        }
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaSerialize for LaGouraudPoint {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let x = self.x.map(|value| value.to_string());
        let y = self.y.map(|value| value.to_string());
        let element_name = crate::model_xml::ofd_name(writer, "Point");
        let mut event =
            ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str());
        if let Some(value) = &x {
            event = event.attr("X", value.as_str());
        }
        if let Some(value) = &y {
            event = event.attr("Y", value.as_str());
        }
        writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
        write_color(writer, &self.color)?;
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaSerialize for LaGouraudShd {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let vertices = self.vertices_per_row.to_string();
        let extend = self.extend.map(|value| value.to_string());
        let element_name = crate::model_xml::ofd_name(writer, "LaGourandShd");
        let mut event =
            ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str())
                .attr("VerticesPerRow", vertices.as_str());
        if let Some(value) = &extend {
            event = event.attr("Extend", value.as_str());
        }
        writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
        for point in &self.points {
            point.serialize(writer)?;
        }
        if let Some(color) = &self.back_color {
            write_color_named(writer, "BackColor", color)?;
        }
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

fn write_color<W: Write>(writer: &mut Serializer<W>, color: &Color) -> ofdbox_yaserde::Result<()> {
    write_color_named(writer, "Color", color)
}

fn write_color_named<W: Write>(
    writer: &mut Serializer<W>,
    name: &str,
    color: &Color,
) -> ofdbox_yaserde::Result<()> {
    let value = color.value.as_ref().map(ToString::to_string);
    let index = color.index.map(|value| value.to_string());
    let color_space = color.color_space.map(|value| value.to_string());
    let alpha = (color.alpha != 255).then(|| color.alpha.to_string());
    let element_name = crate::model_xml::ofd_name(writer, name);
    let mut event = ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str());
    if let Some(value) = &value {
        event = event.attr("Value", value.as_str());
    }
    if let Some(value) = &index {
        event = event.attr("Index", value.as_str());
    }
    if let Some(space) = &color_space {
        event = event.attr("ColorSpace", space.as_str());
    }
    if let Some(value) = &alpha {
        event = event.attr("Alpha", value.as_str());
    }
    writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
    color.kind.serialize(writer)?;
    writer
        .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
        .map_err(ofdbox_yaserde::Error::custom)?;
    Ok(())
}

/// 动作集合的便捷类型。
pub type PageActions = Actions;

/// 单个动作的便捷类型。
pub type PageAction = Action;

impl YaSerialize for Box<Color> {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        (**self).serialize(writer)
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        (**self).serialize_attributes(attributes, namespace)
    }
}

impl YaDeserialize for Box<Color> {
    fn deserialize<R: Read>(reader: &mut Deserializer<R>) -> ofdbox_yaserde::Result<Self> {
        Ok(Box::new(Color::deserialize(reader)?))
    }
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_line_width() -> f64 {
    0.353
}

fn default_miter_limit() -> f64 {
    4.234
}

fn default_alpha() -> i32 {
    255
}

fn default_hscale() -> f64 {
    1.0
}

fn default_weight() -> i32 {
    400
}

fn default_cap() -> Cap {
    Cap::Butt
}

fn default_join() -> Join {
    Join::Miter
}

fn default_zero_f64() -> f64 {
    0.0
}

fn default_i32_zero() -> i32 {
    0
}

fn default_rule() -> PathRule {
    PathRule::NonZero
}

fn default_string() -> String {
    String::new()
}

fn default_template_z_order() -> TemplateZOrder {
    TemplateZOrder::Background
}

fn default_one_i32() -> i32 {
    1
}

impl Default for Border {
    fn default() -> Self {
        Self {
            color: None,
            line_width: 0.353,
            horizontal_corner_radius: 0.0,
            vertical_corner_radius: 0.0,
            dash_offset: 0.0,
            dash_pattern: None,
        }
    }
}

fn default_map_type() -> MapType {
    MapType::Direct
}
