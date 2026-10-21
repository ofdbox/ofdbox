//! wgpu 后端（W0：context、target、compute 实色路径）。

#[cfg(not(feature = "wgpu"))]
mod disabled;
#[cfg(feature = "wgpu")]
mod enabled;

#[cfg(not(feature = "wgpu"))]
pub use disabled::WgpuBackend;
#[cfg(feature = "wgpu")]
pub use enabled::{WgpuBackend, WgpuContext, WgpuTarget};
