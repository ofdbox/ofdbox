//! 批量把目录下的 OFD 文档转成 PNG。
//!
//! 用法：
//! `cargo run -p ofdbox-render --release --example render_batch -- <输入目录> <输出目录>`
//!
//! 每个文档输出到独立子目录，页面命名为 `page_0001.png`。
//! 无法读取或渲染的文档/页面会被跳过，并记录到输出目录的 `_skipped.txt`；
//! 渲染过程中跳过的特性（竖排文字、PageBlock 等）聚合到 `_diagnostics.txt`。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ofdbox_core::OfdPackage;
use ofdbox_render::{
    Backend, BackendFrame, FontProvider, FontRequest, OfdFrontend, PoolBudget, RasterTarget,
    RenderDiagnostic, RenderError, RenderOptions, ResourcePool, VelloCpuBackend,
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
    let mut diagnostics = DiagnosticsReport::default();
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
        let mut document_diagnostics = DocumentDiagnostics::new(&label);
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
            diagnostics.record(&mut document_diagnostics, index + 1, &scene.diagnostics);
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
        if document_diagnostics.total > 0 {
            diagnostics.documents.push(document_diagnostics);
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
    let report = diagnostics.render(documents, pages, failed_documents, failed_pages);
    let _ = fs::write(output.join("_diagnostics.txt"), report);
    eprintln!(
        "诊断: total={} pages_with_diagnostics={}",
        diagnostics.total_count, diagnostics.pages_with_diagnostics
    );
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
    let charset = request.charset.unwrap_or_default().to_ascii_lowercase();
    let bold = request.bold;
    let italic = request.italic;
    let root = Path::new("C:/Windows/Fonts");
    let mut candidates: Vec<&str> = Vec::new();

    if charset.contains("symbol") {
        candidates.push("symbol.ttf");
    }
    if charset.contains("big5") {
        candidates.push("mingliub.ttc");
    }

    if name.contains("仿宋") || name.contains("fangsong") {
        candidates.push("simfang.ttf");
    } else if name.contains("楷") || name.contains("kai") {
        candidates.push("simkai.ttf");
    } else if name.contains("黑") || name.contains("hei") {
        candidates.push("simhei.ttf");
    } else if name.contains("宋") || name.contains("song") {
        candidates.push("simsun.ttc");
    } else if name.contains("雅黑") || name.contains("yahei") {
        if bold {
            candidates.extend(["msyhbd.ttc", "msyh.ttc"]);
        } else if italic {
            candidates.extend(["msyhl.ttc", "msyh.ttc"]);
        } else {
            candidates.extend(["msyh.ttc", "msyh.ttf"]);
        }
    } else if name.contains("times") {
        if bold && italic {
            candidates.extend(["timesbi.ttf", "timesi.ttf", "timesbd.ttf"]);
        } else if bold {
            candidates.extend(["timesbd.ttf", "times.ttf"]);
        } else if italic {
            candidates.extend(["timesi.ttf", "times.ttf"]);
        } else {
            candidates.push("times.ttf");
        }
    } else if name.contains("arial") {
        if bold && italic {
            candidates.extend(["arialbi.ttf", "ariali.ttf", "arialbd.ttf"]);
        } else if bold {
            candidates.extend(["arialbd.ttf", "arial.ttf"]);
        } else if italic {
            candidates.extend(["ariali.ttf", "arial.ttf"]);
        } else {
            candidates.push("arial.ttf");
        }
    } else if name.contains("courier") || name.contains("cour") {
        if bold && italic {
            candidates.extend(["courbi.ttf", "couri.ttf", "courbd.ttf"]);
        } else if bold {
            candidates.extend(["courbd.ttf", "cour.ttf"]);
        } else if italic {
            candidates.extend(["couri.ttf", "cour.ttf"]);
        } else {
            candidates.push("cour.ttf");
        }
    }

    if request.fixed_width {
        candidates.extend(["consola.ttf", "cour.ttf"]);
    } else if request.serif {
        if bold {
            candidates.extend(["timesbd.ttf", "times.ttf"]);
        } else {
            candidates.push("times.ttf");
        }
    } else if bold {
        candidates.extend(["msyhbd.ttc", "simhei.ttf", "arialbd.ttf"]);
    } else if italic {
        candidates.extend(["ariali.ttf", "timesi.ttf"]);
    }
    candidates.extend(["msyh.ttc", "simsun.ttc", "simhei.ttf", "arial.ttf"]);

    for candidate in candidates {
        let path = root.join(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

#[derive(Default)]
struct DiagnosticsReport {
    total: BTreeMap<&'static str, usize>,
    total_count: usize,
    pages_with_diagnostics: usize,
    documents: Vec<DocumentDiagnostics>,
}

impl DiagnosticsReport {
    fn record(
        &mut self,
        document: &mut DocumentDiagnostics,
        page: usize,
        diagnostics: &[RenderDiagnostic],
    ) {
        if diagnostics.is_empty() {
            return;
        }
        self.pages_with_diagnostics += 1;
        let mut entries = BTreeMap::new();
        for diagnostic in diagnostics {
            let code = diagnostic.code.name();
            *self.total.entry(code).or_insert(0) += 1;
            *document.by_code.entry(code).or_insert(0) += 1;
            document.total += 1;
            self.total_count += 1;
            let entry = entries
                .entry((code.to_owned(), diagnostic.message.clone()))
                .or_insert_with(PageDiagnosticEntry::default);
            entry.count += 1;
            if let Some(id) = diagnostic.object_id.as_deref()
                && entry.ids.len() < 4
                && !entry.ids.iter().any(|existing| existing == id)
            {
                entry.ids.push(id.to_owned());
            }
        }
        document.pages.push(PageDiagnostics { page, entries });
    }

    fn render(
        &self,
        documents: usize,
        pages: usize,
        failed_documents: usize,
        failed_pages: usize,
    ) -> String {
        let mut text = String::new();
        let _ = writeln!(text, "OFD 渲染诊断报告");
        let _ = writeln!(text, "DPI: {DPI}");
        let _ = writeln!(
            text,
            "文档: {documents} 成功 / {failed_documents} 失败, 页面: {pages}, 构建失败页: {failed_pages}"
        );
        let _ = writeln!(
            text,
            "诊断总数: {}，有诊断页面: {}",
            self.total_count, self.pages_with_diagnostics
        );
        let _ = writeln!(text, "构建失败详情见 _skipped.txt");
        let _ = writeln!(text);
        let _ = writeln!(text, "## 汇总");
        if self.total.is_empty() {
            let _ = writeln!(text, "未发现诊断。");
        } else {
            for (code, count) in &self.total {
                let _ = writeln!(text, "{code}: {count}");
            }
        }
        let _ = writeln!(text);
        let _ = writeln!(text, "## 明细");
        for document in &self.documents {
            let summary = document
                .by_code
                .iter()
                .map(|(code, count)| format!("{code}={count}"))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                text,
                "{}: {} 条 ({summary})",
                document.label, document.total
            );
            for page in &document.pages {
                let _ = writeln!(text, "  page {}", page.page);
                for ((code, message), entry) in &page.entries {
                    let ids = if entry.ids.is_empty() {
                        String::new()
                    } else {
                        format!(" IDs={}", entry.ids.join(","))
                    };
                    let _ = writeln!(text, "    [{code}] x{} {message}{ids}", entry.count);
                }
            }
        }
        text
    }
}

struct DocumentDiagnostics {
    label: String,
    total: usize,
    by_code: BTreeMap<&'static str, usize>,
    pages: Vec<PageDiagnostics>,
}

impl DocumentDiagnostics {
    fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            total: 0,
            by_code: BTreeMap::new(),
            pages: Vec::new(),
        }
    }
}

struct PageDiagnostics {
    page: usize,
    entries: BTreeMap<(String, String), PageDiagnosticEntry>,
}

#[derive(Default)]
struct PageDiagnosticEntry {
    count: usize,
    ids: Vec<String>,
}
