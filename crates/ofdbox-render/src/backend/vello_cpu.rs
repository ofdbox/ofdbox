use std::collections::HashMap;
use std::sync::Arc;

use vello_cpu::kurbo::{
    Affine, Arc as KurboArc, BezPath, Point, Rect as KurboRect, Stroke, SvgArc, Vec2,
};
use vello_cpu::peniko::color::{AlphaColor, DynamicColor, Srgb};
use vello_cpu::peniko::{
    Blob, Brush, ColorStop, Extend, FontData, Gradient, ImageBrush, ImageSampler,
};
use vello_cpu::peniko::{Fill, ImageAlphaType};
use vello_cpu::{Glyph, ImageSource, PaintType, PixelMetadata, Pixmap, RenderContext, Resources};

use crate::asset::{AssetId, FontAsset};
use crate::backend::{Backend, BackendCapabilities, BackendFrame, BackendInfo};
use crate::error::RenderError;
use crate::geometry::{Matrix, Size};
use crate::image::DecodedImage;
use crate::options::RenderOptions;
use crate::paint::{GradientExtend, GradientStop, Paint, RadialGradient, Rgba};
use crate::path::{FillRule, LineCap, LineJoin, Path, PathSegment, StrokeStyle};
use crate::scene::{Element, Group, ImageElement, PathElement, Scene, TextElement};

/// RGBA8 像素目标。
#[derive(Clone, Debug)]
pub struct RasterTarget {
    width: u16,
    height: u16,
    dpi: f64,
    data: Vec<u8>,
}

impl RasterTarget {
    /// 创建指定像素尺寸的目标。
    pub fn new(width: u16, height: u16, dpi: f64) -> Self {
        Self {
            width,
            height,
            dpi,
            data: vec![0; usize::from(width) * usize::from(height) * 4],
        }
    }

    /// 按毫米尺寸和 DPI 创建目标。
    pub fn from_size(size: Size, dpi: f64) -> Result<Self, RenderError> {
        if !(dpi.is_finite() && dpi > 0.0)
            || !(size.width.is_finite() && size.width > 0.0)
            || !(size.height.is_finite() && size.height > 0.0)
        {
            return Err(RenderError::InvalidPage);
        }
        let width = (size.width * dpi / 25.4).ceil();
        let height = (size.height * dpi / 25.4).ceil();
        if width > f64::from(u16::MAX) || height > f64::from(u16::MAX) {
            return Err(RenderError::Backend("目标尺寸超出 u16 范围".to_owned()));
        }
        Ok(Self::new(width as u16, height as u16, dpi))
    }

    /// 宽度，像素。
    pub fn width(&self) -> u16 {
        self.width
    }

    /// 高度，像素。
    pub fn height(&self) -> u16 {
        self.height
    }

    /// 分辨率。
    pub fn dpi(&self) -> f64 {
        self.dpi
    }

    /// 非预乘 RGBA8 数据。
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// 取出像素数据。
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }
}

/// 基于 `vello_cpu` 的位图后端。
pub struct VelloCpuBackend {
    options: RenderOptions,
    resources: Resources,
    fonts: HashMap<AssetId, FontData>,
    pixmaps: HashMap<AssetId, Arc<Pixmap>>,
}

impl VelloCpuBackend {
    /// 创建后端。
    pub fn new(options: RenderOptions) -> Self {
        Self {
            options,
            resources: Resources::new(),
            fonts: HashMap::new(),
            pixmaps: HashMap::new(),
        }
    }

    /// 使用默认选项创建。
    pub fn with_defaults() -> Self {
        Self::new(RenderOptions::default())
    }

    fn font_data(&mut self, font: &FontAsset) -> Option<FontData> {
        if let Some(data) = self.fonts.get(&font.id()) {
            return Some(data.clone());
        }
        // glifo 对无法解析的字体会直接 panic，先按同一字体栈校验。
        if skrifa::FontRef::from_index(font.data(), font.face_index()).is_err() {
            return None;
        }
        let blob = Blob::new(Arc::new(font.data().to_vec()));
        let data = FontData::new(blob, font.face_index());
        self.fonts.insert(font.id(), data.clone());
        Some(data)
    }

