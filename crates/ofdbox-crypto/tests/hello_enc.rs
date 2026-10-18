use std::path::PathBuf;

use base64::Engine;
use ofdbox_crypto::{decrypt_cbc, unwrap_file_key};

#[test]
fn decrypts_hello_enc_sample() {
    let _ = dotenvy::dotenv();
    let Some(dir) = std::env::var_os("OFDBOX_TESTDATA_DIR").map(PathBuf::from) else {
        return;
    };
    let base = dir.join("ofd-files").join("hello-enc");
    let seed = std::fs::read_to_string(base.join("decryptseed.dat")).expect("decryptseed");
    let iv = base64_value(&seed, "ofd:IVValue");
    let wrapped = base64_value(&seed, "ofd:EncryptedWK");

    let file_key = unwrap_file_key("12345678", &iv, &wrapped).expect("unwrap FEK");
    assert_eq!(hex(&file_key), "9d1a194b95afe486ade428fd4ff2dc35");

    let map = std::fs::read(base.join("entriesmap.dat")).expect("entries map");
    let map = decrypt_cbc(&file_key, &iv, &map).expect("decrypt map");
    let map = String::from_utf8(map).expect("map utf8");
    assert!(map.contains("EncryptEntries"));
    assert!(map.contains(r#"Path="/OFD.xml" EPath="/ofd.dat""#));

    let root = std::fs::read(base.join("ofd.dat")).expect("ofd.dat");
    let root = decrypt_cbc(&file_key, &iv, &root).expect("decrypt OFD.xml");
    let root = String::from_utf8(root).expect("OFD.xml utf8");
    assert!(root.contains("<ofd:OFD"));
}

fn base64_value(xml: &str, tag: &str) -> Vec<u8> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open).expect("open tag") + open.len();
    let end = xml[start..].find(&close).expect("close tag") + start;
    base64::engine::general_purpose::STANDARD
        .decode(&xml[start..end])
        .expect("base64")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
