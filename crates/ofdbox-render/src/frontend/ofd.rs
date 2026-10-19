//! OFD 前端。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ofdbox_core::{
    ColorView, CompositeGraphicUnitView, DrawParamView, GraphicObjectKind, GraphicObjectView,
    OfdPackage, PageView, ResView, TextCodeView,
};

use crate::error::RenderError;
use crate::font::{FontProvider, FontRequest};
use crate::geometry::{Matrix, Point, Rect, Size};
use crate::image::ImageFormat;
use crate::options::{RenderOptions, UnsupportedPolicy};
use crate::paint::{GradientExtend, GradientStop, LinearGradient, Paint, RadialGradient, Rgba};
use crate::path::{FillRule, LineCap, LineJoin, Path, StrokeStyle};
use crate::resource::{ResourcePool, SceneKey};
use crate::scene::{
    Clip, DiagnosticCode, Element, Group, ImageElement, PathElement, PositionedGlyph, Scene,
    StrokePaint, TextElement,
};

/// OFD 场景前端。
pub struct OfdFrontend<'a> {
    package: &'a mut OfdPackage,
    pool: &'a ResourcePool,
    options: RenderOptions,
    font_provider: Option<Box<dyn FontProvider>>,
}

impl<'a> OfdFrontend<'a> {
    /// 创建 OFD 前端。
    pub fn new(
        package: &'a mut OfdPackage,
        pool: &'a ResourcePool,
        options: RenderOptions,
    ) -> Self {
        Self {
            package,
            pool,
            options,
            font_provider: None,
        }
    }

    /// 注入系统字体或自定义字体来源。
    pub fn with_font_provider(mut self, provider: Box<dyn FontProvider>) -> Self {
        self.font_provider = Some(provider);
        self
    }

    /// 构建页面场景。
    pub fn build_page(&mut self, page: &PageView) -> Result<Scene, RenderError> {
        let provider = &mut self.font_provider;
        build_scene(self.package, self.pool, &self.options, provider, page)
    }

    /// 构建并缓存页面场景。编辑器修改页面后应传入新的修订号。
    pub fn build_page_cached(
        &mut self,
        page: &PageView,
        key: SceneKey,
    ) -> Result<Arc<Scene>, RenderError> {
        let package = &mut *self.package;
        let pool = self.pool;
        let options = &self.options;
        let provider = &mut self.font_provider;
        pool.scene(key, || build_scene(package, pool, options, provider, page))
    }
}

fn build_scene(
    package: &mut OfdPackage,
    pool: &ResourcePool,
    options: &RenderOptions,
    font_provider: &mut Option<Box<dyn FontProvider>>,
    page: &PageView,
) -> Result<Scene, RenderError> {
    let page_entry = package.page_entry_name(page)?;
    let document = package.parse_document()?;
    let page_content = package.parse_page_for(page)?;
    let mut resources = ResourceSet::default();
    resources.collect_document(package, &document)?;
    resources.collect_page(package, &page_entry, &page_content)?;

    let physical = page_content
        .area()
        .and_then(|area| area.physical_box().and_then(parse_rect))
        .or_else(|| {
            document
                .common_data()
                .and_then(|common| common.page_area())
                .and_then(|area| area.physical_box().and_then(parse_rect))
        })
        .unwrap_or_else(|| Rect::new(0.0, 0.0, 210.0, 297.0));
    let mut builder = Builder {
        package,
        pool,
        options,
        resources,
        font_provider,
        diagnostics: Vec::new(),
        depth: 0,
        composites: HashSet::new(),
    };
    let mut children = Vec::new();
    if page_content.area().is_none()
        && document
            .common_data()
            .and_then(|common| common.page_area())
            .is_none()
    {
        builder.diagnostic(
            DiagnosticCode::Geometry,
            page.id(),
            "页面未声明尺寸，使用 A4 默认值",
        );
    }
    if let Some(content) = page_content.content() {
        for layer in content.layers() {
            for object in layer.objects() {
                builder.render_object(&object, &mut children);
            }
        }
    }
    if options.draw_annotations {
        builder.render_annotations(&document, page, &mut children);
    }
    if options.unsupported == UnsupportedPolicy::Error
        && let Some(diagnostic) = builder.diagnostics.first()
    {
        return Err(RenderError::Unsupported(diagnostic.message.clone()));
    }
    let mut root = Group::new(Matrix::translate(-physical.x, -physical.y));
    root.children = children;
    Ok(Scene {
        size: Size::new(physical.width, physical.height),
        root,
        diagnostics: builder.diagnostics,
    })
}

#[derive(Default)]
struct ResourceSet {
    fonts: HashMap<String, FontResource>,
    images: HashMap<String, MediaResource>,
    colors: HashMap<String, ColorSpaceResource>,
    draw_params: HashMap<String, DrawParamResource>,
    composites: HashMap<String, CompositeGraphicUnitView>,
}

