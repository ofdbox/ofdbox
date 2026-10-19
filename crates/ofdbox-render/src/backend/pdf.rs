//! PDF 矢量后端占位。

use crate::error::RenderError;
use crate::scene::Scene;

/// PDF 后端占位类型。
#[derive(Debug, Default)]
pub struct PdfBackend;

impl PdfBackend {
    /// 创建占位后端。
    pub fn new() -> Self {
        Self
    }

    /// 尚未实现。
    pub fn render(&mut self, _scene: &Scene) -> Result<Vec<u8>, RenderError> {
        Err(RenderError::Unsupported("PDF 后端尚未实现".to_owned()))
    }
}
