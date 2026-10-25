//! GB/T 20520 时间戳令牌（`TimeStampToken`）的解析与验证。
//!
//! 令牌是 CMS `SignedData`，`eContent` 为 `TSTInfo`；TSA 对 `TSTInfo`
//! 的摘要签名放在 `signerInfos` 中。38540 的 `timeStamp` 字段保存令牌 DER，
//! 其 `messageImprint` 覆盖签章值（`signature`）的摘要。

use cms::cert::CertificateChoices;
use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use x509_cert::der::asn1::{ObjectIdentifier, OctetString};
use x509_cert::der::{Decode, Encode};

use crate::certificate::CertificatePolicy;
use crate::der;
use crate::error::SealError;

/// PKCS#7 `signedData` 的 OID。
const ID_SIGNED_DATA: &str = "1.2.840.113549.1.7.2";
/// GB/T 35275（沿用 GM/T 0010）的 `signedData` OID。
const ID_SIGNED_DATA_GMT: &str = "1.2.156.10197.6.1.4.2.2";
/// SM3 的 OID。
const ID_SM3: &str = "1.2.156.10197.1.401";
/// `id-kp-timeStamping` 的 OID。
const ID_KP_TIME_STAMPING: &str = "1.3.6.1.5.5.7.3.8";

/// 解析后的时间戳令牌。
#[derive(Clone, Debug)]
pub struct TimeStampToken {
    /// `TSTInfo` 的完整 DER。
    pub tst_info: Vec<u8>,
    /// 签名者（TSA）证书 DER。
    pub certificate: Vec<u8>,
    /// 时间戳时间（GeneralizedTime 文本）。
    pub gen_time: String,
    /// 时间戳策略 OID。
    pub policy: String,
    /// 序列号。
    pub serial_number: i64,
    /// `messageImprint` 的摘要算法 OID。
    pub hash_algorithm: String,
    /// `messageImprint` 的摘要值。
    pub message_imprint: Vec<u8>,
    /// `TSTInfo.nonce`。
    pub nonce: Option<i64>,
    /// 是否带 `accuracy`。
    pub accuracy: bool,
    /// `TSTInfo.ordering`。
    pub ordering: Option<bool>,
}

/// 时间戳验证结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeStampVerification {
    /// 时间戳时间。
    pub gen_time: String,
    /// 时间戳策略 OID。
    pub policy: String,
    /// 序列号。
    pub serial_number: i64,
    /// 是否按提供的策略完成了 TSA 证书链验证。
    pub certificate_validated: bool,
    /// 令牌 nonce。
    pub nonce: Option<i64>,
    /// 是否带 accuracy。
    pub accuracy: bool,
}

/// 时间戳验证参数。
#[derive(Clone, Copy, Debug)]
pub struct TimeStampVerifyOptions<'a> {
    /// SM2 签名者标识。
    pub dist_id: &'a str,
    /// TSA 证书链验证策略；为 `None` 时只验证令牌签名与摘要。
    pub certificate: Option<&'a CertificatePolicy>,
    /// 是否要求 TSA 证书带 `id-kp-timeStamping` 扩展。
    pub require_time_stamping_eku: bool,
    /// 期望的 nonce；`Some` 时与令牌不一致即失败。
    pub nonce: Option<i64>,
    /// 期望的 TSA 策略 OID；`Some` 时与令牌不一致即失败。
    pub policy: Option<&'a str>,
}

impl<'a> Default for TimeStampVerifyOptions<'a> {
    fn default() -> Self {
        Self {
            dist_id: ofdbox_crypto::DEFAULT_DIST_ID,
            certificate: None,
            require_time_stamping_eku: true,
            nonce: None,
            policy: None,
        }
    }
}

