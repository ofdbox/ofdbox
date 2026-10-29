//! `Definition.xsd` 中的公共简单类型和复杂类型。
//!
//! 该模块没有对应的文档根对象，只提供其他模块共享的类型。

use std::io::{Read, Write};

use ofdbox_yaserde::de::Deserializer;
use ofdbox_yaserde::ser::Serializer;
use ofdbox_yaserde::{YaDeserialize, YaSerialize};

/// 文档内对象 ID。
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    ofdbox_yaserde::DefaultYaSerde,
)]
pub struct StId(pub u32);

impl StId {
    /// 创建 ID。
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// 获取数值。
    pub fn value(self) -> u32 {
        self.0
    }
}

impl From<u32> for StId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

/// 资源引用 ID。
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    ofdbox_yaserde::DefaultYaSerde,
)]
pub struct StRefId(pub u32);

impl StRefId {
    /// 创建资源引用 ID。
    pub fn new(value: u32) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for StId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::str::FromStr for StId {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.trim().parse::<u32>().unwrap_or(0)))
    }
}

impl std::fmt::Display for StRefId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::str::FromStr for StRefId {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.trim().parse::<u32>().unwrap_or(0)))
    }
}

impl From<u32> for StRefId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

/// 包内相对路径。
#[derive(
    Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, ofdbox_yaserde::PrimitiveYaSerde,
)]
pub struct StLoc(pub String);

impl StLoc {
    /// 创建路径。
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// 路径字符串。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for StLoc {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// 数值数组。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::DefaultYaSerde)]
pub struct StArray(pub Vec<f64>);

impl StArray {
    /// 创建数值数组。
    pub fn new(values: Vec<f64>) -> Self {
        Self(values)
    }
}

/// 坐标点。
#[derive(Clone, Copy, Debug, Default, PartialEq, ofdbox_yaserde::DefaultYaSerde)]
pub struct StPos {
    /// X 坐标。
    pub x: f64,
    /// Y 坐标。
    pub y: f64,
}

/// 矩形区域，对应 `x y width height`。
#[derive(Clone, Copy, Debug, Default, PartialEq, ofdbox_yaserde::DefaultYaSerde)]
pub struct StBox {
    /// 左上角 X。
    pub x: f64,
    /// 左上角 Y。
    pub y: f64,
    /// 宽。
    pub width: f64,
    /// 高。
    pub height: f64,
}

impl StBox {
    /// 创建区域。
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

impl std::fmt::Display for StArray {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = self
            .0
            .iter()
            .map(f64::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        formatter.write_str(&text)
    }
}

impl std::str::FromStr for StArray {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let values = value
            .split_whitespace()
            .map(|item| item.parse::<f64>().unwrap_or(0.0))
            .collect::<Vec<_>>();
        Ok(Self(values))
    }
}

/// 颜色分量数组。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::DefaultYaSerde)]
pub struct ColorArray(pub Vec<u32>);

impl ColorArray {
    /// 创建颜色分量数组。
    pub fn new(values: Vec<u32>) -> Self {
        Self(values)
    }
}

impl std::fmt::Display for ColorArray {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = self
            .0
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        formatter.write_str(&text)
    }
}

impl std::str::FromStr for ColorArray {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let values = value
            .split_whitespace()
            .map(|token| {
                if let Some(hex) = token.strip_prefix('#') {
                    u32::from_str_radix(hex, 16).unwrap_or(0)
                } else {
                    token.parse::<u32>().unwrap_or(0)
                }
            })
            .collect::<Vec<_>>();
        Ok(Self(values))
    }
}

/// 文字增量数组。
#[derive(Clone, Debug, Default, PartialEq, ofdbox_yaserde::DefaultYaSerde)]
pub struct DeltaArray(pub Vec<f64>);

impl DeltaArray {
    /// 创建增量数组。
    pub fn new(values: Vec<f64>) -> Self {
        Self(values)
    }
}

impl std::fmt::Display for DeltaArray {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = self
            .0
            .iter()
            .map(f64::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        formatter.write_str(&text)
    }
}

