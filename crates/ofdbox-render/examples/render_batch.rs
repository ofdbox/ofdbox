//! 批量把目录下的 OFD 文档转成 PNG。
//!
//! 用法：
//! `cargo run -p ofdbox-render --release --example render_batch -- <输入目录> <输出目录>`
//!
//! 每个文档输出到独立子目录，页面命名为 `page_0001.png`。
//! 无法读取或渲染的文档/页面会被跳过，并记录到输出目录的 `_skipped.txt`。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ofdbox_core::OfdPackage;
use ofdbox_render::{
    Backend, BackendFrame, FontProvider, FontRequest, OfdFrontend, PoolBudget, RasterTarget,
    RenderError, RenderOptions, ResourcePool, VelloCpuBackend,
};

const DPI: f64 = 96.0;

type FontCache = Arc<Mutex<HashMap<PathBuf, Option<Arc<[u8]>>>>>;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(input) = args.next().map(PathBuf::from) else {
        eprintln!("用法: render_batch <输入目录> <输出目录>");
        std::process::exit(2);
    };
    let Some(output) = args.next().map(PathBuf::from) else {
        eprintln!("用法: render_batch <输入目录> <输出目录>");
        std::process::exit(2);
    };
    if !input.is_dir() {
        eprintln!("输入目录不存在: {}", input.display());
        std::process::exit(2);
    }
    fs::create_dir_all(&output).expect("create output directory");

    let mut packages = Vec::new();
    collect_packages(&input, &mut packages);
    packages.sort();
    eprintln!(
        "输入 {} 个 OFD，DPI {DPI}，输出 {}",
        packages.len(),
        output.display()
    );

    let pool = ResourcePool::new(PoolBudget {
        font_bytes: 128 * 1024 * 1024,
        image_bytes: 256 * 1024 * 1024,
        scene_bytes: 32 * 1024 * 1024,
    });
    let fonts: FontCache = Arc::new(Mutex::new(HashMap::new()));
    let mut backend = VelloCpuBackend::new(RenderOptions {
        dpi: DPI,
        ..Default::default()
    });
    let mut folders = FolderNames::default();
    let mut skipped = Vec::new();
    let started = Instant::now();
    let mut documents = 0usize;
    let mut failed_documents = 0usize;
    let mut pages = 0usize;
    let mut failed_pages = 0usize;

    for path in &packages {
        let label = path
            .strip_prefix(&input)
            .unwrap_or(path)
            .display()
            .to_string();
        let folder_name = folders.next(path);
        let folder = output.join(&folder_name);
        let mut package = match OfdPackage::open(path) {
            Ok(package) => package,
            Err(error) => {
                skipped.push(format!("{label}: {error}"));
                failed_documents += 1;
                continue;
            }
        };
        let document = match package.parse_document() {
            Ok(document) => document,
            Err(error) => {
                skipped.push(format!("{label}: {error}"));
                failed_documents += 1;
                continue;
            }
        };
        let Some(pages_view) = document.pages() else {
            skipped.push(format!("{label}: 没有页面"));
            failed_documents += 1;
            continue;
        };
        fs::create_dir_all(&folder).expect("create document directory");
        let provider = SystemFonts {
            cache: Arc::clone(&fonts),
        };
        let mut frontend = OfdFrontend::new(
            &mut package,
            &pool,
            RenderOptions {
                dpi: DPI,
                ..Default::default()
            },
        )
        .with_font_provider(Box::new(provider));
        let mut document_pages = 0usize;
        let mut document_failed = 0usize;
        for (index, page) in pages_view.pages().enumerate() {
            pages += 1;
            let scene = match frontend.build_page(&page) {
                Ok(scene) => scene,
                Err(error) => {
                    skipped.push(format!("{label} [page {}]: {error}", index + 1));
                    failed_pages += 1;
                    document_failed += 1;
                    continue;
                }
            };
            let mut target = match RasterTarget::from_size(scene.size, DPI) {
                Ok(target) => target,
                Err(error) => {
                    skipped.push(format!("{label} [page {}]: {error}", index + 1));
                    failed_pages += 1;
                    document_failed += 1;
                    continue;
                }
            };
            let mut frame = match backend.begin_frame((), &mut target) {
                Ok(frame) => frame,
                Err(error) => {
                    skipped.push(format!("{label} [page {}]: {error}", index + 1));
                    failed_pages += 1;
                    document_failed += 1;
                    continue;
                }
            };
            if let Err(error) = frame.draw(&scene) {
                skipped.push(format!("{label} [page {}]: {error}", index + 1));
                failed_pages += 1;
                document_failed += 1;
                continue;
            }
            if let Err(error) = frame.finish() {
                skipped.push(format!("{label} [page {}]: {error}", index + 1));
                failed_pages += 1;
                document_failed += 1;
                continue;
            }
            let file = folder.join(format!("page_{:04}.png", index + 1));
            if let Err(error) = write_png(&file, &target) {
                skipped.push(format!("{label} [page {}]: {error}", index + 1));
                failed_pages += 1;
                document_failed += 1;
                continue;
            }
            document_pages += 1;
        }
        if document_pages == 0 {
            let _ = fs::remove_dir(&folder);
            failed_documents += 1;
        } else {
            documents += 1;
            eprintln!("{folder_name}: ok={document_pages} failed={document_failed}");
        }
    }

    if !skipped.is_empty() {
        let report = skipped.join("\n");
        let _ = fs::write(output.join("_skipped.txt"), format!("{report}\n"));
    }
    eprintln!(
        "完成: docs={documents} failed_docs={failed_documents} pages={pages} failed_pages={failed_pages} elapsed={:.1}s",
        started.elapsed().as_secs_f64()
    );
}

