//! PDF 文档后端。
//!
//! 当前阶段把 [`Scene`] 写成矢量 PDF：页面坐标、路径、图片、裁剪、
//! 渐变和文字。文字优先嵌入字体，字体不可用时退化为字形轮廓。

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::Arc;

use crate::asset::{AssetId, FontAsset, ImageAsset};
use crate::error::RenderError;
use crate::geometry::{Matrix, Point};
use crate::paint::{Paint, Rgba};
use crate::path::{FillRule, LineCap, LineJoin, Path, PathSegment};
use crate::scene::{Element, Group, ImageElement, PathElement, Scene, TextElement};

/// PDF 版本。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PdfVersion {
    /// PDF 1.4，兼容旧阅读器。
    V14,
    /// PDF 1.7，默认值。
    #[default]
    V17,
}

impl PdfVersion {
    fn header(self) -> &'static [u8] {
        match self {
            Self::V14 => b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n",
            Self::V17 => b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n",
        }
    }
}

/// 文字输出方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextMode {
    /// 优先嵌入字体，失败时转成字形轮廓。
    #[default]
    EmbedThenOutline,
    /// 始终输出字形轮廓。
    Outline,
}

/// PDF 输出选项。
#[derive(Clone, Debug)]
pub struct PdfOptions {
    /// PDF 版本。
    pub version: PdfVersion,
    /// 文字输出方式。
    pub text_mode: TextMode,
    /// 单张图片允许的最大像素数。
    pub max_image_pixels: u64,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            version: PdfVersion::V17,
            text_mode: TextMode::EmbedThenOutline,
            max_image_pixels: 64 * 1024 * 1024,
        }
    }
}

/// PDF 文档元数据。
#[derive(Clone, Debug, Default)]
pub struct PdfMetadata {
    /// 标题。
    pub title: Option<String>,
    /// 作者。
    pub author: Option<String>,
    /// 主题。
    pub subject: Option<String>,
    /// 关键词。
    pub keywords: Option<String>,
    /// 创建工具。
    pub creator: Option<String>,
    /// 生成工具。
    pub producer: Option<String>,
}

/// PDF 书签。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfOutline {
    /// 标题。
    pub title: String,
    /// 目标页索引，从 0 开始。
    pub page: usize,
}
struct PdfPage {
    width: f64,
    height: f64,
    content: u32,
    resources: String,
}

#[derive(Clone)]
struct ImageResource {
    name: String,
    object: u32,
    width: u32,
    height: u32,
}

#[derive(Clone)]
struct ShadingResource {
    name: String,
    object: u32,
}

#[derive(Clone)]
struct FontResource {
    name: String,
    object: u32,
    embedded: bool,
    file: Option<u32>,
    cid: u32,
}

impl PdfDocument {
    fn collect_images(&self, group: &Group, images: &mut HashMap<AssetId, Arc<ImageAsset>>) {
        for element in &group.children {
            match element {
                Element::Group(group) => self.collect_images(group, images),
                Element::Image(image) => {
                    images
                        .entry(image.image.id())
                        .or_insert_with(|| Arc::clone(&image.image));
                }
                Element::Path(_) | Element::Text(_) => {}
            }
        }
    }

    fn collect_fonts(
        &self,
        group: &Group,
        fonts: &mut HashMap<AssetId, Arc<FontAsset>>,
        glyphs: &mut HashMap<AssetId, HashSet<u16>>,
    ) {
        for element in &group.children {
            match element {
                Element::Group(group) => self.collect_fonts(group, fonts, glyphs),
                Element::Text(text) => {
                    let id = text.font.id();
                    fonts.entry(id).or_insert_with(|| Arc::clone(&text.font));
                    let used = glyphs.entry(id).or_default();
                    used.extend(
                        text.glyphs
                            .iter()
                            .filter_map(|glyph| u16::try_from(glyph.id).ok()),
                    );
                }
                Element::Path(_) | Element::Image(_) => {}
            }
        }
    }

    fn collect_alphas(&self, group: &Group, inherited: f32, alphas: &mut HashSet<u8>) {
        let opacity = inherited * group.opacity.clamp(0.0, 1.0);
        for element in &group.children {
            match element {
                Element::Group(group) => self.collect_alphas(group, opacity, alphas),
                Element::Path(path) => {
                    if let Some(paint) = &path.fill {
                        insert_alpha(alphas, opacity * paint_alpha(paint));
                    }
                    if let Some(stroke) = &path.stroke {
                        insert_alpha(alphas, opacity * paint_alpha(&stroke.paint));
                    }
                }
                Element::Text(text) => {
                    if let Some(paint) = &text.fill {
                        insert_alpha(alphas, opacity * paint_alpha(paint));
                    }
                    if let Some(stroke) = &text.stroke {
                        insert_alpha(alphas, opacity * paint_alpha(&stroke.paint));
                    }
                }
                Element::Image(image) => {
                    insert_alpha(alphas, opacity * image.alpha);
                }
            }
        }
    }

    fn embed_ext_gstate(&mut self, alpha: u8) -> u32 {
        let value = number(f64::from(alpha) / 255.0);
        self.writer
            .add(format!("<< /Type /ExtGState /ca {value} /CA {value} >>").into_bytes())
    }
    fn embed_font(&mut self, font: &FontAsset, name: String) -> Result<FontResource, RenderError> {
        if self.options.text_mode == TextMode::Outline || !can_embed_font(font) {
            return Ok(FontResource {
                name,
                object: 0,
                embedded: false,
                file: None,
                cid: 0,
            });
        }
        let face = ttf_parser::Face::parse(font.data(), font.face_index())
            .map_err(|error| RenderError::Unsupported(format!("字体解析失败: {error}")))?;
        let units = face.units_per_em();
        if units == 0 {
            return Ok(FontResource {
                name,
                object: 0,
                embedded: false,
                file: None,
                cid: 0,
            });
        }
        let scale = 1000.0 / f64::from(units);
        let file = self.writer.stream(
            &format!("/Filter /FlateDecode /Length1 {}", font.data().len()),
            &deflate(font.data())?,
        );
        let bbox = face.global_bounding_box();
        let descriptor = self.writer.add(
            format!(
                "<< /Type /FontDescriptor /FontName /{} /Flags 4 /FontBBox [{} {} {} {}] /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile2 {file} 0 R >>",
                name,
                number(f64::from(bbox.x_min) * scale),
                number(f64::from(bbox.y_min) * scale),
                number(f64::from(bbox.x_max) * scale),
                number(f64::from(bbox.y_max) * scale),
                number(f64::from(face.ascender()) * scale),
                number(f64::from(face.descender()) * scale),
                number(f64::from(face.capital_height().unwrap_or(face.ascender())) * scale)
            )
            .into_bytes(),
        );
        let cid = self.writer.add(
            format!(
                "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {descriptor} 0 R /CIDToGIDMap /Identity /DW 1000 >>",
                name
            )
            .into_bytes(),
        );
        let object = self.writer.add(
            format!(
                "<< /Type /Font /Subtype /Type0 /BaseFont /{} /Encoding /Identity-H /DescendantFonts [{cid} 0 R] >>",
                name
            )
            .into_bytes(),
        );
        Ok(FontResource {
            name,
            object,
            embedded: true,
            file: Some(file),
            cid,
        })
    }

