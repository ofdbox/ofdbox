//! GB/T 20520 时间戳令牌（`TimeStampToken`）的解析与验证。
//!
//! 令牌是 CMS `SignedData`，`eContent` 为 `TSTInfo`；TSA 对 `TSTInfo`
//! 的摘要签名放在 `signerInfos` 中。38540 的 `timeStamp` 字段保存令牌 DER，
//! 其 `messageImprint` 覆盖签章值（`signature`）的摘要。

use cms::cert::CertificateChoices;
use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use x509_cert::der::asn1::OctetString;
use x509_cert::der::{Decode, Encode};

use crate::certificate::{CertificatePolicy, validate_certificate};
use crate::der;
use crate::error::SealError;

/// CMS `signedData` 的 OID。
const ID_SIGNED_DATA: &str = "1.2.840.113549.1.7.2";
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
}

impl<'a> Default for TimeStampVerifyOptions<'a> {
    fn default() -> Self {
        Self {
            dist_id: ofdbox_crypto::DEFAULT_DIST_ID,
            certificate: None,
            require_time_stamping_eku: true,
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

    let certificate_validated = match options.certificate {
        Some(policy) => {
            validate_certificate(&parsed.certificate, policy)?;
            true
        }
        None => false,
    };

    Ok(TimeStampVerification {
        gen_time: parsed.gen_time,
        policy: parsed.policy,
        serial_number: parsed.serial_number,
        certificate_validated,
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
    if info.content_type.to_string() != ID_SIGNED_DATA {
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

    Ok(TstInfoFields {
        gen_time: gen_time.to_owned(),
        policy,
        serial_number,
        hash_algorithm,
        message_imprint,
    })
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
