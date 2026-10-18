use ofdbox_crypto::{CryptoError, verify_sm3, verify_sm3_raw};

/// 厂商实现同时存在 DER `SEQUENCE { r, s }` 与 `r || s` 两种签名编码。
pub(crate) fn verify_sm2(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
    dist_id: &str,
) -> Result<(), CryptoError> {
    if signature.len() >= 2
        && signature[0] == 0x30
        && usize::from(signature[1]) == signature.len() - 2
    {
        verify_sm3(public_key, message, signature, dist_id)
    } else {
        verify_sm3_raw(public_key, message, signature, dist_id)
    }
}
