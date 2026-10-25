//! HMAC-SM3。

use sm3::{Digest, Sm3};

use crate::error::AlgorithmError;

const BLOCK_SIZE: usize = 64;
const OUTPUT_SIZE: usize = 32;

/// 消息认证码算法。
pub trait Mac: Send + Sync {
    /// 计算 MAC。
    fn mac(&self, key: &[u8], data: &[u8]) -> Result<Vec<u8>, AlgorithmError>;

    /// 校验 MAC。
    fn verify(&self, key: &[u8], data: &[u8], tag: &[u8]) -> Result<(), AlgorithmError> {
        if self.mac(key, data)?.as_slice() == tag {
            Ok(())
        } else {
            Err(AlgorithmError::InvalidSignature)
        }
    }
}

/// HMAC-SM3。
#[derive(Clone, Copy, Debug, Default)]
pub struct HmacSm3;

impl Mac for HmacSm3 {
    fn mac(&self, key: &[u8], data: &[u8]) -> Result<Vec<u8>, AlgorithmError> {
        let mut key_block = [0u8; BLOCK_SIZE];
        if key.len() > BLOCK_SIZE {
            key_block[..OUTPUT_SIZE].copy_from_slice(&Sm3::digest(key));
        } else {
            key_block[..key.len()].copy_from_slice(key);
        }

        let mut inner = [0x36u8; BLOCK_SIZE];
        let mut outer = [0x5cu8; BLOCK_SIZE];
        for (index, byte) in key_block.iter().enumerate() {
            inner[index] ^= byte;
            outer[index] ^= byte;
        }

        let inner_hash = {
            let mut hasher = Sm3::new();
            hasher.update(inner);
            hasher.update(data);
            hasher.finalize()
        };
        let mut hasher = Sm3::new();
        hasher.update(outer);
        hasher.update(inner_hash);
        Ok(hasher.finalize().to_vec())
    }
}