    fn collect_gradients(&self, group: &Group, gradients: &mut HashMap<String, Paint>) {
        for element in &group.children {
            match element {
                Element::Group(group) => self.collect_gradients(group, gradients),
                Element::Path(path) => {
                    if let Some(paint) = &path.fill
                        && matches!(paint, Paint::Linear(_) | Paint::Radial(_))
                    {
                        gradients
                            .entry(format!("{paint:?}"))
                            .or_insert_with(|| paint.clone());
                    }
                }
                Element::Image(_) | Element::Text(_) => {}
            }
        }
    }

    fn embed_gradient(&mut self, paint: &Paint) -> Result<ShadingResource, RenderError> {
        let (stops, coords, shading_type) = match paint {
            Paint::Linear(gradient) => (
                &gradient.stops,
                format!(
                    "[{} {} {} {}]",
                    number(gradient.start.x),
                    number(gradient.start.y),
                    number(gradient.end.x),
                    number(gradient.end.y)
                ),
                2,
            ),
            Paint::Radial(gradient) => (
                &gradient.stops,
                format!(
                    "[{} {} {} {} {} {}]",
                    number(gradient.start_center.x),
                    number(gradient.start_center.y),
                    number(gradient.start_radius),
                    number(gradient.end_center.x),
                    number(gradient.end_center.y),
                    number(gradient.end_radius)
                ),
                3,
            ),
            Paint::Solid(_) => {
                return Err(RenderError::Unsupported("纯色没有 Shading".to_owned()));
            }
        };
        if stops.len() < 2 {
            return Err(RenderError::Unsupported("渐变缺少颜色段".to_owned()));
        }
        let mut functions = Vec::new();
        for pair in stops.windows(2) {
            let body = format!(
                "<< /FunctionType 2 /Domain [0 1] /C0 [{} {} {}] /C1 [{} {} {}] /N 1 >>",
                number(f64::from(pair[0].color.r)),
                number(f64::from(pair[0].color.g)),
                number(f64::from(pair[0].color.b)),
                number(f64::from(pair[1].color.r)),
                number(f64::from(pair[1].color.g)),
                number(f64::from(pair[1].color.b))
            );
            functions.push(self.writer.add(body.into_bytes()));
        }
        let function = if functions.len() == 1 {
            format!("{} 0 R", functions[0])
        } else {
            let bounds = stops[1..stops.len() - 1]
                .iter()
                .map(|stop| number(f64::from(stop.offset).clamp(0.0, 1.0)))
                .collect::<Vec<_>>()
                .join(" ");
            let function_count = functions.len();
            let functions = functions
                .iter()
                .map(|id| format!("{id} 0 R"))
                .collect::<Vec<_>>()
                .join(" ");
            let encode = std::iter::repeat_n("0 1", function_count)
                .collect::<Vec<_>>()
                .join(" ");
            let id = self.writer.add(
                format!(
                    "<< /FunctionType 3 /Domain [0 1] /Functions [{functions}] /Bounds [{bounds}] /Encode [{encode}] >>"
                )
                .into_bytes(),
            );
            format!("{id} 0 R")
        };
        let name_index = self.writer.objects.len() + 1;
        let object = self.writer.add(
            format!(
                "<< /ShadingType {shading_type} /ColorSpace /DeviceRGB /Coords {coords} /Function {function} /Extend [true true] >>"
            )
            .into_bytes(),
        );
        Ok(ShadingResource {
            name: format!("Sh{name_index}"),
            object,
        })
    }

    fn embed_image(
        &mut self,
        image: &ImageAsset,
        name: String,
    ) -> Result<ImageResource, RenderError> {
        let decoded = image.decoded(self.options.max_image_pixels)?;
        let mut rgb = Vec::with_capacity(decoded.rgba.len() / 4 * 3);
        let mut alpha = Vec::with_capacity(decoded.rgba.len() / 4);
        let mut has_alpha = false;
        for pixel in decoded.rgba.as_chunks::<4>().0 {
            rgb.extend_from_slice(&pixel[..3]);
            alpha.push(pixel[3]);
            has_alpha |= pixel[3] != 255;
        }
        let mut dict = format!(
            "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode",
            decoded.width, decoded.height
        );
        if has_alpha {
            let mask = self.writer.stream(
                &format!(
                    "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode",
                    decoded.width, decoded.height
                ),
                &deflate(&alpha)?,
            );
            dict.push_str(&format!(" /SMask {mask} 0 R"));
        }
        let object = self.writer.stream(&dict, &deflate(&rgb)?);
        Ok(ImageResource {
            name,
            object,
            width: decoded.width,
            height: decoded.height,
        })
    }
}

/// 多页 PDF 文档。
pub struct PdfDocument {
    options: PdfOptions,
    writer: PdfWriter,
    pages: Vec<PdfPage>,
    metadata: PdfMetadata,
    outlines: Vec<PdfOutline>,
    image_cache: HashMap<AssetId, ImageResource>,
    font_cache: HashMap<AssetId, FontResource>,
    font_glyphs: HashMap<AssetId, HashSet<u16>>,
    font_assets: HashMap<AssetId, Arc<FontAsset>>,
}

impl PdfDocument {
    /// 创建 PDF 文档。
    pub fn new(options: PdfOptions) -> Self {
        Self {
            writer: PdfWriter::new(options.version),
            options,
            pages: Vec::new(),
            metadata: PdfMetadata::default(),
            outlines: Vec::new(),
            image_cache: HashMap::new(),
            font_cache: HashMap::new(),
            font_glyphs: HashMap::new(),
            font_assets: HashMap::new(),
        }
    }

    /// 设置文档元数据。
    pub fn set_metadata(&mut self, metadata: PdfMetadata) {
        self.metadata = metadata;
    }

    /// 添加书签，页索引从 0 开始。
    pub fn add_outline(&mut self, title: impl Into<String>, page: usize) {
        self.outlines.push(PdfOutline {
            title: title.into(),
            page,
        });
    }