impl std::str::FromStr for DeltaArray {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let tokens = value.split_whitespace().collect::<Vec<_>>();
        let mut values = Vec::with_capacity(tokens.len());
        let mut index = 0;

        while index < tokens.len() {
            // Some vendors encode repeated deltas as `g <count> <value>`.
            // Example: `1 2 3 4 g 4 1 2 3` expands to
            // `1 2 3 4 1 1 1 1 2 3`.
            if tokens[index].eq_ignore_ascii_case("g")
                && let (Some(count), Some(repeated)) = (
                    tokens
                        .get(index + 1)
                        .and_then(|item| item.parse::<usize>().ok()),
                    tokens
                        .get(index + 2)
                        .and_then(|item| item.parse::<f64>().ok()),
                )
            {
                values.extend(std::iter::repeat_n(repeated, count));
                index += 3;
                continue;
            }

            values.push(tokens[index].parse::<f64>().unwrap_or(0.0));
            index += 1;
        }

        Ok(Self(values))
    }
}

impl std::fmt::Display for StBox {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} {} {} {}",
            self.x, self.y, self.width, self.height
        )
    }
}

impl std::str::FromStr for StBox {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let values = value
            .split_whitespace()
            .map(|item| {
                item.parse::<f64>()
                    .map_err(|error| format!("无法解析区域值 {item}: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if values.len() != 4 {
            return Err(format!("区域需要 4 个值，实际 {}", values.len()));
        }
        Ok(Self {
            x: values[0],
            y: values[1],
            width: values[2],
            height: values[3],
        })
    }
}

impl std::fmt::Display for StPos {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} {}", self.x, self.y)
    }
}