/// 解析时间戳令牌。
pub fn parse_time_stamp_token(bytes: &[u8]) -> Result<TimeStampToken, SealError> {
    let (signed_data, tst_info) = decode_token(bytes)?;
    let fields = parse_tst_info(&tst_info)?;
    let certificate = signed_data
        .certificates
        .as_ref()
        .and_then(|certificates| {
            certificates.0.iter().find_map(|choice| match choice {
                CertificateChoices::Certificate(certificate) => certificate.to_der().ok(),
                _ => None,
            })
        })
        .ok_or(SealError::MissingCertificate)?;

    Ok(TimeStampToken {
        tst_info,
        certificate,
        gen_time: fields.gen_time,
        policy: fields.policy,
        serial_number: fields.serial_number,
        hash_algorithm: fields.hash_algorithm,
        message_imprint: fields.message_imprint,
        nonce: fields.nonce,
        accuracy: fields.accuracy,
        ordering: fields.ordering,
    })
}

/// 验证时间戳令牌，并确认它覆盖的是 `signed_value`（签章值）。
///
/// - `token`：`timeStamp [0]` 中的令牌 DER；
/// - `signed_value`：`SES_Signature.signature` 的字节内容；
/// - 只支持 SM2 + SM3 的令牌。
pub fn verify_time_stamp_token(
    token: &[u8],
    signed_value: &[u8],
    options: &TimeStampVerifyOptions<'_>,
) -> Result<TimeStampVerification, SealError> {
    let parsed = parse_time_stamp_token(token)?;
    if parsed.hash_algorithm != ID_SM3 {
        return Err(SealError::UnsupportedAlgorithm(parsed.hash_algorithm));
    }

    // TSA 对 TSTInfo 的 CMS 签名；内部会校验 signedAttrs 的 messageDigest。
    ofdbox_crypto::verify_signed_data(&parsed.tst_info, token, options.dist_id)
        .map_err(|_| SealError::SignatureInvalid)?;

    // 令牌必须覆盖当前签章值，防止把别处的令牌搬过来。
    let actual = ofdbox_crypto::sm3_digest(signed_value);
    if parsed.message_imprint != actual.as_slice() {
        return Err(SealError::SignatureInvalid);
    }

    if options.require_time_stamping_eku && !has_time_stamping_eku(&parsed.certificate)? {
        return Err(SealError::InvalidCertificate);
    }
    if let Some(expected) = options.nonce
        && parsed.nonce != Some(expected)
    {
        return Err(SealError::Invalid("时间戳 nonce 不匹配"));
    }
    if let Some(expected) = options.policy
        && parsed.policy != expected
    {
        return Err(SealError::Invalid("时间戳策略不匹配"));
    }

    let certificate_validated = match options.certificate {
        Some(policy) => {
            // 长期验证：默认在 genTime 时刻校验证书链；策略显式指定时间时以其为准。
            let unix = policy
                .verification_time
                .or_else(|| unix_from_gen_time(&parsed.gen_time));
            crate::certificate::validate_certificate_at(&parsed.certificate, policy, unix)?;
            true
        }
        None => false,
    };

    Ok(TimeStampVerification {
        gen_time: parsed.gen_time,
        policy: parsed.policy,
        serial_number: parsed.serial_number,
        certificate_validated,
        nonce: parsed.nonce,
        accuracy: parsed.accuracy,
    })
}

/// 把时间戳令牌附加到已有的 `SES_Signature` 上。
///
/// 签章值必须先产生，TSA 才能对它签发令牌；因此典型流程是：
/// `produce` → `parse_ses_signature` 取 `signature` → 向 TSA 请求令牌 →
/// 调用本函数重新组装。已有 `timeStamp` 会被替换。
pub fn attach_time_stamp(signed_value: &[u8], token: &[u8]) -> Result<Vec<u8>, SealError> {
    let (root, end) = der::parse_tlv(signed_value, 0, signed_value.len())?;
    if !signed_value[end..].iter().all(|byte| *byte == 0) {
        return Err(SealError::Invalid("签章数据末尾存在多余字节"));
    }
    der::expect_tag(root, der::TAG_SEQUENCE, "签章数据根节点不是 SEQUENCE")?;
    let items = der::items(root)?;
    if items.len() < 4 {
        return Err(SealError::Invalid("签章数据字段不足"));
    }

    let mut content = Vec::new();
    for item in items.iter().take(4) {
        content.extend_from_slice(item.full);
    }
    let mut bits = vec![0u8];
    bits.extend_from_slice(token);
    content.extend(der::wrap(0xA0, &der::wrap(0x03, &bits)));
    Ok(der::wrap(0x30, &content))
}

