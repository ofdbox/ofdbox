//! PDF 页面前端：把 PDF 页面转换为与 OFD 共用的 [`Scene`]。

use std::collections::HashMap;
use std::sync::Arc;

use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::asset::FontAsset;
use crate::error::RenderError;
use crate::geometry::{Matrix, Point, Size};
use crate::image::ImageFormat;
use crate::paint::{Paint, Rgba};
use crate::path::{FillRule, LineCap, LineJoin, Path, StrokeStyle};
use crate::resource::ResourcePool;
use crate::scene::{
    DiagnosticCode, Element, Group, ImageElement, PathElement, PositionedGlyph, RenderDiagnostic,
    Scene, StrokePaint, TextElement,
};

const PT_TO_MM: f64 = 25.4 / 72.0;
const MAX_CONTENT_BYTES: usize = 64 * 1024 * 1024;

type PageBox = (f64, f64, f64, f64);

/// PDF 文档。
pub struct PdfFrontend {
    document: Document,
    fonts: HashMap<Vec<u8>, Option<Arc<FontState>>>,
}

struct FontState {
    font: Arc<FontAsset>,
    identity: bool,
    to_unicode: HashMap<u32, u32>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct ClipState {
    path: Arc<Path>,
    rule: FillRule,
    transform: Matrix,
}

#[derive(Clone)]
struct State {
    ctm: Matrix,
    fill: Paint,
    stroke: Paint,
    style: StrokeStyle,
    fill_alpha: f32,
    stroke_alpha: f32,
    clip: Option<ClipState>,
    font: Option<Arc<FontState>>,
    font_size: f64,
    char_spacing: f64,
    word_spacing: f64,
    horizontal_scale: f64,
    leading: f64,
    text_matrix: Matrix,
    line_matrix: Matrix,
}

impl Default for State {
    fn default() -> Self {
        Self {
            ctm: Matrix::IDENTITY,
            fill: Paint::Solid(Rgba::BLACK),
            stroke: Paint::Solid(Rgba::BLACK),
            style: StrokeStyle::default(),
            fill_alpha: 1.0,
            stroke_alpha: 1.0,
            clip: None,
            font: None,
            font_size: 0.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            horizontal_scale: 1.0,
            leading: 0.0,
            text_matrix: Matrix::IDENTITY,
            line_matrix: Matrix::IDENTITY,
        }
    }
}

impl Default for PdfFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfFrontend {
    /// 创建空前端；加载前调用 [`Self::from_bytes`] 或 [`Self::open`]。
    pub fn new() -> Self {
        Self {
            document: Document::new(),
            fonts: HashMap::new(),
        }
    }

    /// 从文件加载 PDF。
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, RenderError> {
        let document = Document::load(path).map_err(pdf_error)?;
        Ok(Self {
            document,
            fonts: HashMap::new(),
        })
    }

    /// 从内存加载 PDF。
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, RenderError> {
        let document = Document::load_mem(bytes.as_ref()).map_err(pdf_error)?;
        Ok(Self {
            document,
            fonts: HashMap::new(),
        })
    }

    /// 页数。
    pub fn page_count(&self) -> usize {
        self.document.get_pages().len()
    }

    /// 页面尺寸，单位为毫米。
    pub fn page_size(&self, index: usize) -> Result<Size, RenderError> {
        let (_, box_) = self.page_box(index)?;
        Ok(Size::new(
            (box_.2 - box_.0) * PT_TO_MM,
            (box_.3 - box_.1) * PT_TO_MM,
        ))
    }

    /// 构建页面场景。
    pub fn build_page(&mut self, index: usize, pool: &ResourcePool) -> Result<Scene, RenderError> {
        let page_id = *self
            .document
            .get_pages()
            .get(&u32::try_from(index + 1).map_err(|_| RenderError::InvalidPage)?)
            .ok_or(RenderError::InvalidPage)?;
        let page = self
            .document
            .get_dictionary(page_id)
            .map_err(pdf_error)?
            .clone();
        let (x0, y0, x1, y1) = self.page_box(index)?.1;
        let width = (x1 - x0) * PT_TO_MM;
        let height = (y1 - y0) * PT_TO_MM;
        let page_transform = Matrix::scale(PT_TO_MM, -PT_TO_MM)
            .then(Matrix::translate(-x0 * PT_TO_MM, y0 * PT_TO_MM + height));
        let resources = page
            .get(b"Resources")
            .ok()
            .and_then(|object| self.resolve(object))
            .and_then(|object| object.as_dict().ok())
            .cloned()
            .unwrap_or_default();
        let content = self
            .document
            .get_page_content_with_limit(page_id, MAX_CONTENT_BYTES)
            .map_err(pdf_error)?;
        let content = Content::decode(&content).map_err(pdf_error)?;
        self.fonts.clear();
        let mut scene = Scene {
            size: Size::new(width, height),
            root: Group::new(page_transform),
            diagnostics: Vec::new(),
        };
        let mut state = State::default();
        let mut stack = Vec::new();
        let mut path = Path::new();
        let mut current = Point::default();
        let mut pending_clip: Option<ClipState> = None;
        self.execute(
            &content.operations,
            &resources,
            pool,
            &mut scene,
            &mut state,
            &mut stack,
            &mut path,
            &mut current,
            &mut pending_clip,
            0,
        )?;
        Ok(scene)
    }

