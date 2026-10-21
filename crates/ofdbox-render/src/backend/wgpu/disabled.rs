//! 未启用 `wgpu` feature 时的占位实现。

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

    /// 未启用 wgpu 时始终返回不支持。
    pub fn draw(&mut self, _scene: &Scene) -> Result<(), RenderError> {
        Err(RenderError::Unsupported(
            "wgpu 后端未启用，请开启 render/wgpu feature".to_owned(),
        ))
    }
}
