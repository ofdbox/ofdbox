use crate::geometry::Point;

/// 非预乘 RGBA 颜色，分量范围为 0.0–1.0。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    /// 红。
    pub r: f32,
    /// 绿。
    pub g: f32,
    /// 蓝。
    pub b: f32,
    /// 透明度。
    pub a: f32,
}

impl Rgba {
    /// 黑色。
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0, 1.0);
    /// 白色。
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0, 1.0);
    /// 透明。
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    /// 创建颜色。
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// 从 8 位分量创建颜色。
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            f32::from(a) / 255.0,
        )
    }

    /// 返回 8 位非预乘分量。
    pub fn to_rgba8(self) -> [u8; 4] {
        [
            (self.r.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.g.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.b.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.a.clamp(0.0, 1.0) * 255.0).round() as u8,
        ]
    }
}

/// 渐变颜色节点。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    /// 归一化位置。
    pub offset: f32,
    /// 颜色。
    pub color: Rgba,
}

/// 渐变延展方式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GradientExtend {
    /// 两端保持端点颜色。
    #[default]
    Pad,
    /// 重复。
    Repeat,
    /// 反射。
    Reflect,
}

/// 轴向渐变。
#[derive(Clone, Debug, PartialEq)]
pub struct LinearGradient {
    /// 起点。
    pub start: Point,
    /// 终点。
    pub end: Point,
    /// 颜色节点。
    pub stops: Vec<GradientStop>,
    /// 延展方式。
    pub extend: GradientExtend,
}

/// 径向渐变。
#[derive(Clone, Debug, PartialEq)]
pub struct RadialGradient {
    /// 起始圆心。
    pub start_center: Point,
    /// 起始半径。
    pub start_radius: f64,
    /// 结束圆心。
    pub end_center: Point,
    /// 结束半径。
    pub end_radius: f64,
    /// 颜色节点。
    pub stops: Vec<GradientStop>,
    /// 延展方式。
    pub extend: GradientExtend,
}

/// 填充或描边使用的画笔。
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    /// 纯色。
    Solid(Rgba),
    /// 轴向渐变。
    Linear(LinearGradient),
    /// 径向渐变。
    Radial(RadialGradient),
}