    /// 添加一页。
    pub fn add_page(&mut self, scene: &Scene) -> Result<(), RenderError> {
        let scale = 72.0 / 25.4;
        let width = scene.size.width * scale;
        let height = scene.size.height * scale;
        if !(width.is_finite() && width > 0.0 && height.is_finite() && height > 0.0) {
            return Err(RenderError::InvalidPage);
        }
        let mut collected = HashMap::new();
        self.collect_images(&scene.root, &mut collected);
        let mut ordered = collected.into_iter().collect::<Vec<_>>();
        ordered.sort_by_key(|(id, _)| *id);
        let mut images = HashMap::new();
        let mut xobjects = String::new();
        for (id, image) in ordered {
            let resource = if let Some(resource) = self.image_cache.get(&id).cloned() {
                resource
            } else {
                let name = format!("Im{}", self.image_cache.len() + 1);
                let resource = self.embed_image(&image, name)?;
                self.image_cache.insert(id, resource.clone());
                resource
            };
            xobjects.push_str(&format!("/{} {} 0 R ", resource.name, resource.object));
            images.insert(id, resource);
        }
        let mut collected_fonts = HashMap::new();
        let mut page_glyphs = HashMap::new();
        self.collect_fonts(&scene.root, &mut collected_fonts, &mut page_glyphs);
        for (id, glyphs) in page_glyphs {
            self.font_glyphs.entry(id).or_default().extend(glyphs);
        }
        let mut ordered_fonts = collected_fonts.into_iter().collect::<Vec<_>>();
        ordered_fonts.sort_by_key(|(id, _)| *id);
        let mut fonts = HashMap::new();
        let mut font_resources = String::new();
        for (id, font) in ordered_fonts {
            self.font_assets
                .entry(id)
                .or_insert_with(|| Arc::clone(&font));
            let resource = if let Some(resource) = self.font_cache.get(&id).cloned() {
                resource
            } else {
                let name = format!("F{}", self.font_cache.len() + 1);
                let resource = self.embed_font(&font, name)?;
                self.font_cache.insert(id, resource.clone());
                resource
            };
            if resource.embedded {
                font_resources.push_str(&format!("/{} {} 0 R ", resource.name, resource.object));
            }
            fonts.insert(id, resource);
        }

        let mut collected_gradients = HashMap::new();
        self.collect_gradients(&scene.root, &mut collected_gradients);
        let mut ordered_gradients = collected_gradients.into_iter().collect::<Vec<_>>();
        ordered_gradients.sort_by(|left, right| left.0.cmp(&right.0));
        let mut gradients = HashMap::new();
        let mut shadings = String::new();
        for (key, paint) in ordered_gradients {
            let resource = self.embed_gradient(&paint)?;
            shadings.push_str(&format!("/{} {} 0 R ", resource.name, resource.object));
            gradients.insert(key, resource);
        }
        let mut collected_alphas = HashSet::new();
        self.collect_alphas(&scene.root, 1.0, &mut collected_alphas);
        let mut ordered_alphas = collected_alphas.into_iter().collect::<Vec<_>>();
        ordered_alphas.sort_unstable();
        let mut alphas = HashMap::new();
        let mut ext_gstates = String::new();
        for alpha in ordered_alphas {
            let object = self.embed_ext_gstate(alpha);
            let name = format!("GS{alpha}");
            ext_gstates.push_str(&format!("/{name} {object} 0 R "));
            alphas.insert(alpha, name);
        }

        let mut resources = Vec::new();
        if !xobjects.is_empty() {
            resources.push(format!("/XObject << {} >>", xobjects.trim_end()));
        }
        if !shadings.is_empty() {
            resources.push(format!("/Shading << {} >>", shadings.trim_end()));
        }
        if !font_resources.is_empty() {
            resources.push(format!("/Font << {} >>", font_resources.trim_end()));
        }
        if !ext_gstates.is_empty() {
            resources.push(format!("/ExtGState << {} >>", ext_gstates.trim_end()));
        }
        let resources = resources.join(" ");

        let mut content = ContentBuilder::new(&images, &gradients, &fonts, &alphas);
        content.raw(&format!("q\n{scale} 0 0 -{scale} 0 {height} cm\n"));
        content.group(&scene.root, 1.0);
        content.raw("Q\n");
        let content = self
            .writer
            .stream("/Filter /FlateDecode", &deflate(&content.into_bytes())?);
        self.pages.push(PdfPage {
            width,
            height,
            content,
            resources,
        });
        Ok(())
    }

    /// 当前页数。
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// 完成 PDF 并返回字节。
    pub fn finish(mut self) -> Result<Vec<u8>, RenderError> {
        self.subset_fonts()?;
        self.write_to_unicode()?;
        for outline in &self.outlines {
            if outline.page >= self.pages.len() {
                return Err(RenderError::InvalidPage);
            }
        }
        let pages_id = self.writer.placeholder();
        let mut kids = String::new();
        let mut page_ids = Vec::with_capacity(self.pages.len());
        for page in &self.pages {
            let page_id = self.writer.add(
                format!(
                    "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {} {}] /Contents {} 0 R /Resources << {} >> >>",
                    number(page.width),
                    number(page.height),
                    page.content,
                    page.resources
                )
                .into_bytes(),
            );
            page_ids.push(page_id);
            kids.push_str(&format!("{page_id} 0 R "));
        }
        self.writer.set(
            pages_id,
            format!(
                "<< /Type /Pages /Kids [{}] /Count {} >>",
                kids.trim_end(),
                self.pages.len()
            )
            .into_bytes(),
        );

        let outlines_id = if self.outlines.is_empty() {
            None
        } else {
            let root = self.writer.placeholder();
            let item_ids = self
                .outlines
                .iter()
                .map(|_| self.writer.placeholder())
                .collect::<Vec<_>>();
            for (index, outline) in self.outlines.iter().enumerate() {
                let mut body = format!(
                    "<< /Title {} /Parent {root} 0 R",
                    pdf_text_string(&outline.title)
                );
                if index > 0 {
                    body.push_str(&format!(" /Prev {} 0 R", item_ids[index - 1]));
                }
                if let Some(next) = item_ids.get(index + 1) {
                    body.push_str(&format!(" /Next {next} 0 R"));
                }
                body.push_str(&format!(
                    " /Dest [{} 0 R /XYZ null null null] >>",
                    page_ids[outline.page]
                ));
                self.writer.set(item_ids[index], body.into_bytes());
            }
            self.writer.set(
                root,
                format!(
                    "<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {} >>",
                    item_ids[0],
                    item_ids[item_ids.len() - 1],
                    item_ids.len()
                )
                .into_bytes(),
            );
            Some(root)
        };

        let info_id = self.metadata_info();
        let mut catalog = format!("<< /Type /Catalog /Pages {pages_id} 0 R");
        if let Some(outlines) = outlines_id {
            catalog.push_str(&format!(" /Outlines {outlines} 0 R /PageMode /UseOutlines"));
        }
        catalog.push_str(" >>");
        let catalog_id = self.writer.add(catalog.into_bytes());
        Ok(self.writer.finish(catalog_id, info_id))
    }