fn collect_packages(dir: &Path, output: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            collect_packages(&path, output);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ofd"))
        {
            output.push(path);
        }
    }
}

fn write_png(path: &Path, target: &RasterTarget) -> Result<(), RenderError> {
    let file = fs::File::create(path)?;
    let mut encoder = png::Encoder::new(
        BufWriter::new(file),
        u32::from(target.width()),
        u32::from(target.height()),
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder
        .write_header()
        .map_err(|error| RenderError::Image(error.to_string()))?;
    writer
        .write_image_data(target.data())
        .map_err(|error| RenderError::Image(error.to_string()))?;
    writer
        .finish()
        .map_err(|error| RenderError::Image(error.to_string()))
}

#[derive(Default)]
struct FolderNames {
    used: HashSet<String>,
}

impl FolderNames {
    fn next(&mut self, path: &Path) -> String {
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("document");
        let mut base = sanitize(stem);
        if base.is_empty() {
            base = "document".to_owned();
        }
        if base.chars().count() > 60 {
            base = format!(
                "{}_{}",
                base.chars().take(60).collect::<String>(),
                short_hash(stem)
            );
        }
        let mut candidate = base.clone();
        let mut index = 1;
        while !self.used.insert(candidate.to_lowercase()) {
            index += 1;
            candidate = format!("{base}_{index}");
        }
        candidate
    }
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_owned()
}

fn short_hash(value: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:08x}", hasher.finish() as u32)
}

struct SystemFonts {
    cache: FontCache,
}

impl FontProvider for SystemFonts {
    fn load(&mut self, request: &FontRequest<'_>) -> Result<Option<Arc<[u8]>>, RenderError> {
        let Some(path) = select_font(request) else {
            return Ok(None);
        };
        if let Some(cached) = self.cache.lock().expect("font cache").get(&path) {
            return Ok(cached.clone());
        }
        let bytes = fs::read(&path).ok().map(Arc::<[u8]>::from);
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
    let root = Path::new("C:/Windows/Fonts");
    let candidates: &[&str] = if name.contains("仿宋") || name.contains("fangsong") {
        &["simfang.ttf"]
    } else if name.contains("楷") || name.contains("kai") {
        &["simkai.ttf"]
    } else if name.contains("黑") || name.contains("hei") {
        &["simhei.ttf"]
    } else if name.contains("宋") || name.contains("song") {
        &["simsun.ttc"]
    } else if name.contains("雅黑") || name.contains("yahei") {
        &["msyh.ttc", "msyh.ttf"]
    } else if name.contains("times") {
        &["times.ttf"]
    } else if name.contains("arial") {
        &["arial.ttf"]
    } else {
        &["msyh.ttc", "simsun.ttc", "simhei.ttf", "arial.ttf"]
    };
    for candidate in candidates {
        let path = root.join(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}
