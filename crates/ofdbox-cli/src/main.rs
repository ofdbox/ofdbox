mod font;

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use font::SystemFonts;
use ofdbox_core::OfdPackage;
use ofdbox_render::{
    Backend, BackendFrame, ImageFormat, OfdBackend, OfdFrontend, PdfDocument, PdfFrontend,
    PdfMetadata, PdfOptions, PoolBudget, RasterTarget, RenderOptions, ResourcePool, Scene, Size,
    VelloCpuBackend,
};

#[derive(Parser)]
#[command(name = "ofdbox", version, about = "OFD / PDF / PNG 批量转换工具")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 批量转换文件或目录。
    Convert(ConvertArgs),
}

#[derive(Clone, Copy, ValueEnum)]
enum InputFormat {
    Ofd,
    Pdf,
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Ofd,
    Pdf,
    Png,
}

#[derive(Parser)]
struct ConvertArgs {
    /// 输入文件或目录，可重复。
    #[arg(required = true)]
    inputs: Vec<PathBuf>,

    /// 目标格式。
    #[arg(short = 't', long, value_enum)]
    to: OutputFormat,

    /// 输入格式；不填时按扩展名识别。
    #[arg(short = 'f', long, value_enum)]
    from: Option<InputFormat>,

    /// 输出目录。
    #[arg(short = 'o', long, default_value = "converted")]
    output: PathBuf,

    /// 页码，例如 1-3,5；默认全部。
    #[arg(long, default_value = "1-")]
    pages: String,

    /// PNG 渲染 DPI。
    #[arg(long, default_value_t = 144.0)]
    dpi: f64,

    /// PDF 转 OFD 时保留矢量路径（实验性）。
    #[arg(long)]
    vector: bool,

    /// 已存在文件直接覆盖；默认自动改名。
    #[arg(long)]
    force: bool,

    /// 不递归子目录。
    #[arg(long)]
    no_recursive: bool,

    /// 只输出错误和汇总。
    #[arg(long)]
    quiet: bool,

    /// 将汇总和失败明细写入文件。
    #[arg(long)]
    report: Option<PathBuf>,
}