    fn subset_fonts(&mut self) -> Result<(), RenderError> {
        let tasks = self
            .font_cache
            .iter()
            .filter_map(|(id, resource)| {
                let file = resource.file?;
                let font = self.font_assets.get(id)?;
                let glyphs = self.font_glyphs.get(id)?;
                Some((file, Arc::clone(font), glyphs.clone()))
            })
            .collect::<Vec<_>>();
        for (file, font, glyphs) in tasks {
            if glyphs.is_empty() {
                continue;
            }
            let Some(data) = subset_truetype(font.data(), font.face_index(), &glyphs) else {
                continue;
            };
            let compressed = deflate(&data)?;
            self.writer.replace_stream(
                file,
                &format!("/Filter /FlateDecode /Length1 {}", data.len()),
                &compressed,
            );
        }
        Ok(())
    }

    fn write_to_unicode(&mut self) -> Result<(), RenderError> {
        let tasks = self
            .font_cache
            .iter()
            .filter_map(|(id, resource)| {
                if !resource.embedded {
                    return None;
                }
                let font = self.font_assets.get(id)?;
                let glyphs = self.font_glyphs.get(id)?;
                Some((resource.clone(), Arc::clone(font), glyphs.clone()))
            })
            .collect::<Vec<_>>();
        for (resource, font, glyphs) in tasks {
            let Some(cmap) = build_to_unicode(&font, &glyphs) else {
                continue;
            };
            let stream = self.writer.stream("", cmap.as_bytes());
            self.writer.set(
                resource.object,
                format!(
                    "<< /Type /Font /Subtype /Type0 /BaseFont /{} /Encoding /Identity-H /DescendantFonts [{} 0 R] /ToUnicode {stream} 0 R >>",
                    resource.name, resource.cid
                )
                .into_bytes(),
            );
        }
        Ok(())
    }

    fn metadata_info(&mut self) -> Option<u32> {
        let entries = [
            ("Title", self.metadata.title.as_deref()),
            ("Author", self.metadata.author.as_deref()),
            ("Subject", self.metadata.subject.as_deref()),
            ("Keywords", self.metadata.keywords.as_deref()),
            ("Creator", self.metadata.creator.as_deref()),
            ("Producer", self.metadata.producer.as_deref()),
        ];
        let mut body = String::new();
        for (key, value) in entries {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                body.push_str(&format!(" /{key} {}", pdf_text_string(value)));
            }
        }
        if body.is_empty() {
            None
        } else {
            Some(self.writer.add(format!("<<{body} >>").into_bytes()))
        }
    }
}

/// 单页 PDF 便捷后端。
#[derive(Debug, Default)]
pub struct PdfBackend {
    options: PdfOptions,
}

impl PdfBackend {
    /// 使用默认选项创建后端。
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用指定选项创建后端。
    pub fn with_options(options: PdfOptions) -> Self {
        Self { options }
    }

    /// 把单个场景写成单页 PDF。
    pub fn render(&mut self, scene: &Scene) -> Result<Vec<u8>, RenderError> {
        let mut document = PdfDocument::new(self.options.clone());
        document.add_page(scene)?;
        document.finish()
    }
}

struct ContentBuilder<'a> {
    content: String,
    current: Point,
    start: Point,
    images: &'a HashMap<AssetId, ImageResource>,
    gradients: &'a HashMap<String, ShadingResource>,
    fonts: &'a HashMap<AssetId, FontResource>,
    alphas: &'a HashMap<u8, String>,
}

impl<'a> ContentBuilder<'a> {
    fn new(
        images: &'a HashMap<AssetId, ImageResource>,
        gradients: &'a HashMap<String, ShadingResource>,
        fonts: &'a HashMap<AssetId, FontResource>,
        alphas: &'a HashMap<u8, String>,
    ) -> Self {
        Self {
            content: String::new(),
            current: Point::default(),
            start: Point::default(),
            images,
            gradients,
            fonts,
            alphas,
        }
    }

    fn raw(&mut self, text: &str) {
        self.content.push_str(text);
    }

    fn into_bytes(self) -> Vec<u8> {
        self.content.into_bytes()
    }

    fn group(&mut self, group: &Group, inherited: f32) {
        let opacity = inherited * group.opacity.clamp(0.0, 1.0);
        self.raw("q\n");
        self.matrix(group.transform);
        if let Some(clip) = &group.clip {
            self.path(&clip.path);
            match clip.rule {
                FillRule::NonZero => self.raw("W n\n"),
                FillRule::EvenOdd => self.raw("W* n\n"),
            }
        }
        for child in &group.children {
            match child {
                Element::Group(child) => self.group(child, opacity),
                Element::Path(path) => self.path_element(path, opacity),
                Element::Image(image) => self.image_element(image, opacity),
                Element::Text(text) => self.text_element(text, opacity),
            }
        }
        self.raw("Q\n");
    }

    fn apply_alpha(&mut self, alpha: f32) {
        if let Some(name) = self.alphas.get(&alpha_byte(alpha)).cloned() {
            self.raw(&format!("/{name} gs\n"));
        }
    }

    fn path_element(&mut self, element: &PathElement, opacity: f32) {
        if let Some(paint) = &element.fill {
            let alpha = opacity * paint_alpha(paint);
            self.raw("q\n");
            self.apply_alpha(alpha);
            if let Some(name) = self.gradient_name(paint) {
                self.path(&element.path);
                match element.fill_rule {
                    FillRule::NonZero => self.raw("W n\n"),
                    FillRule::EvenOdd => self.raw("W* n\n"),
                }
                self.raw(&format!("/{name} sh\n"));
            } else if let Some(color) = solid_paint(paint) {
                self.path(&element.path);
                self.set_fill_color(color);
                match element.fill_rule {
                    FillRule::NonZero => self.raw("f\n"),
                    FillRule::EvenOdd => self.raw("f*\n"),
                }
            }
            self.raw("Q\n");
        }
        if let Some(stroke) = &element.stroke
            && let Some(color) = solid_paint(&stroke.paint)
        {
            self.raw("q\n");
            self.apply_alpha(opacity * paint_alpha(&stroke.paint));
            self.path(&element.path);
            self.set_stroke_color(color);
            self.set_stroke_style(&stroke.style);
            self.raw("S\nQ\n");
        }
    }

    fn text_element(&mut self, element: &TextElement, opacity: f32) {
        if element.stroke.is_none()
            && let Some(resource) = self.fonts.get(&element.font.id())
            && resource.embedded
            && let Some(color) = element.fill.as_ref().and_then(solid_paint)
        {
            self.embedded_text(element, resource, color, opacity * color.a);
            return;
        }
        self.outline_text(element, opacity);
    }

