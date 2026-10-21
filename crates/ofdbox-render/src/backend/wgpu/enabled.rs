//! wgpu compute 后端，W0 支持实色填充路径。

use std::borrow::Cow;
use std::sync::Arc;

use wgpu::util::DeviceExt;

use crate::error::RenderError;
use crate::geometry::{Matrix, Point, Size};
use crate::paint::Paint;
use crate::path::{FillRule, PathSegment};
use crate::scene::{Element, Group, Scene};

#[derive(Clone, Copy)]
struct DrawPath {
    start: u32,
    count: u32,
    rule: u32,
    kind: u32,
    color: [f32; 4],
    width: f32,
    clip_start: u32,
    clip_count: u32,
    clip_rule: u32,
}

/// wgpu 设备上下文。
pub struct WgpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl WgpuContext {
    /// 创建默认适配器和设备。
    pub async fn new() -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|error| RenderError::Backend(format!("wgpu adapter: {error}")))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("ofdbox-wgpu"),
                ..Default::default()
            })
            .await
            .map_err(|error| RenderError::Backend(format!("wgpu device: {error}")))?;
        Ok(Self { device, queue })
    }

    /// 阻塞创建上下文。
    pub fn new_blocking() -> Result<Self, RenderError> {
        pollster::block_on(Self::new())
    }

    /// 设备引用。
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// 队列引用。
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}

/// wgpu 渲染目标。
pub struct WgpuTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    dpi: f64,
}

impl WgpuTarget {
    /// 按 Scene 尺寸创建离屏目标。
    pub fn new(context: &WgpuContext, size: Size, dpi: f64) -> Result<Self, RenderError> {
        let width = (size.width / 25.4 * dpi).ceil().max(1.0) as u32;
        let height = (size.height / 25.4 * dpi).ceil().max(1.0) as u32;
        let texture = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ofdbox-wgpu-target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(Self {
            texture,
            view,
            width,
            height,
            dpi,
        })
    }

    /// 宽度（像素）。
    pub fn width(&self) -> u32 {
        self.width
    }

    /// 高度（像素）。
    pub fn height(&self) -> u32 {
        self.height
    }

