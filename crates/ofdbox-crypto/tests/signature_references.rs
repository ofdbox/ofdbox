use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ofdbox_core::{OfdError, OfdPackage};
use ofdbox_crypto::PasswordBackend;

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            collect(&path, files);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ofd"))
        {
            files.push(path);
        }
    }
}

#[test]
fn corpus_reference_digests_verify() {
    let _ = dotenvy::dotenv();
    let Some(dir) = std::env::var_os("OFDBOX_TESTDATA_DIR").map(PathBuf::from) else {
        return;
    };
    let mut files = Vec::new();
    collect(&dir, &mut files);

    // 摘要校验不需要口令，这里只借用后端的 digest 实现。
    let backend = PasswordBackend::new("");
    let mut packages = 0usize;
    let mut encrypted_skipped = 0usize;
    let mut signatures = 0usize;
    let mut references = 0usize;
    let mut matched = 0usize;
    let mut errors = Vec::new();
    let mut unsupported = Vec::new();
    let mut by_method: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut raw_methods: BTreeMap<String, usize> = BTreeMap::new();

    for path in files {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(mut package) = OfdPackage::from_bytes(bytes) else {
            continue;
        };
        if package.contains("Encryptions.xml") {
            encrypted_skipped += 1;
            continue;
        }
        let Ok(index) = package.parse_signatures() else {
            continue;
        };
        let locs: Vec<String> = index
            .signatures()
            .filter_map(|entry| entry.base_loc().map(str::to_owned))
            .collect();
        let mut package_signed = false;
        for loc in locs {
            let Ok(signature) = package.parse_signature(&loc) else {
                errors.push(format!("{}: 无法解析签名描述 {loc}", path.display()));
                continue;
            };
            package_signed = true;
            signatures += 1;
            if let Some(references) = signature.signed_info().and_then(|info| info.references()) {
                let raw = references.check_method().unwrap_or("<缺失>").to_owned();
                *raw_methods.entry(raw).or_default() += 1;
            }
            match package.verify_references(&signature, &backend) {
                Ok(checks) => {
                    references += checks.len();
                    for check in &checks {
                        let entry = by_method
                            .entry(check.check_method.clone())
                            .or_insert((0, 0));
                        entry.1 += 1;
                        if check.expected == check.actual {
                            matched += 1;
                            entry.0 += 1;
                        }
                    }
                    for check in checks.iter().filter(|check| check.expected != check.actual) {
                        errors.push(format!(
                            "{}: {} 摘要不一致（{}）",
                            path.display(),
                            check.file_ref,
                            check.check_method
                        ));
                    }
                }
                Err(OfdError::UnsupportedDigest(method)) => {
                    unsupported.push(format!("{}: {method}", path.display()));
                }
                Err(error) => errors.push(format!("{}: 摘要校验失败 {error}", path.display())),
            }
        }
        if package_signed {
            packages += 1;
        }
    }

    eprintln!(
        "签名包 {packages}，签名 {signatures}，引用 {references}，摘要一致 {matched}，\
         跳过加密包 {encrypted_skipped}，不一致 {}，不支持 {}",
        errors.len(),
        unsupported.len()
    );
    for (method, (ok, total)) in &by_method {
        eprintln!("  {method}: {ok}/{total}");
    }
    for (method, count) in &raw_methods {
        eprintln!("  原始写法 {method}: {count}");
    }
    for error in errors.iter().take(10) {
        eprintln!("  {error}");
    }
    for error in unsupported.iter().take(10) {
        eprintln!("  不支持: {error}");
    }

    assert!(unsupported.is_empty(), "存在无法识别的摘要方法");
    assert!(
        raw_methods
            .keys()
            .any(|method| method.starts_with("1.2.156.10197")),
        "语料里应出现 SM3 的 OID 写法，否则本用例失去意义: {raw_methods:?}"
    );
    assert!(references > 500, "引用数量过少: {references}");
    assert!(
        matched * 100 >= references * 95,
        "摘要一致率过低: {matched}/{references}"
    );
}
