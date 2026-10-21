//! 打开 OFD 并显示第一页的桌面示例。
//!
//! 运行：
//! `cargo run -p ofdbox-render --release --example ofd_viewer`

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

use eframe::egui;
use ofdbox_core::OfdPackage;
use ofdbox_render::{
    Backend, BackendFrame, FontProvider, FontRequest, OfdFrontend, RasterTarget, RenderError,
    RenderOptions, ResourcePool, Scene, VelloCpuBackend,
};

const MAX_RENDER_WIDTH: u32 = 4096;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "OFDBox Viewer",
        options,
        Box::new(|cc| Ok(Box::new(Viewer::new(cc)))),
    )
}

enum Message {
    Loaded(Result<Arc<Scene>, String>),
    Rendered(Result<Rendered, String>),
}

struct Rendered {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

struct Viewer {
    scene: Option<Arc<Scene>>,
    texture: Option<egui::TextureHandle>,
    file: Option<String>,
    error: Option<String>,
    loading: bool,
    status: String,
    zoom: f32,
    last_requested_width: u32,
    receiver: Option<Receiver<Message>>,
}

impl Viewer {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_cjk_font(&cc.egui_ctx);
        let mut app = Self {
            scene: None,
            texture: None,
            file: None,
            error: None,
            loading: false,
            status: String::new(),
            zoom: 1.0,
            last_requested_width: 0,
            receiver: None,
        };
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("OFD", &["ofd"])
            .pick_file()
        {
            app.start_load(&cc.egui_ctx, path);
        }
        app
    }

    fn pick(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("OFD", &["ofd"])
            .pick_file()
        {
            self.start_load(ctx, path);
        }
    }

    fn start_load(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.error = None;
        self.texture = None;
        self.scene = None;
        self.last_requested_width = 0;
        self.loading = true;
        self.status = "解析 OFD…".to_owned();
        self.file = Some(path.display().to_string());
        let (sender, receiver) = channel();
        self.receiver = Some(receiver);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = load_scene(&path);
            let _ = sender.send(Message::Loaded(result));
            ctx.request_repaint();
        });
    }

    fn ensure_render(&mut self, ctx: &egui::Context, available: egui::Vec2) {
        if self.scene.is_none() || self.loading {
            return;
        }
        let ppp = ctx.pixels_per_point();
        let width = (available.x * self.zoom * ppp)
            .round()
            .clamp(320.0, MAX_RENDER_WIDTH as f32) as u32;
        if self.last_requested_width != 0
            && width.abs_diff(self.last_requested_width) < self.last_requested_width / 10
        {
            return;
        }
        self.last_requested_width = width;
        self.loading = true;
        self.status = "渲染中…".to_owned();
        let scene = Arc::clone(self.scene.as_ref().expect("scene"));
        let (sender, receiver) = channel();
        self.receiver = Some(receiver);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = render_scene(&scene, width);
            let _ = sender.send(Message::Rendered(result));
            ctx.request_repaint();
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        while let Ok(message) = receiver.try_recv() {
            match message {
                Message::Loaded(Ok(scene)) => {
                    self.scene = Some(scene);
                    self.loading = false;
                    self.last_requested_width = 0;
                }
                Message::Loaded(Err(error)) | Message::Rendered(Err(error)) => {
                    self.loading = false;
                    self.error = Some(error);
                }
                Message::Rendered(Ok(rendered)) => {
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [rendered.width as usize, rendered.height as usize],
                        &rendered.pixels,
                    );
                    self.texture = Some(ctx.load_texture("ofd-page", image, Default::default()));
                    self.loading = false;
                }
            }
        }
    }
}

impl eframe::App for Viewer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("打开 OFD").clicked() {
                    self.pick(ctx);
                }
                if let Some(file) = &self.file {
                    ui.label(file);
                }
                ui.separator();
                ui.label("缩放");
                if ui.button("−").clicked() {
                    self.zoom = (self.zoom - 0.25).max(0.25);
                    self.last_requested_width = 0;
                }
                if ui.button("+").clicked() {
                    self.zoom = (self.zoom + 0.25).min(4.0);
                    self.last_requested_width = 0;
                }
                ui.label(format!("{:.2}x", self.zoom));
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::RED, error);
                return;
            }
            let available = ui.available_size();
            self.ensure_render(ctx, available);
            if let Some(texture) = &self.texture {
                let size = texture.size_vec2();
                let scale = (available.x / size.x).min(available.y / size.y).min(1.0);
                ui.centered_and_justified(|ui| {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        texture.id(),
                        size * scale,
                    )));
                });
            }
            if self.loading {
                egui::Area::new("loading".into())
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(&self.status);
                            });
                        });
                    });
            } else if self.texture.is_none() {
                ui.centered_and_justified(|ui| ui.label("请选择 OFD 文件"));
            }
        });
    }
}

