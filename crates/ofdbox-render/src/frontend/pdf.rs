//! PDF 场景前端占位。
//!
//! 后续将把 PDF 内容流转换为与 OFD 相同的 [`crate::Scene`]，
//! 只使用两种格式能力的交集。

use crate::error::RenderError;
use crate::scene::Scene;

/// PDF 前端占位类型。
#[derive(Debug, Default)]
pub struct PdfFrontend;

impl PdfFrontend {
    /// 创建占位前端。
    pub fn new() -> Self {
        Self
    }

    /// 尚未实现。
    pub fn build_scene(&mut self) -> Result<Scene, RenderError> {
        Err(RenderError::Unsupported("PDF 前端尚未实现".to_owned()))
    }
}