#[derive(Clone)]
struct FontResource {
    file: Option<String>,
    dir: String,
    name: Option<String>,
    family: Option<String>,
    charset: Option<String>,
    bold: bool,
    italic: bool,
}

#[derive(Clone)]
struct MediaResource {
    format: ImageFormat,
    file: String,
    dir: String,
}

struct ColorSpaceResource {
    kind: String,
    palette: Vec<String>,
}

struct DrawParamResource {
    line_width: Option<f64>,
    join: Option<LineJoin>,
    cap: Option<LineCap>,
    dash_offset: f64,
    dash: Vec<f64>,
    miter_limit: Option<f64>,
}

impl ResourceSet {
    fn collect_document(
        &mut self,
        package: &mut OfdPackage,
        document: &ofdbox_core::DocumentView,
    ) -> Result<(), RenderError> {
        let Some(common) = document.common_data() else {
            return Ok(());
        };
        let document_dir = package
            .document_entry_name()
            .map(parent_dir)
            .unwrap_or("")
            .to_owned();
        for loc in common.public_res().chain(common.document_res()) {
            let loc = loc.to_owned();
            let entry = package.resolve_entry(&document_dir, &loc)?;
            self.collect_res(package, &entry)?;
        }
        Ok(())
    }

    fn collect_page(
        &mut self,
        package: &mut OfdPackage,
        page_entry: &str,
        page: &ofdbox_core::PageContentView,
    ) -> Result<(), RenderError> {
        let page_dir = parent_dir(page_entry).to_owned();
        for loc in page.page_res() {
            let loc = loc.to_owned();
            let entry = package.resolve_entry(&page_dir, &loc)?;
            self.collect_res(package, &entry)?;
        }
        Ok(())
    }

    fn collect_res(&mut self, package: &mut OfdPackage, entry: &str) -> Result<(), RenderError> {
        let res = package.parse_res(entry)?;
        let mut dir = parent_dir(entry).to_owned();
        if let Some(base) = res.base_loc().map(str::to_owned) {
            dir = package.resolve_entry(&dir, &base)?;
        }
        self.collect_from_res(&res, dir);
        Ok(())
    }

    fn collect_from_res(&mut self, res: &ResView, dir: String) {
        for font in res.fonts() {
            if let Some(id) = font.id() {
                self.fonts.insert(
                    id.to_owned(),
                    FontResource {
                        file: font.font_file().map(str::to_owned),
                        dir: dir.clone(),
                        name: font.font_name().map(str::to_owned),
                        family: font.family_name().map(str::to_owned),
                        charset: font.charset().map(str::to_owned),
                        bold: font.bold().unwrap_or(false),
                        italic: font.italic().unwrap_or(false),
                    },
                );
            }
        }
        for media in res.multi_medias() {
            let Some(id) = media.id() else { continue };
            let Some(file) = media.media_file() else {
                continue;
            };
            let Some(format) = media.format() else {
                continue;
            };
            self.images.insert(
                id.to_owned(),
                MediaResource {
                    format: ImageFormat::from_media_format(format),
                    file: file.to_owned(),
                    dir: dir.clone(),
                },
            );
        }
        for color in res.color_spaces() {
            let Some(id) = color.id() else { continue };
            self.colors.insert(
                id.to_owned(),
                ColorSpaceResource {
                    kind: color.color_space_type().unwrap_or_default().to_owned(),
                    palette: color.palette().map(str::to_owned).collect(),
                },
            );
        }
        for param in res.draw_params() {
            let Some(id) = param.id() else { continue };
            self.draw_params.insert(id.to_owned(), draw_param(&param));
        }
        for unit in res.composite_graphic_units() {
            if let Some(id) = unit.id() {
                self.composites.insert(id.to_owned(), unit);
            }
        }
    }
}

fn draw_param(param: &DrawParamView) -> DrawParamResource {
    DrawParamResource {
        line_width: param.line_width(),
        join: param.join().and_then(parse_join),
        cap: param.cap().and_then(parse_cap),
        dash_offset: param.dash_offset().unwrap_or(0.0),
        dash: param.dash_pattern().map(parse_floats).unwrap_or_default(),
        miter_limit: param.miter_limit(),
    }
}

struct Builder<'a> {
    package: &'a mut OfdPackage,
    pool: &'a ResourcePool,
    options: &'a RenderOptions,
    resources: ResourceSet,
    font_provider: &'a mut Option<Box<dyn FontProvider>>,
    diagnostics: Vec<crate::scene::RenderDiagnostic>,
    depth: usize,
    composites: HashSet<String>,
}

