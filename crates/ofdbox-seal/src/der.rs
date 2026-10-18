use crate::error::SealError;

pub(crate) const TAG_INTEGER: u8 = 0x02;
pub(crate) const TAG_BIT_STRING: u8 = 0x03;
pub(crate) const TAG_OCTET_STRING: u8 = 0x04;
pub(crate) const TAG_OID: u8 = 0x06;
pub(crate) const TAG_UTF8_STRING: u8 = 0x0c;
pub(crate) const TAG_IA5_STRING: u8 = 0x16;
pub(crate) const TAG_UTC_TIME: u8 = 0x17;
pub(crate) const TAG_GENERALIZED_TIME: u8 = 0x18;
pub(crate) const TAG_SEQUENCE: u8 = 0x30;

/// 单个 SEQUENCE 允许的子节点数，防止异常结构占用过多内存。
const MAX_CHILDREN: usize = 1024;
const MAX_OID_ARCS: usize = 64;

/// 一个已定界的 DER TLV。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tlv<'a> {
    pub(crate) tag: u8,
    pub(crate) constructed: bool,
    pub(crate) value: &'a [u8],
    pub(crate) full: &'a [u8],
}

impl<'a> Tlv<'a> {
    pub(crate) fn children(self) -> Result<Children<'a>, SealError> {
        if !self.constructed {
            return Err(SealError::Invalid("期望构造类型"));
        }
        Ok(Children {
            input: self.value,
            offset: 0,
            count: 0,
        })
    }
}

/// 在已定界的构造类型内部逐项扫描。
pub(crate) struct Children<'a> {
    input: &'a [u8],
    offset: usize,
    count: usize,
}

impl<'a> Iterator for Children<'a> {
    type Item = Result<Tlv<'a>, SealError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.input.len() {
            return None;
        }
        if self.count >= MAX_CHILDREN {
            self.offset = self.input.len();
            return Some(Err(SealError::LimitExceeded("构造类型子节点过多")));
        }
        match parse_tlv(self.input, self.offset, self.input.len()) {
            Ok((tlv, next)) => {
                self.offset = next;
                self.count += 1;
                Some(Ok(tlv))
            }
            Err(error) => {
                self.offset = self.input.len();
                Some(Err(error))
            }
        }
    }
}

/// 解析 `offset` 处的 TLV，返回节点与下一项的偏移。
///
/// 只支持单字节标签与定长编码；任何越界都在此处被拒绝。
pub(crate) fn parse_tlv(
    input: &[u8],
    offset: usize,
    end: usize,
) -> Result<(Tlv<'_>, usize), SealError> {
    if offset >= end {
        return Err(SealError::Truncated("缺少 TLV 标签"));
    }
    let tag = input[offset];
    if tag & 0x1f == 0x1f {
        return Err(SealError::Invalid("不支持高标签号"));
    }
    let constructed = tag & 0x20 != 0;
    let mut cursor = offset + 1;
    if cursor >= end {
        return Err(SealError::Truncated("缺少长度字段"));
    }
    let first = input[cursor];
    cursor += 1;
    let length = if first & 0x80 == 0 {
        usize::from(first)
    } else {
        let count = usize::from(first & 0x7f);
        if count == 0 {
            return Err(SealError::Invalid("不支持不定长编码"));
        }
        if count > 8 {
            return Err(SealError::LimitExceeded("长度字段过长"));
        }
        if cursor + count > end {
            return Err(SealError::Truncated("长度字段不完整"));
        }
        let mut length = 0usize;
        for &byte in &input[cursor..cursor + count] {
            length = length
                .checked_mul(256)
                .and_then(|value| value.checked_add(usize::from(byte)))
                .ok_or(SealError::LimitExceeded("长度超出平台范围"))?;
        }
        cursor += count;
        length
    };
    let value_end = cursor
        .checked_add(length)
        .ok_or(SealError::LimitExceeded("长度超出平台范围"))?;
    if value_end > end {
        return Err(SealError::Truncated("值内容越过容器边界"));
    }
    Ok((
        Tlv {
            tag,
            constructed,
            value: &input[cursor..value_end],
            full: &input[offset..value_end],
        },
        value_end,
    ))
}

pub(crate) fn bit_string_content(tlv: Tlv<'_>) -> Option<&[u8]> {
    if tlv.tag != TAG_BIT_STRING || tlv.value.is_empty() || tlv.value[0] != 0 {
        return None;
    }
    Some(&tlv.value[1..])
}

/// 按 DER 规则编码一个单字节标签的 TLV。
pub(crate) fn wrap(tag: u8, value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + 9);
    out.push(tag);
    write_length(&mut out, value.len());
    out.extend_from_slice(value);
    out
}

