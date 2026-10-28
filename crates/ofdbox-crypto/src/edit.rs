//! 加密 OFD 的“解密 -> 编辑 -> 重新加密”工作流。

use std::io::Cursor;

use ofdbox_core::{CryptoBackend, OfdError, OfdFile, OfdPackage, OpenOptions, OpenStorage};

use crate::EncryptOptions;

/// 解密已有加密包，交给闭包编辑，再重新加密。
///
/// 注意：
///
/// - 这是整包重新加密，不是逐 entry 原地更新；
/// - 已有签名会失效；
/// - 未识别/未建模内容仍按 `ofdbox-core` 的模型规则处理。
pub fn edit_encrypted<F>(
    input: &[u8],
    backend: Box<dyn CryptoBackend>,
    options: &EncryptOptions,
    edit: F,
) -> Result<Vec<u8>, OfdError>
where
    F: FnOnce(&mut OfdFile) -> Result<(), OfdError>,
{
    let mut package = OfdPackage::from_bytes(input.to_vec())?;
    package.enable_decryption(backend)?;

    let mut plaintext = Cursor::new(Vec::new());
    package.write_decrypted_to(&mut plaintext)?;

    let mut file = OfdFile::open_bytes_with(
        plaintext.into_inner(),
        OpenOptions {
            storage: OpenStorage::Memory,
        },
    )?;
    edit(&mut file)?;
    let edited = file.to_bytes()?;

    crate::encrypt_package(&edited, options)
}