    fn embedded_text(
        &mut self,
        element: &TextElement,
        resource: &FontResource,
        color: Rgba,
        alpha: f32,
    ) {
        self.raw("q\n");
        self.apply_alpha(alpha);
        self.set_fill_color(color);
        self.raw("BT\n");
        self.raw(&format!("/{} {} Tf\n", resource.name, number(element.size)));
        for glyph in &element.glyphs {
            let Ok(glyph_id) = u16::try_from(glyph.id) else {
                continue;
            };
            let transform = Matrix::scale(1.0, -1.0)
                .then(Matrix::translate(f64::from(glyph.x), f64::from(glyph.y)))
                .then(element.transform);
            self.raw(&format!(
                "{} {} {} {} {} {} Tm\n",
                number(transform.a),
                number(transform.b),
                number(transform.c),
                number(transform.d),
                number(transform.e),
                number(transform.f)
            ));
            self.raw(&format!("<{glyph_id:04X}> Tj\n"));
        }
        self.raw("ET\nQ\n");
    }

    fn outline_text(&mut self, element: &TextElement, opacity: f32) {
        let Ok(face) = ttf_parser::Face::parse(element.font.data(), element.font.face_index())
        else {
            return;
        };
        let units = face.units_per_em();
        if units == 0 {
            return;
        }
        let paint = element
            .fill
            .as_ref()
            .or_else(|| element.stroke.as_ref().map(|stroke| &stroke.paint));
        let Some(color) = paint.and_then(solid_paint) else {
            return;
        };
        self.set_fill_color(color);
        let scale = element.size / f64::from(units);
        for glyph in &element.glyphs {
            let Ok(glyph_id) = u16::try_from(glyph.id) else {
                continue;
            };
            let mut builder = GlyphPathBuilder::default();
            if face
                .outline_glyph(ttf_parser::GlyphId(glyph_id), &mut builder)
                .is_none()
                || builder.path.is_empty()
            {
                continue;
            }
            let transform = Matrix::scale(scale, -scale)
                .then(Matrix::translate(f64::from(glyph.x), f64::from(glyph.y)))
                .then(element.transform);
            self.raw("q\n");
            self.apply_alpha(opacity * color.a);
            self.matrix(transform);
            self.path(&builder.path);
            self.raw("f\nQ\n");
        }
    }

    fn gradient_name(&self, paint: &Paint) -> Option<String> {
        self.gradients
            .get(&format!("{paint:?}"))
            .map(|resource| resource.name.clone())
    }

    fn image_element(&mut self, element: &ImageElement, opacity: f32) {
        let Some(resource) = self.images.get(&element.image.id()) else {
            return;
        };
        self.raw("q\n");
        self.apply_alpha(opacity * element.alpha);
        let flip = Matrix::scale(1.0, -1.0).then(Matrix::translate(0.0, 1.0));
        let transform = flip
            .then(Matrix::scale(
                f64::from(resource.width.max(1)),
                f64::from(resource.height.max(1)),
            ))
            .then(element.transform);
        self.matrix(transform);
        self.raw(&format!("/{} Do\nQ\n", resource.name));
    }

    fn set_fill_color(&mut self, color: Rgba) {
        self.raw(&format!(
            "{} {} {} rg\n",
            number(f64::from(color.r)),
            number(f64::from(color.g)),
            number(f64::from(color.b))
        ));
    }

    fn set_stroke_color(&mut self, color: Rgba) {
        self.raw(&format!(
            "{} {} {} RG\n",
            number(f64::from(color.r)),
            number(f64::from(color.g)),
            number(f64::from(color.b))
        ));
    }

    fn set_stroke_style(&mut self, style: &crate::path::StrokeStyle) {
        self.raw(&format!("{} w\n", number(style.width)));
        let cap = match style.cap {
            LineCap::Butt => 0,
            LineCap::Round => 1,
            LineCap::Square => 2,
        };
        let join = match style.join {
            LineJoin::Miter => 0,
            LineJoin::Round => 1,
            LineJoin::Bevel => 2,
        };
        self.raw(&format!(
            "{cap} J\n{join} j\n{} M\n",
            number(style.miter_limit)
        ));
        if style.dash.is_empty() {
            self.raw("[] 0 d\n");
        } else {
            let dashes = style
                .dash
                .iter()
                .map(|value| number(*value))
                .collect::<Vec<_>>()
                .join(" ");
            self.raw(&format!("[{dashes}] {} d\n", number(style.dash_offset)));
        }
    }

    fn matrix(&mut self, matrix: Matrix) {
        if matrix != Matrix::IDENTITY {
            self.raw(&format!(
                "{} {} {} {} {} {} cm\n",
                number(matrix.a),
                number(matrix.b),
                number(matrix.c),
                number(matrix.d),
                number(matrix.e),
                number(matrix.f)
            ));
        }
    }

    fn path(&mut self, path: &Path) {
        for segment in &path.segments {
            match *segment {
                PathSegment::MoveTo(point) => self.point("m", point),
                PathSegment::LineTo(point) => self.point("l", point),
                PathSegment::QuadraticTo { control, to } => {
                    let current = self.current;
                    let c1 = Point::new(
                        current.x + (control.x - current.x) * 2.0 / 3.0,
                        current.y + (control.y - current.y) * 2.0 / 3.0,
                    );
                    let c2 = Point::new(
                        to.x + (control.x - to.x) * 2.0 / 3.0,
                        to.y + (control.y - to.y) * 2.0 / 3.0,
                    );
                    self.curve(c1, c2, to);
                }
                PathSegment::CubicTo {
                    control1,
                    control2,
                    to,
                } => self.curve(control1, control2, to),
                PathSegment::ArcTo {
                    rx,
                    ry,
                    rotation,
                    large_arc,
                    sweep,
                    to,
                } => self.arc(rx, ry, rotation, large_arc, sweep, to),
                PathSegment::Close => {
                    self.raw("h\n");
                    self.current = self.start;
                }
            }
        }
    }

    fn point(&mut self, op: &str, point: Point) {
        self.raw(&format!("{} {} {op}\n", number(point.x), number(point.y)));
        self.current = point;
        if op == "m" {
            self.start = point;
        }
    }

    fn curve(&mut self, control1: Point, control2: Point, to: Point) {
        self.raw(&format!(
            "{} {} {} {} {} {} c\n",
            number(control1.x),
            number(control1.y),
            number(control2.x),
            number(control2.y),
            number(to.x),
            number(to.y)
        ));
        self.current = to;
    }

    fn arc(&mut self, rx: f64, ry: f64, rotation: f64, large_arc: bool, sweep: bool, to: Point) {
        use kurbo::{Arc as KurboArc, SvgArc, Vec2};
        let arc = SvgArc {
            from: kurbo::Point::new(self.current.x, self.current.y),
            to: kurbo::Point::new(to.x, to.y),
            radii: Vec2::new(rx, ry),
            x_rotation: rotation,
            large_arc,
            sweep,
        };
        if let Some(arc) = KurboArc::from_svg_arc(&arc) {
            arc.to_cubic_beziers(0.1, |p1, p2, p3| {
                self.curve(
                    Point::new(p1.x, p1.y),
                    Point::new(p2.x, p2.y),
                    Point::new(p3.x, p3.y),
                );
            });
        } else {
            self.point("l", to);
        }
    }
}

