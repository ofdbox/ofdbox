//! wgpu 后端占位。
//!
//! 设计上由调用方提供 `&wgpu::Device`、`&wgpu::Queue` 和 `&wgpu::TextureView`，
//! 后端只在帧内借用；当前尚未实现。

use crate::error::RenderError;
use crate::scene::Scene;

/// wgpu 后端占位类型。
#[derive(Debug, Default)]
pub struct WgpuBackend;

impl WgpuBackend {
    /// 创建占位后端。
    pub fn new() -> Self {
        Self
    }

    /// 尚未实现。
    pub fn draw(&mut self, _scene: &Scene) -> Result<(), RenderError> {
        Err(RenderError::Unsupported("wgpu 后端尚未实现".to_owned()))
    }
}