    /// 兼容旧占位接口；PDF 页面需要索引和资源池。
    pub fn build_scene(&mut self) -> Result<Scene, RenderError> {
        Err(RenderError::Unsupported(
            "PDF 前端请使用 build_page(page_index, pool)".to_owned(),
        ))
    }

    fn resolve<'a>(&'a self, object: &'a Object) -> Option<&'a Object> {
        self.document
            .dereference(object)
            .ok()
            .map(|(_, object)| object)
    }

    fn page_box(&self, index: usize) -> Result<(ObjectId, PageBox), RenderError> {
        let page_id = *self
            .document
            .get_pages()
            .get(&u32::try_from(index + 1).map_err(|_| RenderError::InvalidPage)?)
            .ok_or(RenderError::InvalidPage)?;
        let page = self.document.get_dictionary(page_id).map_err(pdf_error)?;
        for key in [b"CropBox".as_slice(), b"MediaBox".as_slice()] {
            if let Ok(object) = page.get(key)
                && let Some(resolved) = self.resolve(object)
                && let Ok(array) = resolved.as_array()
                && let [a, b, c, d] = array.as_slice()
                && let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
                    object_number(a),
                    object_number(b),
                    object_number(c),
                    object_number(d),
                )
            {
                return Ok((page_id, (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))));
            }
        }
        Ok((page_id, (0.0, 0.0, 595.0, 842.0)))
    }

    #[allow(clippy::too_many_arguments)]
    fn execute(
        &mut self,
        operations: &[Operation],
        resources: &Dictionary,
        pool: &ResourcePool,
        scene: &mut Scene,
        state: &mut State,
        stack: &mut Vec<State>,
        path: &mut Path,
        current: &mut Point,
        pending_clip: &mut Option<ClipState>,
        depth: usize,
    ) -> Result<(), RenderError> {
        for operation in operations {
            let op = operation.operator.as_bytes();
            let operands = &operation.operands;
            match op {
                b"q" => stack.push(state.clone()),
                b"Q" => {
                    if let Some(previous) = stack.pop() {
                        *state = previous;
                    }
                }
                b"cm" => {
                    if let Some(matrix) = matrix_operands(operands) {
                        state.ctm = matrix.then(state.ctm);
                    }
                }
                b"w" => {
                    if let Some(width) = operands.first().and_then(object_number) {
                        state.style.width = width;
                    }
                }
                b"J" => {
                    if let Some(cap) = operands.first().and_then(object_number) {
                        state.style.cap = match cap as i64 {
                            1 => LineCap::Round,
                            2 => LineCap::Square,
                            _ => LineCap::Butt,
                        };
                    }
                }
                b"j" => {
                    if let Some(join) = operands.first().and_then(object_number) {
                        state.style.join = match join as i64 {
                            1 => LineJoin::Round,
                            2 => LineJoin::Bevel,
                            _ => LineJoin::Miter,
                        };
                    }
                }
                b"M" => {
                    if let Some(limit) = operands.first().and_then(object_number) {
                        state.style.miter_limit = limit;
                    }
                }
                b"d" => {
                    if let Some(Object::Array(values)) = operands.first() {
                        state.style.dash = values.iter().filter_map(object_number).collect();
                        state.style.dash_offset =
                            operands.get(1).and_then(object_number).unwrap_or(0.0);
                    }
                }
                b"rg" | b"RG" => {
                    if let Some(color) = rgb_operands(operands) {
                        if op == b"rg" {
                            state.fill = Paint::Solid(color);
                        } else {
                            state.stroke = Paint::Solid(color);
                        }
                    }
                }
                b"g" | b"G" => {
                    if let Some(value) = operands.first().and_then(object_number) {
                        let color = Rgba::new(value as f32, value as f32, value as f32, 1.0);
                        if op == b"g" {
                            state.fill = Paint::Solid(color);
                        } else {
                            state.stroke = Paint::Solid(color);
                        }
                    }
                }
                b"k" | b"K" => {
                    if let Some(color) = cmyk_operands(operands) {
                        if op == b"k" {
                            state.fill = Paint::Solid(color);
                        } else {
                            state.stroke = Paint::Solid(color);
                        }
                    }
                }
                b"gs" => {
                    if let Some(name) = operands.first().and_then(object_name)
                        && let Some(dictionary) = resources
                            .get(b"ExtGState")
                            .ok()
                            .and_then(|object| self.resolve(object))
                            .and_then(|object| object.as_dict().ok())
                            .and_then(|dictionary| dictionary.get(name).ok())
                            .and_then(|object| self.resolve(object))
                            .and_then(|object| object.as_dict().ok())
                    {
                        if let Some(alpha) = dictionary.get(b"ca").ok().and_then(object_number) {
                            state.fill_alpha = alpha as f32;
                        }
                        if let Some(alpha) = dictionary.get(b"CA").ok().and_then(object_number) {
                            state.stroke_alpha = alpha as f32;
                        }
                    }
                }
                b"m" => {
                    if let Some(point) = point_operands(operands) {
                        path.move_to(point);
                        *current = point;
                    }
                }
                b"l" => {
                    if let Some(point) = point_operands(operands) {
                        path.line_to(point);
                        *current = point;
                    }
                }
                b"c" => {
                    if let Some((c1, c2, to)) = cubic_operands(operands) {
                        path.cubic_to(c1, c2, to);
                        *current = to;
                    }
                }
                b"v" => {
                    if let Some((c2, to)) = quadratic_operands(operands) {
                        path.cubic_to(*current, c2, to);
                        *current = to;
                    }
                }
                b"y" => {
                    if let Some((c1, to)) = quadratic_operands(operands) {
                        path.cubic_to(c1, to, to);
                        *current = to;
                    }
                }
                b"h" => path.close(),
                b"re" => {
                    if let Some((x, y, width, height)) = rect_operands(operands) {
                        path.move_to(Point::new(x, y));
                        path.line_to(Point::new(x + width, y));
                        path.line_to(Point::new(x + width, y + height));
                        path.line_to(Point::new(x, y + height));
                        path.close();
                        *current = Point::new(x, y);
                    }
                }
                b"W" | b"W*" => {
                    *pending_clip = Some(ClipState {
                        path: Arc::new(path.clone()),
                        rule: if op == b"W*" {
                            FillRule::EvenOdd
                        } else {
                            FillRule::NonZero
                        },
                        transform: state.ctm,
                    });
                }
                b"n" => {
                    if let Some(clip) = pending_clip.take() {
                        state.clip = Some(clip);
                    }
                    path.segments.clear();
                }
                b"S" | b"s" | b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" => {
                    let fill = matches!(op, b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*");
                    let stroke = matches!(op, b"S" | b"s" | b"B" | b"B*" | b"b" | b"b*");
                    let even_odd = matches!(op, b"f*" | b"B*" | b"b*");
                    if op == b"b" || op == b"b*" || op == b"s" {
                        path.close();
                    }
                    if !path.is_empty() {
                        if let Some(clip) = pending_clip.take() {
                            state.clip = Some(clip);
                        }
                        let fill = fill
                            .then(|| color_with_alpha(&state.fill, state.fill_alpha))
                            .flatten();
                        let stroke = stroke
                            .then(|| color_with_alpha(&state.stroke, state.stroke_alpha))
                            .flatten()
                            .map(|paint| StrokePaint {
                                paint,
                                style: state.style.clone(),
                            });
                        push_element(
                            &mut scene.root,
                            state,
                            Element::Path(PathElement {
                                path: Arc::new(path.clone()),
                                fill,
                                stroke,
                                fill_rule: if even_odd {
                                    FillRule::EvenOdd
                                } else {
                                    FillRule::NonZero
                                },
                            }),
                        );
                    }
                    path.segments.clear();
                    if let Some(clip) = pending_clip.take() {
                        state.clip = Some(clip);
                    }
                }
                b"BT" => {
                    state.text_matrix = Matrix::IDENTITY;
                    state.line_matrix = Matrix::IDENTITY;
                }
                b"ET" => {}
                b"Tf" => {
                    if let Some(size) = operands.get(1).and_then(object_number) {
                        state.font_size = size;
                    }
                    if let Some(name) = operands.first().and_then(object_name) {
                        state.font = self.load_font(name, resources, pool);
                    }
                }
                b"Tm" => {
                    if let Some(matrix) = matrix_operands(operands) {
                        state.text_matrix = matrix;
                        state.line_matrix = matrix;
                    }
                }
                b"Td" => {
                    if let Some((x, y)) = pair_operands(operands) {
                        state.line_matrix = Matrix::translate(x, y).then(state.line_matrix);
                        state.text_matrix = state.line_matrix;
                    }
                }
                b"TD" => {
                    if let Some((x, y)) = pair_operands(operands) {
                        state.leading = -y;
                        state.line_matrix = Matrix::translate(x, y).then(state.line_matrix);
                        state.text_matrix = state.line_matrix;
                    }
                }
                b"T*" => {
                    state.line_matrix =
                        Matrix::translate(0.0, -state.leading).then(state.line_matrix);
                    state.text_matrix = state.line_matrix;
                }
                b"Tc" => {
                    if let Some(value) = operands.first().and_then(object_number) {
                        state.char_spacing = value;
                    }
                }
                b"Tw" => {
                    if let Some(value) = operands.first().and_then(object_number) {
                        state.word_spacing = value;
                    }
                }
                b"Tz" => {
                    if let Some(value) = operands.first().and_then(object_number) {
                        state.horizontal_scale = value / 100.0;
                    }
                }
                b"TL" => {
                    if let Some(value) = operands.first().and_then(object_number) {
                        state.leading = value;
                    }
                }
                b"Tj" => {
                    if let Some(Object::String(bytes, _)) = operands.first() {
                        self.write_text(bytes, resources, pool, scene, state, false);
                    }
                }
                b"'" => {
                    state.line_matrix =
                        Matrix::translate(0.0, -state.leading).then(state.line_matrix);
                    state.text_matrix = state.line_matrix;
                    if let Some(Object::String(bytes, _)) = operands.first() {
                        self.write_text(bytes, resources, pool, scene, state, false);
                    }
                }
                b"\"" => {
                    if let Some(word) = operands.first().and_then(object_number) {
                        state.word_spacing = word;
                    }
                    if let Some(spacing) = operands.get(1).and_then(object_number) {
                        state.char_spacing = spacing;
                    }
                    state.line_matrix =
                        Matrix::translate(0.0, -state.leading).then(state.line_matrix);
                    state.text_matrix = state.line_matrix;
                    if let Some(Object::String(bytes, _)) = operands.get(2) {
                        self.write_text(bytes, resources, pool, scene, state, false);
                    }
                }
                b"TJ" => {
                    if let Some(Object::Array(items)) = operands.first() {
                        for item in items {
                            match item {
                                Object::String(bytes, _) => {
                                    self.write_text(bytes, resources, pool, scene, state, true);
                                }
                                other => {
                                    if let Some(adjust) = object_number(other) {
                                        state.text_matrix = Matrix::translate(
                                            -adjust / 1000.0
                                                * state.font_size
                                                * state.horizontal_scale,
                                            0.0,
                                        )
                                        .then(state.text_matrix);
                                    }
                                }
                            }
                        }
                    }
                }
                b"Do" => {
                    if let Some(name) = operands.first().and_then(object_name) {
                        self.draw_xobject(name, resources, pool, scene, state, depth);
                    }
                }
                _ => {
                    scene.diagnostics.push(RenderDiagnostic {
                        code: DiagnosticCode::UnsupportedObject,
                        object_id: None,
                        message: format!("PDF 操作符 {} 未支持", operation.operator),
                    });
                }
            }
        }
        Ok(())
    }

    fn load_font(
        &mut self,
        name: &[u8],
        resources: &Dictionary,
        pool: &ResourcePool,
    ) -> Option<Arc<FontState>> {
        if let Some(cached) = self.fonts.get(name) {
            return cached.clone();
        }
        let loaded = self.load_font_inner(name, resources, pool);
        self.fonts.insert(name.to_vec(), loaded.clone());
        loaded
    }

    fn load_font_inner(
        &self,
        name: &[u8],
        resources: &Dictionary,
        pool: &ResourcePool,
    ) -> Option<Arc<FontState>> {
        let fonts = resources
            .get(b"Font")
            .ok()
            .and_then(|object| self.resolve(object))
            .and_then(|object| object.as_dict().ok())?;
        let dictionary = fonts
            .get(name)
            .ok()
            .and_then(|object| self.resolve(object))
            .and_then(|object| object.as_dict().ok())?;
        let subtype = dictionary
            .get(b"Subtype")
            .ok()
            .and_then(|object| object.as_name().ok())
            .unwrap_or_default();
        let identity = subtype == b"Type0";
        let font_dictionary = if identity {
            dictionary
                .get(b"DescendantFonts")
                .ok()
                .and_then(|object| self.resolve(object))
                .and_then(|object| object.as_array().ok())
                .and_then(|array| array.first())
                .and_then(|object| self.resolve(object))
                .and_then(|object| object.as_dict().ok())
                .unwrap_or(dictionary)
        } else {
            dictionary
        };
        let descriptor = font_dictionary
            .get(b"FontDescriptor")
            .ok()
            .and_then(|object| self.resolve(object))
            .and_then(|object| object.as_dict().ok())?;
        let file = descriptor
            .get(b"FontFile2")
            .ok()
            .and_then(|object| self.resolve(object))
            .and_then(|object| object.as_stream().ok())?;
        let bytes = file
            .decompressed_content()
            .unwrap_or_else(|_| file.content.clone());
        let font = pool.embedded_font_shared(Arc::from(bytes), 0).ok()?;
        let to_unicode = dictionary
            .get(b"ToUnicode")
            .ok()
            .and_then(|object| self.resolve(object))
            .and_then(|object| object.as_stream().ok())
            .and_then(|stream| stream.decompressed_content().ok())
            .map(|bytes| parse_to_unicode(&bytes))
            .unwrap_or_default();
        Some(Arc::new(FontState {
            font,
            identity,
            to_unicode,
        }))
    }

    fn write_text(
        &self,
        bytes: &[u8],
        resources: &Dictionary,
        pool: &ResourcePool,
        scene: &mut Scene,
        state: &mut State,
        _in_array: bool,
    ) {
        let Some(font_state) = &state.font else {
            return;
        };
        let Ok(face) =
            ttf_parser::Face::parse(font_state.font.data(), font_state.font.face_index())
        else {
            return;
        };
        let units = f64::from(face.units_per_em());
        if units == 0.0 {
            return;
        }
        let mut glyphs = Vec::new();
        let mut text = String::new();
        let mut x = 0.0;
        if font_state.identity {
            for pair in bytes.as_chunks::<2>().0 {
                let code = u32::from(u16::from_be_bytes(*pair));
                let glyph = code as u16;
                text.push(
                    font_state
                        .to_unicode
                        .get(&code)
                        .and_then(|value| char::from_u32(*value))
                        .or_else(|| glyph_char(&face, glyph))
                        .unwrap_or('\u{FFFD}'),
                );
                let advance = glyph_advance(&face, glyph, state.font_size, units);
                glyphs.push(PositionedGlyph {
                    id: u32::from(glyph),
                    x: x as f32,
                    y: 0.0,
                });
                x += advance + state.char_spacing;
            }
        } else {
            for &code in bytes {
                let codepoint = font_state
                    .to_unicode
                    .get(&u32::from(code))
                    .copied()
                    .unwrap_or(u32::from(code));
                let Some(character) = char::from_u32(codepoint) else {
                    continue;
                };
                let Some(glyph) = face.glyph_index(character) else {
                    continue;
                };
                text.push(character);
                let advance = glyph_advance(&face, glyph.0, state.font_size, units);
                glyphs.push(PositionedGlyph {
                    id: u32::from(glyph.0),
                    x: x as f32,
                    y: 0.0,
                });
                x += advance + state.char_spacing;
                if character == ' ' {
                    x += state.word_spacing;
                }
            }
        }
        if glyphs.is_empty() {
            return;
        }
        let fill = color_with_alpha(&state.fill, state.fill_alpha);
        let stroke = color_with_alpha(&state.stroke, state.stroke_alpha).map(|paint| StrokePaint {
            paint,
            style: state.style.clone(),
        });
        let element = TextElement {
            font: Arc::clone(&font_state.font),
            size: state.font_size,
            transform: Matrix::scale(1.0, -1.0).then(state.text_matrix),
            glyphs,
            text: Some(text),
            fill,
            stroke,
        };
        push_element(&mut scene.root, state, Element::Text(element));
        state.text_matrix = Matrix::translate(x, 0.0).then(state.text_matrix);
        let _ = resources;
        let _ = pool;
    }

    fn draw_xobject(
        &mut self,
        name: &[u8],
        resources: &Dictionary,
        pool: &ResourcePool,
        scene: &mut Scene,
        state: &State,
        depth: usize,
    ) {
        let Some(xobjects) = resources
            .get(b"XObject")
            .ok()
            .and_then(|object| self.resolve(object))
            .and_then(|object| object.as_dict().ok())
        else {
            return;
        };
        let Some(object) = xobjects
            .get(name)
            .ok()
            .and_then(|object| self.resolve(object))
        else {
            return;
        };
        let Ok(stream) = object.as_stream() else {
            return;
        };
        let subtype = stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|object| object.as_name().ok())
            .unwrap_or_default();
        if subtype == b"Form" {
            if depth >= 8 {
                scene.diagnostics.push(RenderDiagnostic {
                    code: DiagnosticCode::UnsupportedObject,
                    object_id: None,
                    message: "PDF Form XObject 嵌套过深".to_owned(),
                });
                return;
            }
            let form_dictionary = stream.dict.clone();
            let Some(data) = stream
                .decompressed_content()
                .ok()
                .or_else(|| Some(stream.content.clone()))
            else {
                return;
            };
            let Ok(content) = Content::decode(&data) else {
                return;
            };
            let form_resources = form_dictionary
                .get(b"Resources")
                .ok()
                .and_then(|object| self.resolve(object))
                .and_then(|object| object.as_dict().ok())
                .cloned()
                .unwrap_or_else(|| resources.clone());
            let matrix = form_dictionary
                .get(b"Matrix")
                .ok()
                .and_then(|object| self.resolve(object))
                .and_then(|object| object.as_array().ok())
                .and_then(|values| matrix_from_values(values))
                .unwrap_or(Matrix::IDENTITY);
            let mut form_state = state.clone();
            form_state.ctm = matrix.then(state.ctm);
            let mut form_path = Path::new();
            let mut form_current = Point::default();
            let mut form_clip = None;
            let _ = self.execute(
                &content.operations,
                &form_resources,
                pool,
                scene,
                &mut form_state,
                &mut Vec::new(),
                &mut form_path,
                &mut form_current,
                &mut form_clip,
                depth + 1,
            );
            return;
        }
        if subtype != b"Image" {
            return;
        }

        let Some(width) = stream
            .dict
            .get(b"Width")
            .ok()
            .and_then(object_number)
            .filter(|width| *width > 0.0)
        else {
            return;
        };
        let Some(height) = stream
            .dict
            .get(b"Height")
            .ok()
            .and_then(object_number)
            .filter(|height| *height > 0.0)
        else {
            return;
        };
        let filter = stream
            .dict
            .get(b"Filter")
            .ok()
            .and_then(|object| match object {
                Object::Name(name) => Some(name.as_slice()),
                Object::Array(values) => values.first().and_then(|value| value.as_name().ok()),
                _ => None,
            });
        if filter == Some(b"CCITTFaxDecode")
            && let Some(image) = self.ccitt_image(stream, width as u32, height as u32, pool)
        {
            let flip = Matrix::scale(1.0, -1.0).then(Matrix::translate(0.0, 1.0));
            let transform = Matrix::scale(1.0 / width, 1.0 / height).then(flip);
            push_element(
                &mut scene.root,
                state,
                Element::Image(ImageElement {
                    image,
                    transform,
                    alpha: state.fill_alpha,
                }),
            );
            return;
        }
        let image = if filter == Some(b"DCTDecode") {
            pool.embedded_image(ImageFormat::Jpeg, &stream.content, None)
                .ok()
        } else {
            self.image_asset(stream, pool)
        };
        let Some(image) = image else {
            return;
        };
        let flip = Matrix::scale(1.0, -1.0).then(Matrix::translate(0.0, 1.0));
        let transform = Matrix::scale(1.0 / width, 1.0 / height).then(flip);
        push_element(
            &mut scene.root,
            state,
            Element::Image(ImageElement {
                image,
                transform,
                alpha: state.fill_alpha,
            }),
        );
    }

    fn ccitt_image(
        &self,
        stream: &lopdf::Stream,
        width: u32,
        height: u32,
        pool: &ResourcePool,
    ) -> Option<Arc<crate::asset::ImageAsset>> {
        let params = stream
            .dict
            .get(b"DecodeParms")
            .ok()
            .and_then(|object| self.resolve(object))
            .and_then(|object| object.as_dict().ok());
        let k = params
            .and_then(|dictionary| dictionary.get(b"K").ok())
            .and_then(object_number)
            .unwrap_or(0.0) as i64;
        let encoding = if k < 0 {
            hayro_ccitt::EncodingMode::Group4
        } else if k == 0 {
            hayro_ccitt::EncodingMode::Group3_1D
        } else {
            hayro_ccitt::EncodingMode::Group3_2D { k: k as u32 }
        };
        let black_is_1 = params
            .and_then(|dictionary| dictionary.get(b"BlackIs1").ok())
            .and_then(|object| object.as_bool().ok())
            .unwrap_or(false);
        let settings = hayro_ccitt::DecodeSettings {
            columns: width,
            rows: height,
            end_of_block: true,
            end_of_line: params
                .and_then(|dictionary| dictionary.get(b"EndOfLine").ok())
                .and_then(|object| object.as_bool().ok())
                .unwrap_or(false),
            rows_are_byte_aligned: params
                .and_then(|dictionary| dictionary.get(b"EncodedByteAlign").ok())
                .and_then(|object| object.as_bool().ok())
                .unwrap_or(false),
            encoding,
            invert_black: black_is_1,
        };
        let mut context = hayro_ccitt::DecoderContext::new(settings);
        let mut bitmap = BitmapDecoder {
            pixels: Vec::with_capacity(width as usize * height as usize),
        };
        hayro_ccitt::decode(&stream.content, &mut bitmap, &mut context).ok()?;
        if bitmap.pixels.len() < width as usize * height as usize {
            return None;
        }
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for pixel in bitmap
            .pixels
            .into_iter()
            .take(width as usize * height as usize)
        {
            let value = if pixel { 255 } else { 0 };
            rgba.extend_from_slice(&[value, value, value, 255]);
        }
        let mut png_bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
        writer.finish().ok()?;
        pool.embedded_image(ImageFormat::Png, &png_bytes, None).ok()
    }

    fn image_asset(
        &self,
        stream: &lopdf::Stream,
        pool: &ResourcePool,
    ) -> Option<Arc<crate::asset::ImageAsset>> {
        let width = stream
            .dict
            .get(b"Width")
            .ok()
            .and_then(object_number)
            .map(|value| value as u32)?;
        let height = stream
            .dict
            .get(b"Height")
            .ok()
            .and_then(object_number)
            .map(|value| value as u32)?;
        let bits = stream
            .dict
            .get(b"BitsPerComponent")
            .ok()
            .and_then(object_number)
            .unwrap_or(8.0);
        if width == 0 || height == 0 || bits != 8.0 {
            return None;
        }
        let colorspace = stream
            .dict
            .get(b"ColorSpace")
            .ok()
            .and_then(|object| self.color_space(object));
        let components = colorspace?;
        let data = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        if data.len() < width as usize * height as usize * components {
            return None;
        }
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for pixel in data.chunks_exact(components) {
            let (r, g, b) = match components {
                1 => (pixel[0], pixel[0], pixel[0]),
                3 => (pixel[0], pixel[1], pixel[2]),
                4 => {
                    let k = f32::from(pixel[3]) / 255.0;
                    let c = f32::from(pixel[0]) / 255.0;
                    let m = f32::from(pixel[1]) / 255.0;
                    let y = f32::from(pixel[2]) / 255.0;
                    (
                        ((1.0 - c) * (1.0 - k) * 255.0) as u8,
                        ((1.0 - m) * (1.0 - k) * 255.0) as u8,
                        ((1.0 - y) * (1.0 - k) * 255.0) as u8,
                    )
                }
                _ => return None,
            };
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
        if let Some(mask) = stream.dict.get(b"SMask").ok()
            && let Some(mask) = self
                .resolve(mask)
                .and_then(|object| object.as_stream().ok())
            && let Ok(mask_data) = mask.decompressed_content()
            && mask_data.len() >= rgba.len() / 4
        {
            for (pixel, alpha) in rgba.as_chunks_mut::<4>().0.iter_mut().zip(mask_data) {
                pixel[3] = alpha;
            }
        }
        let mut png_bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
        writer.finish().ok()?;
        pool.embedded_image(ImageFormat::Png, &png_bytes, None).ok()
    }

    fn color_space(&self, object: &Object) -> Option<usize> {
        let object = self.resolve(object).unwrap_or(object);
        match object {
            Object::Name(name) => match name.as_slice() {
                b"DeviceGray" | b"G" => Some(1),
                b"DeviceRGB" | b"RGB" => Some(3),
                b"DeviceCMYK" | b"CMYK" => Some(4),
                _ => None,
            },
            Object::Array(array) => {
                let name = array.first()?.as_name().ok()?;
                match name {
                    b"ICCBased" => {
                        let stream = self.resolve(array.get(1)?)?.as_stream().ok()?;
                        stream
                            .dict
                            .get(b"N")
                            .ok()
                            .and_then(object_number)
                            .map(|value| value as usize)
                    }
                    _ => self.color_space(array.first()?),
                }
            }
            _ => None,
        }
    }
}

