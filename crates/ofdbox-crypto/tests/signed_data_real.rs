use std::path::{Path, PathBuf};

use ofdbox_core::{OfdPackage, SignatureVerifiers};
use ofdbox_crypto::Sm2SignedDataVerifier;

fn corpus_dir() -> Option<PathBuf> {
    let _ = dotenvy::dotenv();
    std::env::var_os("OFDBOX_TESTDATA_DIR").map(PathBuf::from)
}

fn verify_sample(path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let mut package = OfdPackage::from_bytes(bytes).map_err(|error| error.to_string())?;
    let index = package
        .parse_signatures()
        .map_err(|error| error.to_string())?;
    let entry = index
        .signatures()
        .next()
        .ok_or_else(|| "签名列表为空".to_owned())?;
    let base_loc = entry
        .base_loc()
        .ok_or_else(|| "缺少 BaseLoc".to_owned())?
        .to_owned();
    let signature = package
        .parse_signature(&base_loc)
        .map_err(|error| error.to_string())?;
    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    package
        .verify_signature(&signature, &verifiers)
        .map_err(|error| error.to_string())
}

#[test]
fn verifies_real_sm2_signed_data_samples() {
    let Some(dir) = corpus_dir() else {
        return;
    };
    let base = dir.join("ofd-files").join("铁路电子客票样例");
    if !base.is_dir() {
        return;
    }

    let samples = [
        "售票样例.ofd",
        "售票换开样例.ofd",
        "退票样例.ofd",
        "退票换开样例.ofd",
    ];
    let mut failures = Vec::new();
    for name in samples {
        if let Err(error) = verify_sample(&base.join(name)) {
            failures.push(format!("{name}: {error}"));
        }
    }
    assert!(
        failures.is_empty(),
        "国密 SignedData 验签失败:\n{}",
        failures.join("\n")
    );
}