struct Stats {
    docs: usize,
    pages: usize,
    failed: usize,
    skipped: Vec<String>,
    diagnostics: BTreeMap<&'static str, usize>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("错误: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let Cli {
        command: Command::Convert(args),
    } = Cli::parse();
    if !(args.dpi.is_finite() && args.dpi > 0.0) {
        anyhow::bail!("--dpi 必须是正数");
    }
    let pages = PageSpec::parse(&args.pages)?;
    fs::create_dir_all(&args.output)
        .with_context(|| format!("无法创建输出目录 {}", args.output.display()))?;
    let mut files = Vec::new();
    for input in &args.inputs {
        if input.is_file() {
            files.push((input.clone(), PathBuf::new()));
        } else if input.is_dir() {
            collect_files(input, input, args.from, args.no_recursive, &mut files);
        } else {
            anyhow::bail!("输入不存在: {}", input.display());
        }
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        anyhow::bail!("没有找到可转换的输入文件");
    }

    let pool = ResourcePool::new(PoolBudget::default());
    let mut stats = Stats {
        docs: 0,
        pages: 0,
        failed: 0,
        skipped: Vec::new(),
        diagnostics: BTreeMap::new(),
    };
    let mut used = HashSet::new();
    let started = std::time::Instant::now();
    for (file, relative) in &files {
        let file_started = std::time::Instant::now();
        let format = match args.from {
            Some(format) => format,
            None => match extension(file).as_str() {
                "ofd" => InputFormat::Ofd,
                "pdf" => InputFormat::Pdf,
                _ => {
                    stats
                        .skipped
                        .push(format!("{}: 无法识别格式", file.display()));
                    continue;
                }
            },
        };
        let stem = unique_stem(file, &mut used);
        let label = file.display().to_string();
        let result = convert(
            file,
            format,
            args.to,
            &args.output,
            relative,
            &stem,
            &pages,
            args.dpi,
            args.vector,
            args.force,
            &pool,
            &mut stats,
        );
        match result {
            Ok(page_count) => {
                stats.docs += 1;
                stats.pages += page_count;
                if !args.quiet {
                    println!(
                        "{label}: {page_count} 页 -> {}/{} ({:.2}s)",
                        relative.display(),
                        stem,
                        file_started.elapsed().as_secs_f64()
                    );
                }
            }
            Err(error) => {
                stats.failed += 1;
                stats.skipped.push(format!("{label}: {error:#}"));
                if !args.quiet {
                    eprintln!("{label}: {error:#}");
                }
            }
        }
    }
    let elapsed = started.elapsed().as_secs_f64();
    let summary = format!(
        "完成: 文档={} 页={} 失败={} 跳过={} 耗时={elapsed:.2}s 文档/s={:.2} 页/s={:.2}",
        stats.docs,
        stats.pages,
        stats.failed,
        stats.skipped.len(),
        stats.docs as f64 / elapsed.max(0.001),
        stats.pages as f64 / elapsed.max(0.001)
    );
    eprintln!("{summary}");
    if !stats.diagnostics.is_empty() {
        eprintln!(
            "诊断: {}",
            stats
                .diagnostics
                .iter()
                .map(|(code, count)| format!("{code}={count}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    if let Some(report) = args.report {
        let mut text = format!(
            "{summary}
"
        );
        for (code, count) in &stats.diagnostics {
            text.push_str(&format!(
                "{code}: {count}
"
            ));
        }
        for item in &stats.skipped {
            text.push_str(&format!(
                "FAILED {item}
"
            ));
        }
        fs::write(&report, text).with_context(|| format!("无法写报告 {}", report.display()))?;
    }
    if stats.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn convert(
    file: &Path,
    from: InputFormat,
    to: OutputFormat,
    output: &Path,
    relative: &Path,
    stem: &str,
    pages: &PageSpec,
    dpi: f64,
    vector: bool,
    force: bool,
    pool: &ResourcePool,
    stats: &mut Stats,
) -> Result<usize> {
    match (from, to) {
        (InputFormat::Ofd, OutputFormat::Pdf) => {
            convert_ofd_pdf(file, output, relative, stem, pages, force, pool, stats)
        }
        (InputFormat::Ofd, OutputFormat::Png) => {
            convert_ofd_png(file, output, relative, stem, pages, dpi, force, pool, stats)
        }
        (InputFormat::Pdf, OutputFormat::Png) => {
            convert_pdf_png(file, output, relative, stem, pages, dpi, force, pool, stats)
        }
        (InputFormat::Pdf, OutputFormat::Ofd) => convert_pdf_ofd(
            file, output, relative, stem, pages, vector, force, pool, stats,
        ),
        (InputFormat::Ofd, OutputFormat::Ofd) | (InputFormat::Pdf, OutputFormat::Pdf) => {
            anyhow::bail!("输入和输出格式相同")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn convert_pdf_png(
    file: &Path,
    output: &Path,
    relative: &Path,
    stem: &str,
    pages: &PageSpec,
    dpi: f64,
    force: bool,
    pool: &ResourcePool,
    stats: &mut Stats,
) -> Result<usize> {
    let mut frontend = PdfFrontend::open(file)?;
    let selected = pages.select(frontend.page_count());
    if selected.is_empty() {
        anyhow::bail!("页码超出范围");
    }
    let mut renderer = VelloCpuBackend::new(RenderOptions {
        dpi,
        ..Default::default()
    });
    for index in &selected {
        let scene = frontend.build_page(*index, pool)?;
        record_diagnostics(stats, &scene);
        let target = png_path(output, relative, stem, index + 1, selected.len() > 1, force);
        render_png(&mut renderer, &scene, &target, dpi)?;
    }
    Ok(selected.len())
}

#[allow(clippy::too_many_arguments)]
fn convert_pdf_ofd(
    file: &Path,
    output: &Path,
    relative: &Path,
    stem: &str,
    pages: &PageSpec,
    vector: bool,
    force: bool,
    pool: &ResourcePool,
    stats: &mut Stats,
) -> Result<usize> {
    let mut frontend = PdfFrontend::open(file)?;
    let selected = pages.select(frontend.page_count());
    if selected.is_empty() {
        anyhow::bail!("页码超出范围");
    }
    let mut scenes = Vec::new();
    let mut renderer = VelloCpuBackend::new(RenderOptions {
        dpi: 144.0,
        ..Default::default()
    });
    for index in &selected {
        let scene = frontend.build_page(*index, pool)?;
        record_diagnostics(stats, &scene);
        if vector {
            scenes.push(scene);
        } else {
            scenes.push(rasterize(&mut renderer, &scene, pool)?);
        }
    }
    let bytes = OfdBackend::new().render_pages(&scenes)?;
    let path = target_path(output, relative, stem, "ofd", force);
    fs::write(&path, bytes).with_context(|| format!("无法写 {}", path.display()))?;
    Ok(scenes.len())
}

#[allow(clippy::too_many_arguments)]
fn convert_ofd_png(
    file: &Path,
    output: &Path,
    relative: &Path,
    stem: &str,
    pages: &PageSpec,
    dpi: f64,
    force: bool,
    pool: &ResourcePool,
    stats: &mut Stats,
) -> Result<usize> {
    let mut package = OfdPackage::open(file)?;
    let document = package.parse_document()?;
    let Some(pages_view) = document.pages() else {
        anyhow::bail!("OFD 没有页面");
    };
    let total = pages_view.pages().count();
    let selected = pages.select(total);
    if selected.is_empty() {
        anyhow::bail!("页码超出范围");
    }
    let mut frontend = OfdFrontend::new(
        &mut package,
        pool,
        RenderOptions {
            dpi,
            ..Default::default()
        },
    )
    .with_font_provider(Box::new(SystemFonts::new()));
    let mut renderer = VelloCpuBackend::new(RenderOptions {
        dpi,
        ..Default::default()
    });
    for index in &selected {
        let Some(page) = pages_view.pages().nth(*index) else {
            continue;
        };
        let scene = frontend.build_page(&page)?;
        record_diagnostics(stats, &scene);
        let target = png_path(output, relative, stem, index + 1, selected.len() > 1, force);
        render_png(&mut renderer, &scene, &target, dpi)?;
    }
    Ok(selected.len())
}

#[allow(clippy::too_many_arguments)]
fn convert_ofd_pdf(
    file: &Path,
    output: &Path,
    relative: &Path,
    stem: &str,
    pages: &PageSpec,
    force: bool,
    pool: &ResourcePool,
    stats: &mut Stats,
) -> Result<usize> {
    let mut package = OfdPackage::open(file)?;
    let document = package.parse_document()?;
    let Some(pages_view) = document.pages() else {
        anyhow::bail!("OFD 没有页面");
    };
    let total = pages_view.pages().count();
    let selected = pages.select(total);
    if selected.is_empty() {
        anyhow::bail!("页码超出范围");
    }
    let mut frontend = OfdFrontend::new(&mut package, pool, RenderOptions::default())
        .with_font_provider(Box::new(SystemFonts::new()));
    let mut pdf = PdfDocument::new(PdfOptions::default());
    pdf.set_metadata(PdfMetadata {
        title: Some(stem.to_owned()),
        creator: Some("OFDBox".to_owned()),
        ..PdfMetadata::default()
    });
    let mut output_pages = 0usize;
    for index in &selected {
        let Some(page) = pages_view.pages().nth(*index) else {
            continue;
        };
        let scene = frontend.build_page(&page)?;
        record_diagnostics(stats, &scene);
        pdf.add_page(&scene)?;
        pdf.add_outline(format!("第 {} 页", index + 1), output_pages);
        output_pages += 1;
    }
    if output_pages == 0 {
        anyhow::bail!("没有可输出的页面");
    }
    let bytes = pdf.finish()?;
    let path = target_path(output, relative, stem, "pdf", force);
    fs::write(&path, bytes).with_context(|| format!("无法写 {}", path.display()))?;
    Ok(output_pages)
}

fn rasterize(renderer: &mut VelloCpuBackend, scene: &Scene, pool: &ResourcePool) -> Result<Scene> {
    let mut target = RasterTarget::from_size(scene.size, 144.0)?;
    let mut frame = renderer.begin_frame((), &mut target)?;
    frame.draw(scene)?;
    frame.finish()?;
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(
            &mut png,
            u32::from(target.width()),
            u32::from(target.height()),
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(target.data())?;
    }
    let image = pool.embedded_image(ImageFormat::Png, &png, None)?;
    let mut root = ofdbox_render::Group::default();
    root.children
        .push(ofdbox_render::Element::Image(ofdbox_render::ImageElement {
            image,
            transform: ofdbox_render::Matrix::scale(
                scene.size.width / f64::from(target.width()),
                scene.size.height / f64::from(target.height()),
            ),
            alpha: 1.0,
        }));
    Ok(Scene {
        size: Size::new(scene.size.width, scene.size.height),
        root,
        diagnostics: Vec::new(),
    })
}

fn render_png(renderer: &mut VelloCpuBackend, scene: &Scene, path: &Path, dpi: f64) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut target = RasterTarget::from_size(scene.size, dpi)?;
    let mut frame = renderer.begin_frame((), &mut target)?;
    frame.draw(scene)?;
    frame.finish()?;
    let file = fs::File::create(path)?;
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        u32::from(target.width()),
        u32::from(target.height()),
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(target.data())?;
    Ok(())
}

fn record_diagnostics(stats: &mut Stats, scene: &Scene) {
    for diagnostic in &scene.diagnostics {
        *stats.diagnostics.entry(diagnostic.code.name()).or_insert(0) += 1;
    }
}

fn png_path(
    output: &Path,
    relative: &Path,
    stem: &str,
    page: usize,
    multi: bool,
    force: bool,
) -> PathBuf {
    let name = if multi {
        format!("page_{page:04}.png")
    } else {
        "page.png".to_owned()
    };
    let dir = output.join(relative).join(stem);
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(name);
    if force || !path.exists() {
        path
    } else {
        dir.join(format!("page_{page:04}_{}.png", suffix()))
    }
}

fn target_path(
    output: &Path,
    relative: &Path,
    stem: &str,
    extension: &str,
    force: bool,
) -> PathBuf {
    let path = output.join(relative).join(format!("{stem}.{extension}"));
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if force || !path.exists() {
        return path;
    }
    output.join(format!("{stem}_{}.{extension}", suffix()))
}

fn suffix() -> String {
    format!("{}", std::process::id() % 10000)
}

fn unique_stem(path: &Path, used: &mut HashSet<String>) -> String {
    let base = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("document");
    let base = base
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
        .to_owned();
    let mut candidate = if base.is_empty() {
        "document".to_owned()
    } else {
        base
    };
    let mut index = 1;
    while !used.insert(candidate.to_lowercase()) {
        index += 1;
        candidate = format!("{}_{index}", candidate);
    }
    candidate
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn collect_files(
    root: &Path,
    dir: &Path,
    from: Option<InputFormat>,
    no_recursive: bool,
    output: &mut Vec<(PathBuf, PathBuf)>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if !no_recursive {
                collect_files(root, &path, from, no_recursive, output);
            }
            continue;
        }
        let matches = match from {
            Some(InputFormat::Ofd) => extension(&path) == "ofd",
            Some(InputFormat::Pdf) => extension(&path) == "pdf",
            None => matches!(extension(&path).as_str(), "ofd" | "pdf"),
        };
        if matches {
            let relative = path
                .strip_prefix(root)
                .ok()
                .and_then(|value| value.parent())
                .map(Path::to_path_buf)
                .unwrap_or_default();
            output.push((path, relative));
        }
    }
}

struct PageSpec {
    ranges: Vec<(usize, Option<usize>)>,
}

impl PageSpec {
    fn parse(value: &str) -> Result<Self> {
        let mut ranges = Vec::new();
        for part in value.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (start, end) = match part.split_once('-') {
                Some((start, end)) => {
                    let start = start.trim().parse::<usize>()?;
                    let end = if end.trim().is_empty() {
                        None
                    } else {
                        Some(end.trim().parse::<usize>()?)
                    };
                    (start, end)
                }
                None => (part.parse::<usize>()?, Some(part.parse::<usize>()?)),
            };
            if start == 0 || end.is_some_and(|end| end < start) {
                anyhow::bail!("页码无效: {part}");
            }
            ranges.push((start, end));
        }
        if ranges.is_empty() {
            anyhow::bail!("页码不能为空");
        }
        Ok(Self { ranges })
    }

    fn select(&self, total: usize) -> Vec<usize> {
        let mut selected = Vec::new();
        for index in 1..=total {
            if self
                .ranges
                .iter()
                .any(|(start, end)| index >= *start && end.is_none_or(|end| index <= end))
            {
                selected.push(index - 1);
            }
        }
        selected
    }
}