struct BitmapDecoder {
    pixels: Vec<bool>,
}

impl hayro_ccitt::Decoder for BitmapDecoder {
    fn push_pixel(&mut self, white: bool) {
        self.pixels.push(white);
    }

    fn push_pixel_chunk(&mut self, white: bool, chunk_count: u32) {
        self.pixels
            .extend(std::iter::repeat_n(white, chunk_count as usize * 8));
    }

    fn next_line(&mut self) {}
}
fn push_element(root: &mut Group, state: &State, element: Element) {
    let mut inner = Group::new(state.ctm);
    inner.children.push(element);
    root.children.push(Element::Group(inner));
}

fn color_with_alpha(paint: &Paint, alpha: f32) -> Option<Paint> {
    match paint {
        Paint::Solid(color) => {
            let mut color = *color;
            color.a *= alpha.clamp(0.0, 1.0);
            Some(Paint::Solid(color))
        }
        _ => None,
    }
}

fn glyph_char(face: &ttf_parser::Face<'_>, glyph: u16) -> Option<char> {
    let cmap = face.tables().cmap?;
    for subtable in cmap.subtables {
        if !subtable.is_unicode() {
            continue;
        }
        let mut result = None;
        subtable.codepoints(|codepoint| {
            if result.is_none()
                && subtable
                    .glyph_index(codepoint)
                    .is_some_and(|id| id.0 == glyph)
            {
                result = char::from_u32(codepoint);
            }
        });
        if result.is_some() {
            return result;
        }
    }
    None
}
fn glyph_advance(face: &ttf_parser::Face<'_>, glyph: u16, size: f64, units: f64) -> f64 {
    face.glyph_hor_advance(ttf_parser::GlyphId(glyph))
        .map(|advance| f64::from(advance) / units * size)
        .unwrap_or(size * 0.5)
}

