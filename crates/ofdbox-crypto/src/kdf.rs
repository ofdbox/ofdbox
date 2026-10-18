use sm3::{Digest, Sm3};

/// GB/T 32918.3—2016 5.4.3 密钥派生函数，杂凑算法为 SM3。
///
/// 计数器从 1 开始，按 32 位大端编码追加在输入比特串之后。
pub fn sm3_kdf(secret: &[u8], out_len: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(out_len);
    let mut counter = 1u32;
    while output.len() < out_len {
        let mut hash = Sm3::new();
        hash.update(secret);
        hash.update(counter.to_be_bytes());
        output.extend_from_slice(&hash.finalize());
        counter = counter.wrapping_add(1);
    }
    output.truncate(out_len);
    output
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