impl Builder<'_> {
    fn render_object(&mut self, object: &GraphicObjectView, output: &mut Vec<Element>) {
        if parse_bool(object.attribute("Visible")) == Some(false) {
            return;
        }
        if self.depth > 8 {
            self.diagnostic(
                DiagnosticCode::UnsupportedObject,
                object.id(),
                "对象嵌套过深",
            );
            return;
        }
        let Some(boundary) = object.boundary().and_then(parse_rect) else {
            self.diagnostic(DiagnosticCode::Geometry, object.id(), "缺少有效 Boundary");
            return;
        };
        if boundary.width <= 0.0 || boundary.height <= 0.0 {
            self.diagnostic(DiagnosticCode::Geometry, object.id(), "Boundary 尺寸无效");
            return;
        }
        let ctm = match object.attribute("CTM") {
            Some(value) => match parse_matrix(value) {
                Some(matrix) => matrix,
                None => {
                    self.diagnostic(DiagnosticCode::Geometry, object.id(), "CTM 无效");
                    return;
                }
            },
            None => Matrix::IDENTITY,
        };

        let mut elements = Vec::new();
        let kind = object.kind();
        match kind {
            GraphicObjectKind::Path => {
                if let Some(element) = self.build_path(object) {
                    elements.push(element);
                }
            }
            GraphicObjectKind::Text => {
                elements.extend(self.build_text(object));
            }
            GraphicObjectKind::Image => {
                if let Some(element) = self.build_image(object) {
                    elements.push(element);
                }
            }
            GraphicObjectKind::Composite => {
                elements.extend(self.build_composite(object, &boundary));
            }
            GraphicObjectKind::PageBlock => {
                self.diagnostic(
                    DiagnosticCode::UnsupportedObject,
                    object.id(),
                    "PageBlock 暂未实现",
                );
            }
            GraphicObjectKind::Other(ref kind) => {
                self.diagnostic(
                    DiagnosticCode::UnsupportedObject,
                    object.id(),
                    format!("未知图元: {kind}"),
                );
            }
        }
        if elements.is_empty() {
            return;
        }

        // 对象裁剪在对象空间，由内向外嵌套。
        let mut inner = elements;
        for (path, clip_ctm) in self.object_clips(object) {
            inner = vec![Element::Group(clip_group(path, clip_ctm, inner))];
        }
        // 真实文件把对象内容放在 Boundary 的局部坐标中：先应用 CTM，
        // 再用 Boundary 左上角在父空间定位；Boundary 同时负责裁剪。
        let object_transform = ctm.then(Matrix::translate(boundary.x, boundary.y));
        let mut object_group = Group::new(object_transform);
        if let Some(alpha) = object
            .attribute("Alpha")
            .and_then(|value| value.parse::<i32>().ok())
        {
            object_group.opacity = (alpha.clamp(0, 255) as f32) / 255.0;
        }
        object_group.children = inner;

        // Boundary 是父空间中的外接矩形，超出部分裁剪。
        let mut wrapper = Group::new(Matrix::IDENTITY);
        wrapper.clip = Some(Clip {
            path: Arc::new(boundary.to_path()),
            rule: FillRule::NonZero,
        });
        wrapper.children.push(Element::Group(object_group));
        output.push(Element::Group(wrapper));
    }

    fn object_clips(&mut self, object: &GraphicObjectView) -> Vec<(Arc<Path>, Matrix)> {
        let Some(clips) = object.clips() else {
            return Vec::new();
        };
        let mut paths = Vec::new();
        for clip in clips.clips() {
            for area in clip.areas() {
                let ctm = area
                    .ctm()
                    .and_then(parse_matrix)
                    .unwrap_or(Matrix::IDENTITY);
                let Some(content) = area.content() else {
                    continue;
                };
                if let Some(boundary) = content.attribute("Boundary").and_then(parse_rect) {
                    paths.push((Arc::new(boundary.to_path()), ctm));
                }
                match content.kind() {
                    GraphicObjectKind::Path => {
                        if let Some(data) = content.abbreviated_data()
                            && let Some(path) = parse_path_data(data)
                        {
                            paths.push((Arc::new(path), ctm));
                        }
                    }
                    other => {
                        self.diagnostic(
                            DiagnosticCode::UnsupportedObject,
                            object.id(),
                            format!("暂不支持的裁剪内容: {other:?}"),
                        );
                    }
                }
            }
        }
        paths
    }

    fn build_path(&mut self, object: &GraphicObjectView) -> Option<Element> {
        let payload = object.payload();
        let data = object
            .attribute("AbbreviatedData")
            .or_else(|| payload.abbreviated_data())?;
        let path = match parse_path_data(data) {
            Some(path) => path,
            None => {
                self.diagnostic(DiagnosticCode::Geometry, object.id(), "路径数据无效");
                return None;
            }
        };
        let fill = parse_bool(object.attribute("Fill")).unwrap_or(false);
        let stroke = parse_bool(object.attribute("Stroke")).unwrap_or(true);
        let fill_paint = if fill {
            object
                .fill_color()
                .and_then(|color| self.paint(&color, object.id()))
        } else {
            None
        };
        let stroke_paint = if stroke {
            let paint = object
                .stroke_color()
                .and_then(|color| self.paint(&color, object.id()))
                .or(Some(Paint::Solid(Rgba::BLACK)));
            paint.map(|paint| StrokePaint {
                paint,
                style: self.stroke_style(object),
            })
        } else {
            None
        };
        let rule = match object.attribute("Rule") {
            Some(value)
                if value.eq_ignore_ascii_case("Even-Odd")
                    || value.eq_ignore_ascii_case("EvenOdd") =>
            {
                FillRule::EvenOdd
            }
            _ => FillRule::NonZero,
        };
        Some(Element::Path(PathElement {
            path: Arc::new(path),
            fill: fill_paint,
            stroke: stroke_paint,
            fill_rule: rule,
        }))
    }

    fn build_text(&mut self, object: &GraphicObjectView) -> Vec<Element> {
        let Some(font_id) = object.attribute("Font") else {
            self.diagnostic(DiagnosticCode::Font, object.id(), "文字缺少 Font");
            return Vec::new();
        };
        let Some(size) = object
            .attribute("Size")
            .and_then(|value| value.parse::<f64>().ok())
        else {
            self.diagnostic(DiagnosticCode::Geometry, object.id(), "文字缺少有效 Size");
            return Vec::new();
        };
        if size <= 0.0 {
            self.diagnostic(DiagnosticCode::Geometry, object.id(), "字号必须为正");
            return Vec::new();
        }
        let read_direction = object
            .attribute("ReadDirection")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(0);
        let char_direction = object
            .attribute("CharDirection")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(0);
        if read_direction != 0 || char_direction != 0 {
            self.diagnostic(
                DiagnosticCode::UnsupportedObject,
                object.id(),
                "暂不支持竖排文字",
            );
            return Vec::new();
        }
        let font = match self.font(font_id) {
            Some(font) => font,
            None => {
                self.diagnostic(DiagnosticCode::Font, object.id(), "字体资源缺失");
                return Vec::new();
            }
        };
        let face = ttf_parser::Face::parse(font.data(), font.face_index()).ok();
        let h_scale = object
            .attribute("HScale")
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(1.0);
        let fill = parse_bool(object.attribute("Fill")).unwrap_or(true);
        let stroke = parse_bool(object.attribute("Stroke")).unwrap_or(false);
        let fill_paint = if fill {
            object
                .fill_color()
                .map(|color| self.paint(&color, object.id()))
                .unwrap_or(Some(Paint::Solid(Rgba::BLACK)))
        } else {
            None
        };
        let stroke_paint = if stroke {
            object
                .stroke_color()
                .and_then(|color| self.paint(&color, object.id()))
                .map(|paint| StrokePaint {
                    paint,
                    style: self.stroke_style(object),
                })
        } else {
            None
        };

        let runs = collect_code_runs(object);
        let all_chars = runs
            .iter()
            .flat_map(|run| run.chars.iter().copied())
            .collect::<Vec<_>>();
        let slots = glyph_slots(object, all_chars.len());

        let mut elements = Vec::new();
        let mut offset = 0usize;
        for run in runs {
            let mut x = run.x;
            let mut y = run.y;
            let mut glyphs = Vec::with_capacity(run.chars.len());
            for (index, character) in run.chars.iter().enumerate() {
                let slot = offset + index;
                let glyph_id = slots.get(slot).copied().flatten().or_else(|| {
                    face.as_ref()
                        .and_then(|face| face.glyph_index(*character))
                        .map(|id| u32::from(id.0))
                });
                let Some(glyph_id) = glyph_id else {
                    continue;
                };
                glyphs.push(PositionedGlyph {
                    id: glyph_id,
                    x: x as f32,
                    y: y as f32,
                });
                let advance = run
                    .delta_x
                    .get(index)
                    .copied()
                    .unwrap_or_else(|| glyph_advance(face.as_ref(), glyph_id, size));
                x += advance * h_scale.max(0.0);
                y += run.delta_y.get(index).copied().unwrap_or(0.0);
            }
            offset += run.chars.len();
            if !glyphs.is_empty() {
                elements.push(Element::Text(TextElement {
                    font: Arc::clone(&font),
                    size,
                    transform: Matrix::IDENTITY,
                    glyphs,
                    fill: fill_paint.clone(),
                    stroke: stroke_paint.clone(),
                }));
            }
        }
        elements
    }

    fn build_image(&mut self, object: &GraphicObjectView) -> Option<Element> {
        let resource_id = object
            .attribute("ResourceID")
            .or_else(|| object.attribute("ResouceID"))?;
        let media = self.resources.images.get(resource_id).cloned()?;
        let file = match self
            .package
            .resolve_entry(&media.dir, &media.file)
            .and_then(|entry| self.package.read_entry(&entry))
        {
            Ok(bytes) => bytes,
            Err(error) => {
                self.diagnostic(
                    DiagnosticCode::Image,
                    object.id(),
                    format!("读取图片失败: {error}"),
                );
                return None;
            }
        };
        let mask = object
            .attribute("ImageMask")
            .and_then(|id| self.resources.images.get(id).cloned())
            .and_then(|mask| {
                let entry = self.package.resolve_entry(&mask.dir, &mask.file).ok()?;
                let bytes = self.package.read_entry(&entry).ok()?;
                Some((mask.format, bytes))
            });
        let mask = mask
            .as_ref()
            .map(|(format, bytes)| (format.clone(), &bytes[..]));
        let image = match self.pool.embedded_image(media.format.clone(), &file, mask) {
            Ok(image) => image,
            Err(error) => {
                self.diagnostic(
                    DiagnosticCode::Image,
                    object.id(),
                    format!("图片资源无效: {error}"),
                );
                return None;
            }
        };
        let decoded = match image.decoded(self.options.max_image_pixels) {
            Ok(decoded) => decoded,
            Err(error) => {
                self.diagnostic(
                    DiagnosticCode::Image,
                    object.id(),
                    format!("图片解码失败: {error}"),
                );
                return None;
            }
        };
        let scale_x = 1.0 / f64::from(decoded.width.max(1));
        let scale_y = 1.0 / f64::from(decoded.height.max(1));
        let alpha = object
            .attribute("Alpha")
            .and_then(|value| value.parse::<i32>().ok())
            .map(|value| (value.clamp(0, 255) as f32) / 255.0)
            .unwrap_or(1.0);
        Some(Element::Image(ImageElement {
            image,
            transform: Matrix::scale(scale_x, scale_y),
            alpha,
        }))
    }

    fn build_composite(&mut self, object: &GraphicObjectView, boundary: &Rect) -> Vec<Element> {
        let Some(resource_id) = object
            .attribute("ResourceID")
            .or_else(|| object.attribute("ResouceID"))
        else {
            self.diagnostic(
                DiagnosticCode::Geometry,
                object.id(),
                "复合对象缺少 ResourceID",
            );
            return Vec::new();
        };
        let Some(unit) = self.resources.composites.get(resource_id).cloned() else {
            self.diagnostic(
                DiagnosticCode::UnsupportedObject,
                object.id(),
                "复合对象资源缺失",
            );
            return Vec::new();
        };
        if !self.composites.insert(resource_id.to_owned()) {
            self.diagnostic(
                DiagnosticCode::UnsupportedObject,
                object.id(),
                "复合对象存在循环引用",
            );
            return Vec::new();
        }
        let unit_width = unit.width().unwrap_or(0.0);
        let unit_height = unit.height().unwrap_or(0.0);
        let mut elements = Vec::new();
        if unit_width > 0.0 && unit_height > 0.0 {
            let mut nested = Vec::new();
            self.depth += 1;
            if let Some(content) = unit.content() {
                for child in content.objects() {
                    self.render_object(&child, &mut nested);
                }
            }
            self.depth -= 1;
            let mut group = Group::new(Matrix::scale(
                boundary.width / unit_width,
                boundary.height / unit_height,
            ));
            group.children = nested;
            elements.push(Element::Group(group));
        }
        self.composites.remove(resource_id);
        elements
    }

    fn render_annotations(
        &mut self,
        document: &ofdbox_core::DocumentView,
        page: &PageView,
        output: &mut Vec<Element>,
    ) {
        if document.annotations().is_none() {
            return;
        }
        let index = match self.package.parse_annotations() {
            Ok(index) => index,
            Err(error) => {
                self.diagnostic(
                    DiagnosticCode::UnsupportedObject,
                    page.id(),
                    format!("注释索引读取失败: {error}"),
                );
                return;
            }
        };
        for entry in index.pages() {
            if entry.page_id() != page.id() {
                continue;
            }
            let Some(loc) = entry.file_loc().map(str::to_owned) else {
                continue;
            };
            let Ok(page_annot) = self.package.parse_page_annot(&loc) else {
                continue;
            };
            for annot in page_annot.annots() {
                if annot.visible() == Some(false) || annot.print() == Some(false) {
                    continue;
                }
                let Some(appearance) = annot.appearance() else {
                    continue;
                };
                for object in appearance.objects() {
                    self.render_object(&object, output);
                }
            }
        }
    }

    fn font(&mut self, id: &str) -> Option<Arc<crate::asset::FontAsset>> {
        let resource = self.resources.fonts.get(id).cloned()?;
        let bytes = if let Some(file) = resource.file.as_deref() {
            let entry = self.package.resolve_entry(&resource.dir, file).ok()?;
            self.package.read_entry(&entry).ok()?.to_vec()
        } else {
            let provider = self.font_provider.as_mut()?;
            let request = FontRequest {
                font_name: resource.name.as_deref(),
                family_name: resource.family.as_deref(),
                charset: resource.charset.as_deref(),
                bold: resource.bold,
                italic: resource.italic,
            };
            provider.load(&request).ok()??
        };
        if bytes.is_empty() {
            return None;
        }
        self.pool.embedded_font(&bytes, 0).ok()
    }

    fn stroke_style(&mut self, object: &GraphicObjectView) -> StrokeStyle {
        let param = object
            .attribute("DrawParam")
            .and_then(|id| self.resources.draw_params.get(id));
        let mut style = StrokeStyle::default();
        if let Some(param) = param {
            style.width = param.line_width.unwrap_or(style.width);
            style.join = param.join.unwrap_or(style.join);
            style.cap = param.cap.unwrap_or(style.cap);
            style.dash_offset = param.dash_offset;
            style.dash = param.dash.clone();
            style.miter_limit = param.miter_limit.unwrap_or(style.miter_limit);
        }
        if let Some(width) = object
            .attribute("LineWidth")
            .and_then(|value| value.parse::<f64>().ok())
        {
            style.width = width;
        }
        if let Some(miter) = object
            .attribute("MiterLimit")
            .and_then(|value| value.parse::<f64>().ok())
        {
            style.miter_limit = miter;
        }
        if let Some(offset) = object
            .attribute("DashOffset")
            .and_then(|value| value.parse::<f64>().ok())
        {
            style.dash_offset = offset;
        }
        if let Some(pattern) = object.attribute("DashPattern") {
            style.dash = parse_floats(pattern);
        }
        if let Some(cap) = object.attribute("Cap").and_then(parse_cap) {
            style.cap = cap;
        }
        if let Some(join) = object.attribute("Join").and_then(parse_join) {
            style.join = join;
        }
        style.width = style.width.max(0.0);
        style
    }

    fn paint(&mut self, color: &ColorView, object_id: Option<&str>) -> Option<Paint> {
        match color.kind() {
            ofdbox_core::ColorKind::Solid => self.solid_paint(color),
            ofdbox_core::ColorKind::AxialShd => {
                let shd = color.axial_shd()?;
                let start = shd.start_point().and_then(parse_point)?;
                let end = shd.end_point().and_then(parse_point)?;
                Some(Paint::Linear(LinearGradient {
                    start,
                    end,
                    stops: self.gradient_stops(shd.segments(), color.alpha()),
                    extend: gradient_extend(shd.extend(), shd.map_type()),
                }))
            }
            ofdbox_core::ColorKind::RadialShd => {
                let shd = color.radial_shd()?;
                let start_center = shd.start_point().and_then(parse_point)?;
                let end_center = shd.end_point().and_then(parse_point)?;
                let start_radius = shd.start_radius().unwrap_or(0.0);
                let end_radius = shd
                    .end_radius()
                    .unwrap_or_else(|| distance(start_center, end_center));
                Some(Paint::Radial(RadialGradient {
                    start_center,
                    start_radius,
                    end_center,
                    end_radius,
                    stops: self.gradient_stops(shd.segments(), color.alpha()),
                    extend: gradient_extend(shd.extend(), shd.map_type()),
                }))
            }
            other => {
                self.diagnostic(
                    DiagnosticCode::Paint,
                    object_id,
                    format!("暂不支持的颜色形态: {other:?}"),
                );
                None
            }
        }
    }

    fn solid_paint(&self, color: &ColorView) -> Option<Paint> {
        let value = if let Some(index) = color.index() {
            let space = color
                .color_space()
                .and_then(|id| self.resources.colors.get(id))?;
            if index < 0 {
                return None;
            }
            space.palette.get(index as usize)?.clone()
        } else {
            color.value()?.to_owned()
        };
        let components = parse_floats(&value);
        let kind = color
            .color_space()
            .and_then(|id| self.resources.colors.get(id))
            .map(|space| space.kind.as_str())
            .unwrap_or("");
        let alpha = color
            .alpha()
            .map(|value| (value.clamp(0, 255) as f32) / 255.0)
            .unwrap_or(1.0);
        Some(Paint::Solid(components_to_rgba(&components, kind, alpha)?))
    }

    fn gradient_stops(
        &mut self,
        segments: impl Iterator<Item = ofdbox_core::SegmentView>,
        alpha: Option<i32>,
    ) -> Vec<GradientStop> {
        let alpha = alpha
            .map(|value| (value.clamp(0, 255) as f32) / 255.0)
            .unwrap_or(1.0);
        let collected = segments
            .filter_map(|segment| {
                let color = segment.color()?;
                let mut stop = match self.solid_paint(&color)? {
                    Paint::Solid(color) => GradientStop {
                        offset: segment.position().unwrap_or(f64::NAN) as f32,
                        color,
                    },
                    _ => return None,
                };
                stop.color.a *= alpha;
                Some(stop)
            })
            .collect::<Vec<_>>();
        distribute_stops(collected)
    }

    fn diagnostic(&mut self, code: DiagnosticCode, id: Option<&str>, message: impl Into<String>) {
        if self.options.unsupported == UnsupportedPolicy::Error {
            self.diagnostics.push(crate::scene::RenderDiagnostic {
                code,
                object_id: id.map(str::to_owned),
                message: message.into(),
            });
            return;
        }
        self.diagnostics.push(crate::scene::RenderDiagnostic {
            code,
            object_id: id.map(str::to_owned),
            message: message.into(),
        });
    }
}

