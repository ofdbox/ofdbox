use crate::geometry::Point;

/// 路径片段。
#[derive(Clone, Debug, PartialEq)]
pub enum PathSegment {
    /// 移动到新子路径起点。
    MoveTo(Point),
    /// 直线到目标点。
    LineTo(Point),
    /// 二次贝塞尔曲线。
    QuadraticTo {
        /// 控制点。
        control: Point,
        /// 终点。
        to: Point,
    },
    /// 三次贝塞尔曲线。
    CubicTo {
        /// 第一控制点。
        control1: Point,
        /// 第二控制点。
        control2: Point,
        /// 终点。
        to: Point,
    },
    /// 椭圆弧。
    ArcTo {
        /// 横轴半径。
        rx: f64,
        /// 纵轴半径。
        ry: f64,
        /// 椭圆旋转角，单位弧度。
        rotation: f64,
        /// 是否使用大弧。
        large_arc: bool,
        /// 是否顺时针。
        sweep: bool,
        /// 终点。
        to: Point,
    },
    /// 闭合当前子路径。
    Close,
}

/// 可复用的路径。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    /// 路径片段。
    pub segments: Vec<PathSegment>,
}

impl Path {
    /// 创建空路径。
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加移动。
    pub fn move_to(&mut self, point: Point) {
        self.segments.push(PathSegment::MoveTo(point));
    }

    /// 添加直线。
    pub fn line_to(&mut self, point: Point) {
        self.segments.push(PathSegment::LineTo(point));
    }

    /// 添加二次曲线。
    pub fn quadratic_to(&mut self, control: Point, to: Point) {
        self.segments.push(PathSegment::QuadraticTo { control, to });
    }

    /// 添加三次曲线。
    pub fn cubic_to(&mut self, control1: Point, control2: Point, to: Point) {
        self.segments.push(PathSegment::CubicTo {
            control1,
            control2,
            to,
        });
    }

    /// 添加椭圆弧。
    #[allow(clippy::too_many_arguments)]
    pub fn arc_to(
        &mut self,
        rx: f64,
        ry: f64,
        rotation: f64,
        large_arc: bool,
        sweep: bool,
        to: Point,
    ) {
        self.segments.push(PathSegment::ArcTo {
            rx,
            ry,
            rotation,
            large_arc,
            sweep,
            to,
        });
    }

    /// 闭合子路径。
    pub fn close(&mut self) {
        self.segments.push(PathSegment::Close);
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

/// 填充规则。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FillRule {
    /// 非零绕数。
    #[default]
    NonZero,
    /// 奇偶规则。
    EvenOdd,
}

/// 线端样式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineCap {
    /// 平头。
    #[default]
    Butt,
    /// 圆头。
    Round,
    /// 方头。
    Square,
}

/// 线段连接样式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineJoin {
    /// 尖角。
    #[default]
    Miter,
    /// 圆角。
    Round,
    /// 斜角。
    Bevel,
}

/// 描边样式，长度单位为毫米。
#[derive(Clone, Debug, PartialEq)]
pub struct StrokeStyle {
    /// 线宽。
    pub width: f64,
    /// 线端。
    pub cap: LineCap,
    /// 连接。
    pub join: LineJoin,
    /// 尖角限制。
    pub miter_limit: f64,
    /// 虚线序列。
    pub dash: Vec<f64>,
    /// 虚线偏移。
    pub dash_offset: f64,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            width: 0.353,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 10.0,
            dash: Vec::new(),
            dash_offset: 0.0,
        }
    }
}