fn write_length(out: &mut Vec<u8>, length: usize) {
    if length < 0x80 {
        out.push(length as u8);
        return;
    }
    let bytes = (length as u64).to_be_bytes();
    let start = bytes.iter().position(|&byte| byte != 0).unwrap_or(7);
    out.push(0x80 | (bytes.len() - start) as u8);
    out.extend_from_slice(&bytes[start..]);
}

pub(crate) fn parse_root(input: &[u8]) -> Result<Tlv<'_>, SealError> {
    let (tlv, end) = parse_tlv(input, 0, input.len())?;
    if end != input.len() {
        return Err(SealError::Invalid("印章数据末尾存在多余字节"));
    }
    Ok(tlv)
}

pub(crate) fn items(tlv: Tlv<'_>) -> Result<Vec<Tlv<'_>>, SealError> {
    tlv.children()?.collect()
}

pub(crate) fn expect_tag(tlv: Tlv<'_>, tag: u8, what: &'static str) -> Result<(), SealError> {
    if tlv.tag != tag {
        return Err(SealError::Invalid(what));
    }
    Ok(())
}

pub(crate) fn integer(tlv: Tlv<'_>) -> Result<i64, SealError> {
    expect_tag(tlv, TAG_INTEGER, "期望 INTEGER")?;
    if tlv.value.is_empty() {
        return Err(SealError::Invalid("INTEGER 为空"));
    }
    if tlv.value.len() > 8 {
        return Err(SealError::LimitExceeded("INTEGER 过长"));
    }
    let mut value: i64 = if tlv.value[0] & 0x80 != 0 { -1 } else { 0 };
    for &byte in tlv.value {
        value = (value << 8) | i64::from(byte);
    }
    Ok(value)
}

pub(crate) fn ia5<'a>(tlv: Tlv<'a>, what: &'static str) -> Result<&'a str, SealError> {
    expect_tag(tlv, TAG_IA5_STRING, what)?;
    text(tlv.value)
}

pub(crate) fn utf8<'a>(tlv: Tlv<'a>, what: &'static str) -> Result<&'a str, SealError> {
    expect_tag(tlv, TAG_UTF8_STRING, what)?;
    std::str::from_utf8(tlv.value).map_err(|_| SealError::Invalid(what))
}

pub(crate) fn octet_string<'a>(tlv: Tlv<'a>, what: &'static str) -> Result<&'a [u8], SealError> {
    expect_tag(tlv, TAG_OCTET_STRING, what)?;
    Ok(tlv.value)
}

pub(crate) fn time<'a>(tlv: Tlv<'a>) -> Result<(&'a str, TimeKind), SealError> {
    let kind = match tlv.tag {
        TAG_UTC_TIME => TimeKind::Utc,
        TAG_GENERALIZED_TIME => TimeKind::Generalized,
        _ => return Err(SealError::Invalid("期望时间类型")),
    };
    Ok((text(tlv.value)?, kind))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TimeKind {
    Utc,
    Generalized,
}

fn text(bytes: &[u8]) -> Result<&str, SealError> {
    if !bytes.is_ascii() {
        return Err(SealError::Invalid("字符串包含非 ASCII 字节"));
    }
    Ok(std::str::from_utf8(bytes).expect("ASCII 一定是合法 UTF-8"))
}

pub(crate) fn oid(tlv: Tlv<'_>) -> Result<String, SealError> {
    expect_tag(tlv, TAG_OID, "期望 OBJECT IDENTIFIER")?;
    if tlv.value.is_empty() {
        return Err(SealError::Invalid("OID 为空"));
    }
    let mut arcs = Vec::new();
    let mut value: u128 = 0;
    let mut pending = false;
    for &byte in tlv.value {
        value = value
            .checked_mul(128)
            .and_then(|current| current.checked_add(u128::from(byte & 0x7f)))
            .ok_or(SealError::LimitExceeded("OID 分量过大"))?;
        pending = byte & 0x80 != 0;
        if !pending {
            if arcs.len() >= MAX_OID_ARCS {
                return Err(SealError::LimitExceeded("OID 分量过多"));
            }
            arcs.push(value);
            value = 0;
        }
    }
    if pending {
        return Err(SealError::Truncated("OID 最后一个分量不完整"));
    }
    let first = arcs[0];
    let (head, second) = if first < 40 {
        (0, first)
    } else if first < 80 {
        (1, first - 40)
    } else {
        (2, first - 80)
    };
    let mut result = format!("{head}.{second}");
    for arc in &arcs[1..] {
        result.push('.');
        result.push_str(&arc.to_string());
    }
    Ok(result)
}
