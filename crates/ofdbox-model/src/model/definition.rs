//! `Definition.xsd` 中的公共简单类型和复杂类型。
//!
//! 该模块没有对应的文档根对象，只提供其他模块共享的类型。

/// 文档内对象 ID。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StRefId(pub u32);

impl StRefId {
    /// 创建资源引用 ID。
    pub fn new(value: u32) -> Self {
        Self(value)
    }
}

impl From<u32> for StRefId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

/// 包内相对路径。
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StArray(pub Vec<f64>);

impl StArray {
    /// 创建数值数组。
    pub fn new(values: Vec<f64>) -> Self {
        Self(values)
    }
}

/// 坐标点。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StPos {
    /// X 坐标。
    pub x: f64,
    /// Y 坐标。
    pub y: f64,
}

/// 矩形区域，对应 `x y width height`。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

/// 目标位置类型。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

/// 跳转目标。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dest {
    /// 目标类型。
    pub dest_type: DestType,
    /// 目标页面 ID。
    pub page_id: StRefId,
    /// 左坐标。
    pub left: Option<f64>,
    /// 上坐标。
    pub top: Option<f64>,
    /// 右坐标。
    pub right: Option<f64>,
    /// 下坐标。
    pub bottom: Option<f64>,
    /// 缩放。
    pub zoom: Option<f64>,
}

/// 页面区域。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageArea {
    /// 物理区域。
    pub physical_box: StBox,
    /// 应用区域。
    pub application_box: Option<StBox>,
    /// 内容区域。
    pub content_box: Option<StBox>,
    /// 出血区域。
    pub bleed_box: Option<StBox>,
}

/// 动作事件。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

/// 动作集合。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Actions {
    /// 动作列表。
    pub actions: Vec<Action>,
}

/// 一个动作。
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    /// 触发事件。
    pub event: ActionEvent,
    /// 动作区域。
    pub region: Option<Region>,
    /// 动作内容。
    pub kind: ActionKind,
}

/// 动作内容。
#[derive(Clone, Debug, PartialEq)]
pub enum ActionKind {
    /// 跳转。
    Goto(Goto),
    /// URI。
    Uri(Uri),
    /// 跳转附件。
    GotoA(GotoA),
    /// 声音。
    Sound(Sound),
    /// 影片。
    Movie(Movie),
}

/// 跳转动作。
#[derive(Clone, Debug, PartialEq)]
pub struct Goto {
    /// 跳转目标。
    pub kind: GotoKind,
}

/// 跳转目标类型。
#[derive(Clone, Debug, PartialEq)]
pub enum GotoKind {
    /// 目标位置。
    Dest(Dest),
    /// 书签名。
    Bookmark(String),
}

/// URI 动作。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Uri {
    /// URI。
    pub uri: String,
    /// 基准路径。
    pub base: Option<String>,
    /// 目标窗口。
    pub target: Option<String>,
}

/// 附件跳转动作。
#[derive(Clone, Debug, PartialEq)]
pub struct GotoA {
    /// 附件 ID。
    pub attach_id: String,
    /// 是否新窗口。
    pub new_window: bool,
}

/// 声音动作。
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    /// 资源 ID。
    pub resource_id: StRefId,
    /// 音量。
    pub volume: Option<i32>,
    /// 是否重复。
    pub repeat: Option<bool>,
    /// 是否同步。
    pub synchronous: Option<bool>,
}

/// 影片操作类型。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

/// 影片动作。
#[derive(Clone, Debug, PartialEq)]
pub struct Movie {
    /// 资源 ID。
    pub resource_id: StRefId,
    /// 操作。
    pub operator: MovieOperator,
}

/// 动作区域。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Region {
    /// 区域列表。
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