fn parse_to_unicode(data: &[u8]) -> HashMap<u32, u32> {
    let text = String::from_utf8_lossy(data);
    let mut map = HashMap::new();
    let mut source: Option<u32> = None;
    for token in text.split_whitespace() {
        if let Some(hex) = token
            .strip_prefix('<')
            .and_then(|value| value.strip_suffix('>'))
            && let Ok(value) = u32::from_str_radix(hex, 16)
        {
            if let Some(source) = source.take() {
                map.insert(source, value);
            } else {
                source = Some(value);
            }
        }
    }
    map
}

fn matrix_operands(operands: &[Object]) -> Option<Matrix> {
    matrix_from_values(operands)
}

fn matrix_from_values(values: &[Object]) -> Option<Matrix> {
    let values = values.iter().filter_map(object_number).collect::<Vec<_>>();
    (values.len() >= 6).then(|| {
        Matrix::new(
            values[0], values[1], values[2], values[3], values[4], values[5],
        )
    })
}

fn point_operands(operands: &[Object]) -> Option<Point> {
    pair_operands(operands).map(|(x, y)| Point::new(x, y))
}

fn pair_operands(operands: &[Object]) -> Option<(f64, f64)> {
    Some((
        operands.first().and_then(object_number)?,
        operands.get(1).and_then(object_number)?,
    ))
}

