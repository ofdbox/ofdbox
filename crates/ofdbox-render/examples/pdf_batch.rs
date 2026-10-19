//! 递归目录下的 OFD 文档转换为 PDF。
//!
//! 用法：
//! `cargo run -p ofdbox-render --release --example pdf_batch -- <输入目录> <输出目录>`
//!
//! 每个文档输出一个 `<名称>.pdf`，页面按原顺序写入，并生成页级书签。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ofdbox_core::OfdPackage;
use ofdbox_render::{
    FontProvider, FontRequest, OfdFrontend, PdfDocument, PdfMetadata, PdfOptions, PoolBudget,
    RenderError, RenderOptions, ResourcePool,
};

type FontCache = Arc<Mutex<HashMap<PathBuf, Option<Arc<[u8]>>>>>;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(input) = args.next().map(PathBuf::from) else {
        eprintln!("用法: pdf_batch <输入目录> <输出目录>");
        std::process::exit(2);
    };
    let Some(output) = args.next().map(PathBuf::from) else {
        eprintln!("用法: pdf_batch <输入目录> <输出目录>");
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
        "发现 {} 个 OFD，输出到 {}",
        packages.len(),
        output.display()
    );

    let pool = ResourcePool::new(PoolBudget {
        font_bytes: 128 * 1024 * 1024,
        image_bytes: 256 * 1024 * 1024,
        scene_bytes: 32 * 1024 * 1024,
    });
    let fonts: FontCache = Arc::new(Mutex::new(HashMap::new()));
    let mut folders = FolderNames::default();
    let mut diagnostics = BTreeMap::<&'static str, usize>::new();
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
        let output_path = output.join(format!("{folder_name}.pdf"));
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
        let provider = SystemFonts {
            cache: Arc::clone(&fonts),
        };
        let mut frontend = OfdFrontend::new(
            &mut package,
            &pool,
            RenderOptions {
                dpi: 96.0,
                ..Default::default()
            },
        )
        .with_font_provider(Box::new(provider));
        let mut pdf = PdfDocument::new(PdfOptions::default());
        pdf.set_metadata(PdfMetadata {
            title: Some(label.clone()),
            creator: Some("OFDBox".to_owned()),
            producer: Some("OFDBox ofdbox-render".to_owned()),
            ..PdfMetadata::default()
        });
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
            for diagnostic in &scene.diagnostics {
                *diagnostics.entry(diagnostic.code.name()).or_insert(0) += 1;
            }
            if let Err(error) = pdf.add_page(&scene) {
                skipped.push(format!("{label} [page {}]: {error}", index + 1));
                failed_pages += 1;
                document_failed += 1;
                continue;
            }
            pdf.add_outline(format!("第 {} 页", index + 1), document_pages);
            document_pages += 1;
        }
        if document_pages == 0 {
            failed_documents += 1;
            continue;
        }
        match pdf.finish() {
            Ok(bytes) => match fs::write(&output_path, bytes) {
                Ok(()) => {
                    documents += 1;
                    eprintln!("{folder_name}: ok={document_pages} failed={document_failed}");
                }
                Err(error) => {
                    skipped.push(format!("{label}: 写入失败: {error}"));
                    failed_documents += 1;
                }
            },
            Err(error) => {
                skipped.push(format!("{label}: 生成 PDF 失败: {error}"));
                failed_documents += 1;
            }
        }
    }

    if !skipped.is_empty() {
        let _ = fs::write(
            output.join("_skipped.txt"),
            format!("{}\n", skipped.join("\n")),
        );
    }
    let mut report = String::new();
    for (code, count) in &diagnostics {
        report.push_str(&format!("{code}: {count}\n"));
    }
    let _ = fs::write(output.join("_diagnostics.txt"), report);
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
    } else if name.contains("tahoma") {
        if bold {
            candidates.extend(["tahomabd.ttf", "tahoma.ttf"]);
        } else {
            candidates.push("tahoma.ttf");
        }
    } else if name.contains("latha") {
        candidates.push("latha.ttf");
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
