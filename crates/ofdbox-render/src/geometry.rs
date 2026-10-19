/// 平面坐标点。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// 横坐标。
    pub x: f64,
    /// 纵坐标。
    pub y: f64,
}

impl Point {
    /// 创建坐标点。
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// 尺寸。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    /// 宽度。
    pub width: f64,
    /// 高度。
    pub height: f64,
}

impl Size {
    /// 创建尺寸。
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

/// 轴对齐矩形。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// 左上角横坐标。
    pub x: f64,
    /// 左上角纵坐标。
    pub y: f64,
    /// 宽度。
    pub width: f64,
    /// 高度。
    pub height: f64,
}

impl Rect {
    /// 创建矩形。
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 转换为路径。
    pub fn to_path(self) -> crate::path::Path {
        let mut path = crate::path::Path::new();
        path.move_to(Point::new(self.x, self.y));
        path.line_to(Point::new(self.x + self.width, self.y));
        path.line_to(Point::new(self.x + self.width, self.y + self.height));
        path.line_to(Point::new(self.x, self.y + self.height));
        path.close();
        path
    }
}

/// 仿射变换矩阵。
///
/// 与 PDF/OFD 的六元组一致：
/// `x' = a*x + c*y + e`，`y' = b*x + d*y + f`。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix {
    /// 横向缩放。
    pub a: f64,
    /// 纵向斜切。
    pub b: f64,
    /// 横向斜切。
    pub c: f64,
    /// 纵向缩放。
    pub d: f64,
    /// 横向平移。
    pub e: f64,
    /// 纵向平移。
    pub f: f64,
}

impl Matrix {
    /// 单位矩阵。
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    /// 创建矩阵。
    pub const fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
        Self { a, b, c, d, e, f }
    }

    /// 平移矩阵。
    pub const fn translate(x: f64, y: f64) -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, x, y)
    }

    /// 缩放矩阵。
    pub const fn scale(x: f64, y: f64) -> Self {
        Self::new(x, 0.0, 0.0, y, 0.0, 0.0)
    }

    /// 组合变换：先应用 `self`，再应用 `other`。
    pub fn then(self, other: Self) -> Self {
        Self {
            a: other.a * self.a + other.c * self.b,
            b: other.b * self.a + other.d * self.b,
            c: other.a * self.c + other.c * self.d,
            d: other.b * self.c + other.d * self.d,
            e: other.a * self.e + other.c * self.f + other.e,
            f: other.b * self.e + other.d * self.f + other.f,
        }
    }

    /// 变换坐标点。
    pub fn apply(self, point: Point) -> Point {
        Point::new(
            self.a * point.x + self.c * point.y + self.e,
            self.b * point.x + self.d * point.y + self.f,
        )
    }

    /// 求逆矩阵。
    pub fn invert(self) -> Option<Self> {
        let determinant = self.a * self.d - self.b * self.c;
        if determinant.abs() < f64::EPSILON {
            return None;
        }
        let inverse = 1.0 / determinant;
        Some(Self {
            a: self.d * inverse,
            b: -self.b * inverse,
            c: -self.c * inverse,
            d: self.a * inverse,
            e: (self.c * self.f - self.d * self.e) * inverse,
            f: (self.b * self.e - self.a * self.f) * inverse,
        })
    }
}

impl Default for Matrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}
