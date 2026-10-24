use std::path::{Path, PathBuf};

use ofdbox_core::{OfdError, OfdPackage, SignatureVerifiers};
use ofdbox_seal::SesSignatureVerifier;

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
fn verifies_real_ses_signatures() {
    let _ = dotenvy::dotenv();
    let Some(dir) = std::env::var_os("OFDBOX_TESTDATA_DIR").map(PathBuf::from) else {
        return;
    };
    let mut files = Vec::new();
    collect(&dir, &mut files);

    let verifiers = SignatureVerifiers::new().with(Box::new(SesSignatureVerifier::new()));
    let mut verified = 0usize;
    let mut skipped = 0usize;
    let mut failures = Vec::new();
    for path in files {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(mut package) = OfdPackage::from_bytes(bytes) else {
            continue;
        };
        let Ok(index) = package.parse_signatures() else {
            continue;
        };
        let locs: Vec<String> = index
            .signatures()
            .filter_map(|entry| entry.base_loc().map(str::to_owned))
            .collect();
        for loc in locs {
            let Ok(signature) = package.parse_signature(&loc) else {
                continue;
            };
            match package.verify_signature(&signature, &verifiers) {
                Ok(()) => verified += 1,
                Err(OfdError::UnsupportedSignature(_)) => skipped += 1,
                Err(error) => failures.push(format!("{}: {loc}: {error}", path.display())),
            }
        }
    }

    eprintln!(
        "SES 验签通过 {verified}，跳过 {skipped}，失败 {}",
        failures.len()
    );
    for failure in failures.iter().take(10) {
        eprintln!("  {failure}");
    }
    assert!(verified >= 30, "通过数量过少: {verified}");
    assert!(failures.is_empty(), "存在验签失败");
}
