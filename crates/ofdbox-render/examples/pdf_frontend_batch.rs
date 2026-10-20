//! 把 PDF 目录批量转为 PNG，用于检查 PDF 前端。
//!
//! 用法：
//! `cargo run -p ofdbox-render --release --example pdf_frontend_batch -- <输入目录> <输出目录>`

use std::fs;
use std::path::{Path, PathBuf};

use ofdbox_render::{
    Backend, BackendFrame, PdfFrontend, PoolBudget, RasterTarget, RenderOptions, ResourcePool,
    Scene, VelloCpuBackend,
};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(input) = args.next().map(PathBuf::from) else {
        eprintln!("用法: pdf_frontend_batch <输入目录> <输出目录>");
        std::process::exit(2);
    };
    let Some(output) = args.next().map(PathBuf::from) else {
        eprintln!("用法: pdf_frontend_batch <输入目录> <输出目录>");
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
    let pool = ResourcePool::new(PoolBudget::default());
    let mut backend = VelloCpuBackend::new(RenderOptions {
        dpi: 96.0,
        ..Default::default()
    });
    let mut pages = 0usize;
    let mut failed = 0usize;
    let mut diagnostics = std::collections::BTreeMap::new();
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
        let folder = output.join(safe_name(path));
        if fs::create_dir_all(&folder).is_err() {
            failed += 1;
            continue;
        }
        for index in 0..frontend.page_count() {
            let scene = match frontend.build_page(index, &pool) {
                Ok(scene) => scene,
                Err(error) => {
                    eprintln!("{label} [page {}]: {error}", index + 1);
                    failed += 1;
                    continue;
                }
            };
            for diagnostic in &scene.diagnostics {
                *diagnostics.entry(diagnostic.code.name()).or_insert(0usize) += 1;
            }
            if let Err(error) = render(&mut backend, &scene, &folder, index + 1) {
                eprintln!("{label} [page {}]: {error}", index + 1);
                failed += 1;
            } else {
                pages += 1;
            }
        }
    }
    let report = diagnostics
        .iter()
        .map(|(code, count)| format!("{code}: {count}"))
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::write(output.join("_diagnostics.txt"), report);
    eprintln!("完成: files={} pages={pages} failed={failed}", files.len());
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

fn safe_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("document")
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn render(
    backend: &mut VelloCpuBackend,
    scene: &Scene,
    folder: &Path,
    page: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut target = RasterTarget::from_size(scene.size, 96.0)?;
    let mut frame = backend.begin_frame((), &mut target)?;
    frame.draw(scene)?;
    frame.finish()?;
    let file = fs::File::create(folder.join(format!("page_{page:04}.png")))?;
    let mut encoder =
        png::Encoder::new(file, u32::from(target.width()), u32::from(target.height()));
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(target.data())?;
    Ok(())
}
