//! 加密包的完整性保护生成。
//!
//! GM/T 0099 的 `OFDEntries.xml` 在加密前生成；本模块把“解密 → 生成
//! `OFDEntries.xml` → 重新加密”封装为一步，避免调用方手工拆分。

use std::io::{Cursor, Seek, Write};

use ofdbox_core::{
    EntriesWriteOptions, EntriesWriteReport, OfdError, OfdPackage, SignedValueProducer,
};

use crate::{EncryptOptions, encrypt_package};

/// 为包生成 `OFDEntries.xml`；如果包已加密，则先解密、生成、再按
/// `encryption` 重新加密。
///
/// 加密包必须已通过 `enable_decryption` 建立解密会话。
pub fn write_ofd_entries_encrypted<W: Write + Seek>(
    package: &mut OfdPackage,
    mut writer: W,
    producer: &dyn SignedValueProducer,
    entries: &EntriesWriteOptions,
    encryption: &EncryptOptions,
) -> Result<EntriesWriteReport, OfdError> {
    if !package.contains("Encryptions.xml") {
        return package.write_ofd_entries(writer, producer, entries);
    }

    let mut plaintext = Cursor::new(Vec::new());
    package.write_decrypted_to(&mut plaintext)?;
    let mut plain = OfdPackage::from_bytes(plaintext.into_inner())?;
    let mut signed = Cursor::new(Vec::new());
    let report = plain.write_ofd_entries(&mut signed, producer, entries)?;
    let encrypted = encrypt_package(&signed.into_inner(), encryption)?;
    writer.write_all(&encrypted)?;
    Ok(report)
}
