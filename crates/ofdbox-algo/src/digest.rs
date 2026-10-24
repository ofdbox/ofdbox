use sm3::{Digest, Sm3};

/// 杂凑算法。
pub trait DigestAlgorithm: Send + Sync {
    /// 计算摘要。
    fn digest(&self, data: &[u8]) -> Vec<u8>;

    /// 输出长度（字节）。
    fn output_len(&self) -> usize;
}

/// SM3 杂凑算法。
#[derive(Clone, Copy, Debug, Default)]
pub struct Sm3Digest;

impl DigestAlgorithm for Sm3Digest {
    fn digest(&self, data: &[u8]) -> Vec<u8> {
        Sm3::digest(data).to_vec()
    }

    fn output_len(&self) -> usize {
        32
    }
}
