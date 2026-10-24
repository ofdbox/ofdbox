use sm3::{Digest, Sm3};

use crate::error::AlgorithmError;

/// 密钥派生函数。
pub trait Kdf: Send + Sync {
    /// 从共享秘密派生指定长度的密钥材料。
    fn derive(&self, secret: &[u8], length: usize) -> Result<Vec<u8>, AlgorithmError>;
}

/// SM3 计数器模式 KDF（GB/T 32918.3 5.4.3）。
#[derive(Clone, Copy, Debug, Default)]
pub struct Sm3Kdf;

impl Kdf for Sm3Kdf {
    fn derive(&self, secret: &[u8], length: usize) -> Result<Vec<u8>, AlgorithmError> {
        let mut output = Vec::with_capacity(length);
        let mut counter = 1u32;
        while output.len() < length {
            let mut hash = Sm3::new();
            hash.update(secret);
            hash.update(counter.to_be_bytes());
            output.extend_from_slice(&hash.finalize());
            counter = counter.wrapping_add(1);
        }
        output.truncate(length);
        Ok(output)
    }
}