fn cubic_operands(operands: &[Object]) -> Option<(Point, Point, Point)> {
    let values = operands
        .iter()
        .filter_map(object_number)
        .collect::<Vec<_>>();
    (values.len() >= 6).then(|| {
        (
            Point::new(values[0], values[1]),
            Point::new(values[2], values[3]),
            Point::new(values[4], values[5]),
        )
    })
}

fn quadratic_operands(operands: &[Object]) -> Option<(Point, Point)> {
    let values = operands
        .iter()
        .filter_map(object_number)
        .collect::<Vec<_>>();
    (values.len() >= 4).then(|| {
        (
            Point::new(values[0], values[1]),
            Point::new(values[2], values[3]),
        )
    })
}

fn rect_operands(operands: &[Object]) -> Option<(f64, f64, f64, f64)> {
    let values = operands
        .iter()
        .filter_map(object_number)
        .collect::<Vec<_>>();
    (values.len() >= 4).then(|| (values[0], values[1], values[2], values[3]))
}

fn rgb_operands(operands: &[Object]) -> Option<Rgba> {
    let values = operands
        .iter()
        .filter_map(object_number)
        .collect::<Vec<_>>();
    (values.len() >= 3)
        .then(|| Rgba::new(values[0] as f32, values[1] as f32, values[2] as f32, 1.0))
}