#[derive(Default)]
struct GlyphPathBuilder {
    path: Path,
}

impl ttf_parser::OutlineBuilder for GlyphPathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(Point::new(f64::from(x), f64::from(y)));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(Point::new(f64::from(x), f64::from(y)));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path.quadratic_to(
            Point::new(f64::from(x1), f64::from(y1)),
            Point::new(f64::from(x), f64::from(y)),
        );
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path.cubic_to(
            Point::new(f64::from(x1), f64::from(y1)),
            Point::new(f64::from(x2), f64::from(y2)),
            Point::new(f64::from(x), f64::from(y)),
        );
    }

    fn close(&mut self) {
        self.path.close();
    }
}

fn can_embed_font(font: &FontAsset) -> bool {
    if font.data().starts_with(b"ttcf") {
        return false;
    }
    let Ok(face) = ttf_parser::Face::parse(font.data(), font.face_index()) else {
        return false;
    };
    [b"head", b"hhea", b"hmtx", b"maxp", b"glyf", b"loca"]
        .iter()
        .all(|tag| {
            face.raw_face()
                .table(ttf_parser::Tag::from_bytes(tag))
                .is_some()
        })
}

fn solid_paint(paint: &Paint) -> Option<Rgba> {
    match paint {
        Paint::Solid(color) => Some(*color),
        _ => None,
    }
}

fn pdf_text_string(value: &str) -> String {
    if value.is_ascii() {
        let mut text = String::from("(");
        for ch in value.chars() {
            match ch {
                '(' => text.push_str("\\("),
                ')' => text.push_str("\\)"),
                '\\' => text.push_str("\\\\"),
                '\n' => text.push_str("\\n"),
                '\r' => text.push_str("\\r"),
                '\t' => text.push_str("\\t"),
                ch if ch.is_control() => text.push_str(&format!("\\{:03o}", ch as u32)),
                ch => text.push(ch),
            }
        }
        text.push(')');
        text
    } else {
        let mut text = String::from("<FEFF");
        for unit in value.encode_utf16() {
            text.push_str(&format!("{unit:04X}"));
        }
        text.push('>');
        text
    }
}
fn paint_alpha(paint: &Paint) -> f32 {
    match paint {
        Paint::Solid(color) => color.a,
        _ => 1.0,
    }
}

fn alpha_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn insert_alpha(alphas: &mut HashSet<u8>, value: f32) {
    let alpha = alpha_byte(value);
    if alpha < 255 {
        alphas.insert(alpha);
    }
}
struct PdfWriter {
    version: PdfVersion,
    objects: Vec<Vec<u8>>,
}

impl PdfWriter {
    fn new(version: PdfVersion) -> Self {
        Self {
            version,
            objects: Vec::new(),
        }
    }

    fn placeholder(&mut self) -> u32 {
        self.objects.push(Vec::new());
        self.objects.len() as u32
    }

    fn set(&mut self, id: u32, body: Vec<u8>) {
        if let Some(slot) = self.objects.get_mut(id as usize - 1) {
            *slot = body;
        }
    }

    fn add(&mut self, body: Vec<u8>) -> u32 {
        self.objects.push(body);
        self.objects.len() as u32
    }

    fn stream(&mut self, dict: &str, data: &[u8]) -> u32 {
        let mut body = format!("<< /Length {} {dict} >>\nstream\n", data.len()).into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(b"\nendstream");
        self.add(body)
    }

    fn replace_stream(&mut self, id: u32, dict: &str, data: &[u8]) {
        let mut body = format!("<< /Length {} {dict} >>\nstream\n", data.len()).into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(b"\nendstream");
        self.set(id, body);
    }

    fn finish(self, root: u32, info: Option<u32>) -> Vec<u8> {
        let mut output = self.version.header().to_vec();
        let mut offsets = Vec::with_capacity(self.objects.len());
        for (index, body) in self.objects.iter().enumerate() {
            offsets.push(output.len() as u64);
            output.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
            output.extend_from_slice(body);
            output.extend_from_slice(b"\nendobj\n");
        }
        let xref = output.len() as u64;
        output.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", self.objects.len() + 1).as_bytes(),
        );
        for offset in offsets {
            output.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        let info = info
            .map(|info| format!(" /Info {info} 0 R"))
            .unwrap_or_default();
        output.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root {} 0 R{info} >>\nstartxref\n{xref}\n%%EOF\n",
                self.objects.len() + 1,
                root
            )
            .as_bytes(),
        );
        output
    }
}

