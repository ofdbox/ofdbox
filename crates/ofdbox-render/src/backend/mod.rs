use crate::scene::Scene;

/// 后端能力描述。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BackendCapabilities {
    /// 是否支持网格渐变。
    pub mesh_gradients: bool,
    /// 是否支持独立图像蒙版。
    pub image_mask: bool,
}

/// 后端信息。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackendInfo {
    /// 后端名称。
    pub name: &'static str,
    /// 能力。
    pub capabilities: BackendCapabilities,
}

/// 一帧的绘制入口。
pub trait BackendFrame {
    /// 绘制完整场景。
    fn draw(&mut self, scene: &Scene) -> Result<(), crate::error::RenderError>;

    /// 提交本帧。
    fn finish(self) -> Result<(), crate::error::RenderError>;
}

/// 图形后端。
///
/// `Device` 和 `Target` 都由调用方拥有，后端只在 `Frame` 生命周期内借用。
/// 这与 GUI 框架管理设备/队列/纹理的模型一致。
pub trait Backend {
    /// 每帧借用的图形设备。
    type Device<'a>
    where
        Self: 'a;
    /// 每帧借用的渲染目标。
    type Target<'a>
    where
        Self: 'a;
    /// 一帧的绘制对象。
    type Frame<'a>: BackendFrame
    where
        Self: 'a;

    /// 后端信息。
    fn info(&self) -> BackendInfo;

    /// 图形设备重建后清空后端持有的资源。
    fn reset(&mut self);

    /// 开始一帧。
    fn begin_frame<'a>(
        &'a mut self,
        device: Self::Device<'a>,
        target: Self::Target<'a>,
    ) -> Result<Self::Frame<'a>, crate::error::RenderError>;
}

#[cfg(feature = "ofd")]
pub mod ofd;
#[cfg(feature = "pdf")]
pub mod pdf;
#[cfg(feature = "cpu")]
pub mod vello_cpu;
pub mod wgpu;
