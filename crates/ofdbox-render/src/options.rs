use crate::paint::Rgba;

/// 场景构建选项。
#[derive(Clone, Debug)]
pub struct RenderOptions {
    /// 位图后端的目标分辨率，单位为 DPI。
    pub dpi: f64,
    /// 页面背景色。
    pub background: Rgba,
    /// 是否绘制页面注释外观。
    pub draw_annotations: bool,
    /// 遇到不支持的图元时的行为。
    pub unsupported: UnsupportedPolicy,
    /// 单张图片允许的最大像素数。
    pub max_image_pixels: u64,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            dpi: 96.0,
            background: Rgba::WHITE,
            draw_annotations: true,
            unsupported: UnsupportedPolicy::Skip,
            max_image_pixels: 64 * 1024 * 1024,
        }
    }
}

/// 不支持特性的处理策略。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedPolicy {
    /// 跳过该图元并记录诊断。
    Skip,
    /// 返回错误并终止场景构建。
    Error,
}