fn cmyk_operands(operands: &[Object]) -> Option<Rgba> {
    let values = operands
        .iter()
        .filter_map(object_number)
        .collect::<Vec<_>>();
    (values.len() >= 4).then(|| {
        let k = values[3] as f32;
        Rgba::new(
            ((1.0 - values[0] as f32) * (1.0 - k)).clamp(0.0, 1.0),
            ((1.0 - values[1] as f32) * (1.0 - k)).clamp(0.0, 1.0),
            ((1.0 - values[2] as f32) * (1.0 - k)).clamp(0.0, 1.0),
            1.0,
        )
    })
}

fn object_number(object: &Object) -> Option<f64> {
    match object {
        Object::Integer(value) => Some(*value as f64),
        Object::Real(value) => Some(f64::from(*value)),
        _ => None,
    }
}

fn object_name(object: &Object) -> Option<&[u8]> {
    object.as_name().ok()
}

fn pdf_error(error: impl std::fmt::Display) -> RenderError {
    RenderError::Unsupported(format!("PDF 解析失败: {error}"))
}

#[cfg(test)]
mod tests {
    use super::PdfFrontend;
    use crate::paint::{Paint, Rgba};
    use crate::path::{FillRule, Path};
    use crate::resource::{PoolBudget, ResourcePool};
    use crate::scene::{Element, Group, PathElement, Scene};
    use crate::{PdfDocument, PdfOptions, Point, Size};