impl std::str::FromStr for StPos {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let values = value
            .split_whitespace()
            .map(|item| {
                item.parse::<f64>()
                    .map_err(|error| format!("无法解析坐标值 {item}: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if values.len() != 2 {
            return Err(format!("坐标需要 2 个值，实际 {}", values.len()));
        }
        Ok(Self {
            x: values[0],
            y: values[1],
        })
    }
}

/// 目标位置类型。
#[derive(Clone, Debug, Default, PartialEq, Eq, ofdbox_yaserde::DefaultYaSerde)]
pub enum DestType {
    /// `XYZ`。
    #[default]
    Xyz,
    /// `Fit`。
    Fit,
    /// `FitH`。
    FitH,
    /// `FitV`。
    FitV,
    /// `FitR`。
    FitR,
    /// 未知值。
    Other(String),
}

crate::impl_xml_enum_codec!(DestType, Other, {
    Xyz => "XYZ",
    Fit => "Fit",
    FitH => "FitH",
    FitV => "FitV",
    FitR => "FitR",
});

/// 跳转目标。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Dest")]
pub struct Dest {
    /// 目标类型。
    #[yaserde(attribute = true, rename = "Type")]
    pub dest_type: DestType,
    /// 目标页面 ID。
    #[yaserde(attribute = true, rename = "PageID")]
    pub page_id: StRefId,
    /// 左坐标。
    #[yaserde(attribute = true, rename = "Left", default = "default_zero_f64")]
    pub left: f64,
    /// 上坐标。
    #[yaserde(attribute = true, rename = "Top", default = "default_zero_f64")]
    pub top: f64,
    /// 右坐标。
    #[yaserde(attribute = true, rename = "Right")]
    pub right: Option<f64>,
    /// 下坐标。
    #[yaserde(attribute = true, rename = "Bottom")]
    pub bottom: Option<f64>,
    /// 缩放。
    #[yaserde(attribute = true, rename = "Zoom")]
    pub zoom: Option<f64>,
}

/// 页面区域。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "PageArea")]
pub struct PageArea {
    /// 物理区域。
    #[yaserde(rename = "PhysicalBox", default = "default_st_box")]
    pub physical_box: StBox,
    /// 应用区域。
    #[yaserde(rename = "ApplicationBox")]
    pub application_box: Option<StBox>,
    /// 内容区域。
    #[yaserde(rename = "ContentBox")]
    pub content_box: Option<StBox>,
    /// 出血区域。
    #[yaserde(rename = "BleedBox")]
    pub bleed_box: Option<StBox>,
}

/// 动作事件。
#[derive(Clone, Debug, Default, PartialEq, Eq, ofdbox_yaserde::DefaultYaSerde)]
pub enum ActionEvent {
    /// 文档打开。
    #[default]
    Do,
    /// 页面打开。
    Po,
    /// 点击。
    Click,
    /// 未知值。
    Other(String),
}

crate::impl_xml_enum_codec!(ActionEvent, Other, {
    Do => "DO",
    Po => "PO",
    Click => "CLICK",
});

/// 动作集合。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Actions")]
pub struct Actions {
    /// 动作列表。
    #[yaserde(rename = "Action")]
    pub actions: Vec<Action>,
}

/// 一个动作。
#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "Action")]
pub struct Action {
    /// 触发事件。
    #[yaserde(attribute = true, rename = "Event")]
    pub event: ActionEvent,
    /// 动作区域。
    #[yaserde(rename = "Region")]
    pub region: Option<Region>,
    /// 动作内容。
    #[yaserde(flatten = true)]
    pub kind: ActionKind,
}

/// 动作内容。
#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
pub enum ActionKind {
    /// 跳转。
    #[yaserde(rename = "Goto")]
    Goto(Goto),
    /// URI。
    #[yaserde(rename = "URI")]
    Uri(Uri),
    /// 跳转附件。
    #[yaserde(rename = "GotoA")]
    GotoA(GotoA),
    /// 声音。
    #[yaserde(rename = "Sound")]
    Sound(Sound),
    /// 影片。
    #[yaserde(rename = "Movie")]
    Movie(Movie),
}

/// 跳转动作。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Goto")]
pub struct Goto {
    /// 跳转目标。
    #[yaserde(flatten = true)]
    pub kind: GotoKind,
}

/// 书签引用。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Bookmark")]
pub struct GotoBookmark {
    /// 书签名。
    #[yaserde(attribute = true, rename = "Name")]
    pub name: String,
}

/// 跳转目标类型。
#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
pub enum GotoKind {
    /// 目标位置。
    #[yaserde(rename = "Dest")]
    Dest(Dest),
    /// 书签名。
    #[yaserde(rename = "Bookmark")]
    Bookmark(GotoBookmark),
}

impl Default for GotoKind {
    fn default() -> Self {
        Self::Bookmark(GotoBookmark::default())
    }
}

impl Default for ActionKind {
    fn default() -> Self {
        Self::Goto(Goto::default())
    }
}

/// URI 动作。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "URI")]
pub struct Uri {
    /// URI。
    #[yaserde(attribute = true, rename = "URI")]
    pub uri: String,
    /// 基准路径。
    #[yaserde(attribute = true, rename = "Base")]
    pub base: Option<String>,
    /// 目标窗口。
    #[yaserde(attribute = true, rename = "Target")]
    pub target: Option<String>,
}

/// 附件跳转动作。
#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "GotoA")]
pub struct GotoA {
    /// 附件 ID。
    #[yaserde(attribute = true, rename = "AttachID")]
    pub attach_id: String,
    /// 是否新窗口。
    #[yaserde(attribute = true, rename = "NewWindow")]
    pub new_window: bool,
}

/// 声音动作。
#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "Sound")]
pub struct Sound {
    /// 资源 ID。
    #[yaserde(attribute = true, rename = "ResourceID")]
    pub resource_id: StRefId,
    /// 音量。
    #[yaserde(attribute = true, rename = "Volume", default = "default_volume")]
    pub volume: i32,
    /// 是否重复。
    #[yaserde(attribute = true, rename = "Repeat", default = "default_false")]
    pub repeat: bool,
    /// 是否同步。
    #[yaserde(attribute = true, rename = "Synchronous", default = "default_false")]
    pub synchronous: bool,
}

/// 影片操作类型。
#[derive(Clone, Debug, Default, PartialEq, Eq, ofdbox_yaserde::DefaultYaSerde)]
pub enum MovieOperator {
    /// 播放。
    #[default]
    Play,
    /// 停止。
    Stop,
    /// 暂停。
    Pause,
    /// 继续。
    Resume,
    /// 未知值。
    Other(String),
}

crate::impl_xml_enum_codec!(MovieOperator, Other, {
    Play => "Play",
    Stop => "Stop",
    Pause => "Pause",
    Resume => "Resume",
});

/// 影片动作。
#[derive(Clone, Debug, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize)]
#[yaserde(rename = "Movie")]
pub struct Movie {
    /// 资源 ID。
    #[yaserde(attribute = true, rename = "ResourceID")]
    pub resource_id: StRefId,
    /// 操作。
    #[yaserde(
        attribute = true,
        rename = "Operator",
        default = "default_movie_operator"
    )]
    pub operator: MovieOperator,
}