fn clip_group(path: Arc<Path>, transform: Matrix, children: Vec<Element>) -> Group {
    let mut group = Group::new(transform);
    group.clip = Some(Clip {
        path,
        rule: FillRule::NonZero,
    });
    group.children = children;
    group
}

struct CodeRun {
    chars: Vec<char>,
    x: f64,
    y: f64,
    delta_x: Vec<f64>,
    delta_y: Vec<f64>,
}

fn collect_code_runs(object: &GraphicObjectView) -> Vec<CodeRun> {
    object
        .text_codes()
        .map(|code: TextCodeView| CodeRun {
            chars: decode_text(code.text()),
            x: code.x().unwrap_or(0.0),
            y: code.y().unwrap_or(0.0),
            delta_x: code.delta_x().map(parse_floats).unwrap_or_default(),
            delta_y: code.delta_y().map(parse_floats).unwrap_or_default(),
        })
        .collect()
}

fn glyph_slots(object: &GraphicObjectView, count: usize) -> Vec<Option<u32>> {
    let mut slots = vec![None; count];
    for transform in object.cg_transforms() {
        let glyphs = transform.glyphs().map(parse_u32s).unwrap_or_default();
        let position = transform.code_position().unwrap_or(0).max(0) as usize;
        let code_count = transform
            .code_count()
            .map(|value| value.max(0) as usize)
            .unwrap_or(glyphs.len());
        if glyphs.len() == code_count {
            for (index, glyph) in glyphs.into_iter().enumerate() {
                if let Some(slot) = slots.get_mut(position + index) {
                    *slot = Some(glyph);
                }
            }
        } else if let Some(glyph) = glyphs.first()
            && let Some(slot) = slots.get_mut(position)
        {
            *slot = Some(*glyph);
        }
    }
    slots
}