    /// 读回 RGBA8 像素。
    pub fn read_rgba(&self, context: &WgpuContext) -> Result<Vec<u8>, RenderError> {
        let row = self.width * 4;
        let padded =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ofdbox-wgpu-readback"),
            size: u64::from(padded) * u64::from(self.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = context.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        context.queue.submit([encoder.finish()]);

        let (sender, receiver) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        context
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| RenderError::Backend(format!("wgpu poll: {error}")))?;
        receiver
            .recv()
            .map_err(|_| RenderError::Backend("wgpu readback channel".to_owned()))?
            .map_err(|error| RenderError::Backend(format!("wgpu readback: {error}")))?;
        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|error| RenderError::Backend(format!("wgpu map range: {error}")))?;
        let mut output = Vec::with_capacity((self.width * self.height * 4) as usize);
        for row in mapped.chunks(padded as usize) {
            output.extend_from_slice(&row[..(self.width * 4) as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(output)
    }
}

/// wgpu compute 后端。
pub struct WgpuBackend {
    context: Arc<WgpuContext>,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

impl WgpuBackend {
    /// 创建后端；需要显式提供已初始化的 context。
    pub fn new(context: WgpuContext) -> Result<Self, RenderError> {
        Self::from_shared(Arc::new(context))
    }

    /// 从共享 context 创建。
    pub fn from_shared(context: Arc<WgpuContext>) -> Result<Self, RenderError> {
        let module = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("ofdbox-wgpu-shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
            });
        let layout = context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ofdbox-wgpu-bind-layout"),
                entries: &[
                    storage_entry(0, true),
                    storage_entry(1, true),
                    storage_entry(2, true),
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("ofdbox-wgpu-pipeline-layout"),
                    bind_group_layouts: &[Some(&layout)],
                    immediate_size: 0,
                });
        let pipeline = context
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("ofdbox-wgpu-pipeline"),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        Ok(Self {
            context,
            pipeline,
            layout,
        })
    }

    /// 渲染到离屏纹理。
    pub fn render_to_target(&self, scene: &Scene, target: &WgpuTarget) -> Result<(), RenderError> {
        let (edges, paths) = collect_paths(scene, target.dpi);
        if paths.is_empty() {
            return Ok(());
        }
        let mut edge_data = Vec::with_capacity(edges.len() * 16);
        for edge in &edges {
            for value in edge {
                edge_data.extend_from_slice(&value.to_ne_bytes());
            }
        }
        let mut path_data = Vec::with_capacity(paths.len() * 48);
        for path in &paths {
            for value in [path.start, path.count, path.rule, path.kind] {
                path_data.extend_from_slice(&value.to_ne_bytes());
            }
            for value in path.color {
                path_data.extend_from_slice(&value.to_ne_bytes());
            }
            path_data.extend_from_slice(&path.width.to_ne_bytes());
            for value in [path.clip_start, path.clip_count, path.clip_rule] {
                path_data.extend_from_slice(&value.to_ne_bytes());
            }
        }
        let edge_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ofdbox-edge-buffer"),
                    contents: &edge_data,
                    usage: wgpu::BufferUsages::STORAGE,
                });
        let path_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ofdbox-path-buffer"),
                    contents: &path_data,
                    usage: wgpu::BufferUsages::STORAGE,
                });
        let params = [
            target.width,
            target.height,
            u32::try_from(paths.len())
                .map_err(|_| RenderError::Backend("too many paths".into()))?,
            ((target.dpi / 25.4) as f32).to_bits(),
        ];
        let mut params_bytes = Vec::with_capacity(16);
        for value in params {
            params_bytes.extend_from_slice(&value.to_ne_bytes());
        }
        let params_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ofdbox-params"),
                    contents: &params_bytes,
                    usage: wgpu::BufferUsages::STORAGE,
                });
        let bind_group = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ofdbox-wgpu-bind-group"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: edge_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: path_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&target.view),
                    },
                ],
            });
        let mut encoder = self
            .context
            .device
            .create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(target.width.div_ceil(8), target.height.div_ceil(8), 1);
        }
        self.context.queue.submit([encoder.finish()]);
        self.context
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| RenderError::Backend(format!("wgpu poll: {error}")))?;
        Ok(())
    }
}

impl WgpuBackend {
    /// 返回共享的 wgpu 上下文。
    pub fn context(&self) -> &WgpuContext {
        &self.context
    }
}

impl std::fmt::Debug for WgpuBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuBackend").finish_non_exhaustive()
    }
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn collect_paths(scene: &Scene, dpi: f64) -> (Vec<[f32; 4]>, Vec<DrawPath>) {
    let scale = dpi / 25.4;
    let mut edges = Vec::new();
    let mut paths = Vec::new();
    collect_group(
        &scene.root,
        Matrix::IDENTITY,
        scale,
        None,
        &mut edges,
        &mut paths,
    );
    (edges, paths)
}

fn collect_group(
    group: &Group,
    parent: Matrix,
    scale: f64,
    inherited_clip: Option<(u32, u32, u32)>,
    edges: &mut Vec<[f32; 4]>,
    paths: &mut Vec<DrawPath>,
) {
    let transform = group.transform.then(parent);
    let clip = if let Some(clip) = &group.clip {
        let start = u32::try_from(edges.len()).unwrap_or(u32::MAX);
        flatten_path(&clip.path, transform, scale, edges);
        let count = u32::try_from(edges.len()).unwrap_or(u32::MAX) - start;
        Some((start, count, u32::from(clip.rule == FillRule::EvenOdd)))
    } else {
        inherited_clip
    };
    for element in &group.children {
        match element {
            Element::Group(child) => collect_group(child, transform, scale, clip, edges, paths),
            Element::Path(path) => {
                let start = u32::try_from(edges.len()).unwrap_or(u32::MAX);
                flatten_path(&path.path, transform, scale, edges);
                let count = u32::try_from(edges.len()).unwrap_or(u32::MAX) - start;
                if count == 0 {
                    continue;
                }
                if let Some(Paint::Solid(color)) = path.fill.as_ref() {
                    paths.push(DrawPath {
                        start,
                        count,
                        rule: u32::from(path.fill_rule == FillRule::EvenOdd),
                        kind: 0,
                        color: [color.r, color.g, color.b, color.a],
                        width: 0.0,
                        clip_start: clip.map_or(0, |value| value.0),
                        clip_count: clip.map_or(0, |value| value.1),
                        clip_rule: clip.map_or(0, |value| value.2),
                    });
                }
                if let Some(stroke) = &path.stroke
                    && let Paint::Solid(color) = &stroke.paint
                {
                    paths.push(DrawPath {
                        start,
                        count,
                        rule: 0,
                        kind: 1,
                        color: [color.r, color.g, color.b, color.a],
                        width: (stroke.style.width * scale) as f32,
                        clip_start: clip.map_or(0, |value| value.0),
                        clip_count: clip.map_or(0, |value| value.1),
                        clip_rule: clip.map_or(0, |value| value.2),
                    });
                }
            }
            Element::Text(_) | Element::Image(_) => {}
        }
    }
}