/// 动作区域。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(rename = "Region")]
pub struct Region {
    /// 区域列表。
    #[yaserde(rename = "Area")]
    pub areas: Vec<RegionArea>,
}

/// 单个区域。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegionArea {
    /// 起点。
    pub start: StPos,
    /// 路径命令。
    pub commands: Vec<RegionCommand>,
}

impl YaSerialize for RegionArea {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        writer
            .write(
                ofdbox_yaserde::__xml::writer::XmlEvent::start_element(
                    crate::model_xml::ofd_name(writer, "Area").as_str(),
                )
                .attr("Start", &self.start.to_string()),
            )
            .map_err(ofdbox_yaserde::Error::custom)?;
        for command in &self.commands {
            command.serialize(writer)?;
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

impl YaDeserialize for RegionArea {
    fn deserialize<R: Read>(reader: &mut Deserializer<R>) -> ofdbox_yaserde::Result<Self> {
        let event = reader.next_event()?;
        let ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { attributes, .. } = event else {
            return Err(ofdbox_yaserde::Error::custom("Region Area 缺少起始元素"));
        };
        let start = attributes
            .iter()
            .find(|attribute| attribute.name.local_name == "Start")
            .map(|attribute| attribute.value.parse::<StPos>())
            .transpose()
            .map_err(ofdbox_yaserde::Error::custom)?
            .unwrap_or_default();

        let mut commands = Vec::new();
        loop {
            let event = reader.peek()?.to_owned();
            match event {
                ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { .. } => {
                    commands.push(RegionCommand::deserialize(reader)?);
                }
                ofdbox_yaserde::__xml::reader::XmlEvent::EndElement { name } => {
                    if name.local_name == "Area" {
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
        Ok(Self { start, commands })
    }
}

/// 区域路径命令。
#[derive(Clone, Debug, PartialEq)]
pub enum RegionCommand {
    /// 移动到。
    Move {
        /// 点。
        point1: StPos,
    },
    /// 直线到。
    Line {
        /// 点。
        point1: StPos,
    },
    /// 二次贝塞尔曲线。
    QuadraticBezier {
        /// 控制点。
        point1: StPos,
        /// 终点。
        point2: StPos,
    },
    /// 三次贝塞尔曲线。
    CubicBezier {
        /// 控制点 1。
        point1: Option<StPos>,
        /// 控制点 2。
        point2: Option<StPos>,
        /// 终点。
        point3: StPos,
    },
    /// 圆弧。
    Arc {
        /// 扫描方向。
        sweep_direction: bool,
        /// 是否大弧。
        large_arc: bool,
        /// 旋转角。
        rotation_angle: f64,
        /// 椭圆尺寸。
        ellipse_size: StArray,
        /// 终点。
        end_point: StPos,
    },
    /// 闭合路径。
    Close,
}

impl YaSerialize for RegionCommand {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let (tag, attrs): (&str, Vec<(String, String)>) = match self {
            Self::Move { point1 } => ("Move", vec![("Point1".into(), point1.to_string())]),
            Self::Line { point1 } => ("Line", vec![("Point1".into(), point1.to_string())]),
            Self::QuadraticBezier { point1, point2 } => (
                "QuadraticBezier",
                vec![
                    ("Point1".into(), point1.to_string()),
                    ("Point2".into(), point2.to_string()),
                ],
            ),
            Self::CubicBezier {
                point1,
                point2,
                point3,
            } => {
                let mut attrs = Vec::new();
                if let Some(point1) = point1 {
                    attrs.push(("Point1".into(), point1.to_string()));
                }
                if let Some(point2) = point2 {
                    attrs.push(("Point2".into(), point2.to_string()));
                }
                attrs.push(("Point3".into(), point3.to_string()));
                ("CubicBezier", attrs)
            }
            Self::Arc {
                sweep_direction,
                large_arc,
                rotation_angle,
                ellipse_size,
                end_point,
            } => (
                "Arc",
                vec![
                    ("SweepDirection".into(), sweep_direction.to_string()),
                    ("LargeArc".into(), large_arc.to_string()),
                    ("RotationAngle".into(), rotation_angle.to_string()),
                    ("EllipseSize".into(), ellipse_size.to_string()),
                    ("EndPoint".into(), end_point.to_string()),
                ],
            ),
            Self::Close => ("Close", Vec::new()),
        };

        let element_name = crate::model_xml::ofd_name(writer, tag);
        let mut event =
            ofdbox_yaserde::__xml::writer::XmlEvent::start_element(element_name.as_str());
        for (key, value) in &attrs {
            event = event.attr(key.as_str(), value.as_str());
        }
        writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
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

impl YaDeserialize for RegionCommand {
    fn deserialize<R: Read>(reader: &mut Deserializer<R>) -> ofdbox_yaserde::Result<Self> {
        let event = reader.next_event()?;
        let ofdbox_yaserde::__xml::reader::XmlEvent::StartElement {
            name, attributes, ..
        } = event
        else {
            return Err(ofdbox_yaserde::Error::custom("RegionCommand 缺少起始元素"));
        };
        let value = |key: &str| {
            attributes
                .iter()
                .find(|attribute| attribute.name.local_name == key)
                .map(|attribute| attribute.value.clone())
        };
        let position = |key: &str| -> ofdbox_yaserde::Result<Option<StPos>> {
            value(key)
                .map(|value| {
                    value
                        .parse::<StPos>()
                        .map_err(ofdbox_yaserde::Error::custom)
                })
                .transpose()
        };
        let command = match name.local_name.as_str() {
            "Move" => Self::Move {
                point1: position("Point1")?.unwrap_or_default(),
            },
            "Line" => Self::Line {
                point1: position("Point1")?.unwrap_or_default(),
            },
            "QuadraticBezier" => Self::QuadraticBezier {
                point1: position("Point1")?.unwrap_or_default(),
                point2: position("Point2")?.unwrap_or_default(),
            },
            "CubicBezier" => Self::CubicBezier {
                point1: position("Point1")?,
                point2: position("Point2")?,
                point3: position("Point3")?.unwrap_or_default(),
            },
            "Arc" => Self::Arc {
                sweep_direction: value("SweepDirection")
                    .map(|value| value == "true")
                    .unwrap_or(false),
                large_arc: value("LargeArc")
                    .map(|value| value == "true")
                    .unwrap_or(false),
                rotation_angle: value("RotationAngle")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_default(),
                ellipse_size: value("EllipseSize")
                    .map(|value| value.parse().map_err(ofdbox_yaserde::Error::custom))
                    .transpose()?
                    .unwrap_or_default(),
                end_point: position("EndPoint")?.unwrap_or_default(),
            },
            "Close" => Self::Close,
            other => {
                return Err(ofdbox_yaserde::Error::custom(format!(
                    "未知 RegionCommand: {other}"
                )));
            }
        };
        let _ = reader.next_event()?;
        Ok(command)
    }
}

fn default_zero_f64() -> f64 {
    0.0
}

fn default_volume() -> i32 {
    100
}

fn default_false() -> bool {
    false
}

fn default_movie_operator() -> MovieOperator {
    MovieOperator::Play
}

fn default_st_box() -> StBox {
    StBox::default()
}