    fn scene() -> Scene {
        let mut path = Path::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(90.0, 10.0));
        path.line_to(Point::new(90.0, 90.0));
        path.close();
        let mut root = Group::default();
        root.children.push(Element::Path(PathElement {
            path: std::sync::Arc::new(path),
            fill: Some(Paint::Solid(Rgba::BLACK)),
            stroke: None,
            fill_rule: FillRule::NonZero,
        }));
        Scene {
            size: Size::new(100.0, 100.0),
            root,
            diagnostics: Vec::new(),
        }
    }

    fn pdf() -> Vec<u8> {
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene()).expect("page");
        document.finish().expect("pdf")
    }

    #[test]
    fn reads_page_geometry_and_paths() {
        let mut frontend = PdfFrontend::from_bytes(pdf()).expect("frontend");
        assert_eq!(frontend.page_count(), 1);
        let size = frontend.page_size(0).expect("size");
        assert!((size.width - 100.0).abs() < 0.01);
        assert!((size.height - 100.0).abs() < 0.01);
        let pool = ResourcePool::new(PoolBudget::default());
        let scene = frontend.build_page(0, &pool).expect("scene");
        assert!(scene.diagnostics.is_empty());
        assert!(!scene.root.children.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn reads_text_and_image() {
        use crate::image::ImageFormat;
        use crate::scene::{ImageElement, PositionedGlyph, TextElement};

        let bytes = std::fs::read("C:/Windows/Fonts/arial.ttf").expect("arial");
        let face = ttf_parser::Face::parse(&bytes, 0).expect("face");
        let glyph = face.glyph_index('A').expect("glyph");
        let pool = ResourcePool::new(PoolBudget::default());
        let font = pool.embedded_font(&bytes, 0).expect("font");
        let mut png_bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_bytes, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("png");
            writer
                .write_image_data(&[
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
                ])
                .expect("pixels");
        }
        let image = pool
            .embedded_image(ImageFormat::Png, &png_bytes, None)
            .expect("image");
        let mut root = Group::default();
        root.children.push(Element::Image(ImageElement {
            image,
            transform: crate::geometry::Matrix::scale(2.0, 2.0),
            alpha: 1.0,
        }));
        root.children.push(Element::Text(TextElement {
            font,
            size: 10.0,
            transform: crate::geometry::Matrix::translate(10.0, 20.0),
            glyphs: vec![PositionedGlyph {
                id: u32::from(glyph.0),
                x: 0.0,
                y: 0.0,
            }],
            text: None,
            fill: Some(Paint::Solid(Rgba::BLACK)),
            stroke: None,
        }));
        let scene = Scene {
            size: Size::new(100.0, 100.0),
            root,
            diagnostics: Vec::new(),
        };
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene).expect("page");
        let pdf = document.finish().expect("pdf");
        let mut frontend = PdfFrontend::from_bytes(pdf).expect("frontend");
        let scene = frontend
            .build_page(0, &ResourcePool::new(PoolBudget::default()))
            .expect("scene");

        fn count(group: &Group) -> (usize, usize) {
            let mut text = 0;
            let mut images = 0;
            for child in &group.children {
                match child {
                    Element::Text(_) => text += 1,
                    Element::Image(_) => images += 1,
                    Element::Group(group) => {
                        let (t, i) = count(group);
                        text += t;
                        images += i;
                    }
                    Element::Path(_) => {}
                }
            }
            (text, images)
        }

        let (text, images) = count(&scene.root);
        assert!(text > 0);
        assert!(images > 0);
    }

    #[test]
    fn invalid_page_is_rejected() {
        let mut frontend = PdfFrontend::from_bytes(pdf()).expect("frontend");
        let pool = ResourcePool::new(PoolBudget::default());
        assert!(frontend.build_page(99, &pool).is_err());
    }
}