fn flatten_path(
    path: &crate::path::Path,
    transform: Matrix,
    scale: f64,
    edges: &mut Vec<[f32; 4]>,
) {
    let mut current = Point::default();
    let mut start = Point::default();
    let push = |edges: &mut Vec<[f32; 4]>, from: Point, to: Point| {
        let from = transform.apply(from);
        let to = transform.apply(to);
        edges.push([
            (from.x * scale) as f32,
            (from.y * scale) as f32,
            (to.x * scale) as f32,
            (to.y * scale) as f32,
        ]);
    };
    for segment in &path.segments {
        match segment {
            PathSegment::MoveTo(point) => {
                current = *point;
                start = *point;
            }
            PathSegment::LineTo(point) => {
                push(edges, current, *point);
                current = *point;
            }
            PathSegment::QuadraticTo { control, to } => {
                let from = current;
                for index in 1..=8 {
                    let t = f64::from(index) / 8.0;
                    let point = quadratic(from, *control, *to, t);
                    push(edges, current, point);
                    current = point;
                }
            }
            PathSegment::CubicTo {
                control1,
                control2,
                to,
            } => {
                let from = current;
                for index in 1..=16 {
                    let t = f64::from(index) / 16.0;
                    let point = cubic(from, *control1, *control2, *to, t);
                    push(edges, current, point);
                    current = point;
                }
            }
            PathSegment::ArcTo { to, .. } => {
                push(edges, current, *to);
                current = *to;
            }
            PathSegment::Close => {
                push(edges, current, start);
                current = start;
            }
        }
    }
}

fn quadratic(from: Point, control: Point, to: Point, t: f64) -> Point {
    let mt = 1.0 - t;
    Point::new(
        mt * mt * from.x + 2.0 * mt * t * control.x + t * t * to.x,
        mt * mt * from.y + 2.0 * mt * t * control.y + t * t * to.y,
    )
}

fn cubic(from: Point, c1: Point, c2: Point, to: Point, t: f64) -> Point {
    let mt = 1.0 - t;
    Point::new(
        mt * mt * mt * from.x
            + 3.0 * mt * mt * t * c1.x
            + 3.0 * mt * t * t * c2.x
            + t * t * t * to.x,
        mt * mt * mt * from.y
            + 3.0 * mt * mt * t * c1.y
            + 3.0 * mt * t * t * c2.y
            + t * t * t * to.y,
    )
}

const SHADER: &str = r#"
struct Edge { a: vec2f, b: vec2f };
struct PathInfo { start: u32, count: u32, rule: u32, kind: u32, color: vec4f, width: f32, clip_start: u32, clip_count: u32, clip_rule: u32 };
struct Params { width: u32, height: u32, count: u32, scale: f32 };

@group(0) @binding(0) var<storage, read> edges: array<Edge>;
@group(0) @binding(1) var<storage, read> paths: array<PathInfo>;
@group(0) @binding(2) var<storage, read> params: Params;
@group(0) @binding(3) var output: texture_storage_2d<rgba8unorm, write>;

