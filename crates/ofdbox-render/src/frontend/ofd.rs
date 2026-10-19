//! OFD 前端。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ofdbox_core::{
    ColorView, CompositeGraphicUnitView, DrawParamView, GraphicObjectKind, GraphicObjectView,
    OfdError, OfdPackage, PageView, ResView, TemplateView,
};

use crate::error::RenderError;
use crate::font::{FontProvider, FontRequest};
use crate::geometry::{Matrix, Point, Rect, Size};
use crate::image::ImageFormat;
use crate::options::{RenderOptions, UnsupportedPolicy};
use crate::paint::{GradientExtend, GradientStop, LinearGradient, Paint, RadialGradient, Rgba};
use crate::path::{FillRule, LineCap, LineJoin, Path, PathSegment, StrokeStyle};
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
    font_cache: HashMap<String, Option<Arc<crate::asset::FontAsset>>>,
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
            font_cache: HashMap::new(),
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
        build_scene(
            self.package,
            self.pool,
            &self.options,
            provider,
            &mut self.font_cache,
            page,
        )
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
        let fonts = &mut self.font_cache;
        pool.scene(key, || {
            build_scene(package, pool, options, provider, fonts, page)
        })
    }
}

fn build_scene(
    package: &mut OfdPackage,
    pool: &ResourcePool,
    options: &RenderOptions,
    font_provider: &mut Option<Box<dyn FontProvider>>,
    font_cache: &mut HashMap<String, Option<Arc<crate::asset::FontAsset>>>,
    page: &PageView,
) -> Result<Scene, RenderError> {
    let document = package.parse_document()?;
    let document_dir = package
        .document_entry_name()
        .map(parent_dir)
        .unwrap_or("")
        .to_owned();
    let page_loc = page
        .base_loc()
        .ok_or_else(|| OfdError::InvalidStructure("Page 缺少 BaseLoc".to_owned()))?;
    let page_entry = resolve_existing(package, &document_dir, page_loc)
        .ok_or_else(|| OfdError::MissingEntry(page_loc.to_owned()))?;
    let page_content = package.parse_page(&page_entry)?;

    let mut templates = HashMap::new();
    if let Some(common) = document.common_data() {
        for template in common.template_pages() {
            let Some(id) = template.id().map(str::to_owned) else {
                continue;
            };
            let Some(loc) = template.base_loc() else {
                continue;
            };
            let Some(entry) = resolve_existing(package, &document_dir, loc) else {
                continue;
            };
            let Ok(content) = package.parse_page(&entry) else {
                continue;
            };
            templates.insert(
                id.clone(),
                TemplateEntry {
                    id,
                    z_order: template.z_order().unwrap_or("Background").to_owned(),
                    entry,
                    content,
                },
            );
        }
    }

    let mut resources = ResourceSet::default();
    resources.collect_document(package, &document);
    for template in templates.values() {
        resources.collect_page(package, &template.entry, &template.content);
    }
    resources.collect_page(package, &page_entry, &page_content);

    let physical = page_content
        .area()
        .and_then(|area| area.physical_box().and_then(parse_rect))
        .or_else(|| {
            page_content.templates().find_map(|reference| {
                let id = reference.template_id()?;
                templates
                    .get(id)?
                    .content
                    .area()
                    .and_then(|area| area.physical_box().and_then(parse_rect))
            })
        })
        .or_else(|| {
            document
                .common_data()
                .and_then(|common| common.page_area())
                .and_then(|area| area.physical_box().and_then(parse_rect))
        })
        .unwrap_or_else(|| Rect::new(0.0, 0.0, 210.0, 297.0));
    let default_color_space = document
        .common_data()
        .and_then(|common| common.default_cs().map(str::to_owned));
    let mut builder = Builder {
        package,
        pool,
        options,
        resources,
        templates,
        default_color_space,
        font_provider,
        font_cache,
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
    let template_refs = page_content.templates().collect::<Vec<_>>();
    let mut visited_templates = HashSet::new();
    builder.render_templates(
        &template_refs,
        TemplateOrder::Background,
        &mut children,
        &mut visited_templates,
    );
    builder.render_templates(
        &template_refs,
        TemplateOrder::Body,
        &mut children,
        &mut visited_templates,
    );
    if let Some(content) = page_content.content() {
        for layer in content.layers() {
            for object in layer.objects() {
                builder.render_object(&object, &mut children);
            }
        }
    }
    builder.render_templates(
        &template_refs,
        TemplateOrder::Foreground,
        &mut children,
        &mut visited_templates,
    );
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
    base: Option<String>,
    name: Option<String>,
    family: Option<String>,
    charset: Option<String>,
    bold: bool,
    italic: bool,
    serif: bool,
    fixed_width: bool,
}

#[derive(Clone)]
struct MediaResource {
    format: ImageFormat,
    file: String,
    dir: String,
    base: Option<String>,
}

struct ColorSpaceResource {
    kind: String,
    /// 每分量位数，标准允许 1/2/4/8/16，默认 8。
    bits: u32,
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
    fn collect_document(&mut self, package: &mut OfdPackage, document: &ofdbox_core::DocumentView) {
        let Some(common) = document.common_data() else {
            return;
        };
        let document_dir = package
            .document_entry_name()
            .map(parent_dir)
            .unwrap_or("")
            .to_owned();
        for loc in common.public_res().chain(common.document_res()) {
            let loc = loc.to_owned();
            if let Some(entry) = resolve_existing(package, &document_dir, &loc) {
                let _ = self.collect_res(package, &entry);
            }
        }
    }

    fn collect_page(
        &mut self,
        package: &mut OfdPackage,
        page_entry: &str,
        page: &ofdbox_core::PageContentView,
    ) {
        let page_dir = parent_dir(page_entry).to_owned();
        for loc in page.page_res() {
            let loc = loc.to_owned();
            if let Some(entry) = resolve_existing(package, &page_dir, &loc) {
                let _ = self.collect_res(package, &entry);
            }
        }
    }

    fn collect_res(&mut self, package: &mut OfdPackage, entry: &str) -> Result<(), RenderError> {
        let res = package.parse_res(entry)?;
        let dir = parent_dir(entry).to_owned();
        let base = res.base_loc().map(str::to_owned);
        self.collect_from_res(&res, dir, base);
        Ok(())
    }

    fn collect_from_res(&mut self, res: &ResView, dir: String, base: Option<String>) {
        for font in res.fonts() {
            if let Some(id) = font.id() {
                self.fonts.insert(
                    id.to_owned(),
                    FontResource {
                        file: font.font_file().map(str::to_owned),
                        dir: dir.clone(),
                        base: base.clone(),
                        name: font.font_name().map(str::to_owned),
                        family: font.family_name().map(str::to_owned),
                        charset: font.charset().map(str::to_owned),
                        bold: font.bold().unwrap_or(false),
                        italic: font.italic().unwrap_or(false),
                        serif: font.serif().unwrap_or(false),
                        fixed_width: font.fixed_width().unwrap_or(false),
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
                    base: base.clone(),
                },
            );
        }
        for color in res.color_spaces() {
            let Some(id) = color.id() else { continue };
            let bits = color
                .bits_per_component()
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| (1..=16).contains(value))
                .unwrap_or(8);
            self.colors.insert(
                id.to_owned(),
                ColorSpaceResource {
                    kind: color.color_space_type().unwrap_or_default().to_owned(),
                    bits,
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
    templates: HashMap<String, TemplateEntry>,
    default_color_space: Option<String>,
    font_provider: &'a mut Option<Box<dyn FontProvider>>,
    font_cache: &'a mut HashMap<String, Option<Arc<crate::asset::FontAsset>>>,
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
        if object.kind() == GraphicObjectKind::PageBlock {
            self.depth += 1;
            for child in object.children() {
                self.render_object(&child, output);
            }
            self.depth -= 1;
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
            GraphicObjectKind::PageBlock => {}
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
        for path in self.object_clips(object) {
            inner = vec![Element::Group(clip_group(path, Matrix::IDENTITY, inner))];
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

    fn object_clips(&mut self, object: &GraphicObjectView) -> Vec<Arc<Path>> {
        let Some(clips) = object.clips() else {
            return Vec::new();
        };
        let mut paths = Vec::new();
        for clip in clips.clips() {
            // 同一 Clip 下的多个 Area 是并集，合并为一条路径；多个 Clip 之间再取交集。
            let mut union = Path::new();
            let mut has_area = false;
            for area in clip.areas() {
                let ctm = area
                    .ctm()
                    .and_then(parse_matrix)
                    .unwrap_or(Matrix::IDENTITY);
                let Some(content) = area.content() else {
                    continue;
                };
                match content.kind() {
                    GraphicObjectKind::Path => {
                        if let Some(data) = content.abbreviated_data()
                            && let Some(path) = parse_path_data(data)
                        {
                            append_transformed_path(&mut union, &path, ctm);
                            has_area = true;
                        }
                    }
                    GraphicObjectKind::Text => {
                        // 标准允许文字轮廓作为裁剪区；当前按外接矩形近似。
                        if let Some(boundary) = content.attribute("Boundary").and_then(parse_rect) {
                            append_transformed_path(&mut union, &boundary.to_path(), ctm);
                            has_area = true;
                            self.diagnostic(
                                DiagnosticCode::UnsupportedObject,
                                object.id(),
                                "文本裁剪按外接矩形近似",
                            );
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
            if has_area {
                paths.push(Arc::new(union));
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
        let fill_color = if fill { object.fill_color() } else { None };
        let pattern = fill_color
            .as_ref()
            .filter(|color| color.kind() == ofdbox_core::ColorKind::Pattern);
        let mesh = fill_color.as_ref().filter(|color| {
            matches!(
                color.kind(),
                ofdbox_core::ColorKind::GouraudShd | ofdbox_core::ColorKind::LaGouraudShd
            )
        });
        let fill_paint = if pattern.is_none() && mesh.is_none() {
            fill_color
                .as_ref()
                .and_then(|color| self.paint(color, object.id()))
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
        if pattern.is_some() || mesh.is_some() {
            let fill_element = if let Some(color) = pattern {
                self.build_pattern_fill(object, &path, color, rule)
            } else if let Some(color) = mesh {
                self.build_mesh_fill(object, &path, color, rule)
            } else {
                None
            };
            let mut children = Vec::new();
            if let Some(element) = fill_element {
                children.push(element);
            }
            if let Some(stroke) = stroke_paint {
                children.push(Element::Path(PathElement {
                    path: Arc::new(path),
                    fill: None,
                    stroke: Some(stroke),
                    fill_rule: rule,
                }));
            }
            if children.is_empty() {
                return None;
            }
            let mut group = Group::new(Matrix::IDENTITY);
            group.children = children;
            return Some(Element::Group(group));
        }
        let gradient_clip = if fill_paint.is_some() {
            fill_color.as_ref().and_then(gradient_fill_clip)
        } else {
            None
        };
        if let Some(clip) = gradient_clip {
            let fill = Element::Path(PathElement {
                path: Arc::new(path.clone()),
                fill: fill_paint,
                stroke: None,
                fill_rule: rule,
            });
            let mut children = vec![Element::Group(clip_group(
                Arc::new(clip),
                Matrix::IDENTITY,
                vec![fill],
            ))];
            if let Some(stroke) = stroke_paint {
                children.push(Element::Path(PathElement {
                    path: Arc::new(path),
                    fill: None,
                    stroke: Some(stroke),
                    fill_rule: rule,
                }));
            }
            let mut group = Group::new(Matrix::IDENTITY);
            group.children = children;
            return Some(Element::Group(group));
        }
        Some(Element::Path(PathElement {
            path: Arc::new(path),
            fill: fill_paint,
            stroke: stroke_paint,
            fill_rule: rule,
        }))
    }

    /// 把底纹展开为按 XStep/YStep 排列的单元，再按填充路径裁剪。
    fn build_pattern_fill(
        &mut self,
        object: &GraphicObjectView,
        path: &Path,
        color: &ColorView,
        rule: FillRule,
    ) -> Option<Element> {
        let pattern = color.pattern()?;
        let width = pattern.width()?;
        let height = pattern.height()?;
        if !(width > 0.0 && height > 0.0) {
            self.diagnostic(DiagnosticCode::Paint, object.id(), "底纹尺寸无效");
            return None;
        }
        let boundary = object.boundary().and_then(parse_rect)?;
        let x_step = pattern.x_step().unwrap_or(width).max(width);
        let y_step = pattern.y_step().unwrap_or(height).max(height);
        let relative_to_page = pattern
            .relative_to()
            .is_some_and(|value| value.eq_ignore_ascii_case("Page"));
        let origin_x = if relative_to_page { -boundary.x } else { 0.0 };
        let origin_y = if relative_to_page { -boundary.y } else { 0.0 };
        let start_col = ((0.0 - origin_x) / x_step).floor() as i64;
        let end_col = ((boundary.width - origin_x) / x_step).ceil() as i64;
        let start_row = ((0.0 - origin_y) / y_step).floor() as i64;
        let end_row = ((boundary.height - origin_y) / y_step).ceil() as i64;
        let columns = end_col - start_col + 1;
        let rows = end_row - start_row + 1;
        if columns <= 0 || rows <= 0 {
            return None;
        }
        let count = i128::from(columns) * i128::from(rows);
        if count > 4096 {
            self.diagnostic(DiagnosticCode::Paint, object.id(), "底纹单元过多");
            return None;
        }

        let mut tile = Vec::new();
        self.depth += 1;
        for child in pattern.objects() {
            self.render_object(&child, &mut tile);
        }
        self.depth -= 1;
        if tile.is_empty() {
            return None;
        }

        let reflect = pattern.reflect_method().unwrap_or("Normal");
        let mirror_columns =
            reflect.eq_ignore_ascii_case("Column") || reflect.eq_ignore_ascii_case("RowAndColumn");
        let mirror_rows =
            reflect.eq_ignore_ascii_case("Row") || reflect.eq_ignore_ascii_case("RowAndColumn");
        // 相邻单元在公共边各自抗锯齿会留下白缝，放大单元并向外扩一点制造重叠。
        const PATTERN_OVERLAP: f64 = 0.25;
        let factor_x = (width + PATTERN_OVERLAP * 2.0) / width;
        let factor_y = (height + PATTERN_OVERLAP * 2.0) / height;
        let tile_clip = Arc::new(Rect::new(0.0, 0.0, width, height).to_path());
        let mut cells = Vec::with_capacity(count as usize);
        for row in start_row..=end_row {
            for col in start_col..=end_col {
                let scale_x = if mirror_columns && col.rem_euclid(2) == 1 {
                    -1.0
                } else {
                    1.0
                };
                let scale_y = if mirror_rows && row.rem_euclid(2) == 1 {
                    -1.0
                } else {
                    1.0
                };
                let x = origin_x + col as f64 * x_step;
                let y = origin_y + row as f64 * y_step;
                let translate_x = if scale_x < 0.0 {
                    x + width + PATTERN_OVERLAP
                } else {
                    x - PATTERN_OVERLAP
                };
                let translate_y = if scale_y < 0.0 {
                    y + height + PATTERN_OVERLAP
                } else {
                    y - PATTERN_OVERLAP
                };
                let transform = Matrix::scale(scale_x * factor_x, scale_y * factor_y)
                    .then(Matrix::translate(translate_x, translate_y));
                let mut cell = Group::new(transform);
                cell.clip = Some(Clip {
                    path: Arc::clone(&tile_clip),
                    rule: FillRule::NonZero,
                });
                cell.children = tile.clone();
                cells.push(Element::Group(cell));
            }
        }

        let mut arrangement = Group::new(
            pattern
                .ctm()
                .and_then(parse_matrix)
                .unwrap_or(Matrix::IDENTITY),
        );
        arrangement.children = cells;
        let mut fill = Group::new(Matrix::IDENTITY);
        fill.clip = Some(Clip {
            path: Arc::new(path.clone()),
            rule,
        });
        fill.opacity = color
            .alpha()
            .map(|alpha| (alpha.clamp(0, 255) as f32) / 255.0)
            .unwrap_or(1.0);
        fill.children.push(Element::Group(arrangement));
        Some(Element::Group(fill))
    }

    /// 把高洛德/网格高洛德渐变细分并展开为纯色三角形。
    ///
    /// vello_cpu/peniko 没有网格渐变画笔，这里按控制点做重心细分，
    /// 用每个子三角形的平均色近似渐变；网格高洛德按 `VerticesPerRow` 切分四边形。
    fn build_mesh_fill(
        &mut self,
        object: &GraphicObjectView,
        path: &Path,
        color: &ColorView,
        rule: FillRule,
    ) -> Option<Element> {
        let alpha = color
            .alpha()
            .map(|value| (value.clamp(0, 255) as f32) / 255.0)
            .unwrap_or(1.0);
        let (points, vertices_per_row, back_color, extend) = match color.kind() {
            ofdbox_core::ColorKind::GouraudShd => {
                let shd = color.gouraud_shd()?;
                let points = shd
                    .points()
                    .filter_map(|point| self.mesh_point(&point, alpha))
                    .collect::<Vec<_>>();
                let back = shd.back_color();
                let back_color = back.as_ref().and_then(|back| match self.solid_paint(back) {
                    Some(Paint::Solid(mut value)) => {
                        value.a *= alpha;
                        Some(value)
                    }
                    _ => None,
                });
                (points, None, back_color, shd.extend().unwrap_or(0) != 0)
            }
            ofdbox_core::ColorKind::LaGouraudShd => {
                let shd = color.la_gouraud_shd()?;
                let points = shd
                    .points()
                    .filter_map(|point| self.mesh_point(&point, alpha))
                    .collect::<Vec<_>>();
                let back = shd.back_color();
                let back_color = back.as_ref().and_then(|back| match self.solid_paint(back) {
                    Some(Paint::Solid(mut value)) => {
                        value.a *= alpha;
                        Some(value)
                    }
                    _ => None,
                });
                let per_row = shd
                    .vertices_per_row()
                    .and_then(|value| usize::try_from(value).ok());
                (points, per_row, back_color, shd.extend().unwrap_or(0) != 0)
            }
            _ => return None,
        };

        let mut triangles = Vec::new();
        match vertices_per_row {
            Some(per_row) if per_row >= 2 && points.len() >= per_row * 2 => {
                let rows = points.len() / per_row;
                for row in 0..rows - 1 {
                    for column in 0..per_row - 1 {
                        let index = row * per_row + column;
                        let a = points[index];
                        let b = points[index + 1];
                        let c = points[index + per_row];
                        let d = points[index + per_row + 1];
                        triangles.push([a, b, d]);
                        triangles.push([a, d, c]);
                    }
                }
            }
            _ => {
                if points.len() < 3 {
                    self.diagnostic(DiagnosticCode::Paint, object.id(), "网格渐变控制点不足");
                    return None;
                }
                for index in 1..points.len() - 1 {
                    triangles.push([points[0], points[index], points[index + 1]]);
                }
            }
        }
        if triangles.is_empty() {
            return None;
        }
        const MAX_TRIANGLES: usize = 8192;
        if triangles.len() > MAX_TRIANGLES {
            self.diagnostic(DiagnosticCode::Paint, object.id(), "网格渐变单元过多");
            triangles.truncate(MAX_TRIANGLES);
        }
        let mut depth = 3u8;
        while depth > 0
            && triangles.len().saturating_mul(4usize.pow(u32::from(depth))) > MAX_TRIANGLES
        {
            depth -= 1;
        }
        let mut output = Vec::new();
        for triangle in &triangles {
            subdivide_mesh(*triangle, depth, &mut output);
        }

        let mut fill = Group::new(Matrix::IDENTITY);
        fill.clip = Some(Clip {
            path: Arc::new(path.clone()),
            rule,
        });
        if extend && let Some(back) = back_color {
            fill.children.push(Element::Path(PathElement {
                path: Arc::new(path.clone()),
                fill: Some(Paint::Solid(back)),
                stroke: None,
                fill_rule: rule,
            }));
        }
        // 先铺一层原始三角形的平均色，细分块之间的抗锯齿缝隙不会再露出背景色。
        for triangle in &triangles {
            fill.children.push(Element::Path(PathElement {
                path: Arc::new(triangle_path(triangle)),
                fill: Some(Paint::Solid(average_color(triangle))),
                stroke: None,
                fill_rule: FillRule::NonZero,
            }));
        }
        for triangle in output {
            fill.children.push(Element::Path(PathElement {
                path: Arc::new(triangle_path(&triangle)),
                fill: Some(Paint::Solid(average_color(&triangle))),
                stroke: None,
                fill_rule: FillRule::NonZero,
            }));
        }
        Some(Element::Group(fill))
    }

    fn mesh_point(&self, point: &ofdbox_core::PointView, alpha: f32) -> Option<MeshPoint> {
        let position = Point::new(point.x()?, point.y()?);
        let color = point.color()?;
        let mut value = match self.solid_paint(&color)? {
            Paint::Solid(value) => value,
            _ => return None,
        };
        value.a *= alpha;
        Some(MeshPoint {
            position,
            color: value,
        })
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
        let mut read_direction = object
            .attribute("ReadDirection")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(0);
        if !matches!(read_direction, 0 | 90 | 180 | 270) {
            self.diagnostic(
                DiagnosticCode::UnsupportedObject,
                object.id(),
                format!("不支持的 ReadDirection: {read_direction}"),
            );
            read_direction = 0;
        }
        let mut char_direction = object
            .attribute("CharDirection")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(0);
        if !matches!(char_direction, 0 | 90 | 180 | 270) {
            self.diagnostic(
                DiagnosticCode::UnsupportedObject,
                object.id(),
                format!("不支持的 CharDirection: {char_direction}"),
            );
            char_direction = 0;
        }
        let runs = collect_code_runs(object);
        let all_chars = runs
            .iter()
            .flat_map(|run| run.chars.iter().copied())
            .collect::<Vec<_>>();
        if all_chars.is_empty() {
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
        let weight = object
            .attribute("Weight")
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|value| (100..=900).contains(value))
            .unwrap_or(400);
        let italic = parse_bool(object.attribute("Italic")).unwrap_or(false);
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
        // Weight >= 600 且没有显式描边时，用细描边模拟粗体。
        let synthetic_bold = if weight >= 600 && stroke_paint.is_none() {
            fill_paint.as_ref().map(|paint| {
                let style = StrokeStyle {
                    width: (size * 0.03).max(0.01),
                    cap: LineCap::Round,
                    join: LineJoin::Round,
                    ..StrokeStyle::default()
                };
                StrokePaint {
                    paint: paint.clone(),
                    style,
                }
            })
        } else {
            None
        };

        let slots = glyph_slots(object, all_chars.len());

        let mut elements = Vec::new();
        let mut offset = 0usize;
        let rotate_glyphs = char_direction != 0;
        let scale_glyphs = (h_scale - 1.0).abs() > f64::EPSILON;
        let transform_glyphs = rotate_glyphs || scale_glyphs || italic;
        let shear = if italic {
            Matrix::new(1.0, 0.0, -0.21, 1.0, 0.0, 0.0)
        } else {
            Matrix::IDENTITY
        };
        for run in runs {
            let mut x = run.x;
            let mut y = run.y;
            let mut glyphs = Vec::with_capacity(run.chars.len());
            for (index, character) in run.chars.iter().enumerate() {
                let slot = offset + index;
                let covered = matches!(slots.get(slot), Some(Some(_)));
                let slot_glyphs = match slots.get(slot) {
                    Some(Some(glyphs)) => glyphs.clone(),
                    Some(None) | None => face
                        .as_ref()
                        .and_then(|face| face.glyph_index(*character))
                        .map(|id| vec![u32::from(id.0)])
                        .unwrap_or_default(),
                };
                let continuation = covered && slot_glyphs.is_empty();
                let mut cluster_x = x;
                let mut cluster_y = y;
                let mut advance_sum = 0.0;
                for glyph_id in slot_glyphs.iter().copied() {
                    let advance = glyph_advance(face.as_ref(), glyph_id, size);
                    if transform_glyphs {
                        let transform = Matrix::scale(h_scale.max(0.0), 1.0)
                            .then(shear)
                            .then(Matrix::rotate(f64::from(char_direction).to_radians()))
                            .then(Matrix::translate(cluster_x, cluster_y));
                        elements.push(Element::Text(TextElement {
                            font: Arc::clone(&font),
                            size,
                            transform,
                            glyphs: vec![PositionedGlyph {
                                id: glyph_id,
                                x: 0.0,
                                y: 0.0,
                            }],
                            fill: fill_paint.clone(),
                            stroke: stroke_paint.clone().or_else(|| synthetic_bold.clone()),
                        }));
                    } else {
                        glyphs.push(PositionedGlyph {
                            id: glyph_id,
                            x: cluster_x as f32,
                            y: cluster_y as f32,
                        });
                    }
                    advance_sum += advance;
                    match read_direction {
                        180 => cluster_x -= advance * h_scale.max(0.0),
                        90 => cluster_y += advance,
                        270 => cluster_y -= advance,
                        _ => cluster_x += advance * h_scale.max(0.0),
                    }
                }
                let fallback_advance = if continuation {
                    0.0
                } else if slot_glyphs.is_empty() {
                    size
                } else {
                    advance_sum
                };
                let delta_x = run.delta_x.get(index).copied();
                let delta_y = run.delta_y.get(index).copied();
                if let Some(delta) = delta_x {
                    x += delta;
                } else {
                    match read_direction {
                        0 => x += fallback_advance * h_scale.max(0.0),
                        180 => x -= fallback_advance * h_scale.max(0.0),
                        _ => {}
                    }
                }
                if let Some(delta) = delta_y {
                    y += delta;
                } else {
                    match read_direction {
                        90 => y += fallback_advance,
                        270 => y -= fallback_advance,
                        _ => {}
                    }
                }
            }
            offset += run.chars.len();
            if !glyphs.is_empty() {
                elements.push(Element::Text(TextElement {
                    font: Arc::clone(&font),
                    size,
                    transform: Matrix::IDENTITY,
                    glyphs,
                    fill: fill_paint.clone(),
                    stroke: stroke_paint.clone().or_else(|| synthetic_bold.clone()),
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
        let file = match read_resource(self.package, &media.dir, media.base.as_deref(), &media.file)
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
                let bytes =
                    read_resource(self.package, &mask.dir, mask.base.as_deref(), &mask.file)
                        .ok()?;
                let format = ImageFormat::detect(&bytes).unwrap_or(mask.format);
                Some((format, bytes))
            });
        let mask = mask
            .as_ref()
            .map(|(format, bytes)| (format.clone(), &bytes[..]));
        let format = ImageFormat::detect(&file).unwrap_or_else(|| media.format.clone());
        let image = match self.pool.embedded_image(format, &file, mask) {
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

    fn render_templates(
        &mut self,
        references: &[TemplateView],
        order: TemplateOrder,
        output: &mut Vec<Element>,
        visited: &mut HashSet<String>,
    ) {
        for reference in references {
            let Some(id) = reference.template_id() else {
                continue;
            };
            let Some(template) = self.templates.get(id).cloned() else {
                self.diagnostic(
                    DiagnosticCode::UnsupportedObject,
                    reference.template_id(),
                    format!("模板页缺失: {id}"),
                );
                continue;
            };
            let z_order = reference.z_order().unwrap_or(&template.z_order);
            if template_order(z_order) == order {
                self.render_template(&template, output, visited);
            }
        }
    }

    fn render_template(
        &mut self,
        template: &TemplateEntry,
        output: &mut Vec<Element>,
        visited: &mut HashSet<String>,
    ) {
        if !visited.insert(template.id.clone()) {
            self.diagnostic(
                DiagnosticCode::UnsupportedObject,
                Some(&template.id),
                "模板页存在循环引用",
            );
            return;
        }
        let references = template.content.templates().collect::<Vec<_>>();
        self.render_templates(&references, TemplateOrder::Background, output, visited);
        self.render_templates(&references, TemplateOrder::Body, output, visited);
        if let Some(content) = template.content.content() {
            for layer in content.layers() {
                for object in layer.objects() {
                    self.render_object(&object, output);
                }
            }
        }
        self.render_templates(&references, TemplateOrder::Foreground, output, visited);
        visited.remove(&template.id);
    }

    fn font(&mut self, id: &str) -> Option<Arc<crate::asset::FontAsset>> {
        let resource = self.resources.fonts.get(id).cloned()?;
        let key = format!("{}/{id}", resource.dir);
        if let Some(cached) = self.font_cache.get(&key) {
            return cached.clone();
        }
        let embedded = resource.file.as_deref().and_then(|file| {
            read_resource(self.package, &resource.dir, resource.base.as_deref(), file).ok()
        });
        let data = embedded.filter(|bytes| !bytes.is_empty()).or_else(|| {
            let provider = self.font_provider.as_mut()?;
            let request = FontRequest {
                font_name: resource.name.as_deref(),
                family_name: resource.family.as_deref(),
                charset: resource.charset.as_deref(),
                bold: resource.bold,
                italic: resource.italic,
                serif: resource.serif,
                fixed_width: resource.fixed_width,
            };
            provider.load(&request).ok()?
        });
        let asset = data
            .filter(|bytes| !bytes.is_empty())
            .and_then(|bytes| self.pool.embedded_font_shared(bytes, 0).ok());
        self.font_cache.insert(key, asset.clone());
        asset
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
                let (start, end) = gradient_axis(start, end, shd.map_type(), shd.map_unit());
                Some(Paint::Linear(LinearGradient {
                    start,
                    end,
                    stops: self.gradient_stops(shd.segments(), color.alpha()),
                    extend: gradient_extend(shd.map_type()),
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
                let (start_center, end_center) =
                    gradient_axis(start_center, end_center, shd.map_type(), shd.map_unit());
                Some(Paint::Radial(RadialGradient {
                    start_center,
                    start_radius,
                    end_center,
                    end_radius,
                    stops: self.gradient_stops(shd.segments(), color.alpha()),
                    extend: gradient_extend(shd.map_type()),
                    eccentricity: shd.eccentricity().unwrap_or(0.0),
                    angle: shd.angle().unwrap_or(0.0),
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
        let space = color
            .color_space()
            .and_then(|id| self.resources.colors.get(id))
            .or_else(|| {
                self.default_color_space
                    .as_deref()
                    .and_then(|id| self.resources.colors.get(id))
            });
        let bits = space.map_or(8, |space| space.bits);
        let value = if let Some(index) = color.index() {
            if index < 0 {
                return None;
            }
            space?.palette.get(index as usize)?.clone()
        } else {
            color.value()?.to_owned()
        };
        let components = parse_color_components(&value, bits);
        let kind = space.map_or("", |space| space.kind.as_str());
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
    let mut runs = Vec::new();
    let mut last_x = 0.0;
    let mut last_y = 0.0;
    for code in object.text_codes() {
        let x = code.x().unwrap_or(last_x);
        let y = code.y().unwrap_or(last_y);
        last_x = x;
        last_y = y;
        runs.push(CodeRun {
            chars: decode_text(code.text()),
            x,
            y,
            delta_x: code.delta_x().map(parse_floats).unwrap_or_default(),
            delta_y: code.delta_y().map(parse_floats).unwrap_or_default(),
        });
    }
    runs
}

/// 每个字符位置的显式字形。
///
/// - `None`：没有 CGTransform，按字符 CMAP 取字形；
/// - `Some(vec![])`：被簇覆盖但不产生字形（多对一的后续字符）；
/// - `Some(glyphs)`：一个或多个字形，整个簇锚定在该编码位置。
fn glyph_slots(object: &GraphicObjectView, count: usize) -> Vec<Option<Vec<u32>>> {
    let mut slots = vec![None; count];
    for transform in object.cg_transforms() {
        let glyphs = transform.glyphs().map(parse_u32s).unwrap_or_default();
        if glyphs.is_empty() {
            continue;
        }
        let position = transform.code_position().unwrap_or(0).max(0) as usize;
        if position >= count {
            continue;
        }
        let code_count = transform
            .code_count()
            .map(|value| value.max(1) as usize)
            .unwrap_or(1);
        let glyph_count = transform
            .glyph_count()
            .map(|value| value.max(1) as usize)
            .unwrap_or(1);
        if code_count == glyph_count && glyphs.len() == code_count {
            for (index, glyph) in glyphs.into_iter().enumerate() {
                if let Some(slot) = slots.get_mut(position + index) {
                    *slot = Some(vec![glyph]);
                }
            }
        } else {
            if let Some(slot) = slots.get_mut(position) {
                *slot = Some(glyphs);
            }
            for index in 1..code_count {
                if let Some(slot) = slots.get_mut(position + index) {
                    *slot = Some(Vec::new());
                }
            }
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
        if character != '\\' {
            output.push(character);
            continue;
        }
        // 只有完整的 `\XXXX` 才消费后续字符，否则保留反斜杠本身。
        let mut lookahead = chars.clone();
        let mut hex = String::with_capacity(4);
        for _ in 0..4 {
            match lookahead.next() {
                Some(next) if next.is_ascii_hexdigit() => hex.push(next),
                _ => {
                    hex.clear();
                    break;
                }
            }
        }
        if hex.len() == 4
            && let Ok(value) = u32::from_str_radix(&hex, 16)
            && let Some(decoded) = char::from_u32(value)
        {
            for _ in 0..4 {
                chars.next();
            }
            output.push(decoded);
        } else {
            output.push('\\');
        }
    }
    output
}

fn append_transformed_path(target: &mut Path, source: &Path, matrix: Matrix) {
    let scale = ((matrix.a * matrix.a + matrix.b * matrix.b).sqrt()
        + (matrix.c * matrix.c + matrix.d * matrix.d).sqrt())
        / 2.0;
    let extra_rotation = matrix.b.atan2(matrix.a);
    for segment in &source.segments {
        match *segment {
            PathSegment::MoveTo(point) => target.move_to(matrix.apply(point)),
            PathSegment::LineTo(point) => target.line_to(matrix.apply(point)),
            PathSegment::QuadraticTo { control, to } => {
                target.quadratic_to(matrix.apply(control), matrix.apply(to));
            }
            PathSegment::CubicTo {
                control1,
                control2,
                to,
            } => {
                target.cubic_to(
                    matrix.apply(control1),
                    matrix.apply(control2),
                    matrix.apply(to),
                );
            }
            PathSegment::ArcTo {
                rx,
                ry,
                rotation,
                large_arc,
                sweep,
                to,
            } => {
                target.arc_to(
                    rx * scale,
                    ry * scale,
                    rotation + extra_rotation,
                    large_arc,
                    sweep,
                    matrix.apply(to),
                );
            }
            PathSegment::Close => target.close(),
        }
    }
}

fn parse_path_data(data: &str) -> Option<Path> {
    let tokens = tokenize_path(data)?;
    let mut path = Path::new();
    let mut index = 0usize;
    let mut command = None;
    while index < tokens.len() {
        if let PathToken::Command(value) = tokens[index] {
            command = Some(value);
            index += 1;
            if command == Some('C') {
                path.close();
                continue;
            }
        }
        let command = command?;
        match command {
            // S 定义子路径起点，M 移动当前点：两者都开始新的子路径。
            'S' | 'M' => {
                let x = number(tokens.get(index)?)?;
                let y = number(tokens.get(index + 1)?)?;
                index += 2;
                path.move_to(Point::new(x, y));
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum PathToken {
    Command(char),
    Number(f64),
}

/// 紧缩路径数据的词法切分。
///
/// 标准只说明 `AbbreviatedData` 是“紧缩的操作符和操作数”，并未规定分隔符；
/// 真实文件里既有 `M 0 0 L 1 0` 也有 `M 0 0L 1 0`。这里按字母/数字边界扫描，
/// 同时接受空格、逗号、正负号和小数点作为隐式分隔符。
fn tokenize_path(data: &str) -> Option<Vec<PathToken>> {
    let bytes = data.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() || byte == b',' {
            index += 1;
            continue;
        }
        if byte.is_ascii_alphabetic() {
            tokens.push(PathToken::Command((byte as char).to_ascii_uppercase()));
            index += 1;
            continue;
        }
        let start = index;
        if byte == b'+' || byte == b'-' {
            index += 1;
        }
        let mut digits = false;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
            digits = true;
        }
        if index < bytes.len() && bytes[index] == b'.' {
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
                digits = true;
            }
        }
        if !digits {
            return None;
        }
        if index < bytes.len() && (bytes[index] == b'e' || bytes[index] == b'E') {
            let exponent = index;
            index += 1;
            if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
                index += 1;
            }
            let mut exponent_digits = false;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
                exponent_digits = true;
            }
            if !exponent_digits {
                index = exponent;
            }
        }
        tokens.push(PathToken::Number(data[start..index].parse::<f64>().ok()?));
    }
    Some(tokens)
}

fn number(token: &PathToken) -> Option<f64> {
    match token {
        PathToken::Number(value) => Some(*value),
        PathToken::Command(_) => None,
    }
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

/// 解析颜色各通道值并归一化到 `[0, 1]`。
///
/// 标准允许十进制或 `#` 开头的十六进制，取值范围 `[0, 2^BitsPerComponent - 1]`；
/// 真实文件里也能见到 `0.0 0.0 0.0` 这类已归一化的写法，按小数处理。
fn parse_color_components(value: &str, bits: u32) -> Vec<f64> {
    let bits = bits.clamp(1, 16);
    let max = f64::from((1u32 << bits) - 1);
    value
        .split(|character: char| character.is_whitespace() || character == ',')
        .filter(|token| !token.is_empty())
        .filter_map(|token| parse_color_component(token, max))
        .collect()
}

fn parse_color_component(token: &str, max: f64) -> Option<f64> {
    let token = token.trim();
    if let Some(hex) = token.strip_prefix('#') {
        let value = i64::from_str_radix(hex, 16).ok()?;
        return Some((value as f64 / max).clamp(0.0, 1.0));
    }
    let value = token.parse::<f64>().ok()?;
    if token.contains('.') || token.contains('e') || token.contains('E') {
        Some(value.clamp(0.0, 1.0))
    } else {
        Some((value / max).clamp(0.0, 1.0))
    }
}

fn components_to_rgba(components: &[f64], kind: &str, alpha: f32) -> Option<Rgba> {
    let normalized = |value: f64| value.clamp(0.0, 1.0) as f32;
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

/// `MapType` 决定渐变区间内如何重复；`Extend` 的 0/1/2/3 是端点延伸，
/// 后端只有 Pad/Repeat/Reflect 三种模式，这里对 Direct 统一用 Pad 近似。
fn gradient_extend(map_type: Option<&str>) -> GradientExtend {
    match map_type {
        Some(value) if value.eq_ignore_ascii_case("Repeat") => GradientExtend::Repeat,
        Some(value) if value.eq_ignore_ascii_case("Reflect") => GradientExtend::Reflect,
        _ => GradientExtend::Pad,
    }
}

/// 非 Direct 时，一个渐变区间的长度由 `MapUnit` 决定（默认轴线长度），
/// 因此把终点缩放到距起点 `MapUnit` 的位置，再交给后端的 Repeat/Reflect。
fn gradient_axis(
    start: Point,
    end: Point,
    map_type: Option<&str>,
    map_unit: Option<f64>,
) -> (Point, Point) {
    let repeating = map_type.is_some_and(|value| {
        value.eq_ignore_ascii_case("Repeat") || value.eq_ignore_ascii_case("Reflect")
    });
    if !repeating {
        return (start, end);
    }
    let length = distance(start, end);
    let unit = map_unit
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(length);
    if length <= f64::EPSILON || !unit.is_finite() || unit <= 0.0 {
        return (start, end);
    }
    let dx = (end.x - start.x) / length;
    let dy = (end.y - start.y) / length;
    (start, Point::new(start.x + dx * unit, start.y + dy * unit))
}

/// Direct 渐变在 `Extend` 不允许延伸的方向上不应继续绘制。
/// 返回一个裁剪路径，用于把填充限制在渐变实际覆盖的区域内。
fn gradient_fill_clip(color: &ColorView) -> Option<Path> {
    match color.kind() {
        ofdbox_core::ColorKind::AxialShd => {
            let shd = color.axial_shd()?;
            if !is_direct(shd.map_type()) {
                return None;
            }
            let extend = shd.extend().unwrap_or(0);
            if extend == 3 {
                return None;
            }
            let start = shd.start_point().and_then(parse_point)?;
            let end = shd.end_point().and_then(parse_point)?;
            axial_clip_path(start, end, extend)
        }
        ofdbox_core::ColorKind::RadialShd => {
            let shd = color.radial_shd()?;
            if !is_direct(shd.map_type()) {
                return None;
            }
            let extend = shd.extend().unwrap_or(0);
            if extend == 3 {
                return None;
            }
            let start = shd.start_point().and_then(parse_point)?;
            let end = shd.end_point().and_then(parse_point)?;
            let start_radius = shd.start_radius().unwrap_or(0.0);
            let end_radius = shd.end_radius().unwrap_or_else(|| distance(start, end));
            let eccentricity = shd.eccentricity().unwrap_or(0.0).clamp(0.0, 0.999);
            let ratio = (1.0 - eccentricity * eccentricity).max(1e-4).sqrt();
            let angle = shd.angle().unwrap_or(0.0);
            radial_clip_path(start, start_radius, end, end_radius, ratio, angle, extend)
        }
        _ => None,
    }
}

fn is_direct(map_type: Option<&str>) -> bool {
    map_type.is_none_or(|value| value.eq_ignore_ascii_case("Direct"))
}

fn axial_clip_path(start: Point, end: Point, extend: i32) -> Option<Path> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length <= f64::EPSILON {
        return None;
    }
    let unit_x = dx / length;
    let unit_y = dy / length;
    let normal_x = -unit_y;
    let normal_y = unit_x;
    const BIG: f64 = 1.0e5;
    let along = |origin: Point, distance: f64| {
        Point::new(origin.x + unit_x * distance, origin.y + unit_y * distance)
    };
    let offset = |origin: Point, distance: f64| {
        Point::new(
            origin.x + normal_x * distance,
            origin.y + normal_y * distance,
        )
    };
    let (from, to) = match extend {
        0 => (start, end),
        1 => (along(start, -BIG), end),
        2 => (start, along(end, BIG)),
        _ => return None,
    };
    let mut path = Path::new();
    path.move_to(offset(from, BIG));
    path.line_to(offset(to, BIG));
    path.line_to(offset(to, -BIG));
    path.line_to(offset(from, -BIG));
    path.close();
    Some(path)
}

#[allow(clippy::too_many_arguments)]
fn radial_clip_path(
    start: Point,
    start_radius: f64,
    end: Point,
    end_radius: f64,
    ratio: f64,
    angle: f64,
    extend: i32,
) -> Option<Path> {
    let end_rx = end_radius.max(0.0);
    let end_ry = (end_radius * ratio).max(0.0);
    let start_rx = start_radius.max(0.0);
    let start_ry = (start_radius * ratio).max(0.0);
    match extend {
        0 => {
            if end_rx <= 0.0 || end_ry <= 0.0 {
                return Some(Path::new());
            }
            let mut path = ellipse_path(end, end_rx, end_ry, angle, false);
            if start_rx > 0.0 && start_ry > 0.0 {
                append_ellipse(&mut path, start, start_rx, start_ry, angle, true);
            }
            Some(path)
        }
        1 => {
            if end_rx <= 0.0 || end_ry <= 0.0 {
                return Some(Path::new());
            }
            Some(ellipse_path(end, end_rx, end_ry, angle, false))
        }
        2 => {
            if start_rx <= 0.0 || start_ry <= 0.0 {
                // 起始半径为 0 时，“起始椭圆之外”就是整个平面。
                return None;
            }
            const BIG: f64 = 1.0e5;
            let mut path = Path::new();
            path.move_to(Point::new(start.x - BIG, start.y - BIG));
            path.line_to(Point::new(start.x + BIG, start.y - BIG));
            path.line_to(Point::new(start.x + BIG, start.y + BIG));
            path.line_to(Point::new(start.x - BIG, start.y + BIG));
            path.close();
            append_ellipse(&mut path, start, start_rx, start_ry, angle, true);
            Some(path)
        }
        _ => None,
    }
}

fn ellipse_path(center: Point, rx: f64, ry: f64, angle: f64, reverse: bool) -> Path {
    let mut path = Path::new();
    append_ellipse(&mut path, center, rx, ry, angle, reverse);
    path
}

fn append_ellipse(path: &mut Path, center: Point, rx: f64, ry: f64, angle: f64, reverse: bool) {
    const STEPS: usize = 64;
    let (sin_a, cos_a) = angle.to_radians().sin_cos();
    for step in 0..=STEPS {
        let mut t = std::f64::consts::TAU * step as f64 / STEPS as f64;
        if reverse {
            t = -t;
        }
        let (sin_t, cos_t) = t.sin_cos();
        let x = rx * cos_t;
        let y = ry * sin_t;
        let point = Point::new(
            center.x + cos_a * x - sin_a * y,
            center.y + sin_a * x + cos_a * y,
        );
        if step == 0 {
            path.move_to(point);
        } else {
            path.line_to(point);
        }
    }
    path.close();
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

#[derive(Clone, Copy, Debug)]
struct MeshPoint {
    position: Point,
    color: Rgba,
}

fn mesh_midpoint(a: MeshPoint, b: MeshPoint) -> MeshPoint {
    let mix = |a: f32, b: f32| (a + b) * 0.5;
    MeshPoint {
        position: Point::new(
            (a.position.x + b.position.x) * 0.5,
            (a.position.y + b.position.y) * 0.5,
        ),
        color: Rgba::new(
            mix(a.color.r, b.color.r),
            mix(a.color.g, b.color.g),
            mix(a.color.b, b.color.b),
            mix(a.color.a, b.color.a),
        ),
    }
}

fn subdivide_mesh(triangle: [MeshPoint; 3], depth: u8, output: &mut Vec<[MeshPoint; 3]>) {
    if depth == 0 {
        output.push(triangle);
        return;
    }
    let [a, b, c] = triangle;
    let ab = mesh_midpoint(a, b);
    let bc = mesh_midpoint(b, c);
    let ca = mesh_midpoint(c, a);
    subdivide_mesh([a, ab, ca], depth - 1, output);
    subdivide_mesh([ab, b, bc], depth - 1, output);
    subdivide_mesh([ca, bc, c], depth - 1, output);
    subdivide_mesh([ab, bc, ca], depth - 1, output);
}

/// 相邻纯色三角形各自抗锯齿会在公共边留下白缝，渲染时把顶点向外扩一点制造重叠。
const MESH_OVERLAP: f64 = 0.25;

fn triangle_path(triangle: &[MeshPoint; 3]) -> Path {
    let center_x = (triangle[0].position.x + triangle[1].position.x + triangle[2].position.x) / 3.0;
    let center_y = (triangle[0].position.y + triangle[1].position.y + triangle[2].position.y) / 3.0;
    let expand = |position: Point| {
        let dx = position.x - center_x;
        let dy = position.y - center_y;
        let length = (dx * dx + dy * dy).sqrt();
        if length <= f64::EPSILON {
            position
        } else {
            Point::new(
                position.x + dx / length * MESH_OVERLAP,
                position.y + dy / length * MESH_OVERLAP,
            )
        }
    };
    let mut path = Path::new();
    path.move_to(expand(triangle[0].position));
    path.line_to(expand(triangle[1].position));
    path.line_to(expand(triangle[2].position));
    path.close();
    path
}

fn average_color(triangle: &[MeshPoint; 3]) -> Rgba {
    let [a, b, c] = triangle;
    let mix = |a: f32, b: f32, c: f32| (a + b + c) / 3.0;
    Rgba::new(
        mix(a.color.r, b.color.r, c.color.r),
        mix(a.color.g, b.color.g, c.color.g),
        mix(a.color.b, b.color.b, c.color.b),
        mix(a.color.a, b.color.a, c.color.a),
    )
}

fn parent_dir(path: &str) -> &str {
    path.rfind('/').map(|index| &path[..index]).unwrap_or("")
}

/// 真实文件既有相对 Document.xml 的路径，也有已经包含 `Doc_0/` 的根相对路径。
fn resolve_existing(package: &OfdPackage, base: &str, loc: &str) -> Option<String> {
    let primary = package.resolve_entry(base, loc).ok()?;
    if package.contains(&primary) {
        return Some(primary);
    }
    let root = package.resolve_entry("", loc).ok()?;
    if package.contains(&root) {
        return Some(root);
    }
    None
}

fn read_resource(
    package: &mut OfdPackage,
    dir: &str,
    base: Option<&str>,
    file: &str,
) -> Result<Arc<[u8]>, OfdError> {
    let mut candidates = Vec::new();
    if let Some(base) = base {
        let base = base.trim_end_matches('/');
        if let Ok(entry) = package.resolve_entry(dir, &format!("{base}/{file}")) {
            candidates.push(entry);
        }
        if let Ok(entry) = package.resolve_entry("", &format!("{base}/{file}")) {
            candidates.push(entry);
        }
    }
    if let Ok(entry) = package.resolve_entry(dir, file) {
        candidates.push(entry);
    }
    if let Ok(entry) = package.resolve_entry("", file) {
        candidates.push(entry);
    }
    for entry in &candidates {
        if package.contains(entry) {
            return package.read_entry(entry);
        }
    }
    // 真实文件里资源引用和 ZIP 条目大小写经常不一致。
    if let Ok(names) = package.entry_names() {
        for candidate in &candidates {
            if let Some(name) = names
                .iter()
                .find(|name| name.eq_ignore_ascii_case(candidate))
            {
                return package.read_entry(name);
            }
        }
    }
    Err(OfdError::MissingEntry(file.to_owned()))
}

#[derive(Clone)]
struct TemplateEntry {
    id: String,
    z_order: String,
    entry: String,
    content: ofdbox_core::PageContentView,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TemplateOrder {
    Background,
    Body,
    Foreground,
}

fn template_order(value: &str) -> TemplateOrder {
    if value.eq_ignore_ascii_case("Foreground") {
        TemplateOrder::Foreground
    } else if value.eq_ignore_ascii_case("Body") {
        TemplateOrder::Body
    } else {
        TemplateOrder::Background
    }
}

#[cfg(test)]
mod tests {
    use super::{components_to_rgba, parse_color_components, parse_path_data};
    use crate::geometry::Point;
    use crate::paint::Rgba;
    use crate::path::PathSegment;

    #[test]
    fn parses_spaced_path() {
        let path = parse_path_data("M 0 0 L 10 0 L 10 10 C").expect("path");
        assert_eq!(path.segments.len(), 4);
        assert_eq!(path.segments[0], PathSegment::MoveTo(Point::new(0.0, 0.0)));
        assert_eq!(path.segments[1], PathSegment::LineTo(Point::new(10.0, 0.0)));
        assert_eq!(path.segments[3], PathSegment::Close);
    }

    #[test]
    fn parses_glued_commands() {
        let path =
            parse_path_data("M 0 0L 215.9 0L 215.9 1152.172L 0 1152.172L 0 0 C").expect("path");
        assert_eq!(path.segments.len(), 6);
        assert_eq!(path.segments[0], PathSegment::MoveTo(Point::new(0.0, 0.0)));
        assert_eq!(
            path.segments[1],
            PathSegment::LineTo(Point::new(215.9, 0.0))
        );
        assert_eq!(
            path.segments[2],
            PathSegment::LineTo(Point::new(215.9, 1152.172))
        );
        assert_eq!(path.segments[5], PathSegment::Close);
    }

    #[test]
    fn parses_numbers_without_separators() {
        // 负号和小数点作为数字之间的隐式分隔符。
        let path = parse_path_data("M1-2L3.5.5").expect("path");
        assert_eq!(
            path.segments,
            vec![
                PathSegment::MoveTo(Point::new(1.0, -2.0)),
                PathSegment::LineTo(Point::new(3.5, 0.5)),
            ]
        );
    }

    #[test]
    fn parses_exponent_and_comma() {
        let path = parse_path_data("M0,0L1e1,-1.5e-1").expect("path");
        assert_eq!(
            path.segments,
            vec![
                PathSegment::MoveTo(Point::new(0.0, 0.0)),
                PathSegment::LineTo(Point::new(10.0, -0.15)),
            ]
        );
    }

    #[test]
    fn starts_new_subpath_on_every_move() {
        let path = parse_path_data("M 0 0 L 1 0 M 2 2 L 3 3").expect("path");
        assert_eq!(
            path.segments,
            vec![
                PathSegment::MoveTo(Point::new(0.0, 0.0)),
                PathSegment::LineTo(Point::new(1.0, 0.0)),
                PathSegment::MoveTo(Point::new(2.0, 2.0)),
                PathSegment::LineTo(Point::new(3.0, 3.0)),
            ]
        );
    }

    #[test]
    fn maps_gradient_extend_and_map_unit() {
        use super::{gradient_axis, gradient_extend};
        use crate::paint::GradientExtend;

        assert_eq!(gradient_extend(Some("Repeat")), GradientExtend::Repeat);
        assert_eq!(gradient_extend(Some("Reflect")), GradientExtend::Reflect);
        assert_eq!(gradient_extend(Some("Direct")), GradientExtend::Pad);
        assert_eq!(gradient_extend(None), GradientExtend::Pad);

        let (_, end) = gradient_axis(
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Some("Repeat"),
            Some(25.0),
        );
        assert_eq!(end, Point::new(25.0, 0.0));

        // Direct 不使用 MapUnit。
        let (_, end) = gradient_axis(
            Point::new(0.0, 0.0),
            Point::new(0.0, 100.0),
            None,
            Some(25.0),
        );
        assert_eq!(end, Point::new(0.0, 100.0));
    }

    #[test]
    fn decodes_text_escapes() {
        use super::decode_text;

        assert_eq!(decode_text("A\\0042C"), vec!['A', 'B', 'C']);
        assert_eq!(decode_text("\\0020"), vec![' ']);
        assert_eq!(decode_text("\\00ZZ"), vec!['\\', '0', '0', 'Z', 'Z']);
        assert_eq!(decode_text("\\zzzz"), vec!['\\', 'z', 'z', 'z', 'z']);
    }

    #[test]
    fn parses_hex_colors_with_bpc() {
        assert_eq!(
            parse_color_components("#ff #0 #80", 8),
            vec![1.0, 0.0, 128.0 / 255.0]
        );
        assert_eq!(
            parse_color_components("#ffff #0000 #8000", 16),
            vec![1.0, 0.0, 32768.0 / 65535.0]
        );
        assert_eq!(parse_color_components("#1 #0", 1), vec![1.0, 0.0]);
    }

    #[test]
    fn parses_integer_and_fractional_color_values() {
        assert_eq!(
            parse_color_components("255 0 128", 8),
            vec![1.0, 0.0, 128.0 / 255.0]
        );
        assert_eq!(
            parse_color_components("0.5 0.25 0.0", 8),
            vec![0.5, 0.25, 0.0]
        );
        assert_eq!(parse_color_components("65535 0 0", 16), vec![1.0, 0.0, 0.0]);
    }

    #[test]
    fn converts_rgb_and_cmyk() {
        assert_eq!(
            components_to_rgba(&[1.0, 0.0, 0.0], "RGB", 1.0),
            Some(Rgba::new(1.0, 0.0, 0.0, 1.0))
        );
        assert_eq!(
            components_to_rgba(&[0.0, 0.0, 0.0, 0.0], "CMYK", 1.0),
            Some(Rgba::new(1.0, 1.0, 1.0, 1.0))
        );
    }

    #[test]
    fn rejects_incomplete_or_unknown_commands() {
        assert!(parse_path_data("M 0").is_none());
        assert!(parse_path_data("X 0 0").is_none());
        assert!(parse_path_data("M 0 0 L 1").is_none());
    }
}
