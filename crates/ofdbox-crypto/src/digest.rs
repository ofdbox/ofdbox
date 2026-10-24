//! 杂凑原语（实现位于 `ofdbox-algo`）。

use ofdbox_algo::{DigestAlgorithm, Sm3Digest};

/// 计算 SM3 杂凑。
pub fn sm3_digest(data: &[u8]) -> [u8; 32] {
    let digest = Sm3Digest.digest(data);
    let mut output = [0u8; 32];
    output.copy_from_slice(&digest);
    output
}

#[cfg(test)]
mod tests {
    use super::sm3_digest;

    #[test]
    fn matches_known_vector() {
        assert_eq!(
            sm3_digest(b"abc"),
            [
                0x66, 0xc7, 0xf0, 0xf4, 0x62, 0xee, 0xed, 0xd9, 0xd1, 0xf2, 0xd4, 0x6b, 0xdc, 0x10,
                0xe4, 0xe2, 0x41, 0x67, 0xc4, 0x87, 0x5c, 0xf2, 0xf7, 0xa2, 0x29, 0x7d, 0xa0, 0x2b,
                0x8f, 0x4b, 0xa8, 0xe0,
            ]
        );
    }
}