fn glyph_advance(face: Option<&ttf_parser::Face<'_>>, glyph: u32, size: f64) -> f64 {
    let Some(face) = face else {
        return size;
    };
    let units = face.units_per_em();
    if units == 0 {
        return size;
    }
    let advance = glyph
        .try_into()
        .ok()
        .and_then(|id| face.glyph_hor_advance(ttf_parser::GlyphId(id)))
        .map(f64::from)
        .unwrap_or(f64::from(units));
    advance / f64::from(units) * size
}

fn decode_text(text: &str) -> Vec<char> {
    let mut output = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\\' {
            let mut hex = String::new();
            for _ in 0..4 {
                if let Some(next) = chars.next()
                    && next.is_ascii_hexdigit()
                {
                    hex.push(next);
                }
            }
            if let Ok(value) = u32::from_str_radix(&hex, 16)
                && let Some(character) = char::from_u32(value)
            {
                output.push(character);
                continue;
            }
            output.push('\\');
        } else {
            output.push(character);
        }
    }
    output
}

fn parse_path_data(data: &str) -> Option<Path> {
    let tokens = data
        .split(|character: char| character.is_whitespace() || character == ',')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let mut path = Path::new();
    let mut index = 0usize;
    let mut command = None;
    let mut first_move = true;
    while index < tokens.len() {
        let token = tokens[index];
        if token.len() == 1
            && token
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
        {
            command = Some(token.chars().next().expect("command"));
            index += 1;
            if command == Some('C') {
                path.close();
                continue;
            }
            if command == Some('M') {
                first_move = true;
            }
        }
        let command = command?;
        match command {
            'S' | 'M' => {
                let x = number(tokens.get(index)?)?;
                let y = number(tokens.get(index + 1)?)?;
                index += 2;
                if command == 'S' || first_move {
                    path.move_to(Point::new(x, y));
                    first_move = false;
                } else {
                    path.line_to(Point::new(x, y));
                }
            }
            'L' => {
                let x = number(tokens.get(index)?)?;
                let y = number(tokens.get(index + 1)?)?;
                index += 2;
                path.line_to(Point::new(x, y));
            }
            'Q' => {
                let x1 = number(tokens.get(index)?)?;
                let y1 = number(tokens.get(index + 1)?)?;
                let x = number(tokens.get(index + 2)?)?;
                let y = number(tokens.get(index + 3)?)?;
                index += 4;
                path.quadratic_to(Point::new(x1, y1), Point::new(x, y));
            }
            'B' => {
                let x1 = number(tokens.get(index)?)?;
                let y1 = number(tokens.get(index + 1)?)?;
                let x2 = number(tokens.get(index + 2)?)?;
                let y2 = number(tokens.get(index + 3)?)?;
                let x = number(tokens.get(index + 4)?)?;
                let y = number(tokens.get(index + 5)?)?;
                index += 6;
                path.cubic_to(Point::new(x1, y1), Point::new(x2, y2), Point::new(x, y));
            }
            'A' => {
                let rx = number(tokens.get(index)?)?;
                let ry = number(tokens.get(index + 1)?)?;
                let angle = number(tokens.get(index + 2)?)?;
                let large = number(tokens.get(index + 3)?)? != 0.0;
                let sweep = number(tokens.get(index + 4)?)? != 0.0;
                let x = number(tokens.get(index + 5)?)?;
                let y = number(tokens.get(index + 6)?)?;
                index += 7;
                let to = Point::new(x, y);
                path.arc_to(rx.abs(), ry.abs(), angle.to_radians(), large, sweep, to);
            }
            _ => return None,
        }
    }
    Some(path)
}

