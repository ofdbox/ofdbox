#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! OFD 页面渲染。
//!
//! 前端把 OFD（后续还有 PDF）转换成与源格式无关的 [`Scene`]，资源经
//! [`ResourcePool`] 跨文档共享，后端只借用调用方的图形设备完成绘制。
//!
//! 当前实现包含 OFD 前端和 `vello_cpu` 位图后端；PDF 前端、wgpu 后端和
//! PDF 后端仅保留类型占位。

mod asset;
mod backend;
mod error;
mod font;
mod frontend;
mod geometry;
mod image;
mod options;
mod paint;
mod path;
mod resource;
mod scene;

pub use asset::{AssetId, FontAsset, ImageAsset};
pub use backend::pdf::PdfBackend;
pub use backend::vello_cpu::{RasterTarget, VelloCpuBackend, VelloCpuFrame};
pub use backend::wgpu::WgpuBackend;
pub use backend::{Backend, BackendCapabilities, BackendFrame, BackendInfo};
pub use error::RenderError;
pub use font::{FontProvider, FontRequest};
pub use frontend::ofd::OfdFrontend;
pub use frontend::pdf::PdfFrontend;
pub use geometry::{Matrix, Point, Rect, Size};
pub use image::{DecodedImage, ImageFormat};
pub use options::{RenderOptions, UnsupportedPolicy};
pub use paint::{GradientExtend, GradientStop, LinearGradient, Paint, RadialGradient, Rgba};
pub use path::{FillRule, LineCap, LineJoin, Path, PathSegment, StrokeStyle};
pub use resource::{
    ContentDigest, FontKey, ImageKey, PoolBudget, PoolStats, ResourcePool, SceneKey,
};
pub use scene::{
    Clip, DiagnosticCode, Element, Group, ImageElement, PathElement, PositionedGlyph,
    RenderDiagnostic, Scene, StrokePaint, TextElement,
};