    fn pixmap(&mut self, image: &ImageElement) -> Result<Arc<Pixmap>, RenderError> {
        if let Some(pixmap) = self.pixmaps.get(&image.image.id()) {
            return Ok(Arc::clone(pixmap));
        }
        let decoded = image.image.decoded(self.options.max_image_pixels)?;
        let pixmap = create_pixmap(&decoded)?;
        let pixmap = Arc::new(pixmap);
        self.pixmaps.insert(image.image.id(), Arc::clone(&pixmap));
        Ok(pixmap)
    }
}

impl Backend for VelloCpuBackend {
    type Device<'a> = ();
    type Target<'a> = &'a mut RasterTarget;
    type Frame<'a>
        = VelloCpuFrame<'a>
    where
        Self: 'a;

    fn info(&self) -> BackendInfo {
        BackendInfo {
            name: "vello_cpu",
            capabilities: BackendCapabilities::default(),
        }
    }

    fn reset(&mut self) {
        self.fonts.clear();
        self.pixmaps.clear();
        self.resources = Resources::new();
    }

    fn begin_frame<'a>(
        &'a mut self,
        (): (),
        target: &'a mut RasterTarget,
    ) -> Result<Self::Frame<'a>, RenderError> {
        let context = RenderContext::new(target.width, target.height);
        Ok(VelloCpuFrame {
            backend: self,
            target,
            context,
        })
    }
}

/// `vello_cpu` 的一帧。
pub struct VelloCpuFrame<'a> {
    backend: &'a mut VelloCpuBackend,
    target: &'a mut RasterTarget,
    context: RenderContext,
}

impl BackendFrame for VelloCpuFrame<'_> {
    fn draw(&mut self, scene: &Scene) -> Result<(), RenderError> {
        let background = self.backend.options.background;
        self.context.set_transform(Affine::IDENTITY);
        self.context.set_paint(color(background));
        self.context.fill_rect(&KurboRect::new(
            0.0,
            0.0,
            f64::from(self.target.width),
            f64::from(self.target.height),
        ));
        let device = Matrix::scale(self.target.dpi / 25.4, self.target.dpi / 25.4);
        self.draw_group(&scene.root, device)
    }

    fn finish(self) -> Result<(), RenderError> {
        let VelloCpuFrame {
            backend,
            target,
            mut context,
        } = self;
        context.flush();
        let mut pixmap = Pixmap::new(target.width, target.height);
        context.render(&mut pixmap, &mut backend.resources);
        for (pixel, source) in target
            .data
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(pixmap.data().iter().copied())
        {
            let alpha = source.a;
            if alpha == 0 {
                pixel.copy_from_slice(&[0, 0, 0, 0]);
            } else {
                let unpremultiply = |value: u8| {
                    u8::try_from((u32::from(value) * 255 + u32::from(alpha) / 2) / u32::from(alpha))
                        .unwrap_or(255)
                };
                pixel.copy_from_slice(&[
                    unpremultiply(source.r),
                    unpremultiply(source.g),
                    unpremultiply(source.b),
                    alpha,
                ]);
            }
        }
        Ok(())
    }
}

