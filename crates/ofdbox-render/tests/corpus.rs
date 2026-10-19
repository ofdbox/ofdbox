use std::fs;
use std::path::{Path, PathBuf};

use ofdbox_core::{OfdError, OfdPackage};
use ofdbox_render::{
    Backend, BackendFrame, OfdFrontend, PoolBudget, RasterTarget, RenderError, RenderOptions,
    ResourcePool, VelloCpuBackend,
};

#[test]
fn renders_real_packages_without_panicking() {
    let _ = dotenvy::dotenv();
    let Some(dir) = std::env::var_os("OFDBOX_TESTDATA_DIR").map(PathBuf::from) else {
        return;
    };
    let root = dir.join("ofd-files");
    if !root.is_dir() {
        return;
    }
    let mut packages = Vec::new();
    collect_packages(&root, &mut packages);
    packages.sort();
    packages.truncate(40);

    let pool = ResourcePool::new(PoolBudget::default());
    let mut pages = 0usize;
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut build_failed = 0usize;
    let mut ofd_missing = 0usize;
    let mut ofd_xml = 0usize;
    let mut ofd_other = 0usize;
    let mut unsupported = 0usize;
    let mut invalid_page = 0usize;
    let mut target_failed = 0usize;
    let mut draw_failed = 0usize;
    for path in packages {
        let Ok(mut package) = OfdPackage::open(&path) else {
            failed += 1;
            continue;
        };
        let Ok(document) = package.parse_document() else {
            failed += 1;
            continue;
        };
        let Some(pages_view) = document.pages() else {
            continue;
        };
        for page in pages_view.pages().take(1) {
            pages += 1;
            let options = RenderOptions {
                dpi: 16.0,
                ..Default::default()
            };
            let scene = match OfdFrontend::new(&mut package, &pool, options).build_page(&page) {
                Ok(scene) => scene,
                Err(error) => {
                    failed += 1;
                    build_failed += 1;
                    match error {
                        RenderError::Ofd(OfdError::MissingEntry(_)) => ofd_missing += 1,
                        RenderError::Ofd(OfdError::Xml(_)) => ofd_xml += 1,
                        RenderError::Ofd(_) => ofd_other += 1,
                        RenderError::Unsupported(_) => unsupported += 1,
                        RenderError::InvalidPage => invalid_page += 1,
                        _ => ofd_other += 1,
                    }
                    continue;
                }
            };
            let Ok(mut target) = RasterTarget::from_size(scene.size, 16.0) else {
                failed += 1;
                target_failed += 1;
                continue;
            };
            let mut backend = VelloCpuBackend::with_defaults();
            let Ok(mut frame) = backend.begin_frame((), &mut target) else {
                failed += 1;
                continue;
            };
            if frame.draw(&scene).is_ok() && frame.finish().is_ok() {
                succeeded += 1;
            } else {
                failed += 1;
                draw_failed += 1;
            }
        }
    }
    eprintln!(
        "渲染样本聚合: pages={pages} succeeded={succeeded} failed={failed} build={build_failed} target={target_failed} draw={draw_failed} missing={ofd_missing} xml={ofd_xml} ofd={ofd_other} unsupported={unsupported} page={invalid_page}"
    );
    assert!(pages > 0, "样本目录中没有可读取页面");
    assert!(succeeded > 0, "没有页面成功渲染");
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