fn decode_token(bytes: &[u8]) -> Result<(SignedData, Vec<u8>), SealError> {
    let info = ContentInfo::from_der(bytes)
        .map_err(|_| SealError::Invalid("时间戳令牌不是 CMS ContentInfo"))?;
    let content_type = info.content_type.to_string();
    if content_type != ID_SIGNED_DATA && content_type != ID_SIGNED_DATA_GMT {
        return Err(SealError::Invalid("时间戳令牌的内容类型不是 signedData"));
    }
    let signed_data: SignedData = info
        .content
        .decode_as()
        .map_err(|_| SealError::Invalid("时间戳令牌不是 CMS SignedData"))?;
    let econtent = signed_data
        .encap_content_info
        .econtent
        .as_ref()
        .ok_or(SealError::Invalid("时间戳令牌缺少 eContent"))?;
    let octets = OctetString::from_der(
        &econtent
            .to_der()
            .map_err(|_| SealError::Invalid("时间戳令牌 eContent 编码无效"))?,
    )
    .map_err(|_| SealError::Invalid("时间戳令牌 eContent 不是 OCTET STRING"))?;
    Ok((signed_data, octets.as_bytes().to_vec()))
}

struct TstInfoFields {
    gen_time: String,
    policy: String,
    serial_number: i64,
    hash_algorithm: String,
    message_imprint: Vec<u8>,
    nonce: Option<i64>,
    accuracy: bool,
    ordering: Option<bool>,
}

fn parse_tst_info(bytes: &[u8]) -> Result<TstInfoFields, SealError> {
    let (root, end) = der::parse_tlv(bytes, 0, bytes.len())?;
    if end != bytes.len() {
        return Err(SealError::Invalid("TSTInfo 末尾存在多余字节"));
    }
    der::expect_tag(root, der::TAG_SEQUENCE, "TSTInfo 不是 SEQUENCE")?;
    let fields = der::items(root)?;
    if fields.len() < 5 {
        return Err(SealError::Invalid("TSTInfo 字段不足"));
    }

    let policy = der::oid(fields[1])?;
    der::expect_tag(fields[2], der::TAG_SEQUENCE, "messageImprint 不是 SEQUENCE")?;
    let imprint = der::items(fields[2])?;
    if imprint.len() != 2 {
        return Err(SealError::Invalid("messageImprint 字段数量不正确"));
    }
    der::expect_tag(
        imprint[0],
        der::TAG_SEQUENCE,
        "messageImprint 摘要算法不是 SEQUENCE",
    )?;
    let algorithm = der::items(imprint[0])?;
    let hash_algorithm = der::oid(
        *algorithm
            .first()
            .ok_or(SealError::Invalid("messageImprint 缺少摘要算法"))?,
    )?;
    let message_imprint = der::octet_string(imprint[1], "messageImprint 摘要无效")?.to_vec();
    let serial_number = der::integer(fields[3])?;
    let (gen_time, _) = der::time(fields[4])?;
    let mut nonce = None;
    let mut accuracy = false;
    let mut ordering = None;
    for field in fields.iter().skip(5) {
        match field.tag {
            0x30 => accuracy = true,
            0x01 => ordering = Some(field.value.first().is_some_and(|byte| *byte != 0)),
            0x02 => nonce = Some(der::integer(*field)?),
            _ => {}
        }
    }

    Ok(TstInfoFields {
        gen_time: gen_time.to_owned(),
        policy,
        serial_number,
        hash_algorithm,
        message_imprint,
        nonce,
        accuracy,
        ordering,
    })
}

