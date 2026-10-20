//! 把 PDF 批量转回 OFD。
//!
//! 用法：
//! `cargo run -p ofdbox-render --release --example pdf_to_ofd -- <输入目录> <输出目录>`

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use ofdbox_core::OfdPackage;
use ofdbox_render::{
    Backend, BackendFrame, Element, Group, ImageElement, ImageFormat, Matrix, OfdBackend,
    PdfFrontend, PoolBudget, RasterTarget, RenderOptions, ResourcePool, Scene, Size,
    VelloCpuBackend,
};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(input) = args.next().map(PathBuf::from) else {
        eprintln!("用法: pdf_to_ofd <输入目录> <输出目录>");
        std::process::exit(2);
    };
    let Some(output) = args.next().map(PathBuf::from) else {
        eprintln!("用法: pdf_to_ofd <输入目录> <输出目录>");
        std::process::exit(2);
    };
    fs::create_dir_all(&output).expect("create output");
    let mut files = Vec::new();
    if input.is_file() {
        files.push(input.clone());
    } else {
        collect(&input, &mut files);
    }
    files.sort();
    let pool = ResourcePool::new(PoolBudget {
        font_bytes: 128 * 1024 * 1024,
        image_bytes: 256 * 1024 * 1024,
        scene_bytes: 32 * 1024 * 1024,
    });
    let mut renderer = VelloCpuBackend::new(RenderOptions {
        dpi: 96.0,
        ..Default::default()
    });
    let mut names = HashSet::new();
    let mut diagnostics = BTreeMap::<String, usize>::new();
    let mut documents = 0usize;
    let mut pages = 0usize;
    let mut failed = 0usize;
    for path in &files {
        let label = path
            .strip_prefix(&input)
            .unwrap_or(path)
            .display()
            .to_string();
        let mut frontend = match PdfFrontend::open(path) {
            Ok(frontend) => frontend,
            Err(error) => {
                eprintln!("{label}: {error}");
                failed += 1;
                continue;
            }
        };
        let mut scenes = Vec::new();
        for index in 0..frontend.page_count() {
            match frontend.build_page(index, &pool) {
                Ok(scene) => {
                    for diagnostic in &scene.diagnostics {
                        *diagnostics
                            .entry(diagnostic.code.name().to_owned())
                            .or_default() += 1;
                    }
                    match rasterize(&mut renderer, &scene, &pool) {
                        Ok(scene) => scenes.push(scene),
                        Err(error) => {
                            eprintln!("{label} [page {}]: 栅格化失败: {error}", index + 1);
                        }
                    }
                }
                Err(error) => {
                    eprintln!("{label} [page {}]: {error}", index + 1);
                }
            }
        }
        if scenes.is_empty() {
            failed += 1;
            continue;
        }
        let bytes = match OfdBackend::new().render_pages(&scenes) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("{label}: 生成 OFD 失败: {error}");
                failed += 1;
                continue;
            }
        };
        if let Err(error) = validate(&bytes) {
            eprintln!("{label}: OFD 回读校验失败: {error}");
            failed += 1;
            continue;
        }
        let name = unique_name(path, &mut names);
        let output_path = output.join(format!("{name}.ofd"));
        if let Err(error) = fs::write(&output_path, bytes) {
            eprintln!("{label}: 写入失败: {error}");
            failed += 1;
            continue;
        }
        documents += 1;
        pages += scenes.len();
        eprintln!("{name}: pages={}", scenes.len());
    }
    let report = diagnostics
        .iter()
        .map(|(code, count)| format!("{code}: {count}"))
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::write(output.join("_diagnostics.txt"), report);
    eprintln!("完成: docs={documents} pages={pages} failed={failed}");
}

fn rasterize(
    backend: &mut VelloCpuBackend,
    scene: &Scene,
    pool: &ResourcePool,
) -> Result<Scene, Box<dyn std::error::Error>> {
    let mut target = RasterTarget::from_size(scene.size, 96.0)?;
    let mut frame = backend.begin_frame((), &mut target)?;
    frame.draw(scene)?;
    frame.finish()?;
    let mut png_bytes = Vec::new();
    let mut encoder = png::Encoder::new(
        &mut png_bytes,
        u32::from(target.width()),
        u32::from(target.height()),
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(target.data())?;
    writer.finish()?;
    let image = pool.embedded_image(ImageFormat::Png, &png_bytes, None)?;
    let mut root = Group::default();
    root.children.push(Element::Image(ImageElement {
        image,
        transform: Matrix::scale(scene.size.width, scene.size.height),
        alpha: 1.0,
    }));
    Ok(Scene {
        size: Size::new(scene.size.width, scene.size.height),
        root,
        diagnostics: Vec::new(),
    })
}

fn validate(bytes: &[u8]) -> Result<(), String> {
    let mut package = OfdPackage::from_bytes(bytes.to_vec()).map_err(|error| error.to_string())?;
    package
        .parse_document()
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn collect(dir: &Path, output: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, output);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
        {
            output.push(path);
        }
    }
}

fn unique_name(path: &Path, used: &mut HashSet<String>) -> String {
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
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
    while !used.insert(candidate.to_lowercase()) {
        index += 1;
        candidate = format!("{base}_{index}");
    }
    candidate
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
