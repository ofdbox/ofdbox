//! 制章时使用的 DER 编码小工具。

use crate::der;
use crate::error::SealError;

/// SM2 with SM3 的 DER OID 内容（1.2.156.10197.1.501）。
pub(crate) const OID_SM2_SM3: &[u8] = &[0x2A, 0x81, 0x1C, 0xCF, 0x55, 0x01, 0x83, 0x75];

pub(crate) fn ia5(value: &str) -> Vec<u8> {
    der::wrap(0x16, value.as_bytes())
}

pub(crate) fn utf8(value: &str) -> Vec<u8> {
    der::wrap(0x0c, value.as_bytes())
}

pub(crate) fn bit_string(value: &[u8]) -> Vec<u8> {
    let mut content = vec![0u8];
    content.extend_from_slice(value);
    der::wrap(0x03, &content)
}

pub(crate) fn integer(value: i64, what: &'static str) -> Result<Vec<u8>, SealError> {
    if value < 0 {
        return Err(SealError::Invalid(what));
    }
    let bytes = value.to_be_bytes();
    let start = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
    let mut content = bytes[start..].to_vec();
    if content[0] & 0x80 != 0 {
        content.insert(0, 0);
    }
    Ok(der::wrap(0x02, &content))
}

fn digits14(value: &str, what: &'static str) -> Result<String, SealError> {
    let digits: String = value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
    if digits.len() == 14 {
        Ok(digits)
    } else {
        Err(SealError::Invalid(what))
    }
}

pub(crate) fn generalized(value: &str) -> Result<Vec<u8>, SealError> {
    let digits = digits14(value, "时间需要 14 位 YYYYMMDDHHMMSS")?;
    Ok(der::wrap(0x18, format!("{digits}Z").as_bytes()))
}

pub(crate) fn utc(value: &str) -> Result<Vec<u8>, SealError> {
    let digits = digits14(value, "时间需要 14 位 YYYYMMDDHHMMSS")?;
    Ok(der::wrap(0x17, format!("{}Z", &digits[2..]).as_bytes()))
}