fn unix_from_gen_time(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() < 19 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        std::str::from_utf8(&bytes[range]).ok()?.parse().ok()
    };
    let year = number(0..4)?;
    let month = number(5..7)? as u32;
    let day = number(8..10)? as u32;
    let hour = number(11..13)?;
    let minute = number(14..16)?;
    let second = number(17..19)?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut offset_minutes = 0i64;
    let mut index = 19;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
    }
    match bytes.get(index) {
        None | Some(b'Z') => {}
        Some(b'+') | Some(b'-') => {
            let sign = if bytes[index] == b'-' { -1 } else { 1 };
            let hours = number(index + 1..index + 3)?;
            let minutes = number(index + 4..index + 6)?;
            offset_minutes = sign * (hours * 60 + minutes);
        }
        _ => return None,
    }
    let days = days_from_civil(year, month, day);
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second - offset_minutes * 60;
    u64::try_from(seconds).ok()
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = ((month + 9) % 12) as i64;
    let day_of_year = (153 * month_prime + 2) / 5 + day as i64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn has_time_stamping_eku(certificate_der: &[u8]) -> Result<bool, SealError> {
    let certificate = x509_cert::Certificate::from_der(certificate_der)
        .map_err(|_| SealError::InvalidCertificate)?;
    let Some(extensions) = certificate.tbs_certificate().extensions() else {
        return Ok(false);
    };
    for extension in extensions {
        if extension.extn_id.to_string() != "2.5.29.37" {
            continue;
        }
        let usage =
            x509_cert::ext::pkix::ExtendedKeyUsage::from_der(extension.extn_value.as_bytes())
                .map_err(|_| SealError::InvalidCertificate)?;
        return Ok(usage
            .0
            .iter()
            .any(|oid| oid.to_string() == ID_KP_TIME_STAMPING));
    }
    Ok(false)
}

/// 时间戳请求（`TimeStampReq`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeStampRequest {
    /// 协议版本，通常为 1。
    pub version: i64,
    /// 摘要算法 OID。
    pub hash_algorithm: String,
    /// 被时间戳数据的摘要。
    pub hashed_message: Vec<u8>,
    /// 请求的 TSA 策略。
    pub policy: Option<String>,
    /// 随机数。
    pub nonce: Option<i64>,
    /// 是否要求 TSA 返回证书。
    pub cert_req: bool,
}

impl TimeStampRequest {
    /// 创建 v1 请求。
    pub fn new(hash_algorithm: impl Into<String>, hashed_message: impl Into<Vec<u8>>) -> Self {
        Self {
            version: 1,
            hash_algorithm: hash_algorithm.into(),
            hashed_message: hashed_message.into(),
            policy: None,
            nonce: None,
            cert_req: false,
        }
    }

    /// 指定 nonce。
    pub fn with_nonce(mut self, nonce: i64) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// 指定策略。
    pub fn with_policy(mut self, policy: impl Into<String>) -> Self {
        self.policy = Some(policy.into());
        self
    }

    /// 要求 TSA 返回证书。
    pub fn with_cert_req(mut self, cert_req: bool) -> Self {
        self.cert_req = cert_req;
        self
    }

    /// 编码为 `TimeStampReq` DER。
    pub fn to_der(&self) -> Result<Vec<u8>, SealError> {
        let algorithm = der::wrap(der::TAG_SEQUENCE, &encode_oid(&self.hash_algorithm)?);
        let imprint = der::wrap(
            der::TAG_SEQUENCE,
            &[
                algorithm,
                der::wrap(der::TAG_OCTET_STRING, &self.hashed_message),
            ]
            .concat(),
        );
        let mut body = Vec::new();
        body.extend(encode_integer(self.version));
        body.extend(imprint);
        if let Some(policy) = &self.policy {
            body.extend(encode_oid(policy)?);
        }
        if let Some(nonce) = self.nonce {
            body.extend(encode_integer(nonce));
        }
        if self.cert_req {
            body.extend(der::wrap(0x01, &[0xFF]));
        }
        Ok(der::wrap(der::TAG_SEQUENCE, &body))
    }
}