impl VelloCpuFrame<'_> {
    fn draw_group(&mut self, group: &Group, parent: Matrix) -> Result<(), RenderError> {
        let transform = group.transform.then(parent);
        self.context.set_transform(affine(transform));
        let layered = group.clip.is_some() || group.opacity < 1.0;
        if layered {
            let clip = group.clip.as_ref().map(|clip| to_bez_path(&clip.path));
            self.context.push_layer(
                clip.as_ref(),
                None,
                (group.opacity < 1.0).then_some(group.opacity),
                None,
                None,
            );
        }
        for element in &group.children {
            match element {
                Element::Group(child) => self.draw_group(child, transform)?,
                Element::Path(path) => self.draw_path(path)?,
                Element::Text(text) => self.draw_text(text, transform)?,
                Element::Image(image) => self.draw_image(image, transform)?,
            }
        }
        if layered {
            self.context.pop_layer();
        }
        Ok(())
    }

    fn draw_path(&mut self, element: &PathElement) -> Result<(), RenderError> {
        let path = to_bez_path(&element.path);
        if let Some(paint) = &element.fill {
            self.context.set_fill_rule(rule(element.fill_rule));
            self.context.set_paint(brush(paint));
            self.context.fill_path(&path);
        }
        if let Some(stroke) = &element.stroke {
            self.context.set_paint(brush(&stroke.paint));
            self.context.set_stroke(kurbo_stroke(&stroke.style));
            self.context.stroke_path(&path);
        }
        Ok(())
    }

    fn draw_text(&mut self, element: &TextElement, parent: Matrix) -> Result<(), RenderError> {
        if element.glyphs.is_empty() {
            return Ok(());
        }
        let transform = element.transform.then(parent);
        self.context.set_transform(affine(transform));
        let Some(font) = self.backend.font_data(&element.font) else {
            return Ok(());
        };
        let glyphs = element.glyphs.iter().map(|glyph| Glyph {
            id: glyph.id,
            x: glyph.x,
            y: glyph.y,
        });
        if let Some(fill) = &element.fill {
            self.context.set_paint(brush(fill));
            // 部分字形缺失时 vello 仍会绘制其余字形；渲染保持尽力而为。
            let _ = self
                .context
                .glyph_run(&mut self.backend.resources, &font)
                .font_size(element.size as f32)
                .fill_glyphs(glyphs.clone());
        }
        if let Some(stroke) = &element.stroke {
            self.context.set_paint(brush(&stroke.paint));
            self.context.set_stroke(kurbo_stroke(&stroke.style));
            let _ = self
                .context
                .glyph_run(&mut self.backend.resources, &font)
                .font_size(element.size as f32)
                .stroke_glyphs(glyphs);
        }
        Ok(())
    }

    fn draw_image(&mut self, element: &ImageElement, parent: Matrix) -> Result<(), RenderError> {
        let pixmap = self.backend.pixmap(element)?;
        let width = f64::from(pixmap.width());
        let height = f64::from(pixmap.height());
        let target = element.transform.then(parent);
        self.context.set_transform(affine(target));
        let image = ImageBrush {
            image: ImageSource::Pixmap(Arc::clone(&pixmap)),
            sampler: ImageSampler {
                alpha: element.alpha.clamp(0.0, 1.0),
                ..Default::default()
            },
        };
        self.context.set_paint(Brush::Image(image));
        self.context
            .fill_rect(&KurboRect::new(0.0, 0.0, width, height));
        Ok(())
    }
}

fn create_pixmap(decoded: &DecodedImage) -> Result<Pixmap, RenderError> {
    if decoded.width > u32::from(u16::MAX) || decoded.height > u32::from(u16::MAX) {
        return Err(RenderError::Image("图片尺寸超出 u16 范围".to_owned()));
    }
    Ok(Pixmap::from_parts(
        decoded.rgba.clone(),
        decoded.width as u16,
        decoded.height as u16,
        PixelMetadata::new(ImageAlphaType::Alpha, true),
    ))
}

fn affine(matrix: Matrix) -> Affine {
    Affine::new([matrix.a, matrix.b, matrix.c, matrix.d, matrix.e, matrix.f])
}

fn color(color: Rgba) -> AlphaColor<Srgb> {
    let [r, g, b, a] = color.to_rgba8();
    AlphaColor::from_rgba8(r, g, b, a)
}

fn rule(rule: FillRule) -> Fill {
    match rule {
        FillRule::NonZero => Fill::NonZero,
        FillRule::EvenOdd => Fill::EvenOdd,
    }
}