fn build_to_unicode(font: &FontAsset, used: &HashSet<u16>) -> Option<String> {
    let face = ttf_parser::Face::parse(font.data(), font.face_index()).ok()?;
    let cmap = face.tables().cmap?;
    let mut mapping = HashMap::<u16, u32>::new();
    for subtable in cmap.subtables {
        if !subtable.is_unicode() {
            continue;
        }
        subtable.codepoints(|codepoint| {
            if (0xD800..=0xDFFF).contains(&codepoint) {
                return;
            }
            if let Some(glyph) = subtable.glyph_index(codepoint) {
                mapping.entry(glyph.0).or_insert(codepoint);
            }
        });
    }
    let mut entries = mapping
        .into_iter()
        .filter(|(glyph, _)| used.contains(glyph))
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.0);
    if entries.is_empty() {
        return None;
    }
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    for chunk in entries.chunks(100) {
        cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (glyph, codepoint) in chunk {
            cmap.push_str(&format!("<{glyph:04X}> <{}>\n", utf16_hex(*codepoint)));
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    Some(cmap)
}

fn utf16_hex(codepoint: u32) -> String {
    if codepoint <= 0xFFFF {
        format!("{codepoint:04X}")
    } else {
        let value = codepoint - 0x10000;
        let high = 0xD800 + (value >> 10);
        let low = 0xDC00 + (value & 0x3FF);
        format!("{high:04X}{low:04X}")
    }
}

fn subset_truetype(data: &[u8], face_index: u32, used: &HashSet<u16>) -> Option<Vec<u8>> {
    if face_index != 0 || data.len() < 12 || data.starts_with(b"ttcf") {
        return None;
    }
    let num_tables = usize::from(be_u16(data, 4)?);
    if num_tables == 0 || 12 + num_tables * 16 > data.len() {
        return None;
    }
    let mut tables = Vec::with_capacity(num_tables);
    for index in 0..num_tables {
        let record = 12 + index * 16;
        let tag: [u8; 4] = data.get(record..record + 4)?.try_into().ok()?;
        let offset = be_u32(data, record + 8)? as usize;
        let length = be_u32(data, record + 12)? as usize;
        let body = data.get(offset..offset.checked_add(length)?)?;
        tables.push((tag, body));
    }
    let find = |tag: &[u8; 4]| {
        tables
            .iter()
            .find(|(name, _)| name == tag)
            .map(|(_, body)| *body)
    };
    let head = find(b"head")?;
    let maxp = find(b"maxp")?;
    let loca = find(b"loca")?;
    let glyf = find(b"glyf")?;
    if head.len() < 54 || maxp.len() < 6 {
        return None;
    }
    let num_glyphs = usize::from(be_u16(maxp, 4)?);
    if num_glyphs == 0 {
        return None;
    }
    let long_loca = be_i16(head, 50)? != 0;
    let mut offsets = Vec::with_capacity(num_glyphs + 1);
    for index in 0..=num_glyphs {
        let offset = if long_loca {
            usize::try_from(be_u32(loca, index * 4)?).ok()?
        } else {
            usize::from(be_u16(loca, index * 2)?) * 2
        };
        offsets.push(offset);
    }
    if offsets.last().copied()? > glyf.len() {
        return None;
    }

    let mut used = used
        .iter()
        .copied()
        .filter(|glyph| usize::from(*glyph) < num_glyphs)
        .collect::<HashSet<_>>();
    used.insert(0);
    let mut queue = used.iter().copied().collect::<Vec<_>>();
    while let Some(glyph) = queue.pop() {
        let index = usize::from(glyph);
        let start = offsets[index];
        let end = offsets[index + 1];
        if end <= start {
            continue;
        }
        let body = &glyf[start..end];
        if be_i16(body, 0)? >= 0 {
            continue;
        }
        let mut position = 10;
        loop {
            let flags = be_u16(body, position)?;
            let component = be_u16(body, position + 2)?;
            position += 4;
            position += if flags & 0x0001 != 0 { 4 } else { 2 };
            if flags & 0x0008 != 0 {
                position += 2;
            } else if flags & 0x0040 != 0 {
                position += 4;
            } else if flags & 0x0080 != 0 {
                position += 8;
            }
            if usize::from(component) < num_glyphs && used.insert(component) {
                queue.push(component);
            }
            if flags & 0x0020 == 0 {
                break;
            }
        }
    }

    let mut new_glyf = Vec::new();
    let mut new_offsets = Vec::with_capacity(num_glyphs + 1);
    for index in 0..num_glyphs {
        new_offsets.push(u32::try_from(new_glyf.len()).ok()?);
        if used.contains(&(index as u16)) {
            new_glyf.extend_from_slice(&glyf[offsets[index]..offsets[index + 1]]);
            while new_glyf.len() % 4 != 0 {
                new_glyf.push(0);
            }
        }
    }
    new_offsets.push(u32::try_from(new_glyf.len()).ok()?);
    let mut new_loca = Vec::with_capacity(new_offsets.len() * 4);
    for offset in new_offsets {
        new_loca.extend_from_slice(&offset.to_be_bytes());
    }
    let mut new_head = head.to_vec();
    new_head[50..52].copy_from_slice(&1_i16.to_be_bytes());
    new_head[8..12].fill(0);

    let mut output_tables = tables
        .iter()
        .map(|(tag, body)| (*tag, body.to_vec()))
        .collect::<Vec<_>>();
    for (tag, body) in &mut output_tables {
        match tag.as_slice() {
            b"glyf" => *body = new_glyf.clone(),
            b"loca" => *body = new_loca.clone(),
            b"head" => *body = new_head.clone(),
            _ => {}
        }
    }
    output_tables.sort_by_key(|table| table.0);

    let mut output = data[..12].to_vec();
    let table_count = u16::try_from(output_tables.len()).ok()?;
    output[4..6].copy_from_slice(&table_count.to_be_bytes());
    let table_offset = 12 + output_tables.len() * 16;
    let mut body = Vec::new();
    for (tag, table) in &output_tables {
        while body.len() % 4 != 0 {
            body.push(0);
        }
        let offset = table_offset + body.len();
        output.extend_from_slice(tag);
        output.extend_from_slice(&checksum(table).to_be_bytes());
        output.extend_from_slice(&u32::try_from(offset).ok()?.to_be_bytes());
        output.extend_from_slice(&u32::try_from(table.len()).ok()?.to_be_bytes());
        body.extend_from_slice(table);
    }
    output.extend_from_slice(&body);
    Some(output)
}

fn be_u16(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn be_i16(data: &[u8], offset: usize) -> Option<i16> {
    Some(i16::from_be_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn be_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn checksum(data: &[u8]) -> u32 {
    let mut sum = 0_u32;
    for chunk in data.chunks(4) {
        let mut word = [0_u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(word));
    }
    sum
}

fn deflate(data: &[u8]) -> Result<Vec<u8>, RenderError> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder
        .write_all(data)
        .map_err(|error| RenderError::Image(error.to_string()))?;
    encoder
        .finish()
        .map_err(|error| RenderError::Image(error.to_string()))
}

fn number(value: f64) -> String {
    let mut text = format!("{value:.4}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{PdfBackend, PdfDocument, PdfMetadata, PdfOptions};
    use crate::geometry::{Matrix, Point, Size};
    use crate::image::ImageFormat;
    use crate::paint::{GradientExtend, GradientStop, LinearGradient, Paint, Rgba};
    use crate::path::{FillRule, Path};
    use crate::resource::{PoolBudget, ResourcePool};
    use crate::scene::{
        Element, Group, ImageElement, PathElement, PositionedGlyph, Scene, TextElement,
    };

    fn pdf_text(pdf: &[u8]) -> String {
        use std::io::Read as _;

        let mut text = String::from_utf8_lossy(pdf).into_owned();
        let mut rest = pdf;
        while let Some(start) = rest.windows(7).position(|window| window == b"stream\n") {
            let body = &rest[start + 7..];
            let Some(end) = body.windows(9).position(|window| window == b"endstream") else {
                break;
            };
            let mut decoded = String::new();
            if flate2::read::ZlibDecoder::new(&body[..end])
                .read_to_string(&mut decoded)
                .is_ok()
            {
                text.push_str(&decoded);
            }
            rest = &body[end + 9..];
        }
        text
    }
    fn scene() -> Scene {
        Scene {
            size: Size::new(100.0, 100.0),
            root: Group::default(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn writes_empty_page() {
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene()).expect("page");
        let pdf = document.finish().expect("pdf");
        let text = pdf_text(&pdf);
        assert!(text.starts_with("%PDF-1.7"));
        assert!(text.contains("/MediaBox [0 0 283.4646 283.4646]"));
        assert!(pdf.ends_with(b"%%EOF\n"));
    }

    #[test]
    fn writes_multiple_pages() {
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene()).expect("page 1");
        document.add_page(&scene()).expect("page 2");
        assert_eq!(document.page_count(), 2);
        let pdf = document.finish().expect("pdf");
        let text = pdf_text(&pdf);
        assert!(text.contains("/Count 2"));
        assert_eq!(text.matches("/MediaBox").count(), 2);
    }
    #[test]
    fn writes_path_commands() {
        let mut path = Path::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(90.0, 10.0));
        path.line_to(Point::new(90.0, 90.0));
        path.close();
        let mut root = Group::default();
        root.children.push(Element::Path(PathElement {
            path: std::sync::Arc::new(path),
            fill: Some(Paint::Solid(Rgba::new(1.0, 0.0, 0.0, 1.0))),
            stroke: None,
            fill_rule: FillRule::NonZero,
        }));
        let scene = Scene {
            size: Size::new(100.0, 100.0),
            root,
            diagnostics: Vec::new(),
        };
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene).expect("page");
        let pdf = document.finish().expect("pdf");
        let text = pdf_text(&pdf);
        assert!(text.contains("10 10 m"));
        assert!(text.contains("90 90 l"));
        assert!(text.contains("1 0 0 rg"));
        assert!(text.contains("f\n"));
    }

    #[test]
    fn writes_image_xobject() {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut encoder = png::Encoder::new(&mut cursor, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("png header");
            writer
                .write_image_data(&[
                    0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255,
                ])
                .expect("png data");
            writer.finish().expect("png finish");
        }
        let pool = ResourcePool::new(PoolBudget::default());
        let image = pool
            .embedded_image(ImageFormat::Png, &cursor.into_inner(), None)
            .expect("image");
        let mut root = Group::default();
        root.children.push(Element::Image(ImageElement {
            image,
            transform: Matrix::scale(0.5, 0.5),
            alpha: 1.0,
        }));
        let scene = Scene {
            size: Size::new(20.0, 20.0),
            root,
            diagnostics: Vec::new(),
        };
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene).expect("page");
        let pdf = document.finish().expect("pdf");
        let text = pdf_text(&pdf);
        assert!(text.contains("/XObject"));
        assert!(text.contains("/Im1"));
        assert!(text.contains("/Im1 Do"));
    }

    #[test]
    fn writes_linear_shading() {
        let mut path = Path::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(90.0, 10.0));
        path.line_to(Point::new(90.0, 90.0));
        path.close();
        let mut root = Group::default();
        root.children.push(Element::Path(PathElement {
            path: std::sync::Arc::new(path),
            fill: Some(Paint::Linear(LinearGradient {
                start: Point::new(10.0, 50.0),
                end: Point::new(90.0, 50.0),
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        color: Rgba::new(1.0, 0.0, 0.0, 1.0),
                    },
                    GradientStop {
                        offset: 1.0,
                        color: Rgba::new(0.0, 0.0, 1.0, 1.0),
                    },
                ],
                extend: GradientExtend::Pad,
            })),
            stroke: None,
            fill_rule: FillRule::NonZero,
        }));
        let scene = Scene {
            size: Size::new(100.0, 100.0),
            root,
            diagnostics: Vec::new(),
        };
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene).expect("page");
        let pdf = document.finish().expect("pdf");
        let text = pdf_text(&pdf);
        assert!(text.contains("/ShadingType 2"));
        assert!(text.contains("/FunctionType 2"));
        assert!(text.contains(" sh\n"));
    }

    #[cfg(windows)]
    #[test]
    fn embeds_text_when_font_available() {
        let Ok(bytes) = std::fs::read("C:/Windows/Fonts/arial.ttf") else {
            return;
        };
        let Ok(face) = ttf_parser::Face::parse(&bytes, 0) else {
            return;
        };
        let Some(glyph) = face.glyph_index('A') else {
            return;
        };
        let pool = ResourcePool::new(PoolBudget::default());
        let font = pool.embedded_font(&bytes, 0).expect("embedded font");
        let mut root = Group::default();
        root.children.push(Element::Text(TextElement {
            font,
            size: 10.0,
            transform: Matrix::IDENTITY,
            glyphs: vec![PositionedGlyph {
                id: u32::from(glyph.0),
                x: 0.0,
                y: 10.0,
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
        let text = pdf_text(&pdf);
        assert!(text.contains("q\n"));
        assert!(text.contains("/Type0"));
        assert!(text.contains(" Tj\n"));
    }

    #[cfg(windows)]
    #[test]
    fn writes_to_unicode_for_embedded_font() {
        let Ok(bytes) = std::fs::read("C:/Windows/Fonts/arial.ttf") else {
            return;
        };
        let Ok(face) = ttf_parser::Face::parse(&bytes, 0) else {
            return;
        };
        let Some(glyph) = face.glyph_index('A') else {
            return;
        };
        let pool = ResourcePool::new(PoolBudget::default());
        let font = pool.embedded_font(&bytes, 0).expect("embedded font");
        let mut root = Group::default();
        root.children.push(Element::Text(TextElement {
            font,
            size: 10.0,
            transform: Matrix::IDENTITY,
            glyphs: vec![PositionedGlyph {
                id: u32::from(glyph.0),
                x: 0.0,
                y: 10.0,
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
        let text = pdf_text(&pdf);
        assert!(text.contains("/ToUnicode"));
        assert!(text.contains("beginbfchar"));
    }
    #[test]
    fn writes_group_opacity() {
        let mut path = Path::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(90.0, 10.0));
        path.line_to(Point::new(90.0, 90.0));
        path.close();
        let mut inner = Group::new(Matrix::IDENTITY);
        inner.opacity = 0.5;
        inner.children.push(Element::Path(PathElement {
            path: std::sync::Arc::new(path),
            fill: Some(Paint::Solid(Rgba::new(1.0, 0.0, 0.0, 1.0))),
            stroke: None,
            fill_rule: FillRule::NonZero,
        }));
        let mut root = Group::default();
        root.children.push(Element::Group(inner));
        let scene = Scene {
            size: Size::new(100.0, 100.0),
            root,
            diagnostics: Vec::new(),
        };
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene).expect("page");
        let pdf = document.finish().expect("pdf");
        let text = pdf_text(&pdf);
        assert!(text.contains("/ExtGState"));
        assert!(text.contains(" gs\n"));
    }
    #[test]
    fn writes_metadata_and_outlines() {
        let mut document = PdfDocument::new(PdfOptions::default());
        document.add_page(&scene()).expect("page 1");
        document.add_page(&scene()).expect("page 2");
        document.set_metadata(PdfMetadata {
            title: Some("Test".to_owned()),
            author: Some("OFDBox".to_owned()),
            ..PdfMetadata::default()
        });
        document.add_outline("First", 0);
        document.add_outline("Second", 1);
        let pdf = document.finish().expect("pdf");
        let text = pdf_text(&pdf);
        assert!(text.contains("/Info"));
        assert!(text.contains("/Title (Test)"));
        assert!(text.contains("/Outlines"));
        assert!(text.contains("/PageMode /UseOutlines"));
        assert!(text.contains("/Title (First)"));
        assert_eq!(text.matches("/Dest [").count(), 2);
    }
    #[test]
    fn backend_writes_single_page() {
        let pdf = PdfBackend::new().render(&scene()).expect("pdf");
        assert!(pdf.starts_with(b"%PDF-1.7"));
    }
}