/// 解析 `TimeStampReq` DER。
pub fn parse_time_stamp_request(bytes: &[u8]) -> Result<TimeStampRequest, SealError> {
    let (root, end) = der::parse_tlv(bytes, 0, bytes.len())?;
    if end != bytes.len() {
        return Err(SealError::Invalid("时间戳请求末尾存在多余字节"));
    }
    der::expect_tag(root, der::TAG_SEQUENCE, "时间戳请求不是 SEQUENCE")?;
    let fields = der::items(root)?;
    if fields.len() < 2 {
        return Err(SealError::Invalid("时间戳请求字段不足"));
    }
    let version = der::integer(fields[0])?;
    der::expect_tag(fields[1], der::TAG_SEQUENCE, "messageImprint 不是 SEQUENCE")?;
    let imprint = der::items(fields[1])?;
    if imprint.len() != 2 {
        return Err(SealError::Invalid("messageImprint 字段数量不正确"));
    }
    let hash_algorithm = if imprint[0].tag == der::TAG_SEQUENCE {
        let algorithm = der::items(imprint[0])?;
        der::oid(
            *algorithm
                .first()
                .ok_or(SealError::Invalid("messageImprint 缺少摘要算法"))?,
        )?
    } else {
        der::oid(imprint[0])?
    };
    let hashed_message = der::octet_string(imprint[1], "messageImprint 摘要无效")?.to_vec();
    let mut policy = None;
    let mut nonce = None;
    let mut cert_req = false;
    for field in fields.iter().skip(2) {
        match field.tag {
            der::TAG_OID => policy = Some(der::oid(*field)?),
            der::TAG_INTEGER => nonce = Some(der::integer(*field)?),
            0x01 => cert_req = field.value.first().is_some_and(|byte| *byte != 0),
            _ => {}
        }
    }
    Ok(TimeStampRequest {
        version,
        hash_algorithm,
        hashed_message,
        policy,
        nonce,
        cert_req,
    })
}

/// 时间戳响应。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeStampResponse {
    /// PKI 状态码。
    pub status: i64,
    /// 状态文本。
    pub status_text: Vec<String>,
    /// 时间戳令牌 DER。
    pub token: Option<Vec<u8>>,
}

/// 解析 `TimeStampResp` DER。
pub fn parse_time_stamp_response(bytes: &[u8]) -> Result<TimeStampResponse, SealError> {
    let (root, end) = der::parse_tlv(bytes, 0, bytes.len())?;
    if end != bytes.len() {
        return Err(SealError::Invalid("时间戳响应末尾存在多余字节"));
    }
    der::expect_tag(root, der::TAG_SEQUENCE, "时间戳响应不是 SEQUENCE")?;
    let fields = der::items(root)?;
    if fields.is_empty() {
        return Err(SealError::Invalid("时间戳响应缺少状态"));
    }
    der::expect_tag(fields[0], der::TAG_SEQUENCE, "PKIStatusInfo 不是 SEQUENCE")?;
    let status_fields = der::items(fields[0])?;
    let status = status_fields
        .first()
        .ok_or(SealError::Invalid("时间戳响应缺少状态码"))
        .and_then(|field| der::integer(*field))?;
    let mut status_text = Vec::new();
    for field in status_fields.iter().skip(1) {
        if field.tag != der::TAG_SEQUENCE {
            continue;
        }
        for text in der::items(*field)? {
            if text.tag == 0x0C
                && let Ok(value) = std::str::from_utf8(text.value)
            {
                status_text.push(value.to_owned());
            }
        }
    }
    let token = fields.get(1).map(|field| field.full.to_vec());
    Ok(TimeStampResponse {
        status,
        status_text,
        token,
    })
}

fn encode_integer(value: i64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let start = bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(bytes.len() - 1);
    let mut content = bytes[start..].to_vec();
    if content[0] & 0x80 != 0 {
        content.insert(0, 0);
    }
    der::wrap(der::TAG_INTEGER, &content)
}

fn encode_oid(oid: &str) -> Result<Vec<u8>, SealError> {
    ObjectIdentifier::new_unwrap(oid)
        .to_der()
        .map_err(|_| SealError::Invalid("时间戳算法 OID 无效"))
}
