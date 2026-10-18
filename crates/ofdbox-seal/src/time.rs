use crate::der::TimeKind;
use crate::error::SealError;

/// 解析验证时间；未显式提供时使用系统时间。
pub(crate) fn resolve_time(explicit: Option<u64>) -> Result<u64, SealError> {
    explicit
        .or_else(system_now)
        .ok_or(SealError::VerificationTimeRequired)
}

#[cfg(not(target_arch = "wasm32"))]
fn system_now() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

#[cfg(target_arch = "wasm32")]
fn system_now() -> Option<u64> {
    None
}

/// 将印章属性中的时间字段解析为 Unix 秒。
pub(crate) fn parse_seal_time(text: &str, kind: TimeKind) -> Result<u64, SealError> {
    let (year, month, day, hour, minute, second) = match kind {
        TimeKind::Utc => parse_utc(text)?,
        TimeKind::Generalized => parse_generalized(text)?,
    };
    if !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return Err(SealError::Invalid("印章时间字段超出范围"));
    }
    let days = days_from_civil(year, month, day);
    if days < 0 {
        return Err(SealError::Invalid("印章时间早于 Unix 纪元"));
    }
    Ok(days as u64 * 86_400 + u64::from(hour) * 3_600 + u64::from(minute) * 60 + u64::from(second))
}

fn parse_utc(text: &str) -> Result<(i64, u32, u32, u32, u32, u32), SealError> {
    let bytes = text.as_bytes();
    if bytes.len() != 13 || bytes[12] != b'Z' || !bytes[..12].iter().all(u8::is_ascii_digit) {
        return Err(SealError::Invalid("UTCTime 格式无效"));
    }
    let year = i64::from(two(bytes, 0)?);
    let year = if year >= 50 { 1900 + year } else { 2000 + year };
    Ok((
        year,
        two(bytes, 2)?,
        two(bytes, 4)?,
        two(bytes, 6)?,
        two(bytes, 8)?,
        two(bytes, 10)?,
    ))
}

fn parse_generalized(text: &str) -> Result<(i64, u32, u32, u32, u32, u32), SealError> {
    let bytes = text.as_bytes();
    let Some(&b'Z') = bytes.last() else {
        return Err(SealError::Invalid("GeneralizedTime 必须以 Z 结尾"));
    };
    let body = &bytes[..bytes.len() - 1];
    let digits = body
        .iter()
        .position(|&byte| byte == b'.')
        .unwrap_or(body.len());
    if digits != 14 || !body[..digits].iter().all(u8::is_ascii_digit) {
        return Err(SealError::Invalid("GeneralizedTime 格式无效"));
    }
    if digits < body.len()
        && (digits + 1 == body.len() || !body[digits + 1..].iter().all(u8::is_ascii_digit))
    {
        return Err(SealError::Invalid("GeneralizedTime 小数部分无效"));
    }
    let year = four(body, 0)?;
    Ok((
        year,
        two(body, 4)?,
        two(body, 6)?,
        two(body, 8)?,
        two(body, 10)?,
        two(body, 12)?,
    ))
}

fn two(bytes: &[u8], offset: usize) -> Result<u32, SealError> {
    let text = std::str::from_utf8(&bytes[offset..offset + 2])
        .map_err(|_| SealError::Invalid("时间字段不是 ASCII 数字"))?;
    text.parse()
        .map_err(|_| SealError::Invalid("时间字段不是 ASCII 数字"))
}

fn four(bytes: &[u8], offset: usize) -> Result<i64, SealError> {
    let text = std::str::from_utf8(&bytes[offset..offset + 4])
        .map_err(|_| SealError::Invalid("时间字段不是 ASCII 数字"))?;
    text.parse()
        .map_err(|_| SealError::Invalid("时间字段不是 ASCII 数字"))
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// 公历日期到 1970-01-01 的天数（Howard Hinnant 算法）。
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = (i64::from(month) + 9) % 12;
    let day_of_year = (153 * month_prime + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::{days_from_civil, parse_seal_time};
    use crate::der::TimeKind;

    #[test]
    fn parses_utc_time_century_pivot() {
        assert_eq!(
            parse_seal_time("230117071431Z", TimeKind::Utc).expect("2023"),
            1_673_939_671
        );
        assert_eq!(
            parse_seal_time("990101000000Z", TimeKind::Utc).expect("1999"),
            915_148_800
        );
        assert_eq!(
            parse_seal_time("000101000000Z", TimeKind::Utc).expect("2000"),
            946_684_800
        );
    }

    #[test]
    fn parses_generalized_time_with_fraction() {
        assert_eq!(
            parse_seal_time("20200422020900Z", TimeKind::Generalized).expect("generalized"),
            1_587_521_340
        );
        assert_eq!(
            parse_seal_time("20200422020900.123Z", TimeKind::Generalized).expect("fraction"),
            1_587_521_340
        );
    }

    #[test]
    fn rejects_invalid_dates() {
        assert!(parse_seal_time("230230000000Z", TimeKind::Utc).is_err());
        assert!(parse_seal_time("20210229000000Z", TimeKind::Generalized).is_err());
        assert!(parse_seal_time("20200101000000", TimeKind::Generalized).is_err());
        assert!(parse_seal_time("20200101000000Z", TimeKind::Utc).is_err());
    }

    #[test]
    fn leap_day_is_valid() {
        assert_eq!(
            days_from_civil(2020, 2, 29) - days_from_civil(2020, 2, 28),
            1
        );
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }
}