fn number(token: &str) -> Option<f64> {
    token.parse().ok()
}

fn parse_u32s(text: &str) -> Vec<u32> {
    text.split_whitespace()
        .filter_map(|value| value.parse().ok())
        .collect()
}

fn parse_floats(text: &str) -> Vec<f64> {
    text.split(|character: char| character.is_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .filter_map(|value| value.parse().ok())
        .collect()
}

fn parse_rect(text: &str) -> Option<Rect> {
    let values = parse_floats(text);
    if values.len() < 4 {
        return None;
    }
    Some(Rect::new(values[0], values[1], values[2], values[3]))
}

fn parse_point(text: &str) -> Option<Point> {
    let values = parse_floats(text);
    if values.len() < 2 {
        return None;
    }
    Some(Point::new(values[0], values[1]))
}

fn parse_matrix(text: &str) -> Option<Matrix> {
    let values = parse_floats(text);
    if values.len() != 6 {
        return None;
    }
    Some(Matrix::new(
        values[0], values[1], values[2], values[3], values[4], values[5],
    ))
}

fn parse_bool(text: Option<&str>) -> Option<bool> {
    match text? {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn parse_cap(text: &str) -> Option<LineCap> {
    match text {
        "Butt" | "butt" => Some(LineCap::Butt),
        "Round" | "round" => Some(LineCap::Round),
        "Square" | "square" => Some(LineCap::Square),
        _ => None,
    }
}

fn parse_join(text: &str) -> Option<LineJoin> {
    match text {
        "Miter" | "miter" => Some(LineJoin::Miter),
        "Round" | "round" => Some(LineJoin::Round),
        "Bevel" | "bevel" => Some(LineJoin::Bevel),
        _ => None,
    }
}

fn components_to_rgba(components: &[f64], kind: &str, alpha: f32) -> Option<Rgba> {
    let normalized = |value: f64| (value / 255.0).clamp(0.0, 1.0) as f32;
    let (r, g, b) = match components.len() {
        1 => {
            let value = normalized(components[0]);
            (value, value, value)
        }
        4 => {
            let c = normalized(components[0]);
            let m = normalized(components[1]);
            let y = normalized(components[2]);
            let k = normalized(components[3]);
            (
                (1.0 - c) * (1.0 - k),
                (1.0 - m) * (1.0 - k),
                (1.0 - y) * (1.0 - k),
            )
        }
        3 if kind.eq_ignore_ascii_case("CMYK") => {
            let c = normalized(components[0]);
            let m = normalized(components[1]);
            let y = normalized(components[2]);
            ((1.0 - c), (1.0 - m), (1.0 - y))
        }
        3 => (
            normalized(components[0]),
            normalized(components[1]),
            normalized(components[2]),
        ),
        _ => return None,
    };
    Some(Rgba::new(r, g, b, alpha))
}

fn gradient_extend(extend: Option<i32>, map_type: Option<&str>) -> GradientExtend {
    if let Some(extend) = extend {
        return match extend {
            1 => GradientExtend::Repeat,
            2 => GradientExtend::Reflect,
            _ => GradientExtend::Pad,
        };
    }
    match map_type {
        Some(value) if value.eq_ignore_ascii_case("Repeat") => GradientExtend::Repeat,
        Some(value) if value.eq_ignore_ascii_case("Reflect") => GradientExtend::Reflect,
        _ => GradientExtend::Pad,
    }
}

fn distribute_stops(mut stops: Vec<GradientStop>) -> Vec<GradientStop> {
    if stops.is_empty() {
        return stops;
    }
    let missing = stops.iter().any(|stop| !stop.offset.is_finite());
    if missing {
        let last = stops.len().saturating_sub(1).max(1);
        for (index, stop) in stops.iter_mut().enumerate() {
            stop.offset = index as f32 / last as f32;
        }
    }
    stops.sort_by(|a, b| {
        a.offset
            .partial_cmp(&b.offset)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    stops
}

fn distance(a: Point, b: Point) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

fn parent_dir(path: &str) -> &str {
    path.rfind('/').map(|index| &path[..index]).unwrap_or("")
}