fn load_scene(path: &Path) -> Result<Arc<Scene>, String> {
    let mut package = OfdPackage::open(path).map_err(|error| error.to_string())?;
    let document = package
        .parse_document()
        .map_err(|error| error.to_string())?;
    let pages = document.pages().ok_or_else(|| "OFD 没有页面".to_owned())?;
    let page = pages
        .pages()
        .next()
        .ok_or_else(|| "OFD 没有页面".to_owned())?;
    let pool = ResourcePool::new(Default::default());
    let mut frontend = OfdFrontend::new(
        &mut package,
        &pool,
        RenderOptions {
            dpi: 144.0,
            ..Default::default()
        },
    )
    .with_font_provider(Box::new(SystemFonts::new()));
    let scene = frontend
        .build_page(&page)
        .map_err(|error| error.to_string())?;
    Ok(Arc::new(scene))
}

fn render_scene(scene: &Scene, width: u32) -> Result<Rendered, String> {
    let width = width.clamp(1, MAX_RENDER_WIDTH);
    let dpi = width as f64 / (scene.size.width / 25.4);
    let mut target = RasterTarget::from_size(scene.size, dpi).map_err(|error| error.to_string())?;
    let mut backend = VelloCpuBackend::new(RenderOptions {
        dpi,
        ..Default::default()
    });
    let mut frame = backend
        .begin_frame((), &mut target)
        .map_err(|error| error.to_string())?;
    frame.draw(scene).map_err(|error| error.to_string())?;
    frame.finish().map_err(|error| error.to_string())?;
    Ok(Rendered {
        width: u32::from(target.width()),
        height: u32::from(target.height()),
        pixels: target.data().to_vec(),
    })
}

type FontCache = Arc<Mutex<HashMap<PathBuf, Option<Arc<[u8]>>>>>;

struct SystemFonts {
    cache: FontCache,
}

impl SystemFonts {
    fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl FontProvider for SystemFonts {
    fn load(&mut self, request: &FontRequest<'_>) -> Result<Option<Arc<[u8]>>, RenderError> {
        let Some(path) = select_font(request) else {
            return Ok(None);
        };
        if let Some(cached) = self.cache.lock().expect("font cache").get(&path) {
            return Ok(cached.clone());
        }
        let bytes = std::fs::read(&path).ok().map(Arc::<[u8]>::from);
        self.cache
            .lock()
            .expect("font cache")
            .insert(path, bytes.clone());
        Ok(bytes)
    }
}

fn select_font(request: &FontRequest<'_>) -> Option<PathBuf> {
    let name = format!(
        "{} {}",
        request.font_name.unwrap_or_default(),
        request.family_name.unwrap_or_default()
    )
    .to_ascii_lowercase();
    let mut candidates: Vec<&str> = Vec::new();
    if name.contains("宋") || name.contains("song") || name.contains("simsun") {
        candidates.extend(["simsun.ttc", "simsun.ttf"]);
    } else if name.contains("黑") || name.contains("hei") {
        candidates.extend(["simhei.ttf", "msyh.ttc"]);
    } else if name.contains("楷") || name.contains("kai") {
        candidates.extend(["simkai.ttf", "simsun.ttc"]);
    } else if name.contains("雅黑") || name.contains("yahei") {
        candidates.extend(["msyh.ttc", "msyh.ttf"]);
    } else if name.contains("times") {
        candidates.extend(["times.ttf", "timesbd.ttf"]);
    } else if name.contains("tahoma") {
        candidates.extend(["tahoma.ttf", "tahomabd.ttf"]);
    } else if name.contains("arial") {
        candidates.extend(["arial.ttf", "arialbd.ttf"]);
    }
    candidates.extend([
        "msyh.ttc",
        "simsun.ttc",
        "simhei.ttf",
        "NotoSansCJK-Regular.ttc",
        "NotoSansSC-Regular.otf",
        "HarmonyOS_Sans_SC_Regular.ttf",
        "arial.ttf",
    ]);
    let mut roots = Vec::new();
    if cfg!(windows) {
        roots.push(PathBuf::from("C:/Windows/Fonts"));
    }
    roots.extend([
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("/system/fonts"),
    ]);
    for root in roots {
        for candidate in &candidates {
            let path = root.join(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("ttf") || extension.eq_ignore_ascii_case("ttc")
                }) {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn install_cjk_font(ctx: &egui::Context) {
    let roots = [
        "C:/Windows/Fonts",
        "/usr/share/fonts",
        "/usr/local/share/fonts",
        "/System/Library/Fonts",
        "/Library/Fonts",
        "/system/fonts",
    ];
    let names = [
        "msyh.ttf",
        "msyh.ttc",
        "simhei.ttf",
        "simsun.ttc",
        "NotoSansCJK-Regular.ttc",
        "NotoSansSC-Regular.otf",
        "HarmonyOS_Sans_SC_Regular.ttf",
    ];
    for root in roots {
        for name in names {
            let path = std::path::Path::new(root).join(name);
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let mut font = egui::FontData::from_owned(bytes);
            font.index = 0;
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert("cjk".to_owned(), Arc::new(font));
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                family.insert(0, "cjk".to_owned());
            }
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                family.push("cjk".to_owned());
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
}
