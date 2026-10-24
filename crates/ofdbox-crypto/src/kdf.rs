use ofdbox_algo::{Kdf, Sm3Kdf};

/// GB/T 32918.3—2016 5.4.3 密钥派生函数，杂凑算法为 SM3。
///
/// 计数器从 1 开始，按 32 位大端编码追加在输入比特串之后。
pub fn sm3_kdf(secret: &[u8], out_len: usize) -> Vec<u8> {
    Sm3Kdf.derive(secret, out_len).expect("SM3-KDF 不会失败")
}

#[cfg(test)]
mod tests {
    use super::sm3_kdf;

    #[test]
    fn matches_sample_kek() {
        let kek = sm3_kdf(b"12345678", 16);
        assert_eq!(hex(&kek), "34d6effbd5bdc7c8020f619bfd8303b5");
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
