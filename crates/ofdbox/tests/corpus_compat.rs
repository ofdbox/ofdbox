//! 外部 OFD 语料的兼容性批量测试。
//!
//! 默认 `cargo test` 不运行（`#[ignore]`），必须显式指定语料目录：
//!
//! ```text
//! $env:OFDBOX_TESTDATA_DIR = "D:\OFDBox-Workspace\OFD文件"
//! cargo test -p ofdbox --test corpus_compat -- --ignored --nocapture
//! ```
//!
//! 这里只做“语料兼容性”回归：不绑定任何具体文档，遍历目录下全部 `.ofd`，
//! 汇总可读、加密、无效与签名/引用失败的数量。

use std::path::{Path, PathBuf};

use ofdbox::{
    GmT0031SignatureVerifier, OfdError, OfdPackage, PasswordBackend, SesSignatureVerifier,
    SignatureVerifiers, SignedValueFormat, Sm2SignedDataVerifier, parse_ses_signature,
};

#[derive(Default)]
struct Stats {
    files: usize,
    invalid: usize,
    encrypted: usize,
    readable: usize,
    structure_failures: Vec<String>,
    repack_failures: Vec<String>,
    signature_ok: usize,
    signature_unsupported: usize,
    signature_failures: Vec<String>,
    reference_ok: usize,
    reference_failures: Vec<String>,
    gmt0031_ok: usize,
    gmt0031_failures: Vec<String>,
    timestamp_present: usize,
}

#[test]
#[ignore = "需要 OFDBOX_TESTDATA_DIR 指向外部 OFD 语料"]
fn corpus_compat() {
    let root = corpus_root();
    let mut files = Vec::new();
    collect(&root, &mut files);
    assert!(
        !files.is_empty(),
        "语料目录里没有 .ofd 文件: {}",
        root.display()
    );

    let mut stats = Stats {
        files: files.len(),
        ..Default::default()
    };
    for path in &files {
        check_package(path, &mut stats);
    }

    eprintln!(
        "语料 {} 份：可读 {}，加密 {}，无效 {}；签名通过 {}（0031 {}，带时间戳 {}）、未支持 {}、失败 {}；引用通过 {}、失败 {}",
        stats.files,
        stats.readable,
        stats.encrypted,
        stats.invalid,
        stats.signature_ok,
        stats.gmt0031_ok,
        stats.timestamp_present,
        stats.signature_unsupported,
        stats.signature_failures.len(),
        stats.reference_ok,
        stats.reference_failures.len()
    );
    for failure in stats
        .structure_failures
        .iter()
        .chain(stats.repack_failures.iter())
        .chain(stats.signature_failures.iter())
        .chain(stats.gmt0031_failures.iter())
        .chain(stats.reference_failures.iter())
    {
        eprintln!("  {failure}");
    }

    assert!(stats.structure_failures.is_empty(), "结构失败");
    assert!(stats.repack_failures.is_empty(), "重打包失败");
    // 签名/引用失败属于厂商兼容性差异（例如对 Signature.xml 做规范化后再签名），
    // 这里只要求已知格式的整体通过量不退化；具体失败逐条打印，便于跟进。
    assert!(stats.signature_ok >= 40, "已知格式签名通过量过低");
    assert!(stats.reference_failures.is_empty(), "引用摘要失败");
}

fn corpus_root() -> PathBuf {
    let path = std::env::var_os("OFDBOX_TESTDATA_DIR")
        .map(PathBuf::from)
        .expect("跑 corpus_compat 必须设置 OFDBOX_TESTDATA_DIR");
    assert!(path.is_dir(), "语料目录不存在: {}", path.display());
    path
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ofd"))
        {
            files.push(path);
        }
    }
}

fn check_package(path: &Path, stats: &mut Stats) {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            stats
                .structure_failures
                .push(format!("{}: 读取失败: {error}", path.display()));
            return;
        }
    };
    let mut package = match OfdPackage::from_bytes(bytes) {
        Ok(package) => package,
        Err(_) => {
            // 语料里有空 ZIP、非 ZIP、PDF 改名的文件，计入“无效”而不是失败。
            stats.invalid += 1;
            return;
        }
    };
    match package.parse_root() {
        Ok(_) => {}
        Err(OfdError::EncryptionRequired) => {
            stats.encrypted += 1;
            return;
        }
        // 空 ZIP、只有目录的包等：不是可解析的 OFD，计入“无效”。
        Err(OfdError::MissingEntry(name)) if name.eq_ignore_ascii_case("OFD.xml") => {
            stats.invalid += 1;
            return;
        }
        Err(error) => {
            stats
                .structure_failures
                .push(format!("{}: {error}", path.display()));
            return;
        }
    }
    if let Err(error) = package.parse_document() {
        stats
            .structure_failures
            .push(format!("{}: {error}", path.display()));
        return;
    }
    stats.readable += 1;

    match package.to_bytes() {
        Ok(bytes) => match OfdPackage::from_bytes(bytes) {
            Ok(mut copy) => {
                if let Err(error) = copy.parse_root() {
                    stats
                        .repack_failures
                        .push(format!("{}: 重打包后无法解析: {error}", path.display()));
                }
            }
            Err(error) => stats
                .repack_failures
                .push(format!("{}: 重打包后无法打开: {error}", path.display())),
        },
        Err(error) => stats
            .repack_failures
            .push(format!("{}: 重打包失败: {error}", path.display())),
    }

    let Ok(index) = package.parse_signatures() else {
        return;
    };
    let verifiers = SignatureVerifiers::new()
        .with(Box::new(Sm2SignedDataVerifier::new()))
        .with(Box::new(SesSignatureVerifier::new()))
        .with(Box::new(GmT0031SignatureVerifier::new()));
    for entry in index.signatures() {
        let Some(base_loc) = entry.base_loc() else {
            continue;
        };
        let signature = match package.parse_signature(base_loc) {
            Ok(signature) => signature,
            Err(error) => {
                stats
                    .signature_failures
                    .push(format!("{}: {base_loc}: {error}", path.display()));
                continue;
            }
        };
        let signed_value_bytes = signature
            .signed_value()
            .and_then(|loc| package.read_entry(loc).ok());
        let format = signed_value_bytes
            .as_deref()
            .map(ofdbox::core::detect_signed_value_format);
        if format == Some(SignedValueFormat::SesSignature)
            && let Some(bytes) = signed_value_bytes.as_deref()
            && let Ok(ses) = parse_ses_signature(bytes)
            && ses.time_stamp.is_some()
        {
            stats.timestamp_present += 1;
        }
        match package.verify_signature(&signature, &verifiers) {
            Ok(()) => {
                stats.signature_ok += 1;
                if format == Some(SignedValueFormat::GmT0031Signature) {
                    stats.gmt0031_ok += 1;
                }
            }
            Err(OfdError::UnsupportedSignature(_)) => stats.signature_unsupported += 1,
            Err(error) => {
                if format == Some(SignedValueFormat::GmT0031Signature) {
                    stats
                        .gmt0031_failures
                        .push(format!("{}: {base_loc}: {error}", path.display()));
                } else {
                    stats
                        .signature_failures
                        .push(format!("{}: {base_loc}: {error}", path.display()));
                }
            }
        }
        match package.verify_references(&signature, &PasswordBackend::new("")) {
            Ok(_) => stats.reference_ok += 1,
            Err(OfdError::UnsupportedDigest(_)) => {}
            Err(error) => stats
                .reference_failures
                .push(format!("{}: {base_loc}: {error}", path.display())),
        }
    }
}