fn inside_range(p: vec2f, start: u32, count: u32, rule: u32) -> bool {
    var winding: i32 = 0;
    for (var ei = 0u; ei < count; ei = ei + 1u) {
        let e = edges[start + ei];
        let ay = e.a.y;
        let by = e.b.y;
        if ((ay <= p.y && by > p.y) || (by <= p.y && ay > p.y)) {
            let t = (p.y - ay) / (by - ay);
            let x = e.a.x + t * (e.b.x - e.a.x);
            if (x > p.x) {
                if (by > ay) { winding = winding + 1; } else { winding = winding - 1; }
            }
        }
    }
    if (rule != 0u) { return (winding & 1) != 0; }
    return winding != 0;
}

fn inside_fill(p: vec2f, info: PathInfo) -> bool {
    return inside_range(p, info.start, info.count, info.rule);
}

fn inside_clip(p: vec2f, info: PathInfo) -> bool {
    if (info.clip_count == 0u) { return true; }
    return inside_range(p, info.clip_start, info.clip_count, info.clip_rule);
}

fn stroke_coverage(p: vec2f, info: PathInfo) -> f32 {
    var coverage = 0.0;
    let half_width = max(info.width * 0.5, 0.5);
    for (var ei = 0u; ei < info.count; ei = ei + 1u) {
        let e = edges[info.start + ei];
        let ab = e.b - e.a;
        let length_sq = max(dot(ab, ab), 0.0001);
        let t = clamp(dot(p - e.a, ab) / length_sq, 0.0, 1.0);
        let nearest = e.a + ab * t;
        let distance = length(p - nearest);
        coverage = max(coverage, clamp(half_width + 0.75 - distance, 0.0, 1.0));
    }
    return coverage;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= params.width || id.y >= params.height) { return; }
    var result = vec4f(1.0, 1.0, 1.0, 1.0);
    for (var pi = 0u; pi < params.count; pi = pi + 1u) {
        let info = paths[pi];
        var coverage = 0.0;
        if (info.kind == 0u) {
            let offsets = array<vec2f, 4>(vec2f(0.25, 0.25), vec2f(0.75, 0.25), vec2f(0.25, 0.75), vec2f(0.75, 0.75));
            for (var si = 0u; si < 4u; si = si + 1u) {
                let p = vec2f(f32(id.x), f32(id.y)) + offsets[si];
                if (inside_fill(p, info) && inside_clip(p, info)) { coverage = coverage + 0.25; }
            }
        } else {
            let center = vec2f(f32(id.x) + 0.5, f32(id.y) + 0.5);
            if (inside_clip(center, info)) {
                coverage = stroke_coverage(center, info);
            }
        }
        if (coverage > 0.0) {
            let a = info.color.a * coverage;
            result = vec4f(info.color.rgb * a + result.rgb * (1.0 - a), 1.0);
        }
    }
    textureStore(output, vec2i(i32(id.x), i32(id.y)), result);
}
"#;

#[cfg(test)]
mod tests {
    use super::{WgpuBackend, WgpuContext, WgpuTarget};
    use crate::geometry::{Point, Size};
    use crate::paint::{Paint, Rgba};
    use crate::path::{FillRule, Path};
    use crate::scene::{Element, Group, PathElement, Scene};

    #[test]
    fn renders_solid_rectangle() {
        let Ok(context) = WgpuContext::new_blocking() else {
            return;
        };
        let mut path = Path::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(50.0, 10.0));
        path.line_to(Point::new(50.0, 50.0));
        path.line_to(Point::new(10.0, 50.0));
        path.close();
        let mut root = Group::default();
        root.children.push(Element::Path(PathElement {
            path: std::sync::Arc::new(path),
            fill: Some(Paint::Solid(Rgba::new(1.0, 0.0, 0.0, 1.0))),
            stroke: None,
            fill_rule: FillRule::NonZero,
        }));
        let scene = Scene {
            size: Size::new(60.0, 60.0),
            root,
            diagnostics: Vec::new(),
        };
        let backend = WgpuBackend::new(context).expect("backend");
        let target = WgpuTarget::new(backend.context(), scene.size, 96.0).expect("target");
        backend.render_to_target(&scene, &target).expect("render");
        let pixels = target.read_rgba(backend.context()).expect("read");
        let x = (30.0 / 25.4 * 96.0) as u32;
        let y = (30.0 / 25.4 * 96.0) as u32;
        let index = ((y * target.width() + x) * 4) as usize;
        assert!(pixels[index] > 200);
        assert!(pixels[index + 1] < 30);
    }
}