fn kurbo_stroke(style: &StrokeStyle) -> Stroke {
    let mut stroke = Stroke::new(style.width);
    stroke.miter_limit = style.miter_limit;
    stroke.start_cap = match style.cap {
        LineCap::Butt => vello_cpu::kurbo::Cap::Butt,
        LineCap::Round => vello_cpu::kurbo::Cap::Round,
        LineCap::Square => vello_cpu::kurbo::Cap::Square,
    };
    stroke.end_cap = stroke.start_cap;
    stroke.join = match style.join {
        LineJoin::Miter => vello_cpu::kurbo::Join::Miter,
        LineJoin::Round => vello_cpu::kurbo::Join::Round,
        LineJoin::Bevel => vello_cpu::kurbo::Join::Bevel,
    };
    if !style.dash.is_empty() {
        stroke.dash_pattern = style.dash.iter().copied().collect();
        stroke.dash_offset = style.dash_offset;
    }
    stroke
}

fn brush(paint: &Paint) -> PaintType {
    match paint {
        Paint::Solid(value) => Brush::Solid(color(*value)),
        Paint::Linear(gradient) => {
            let mut value = Gradient::new_linear(
                Point::new(gradient.start.x, gradient.start.y),
                Point::new(gradient.end.x, gradient.end.y),
            );
            value.extend = extend(gradient.extend);
            value.stops = stops(&gradient.stops);
            Brush::Gradient(value)
        }
        Paint::Radial(radial) => Brush::Gradient(gradient_radial(radial)),
    }
}

fn gradient_radial(radial: &RadialGradient) -> Gradient {
    let mut value = Gradient::new_two_point_radial(
        Point::new(radial.start_center.x, radial.start_center.y),
        radial.start_radius as f32,
        Point::new(radial.end_center.x, radial.end_center.y),
        radial.end_radius as f32,
    );
    value.extend = extend(radial.extend);
    value.stops = stops(&radial.stops);
    value
}

fn stops(stops: &[GradientStop]) -> vello_cpu::peniko::ColorStops {
    let mut value = vello_cpu::peniko::ColorStops::new();
    for stop in stops {
        value.push(ColorStop {
            offset: stop.offset,
            color: DynamicColor::from_alpha_color(color(stop.color)),
        });
    }
    value
}

fn extend(extend: GradientExtend) -> Extend {
    match extend {
        GradientExtend::Pad => Extend::Pad,
        GradientExtend::Repeat => Extend::Repeat,
        GradientExtend::Reflect => Extend::Reflect,
    }
}

fn to_bez_path(path: &Path) -> BezPath {
    let mut output = BezPath::new();
    let mut current = Point::ZERO;
    let mut subpath_start = Point::ZERO;
    for segment in &path.segments {
        match segment {
            PathSegment::MoveTo(point) => {
                current = Point::new(point.x, point.y);
                subpath_start = current;
                output.move_to(current);
            }
            PathSegment::LineTo(point) => {
                current = Point::new(point.x, point.y);
                output.line_to(current);
            }
            PathSegment::QuadraticTo { control, to } => {
                output.quad_to(Point::new(control.x, control.y), Point::new(to.x, to.y));
                current = Point::new(to.x, to.y);
            }
            PathSegment::CubicTo {
                control1,
                control2,
                to,
            } => {
                output.curve_to(
                    Point::new(control1.x, control1.y),
                    Point::new(control2.x, control2.y),
                    Point::new(to.x, to.y),
                );
                current = Point::new(to.x, to.y);
            }
            PathSegment::ArcTo {
                rx,
                ry,
                rotation,
                large_arc,
                sweep,
                to,
            } => {
                let end = Point::new(to.x, to.y);
                let arc = SvgArc {
                    from: current,
                    to: end,
                    radii: Vec2::new(*rx, *ry),
                    x_rotation: *rotation,
                    large_arc: *large_arc,
                    sweep: *sweep,
                };
                if let Some(arc) = KurboArc::from_svg_arc(&arc) {
                    arc.to_cubic_beziers(0.1, |p1, p2, p3| {
                        output.curve_to(p1, p2, p3);
                    });
                } else {
                    output.line_to(end);
                }
                current = end;
            }
            PathSegment::Close => {
                output.close_path();
                current = subpath_start;
            }
        }
    }
    output
}
