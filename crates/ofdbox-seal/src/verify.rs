use ofdbox_crypto::{DEFAULT_DIST_ID, public_key_from_certificate, verify_sm3, verify_sm3_raw};

use crate::der;
use crate::error::SealError;
use crate::fingerprint::SealFormat;
use crate::seal::{Layout, parse_layout};

/// SM2 签名与 SM3 杂凑的算法标识。
pub const SM2_SM3_SIGNATURE_OID: &str = "1.2.156.10197.1.501";

/// 印章验签参数。
#[derive(Clone, Copy, Debug)]
pub struct SealVerifyOptions<'a> {
    /// SM2 签名者标识，默认使用 GB/T 32918 的规定值。
    pub dist_id: &'a str,
}

impl Default for SealVerifyOptions<'static> {
    fn default() -> Self {
        Self {
            dist_id: DEFAULT_DIST_ID,
        }
    }
}

impl SealVerifyOptions<'static> {
    /// 使用默认签名者标识。
    pub fn new() -> Self {
        Self::default()
    }
}

/// 印章验签结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealVerification {
    /// 已验证的印章格式。
    pub format: SealFormat,
    /// 签名算法 OID。
    pub signature_algorithm: String,
}

/// 按调用方指定的格式验证印章签名。
///
/// 只验证制章者签名，不验证证书链、证书有效期、吊销状态或印章有效期。
/// 格式未知或与 `expected` 不一致时不做尝试性验证，直接返回错误。
pub fn verify_seal(bytes: &[u8], expected: SealFormat) -> Result<SealVerification, SealError> {
    verify_seal_with_options(bytes, expected, &SealVerifyOptions::default())
}

/// 带参数地验证印章签名。
pub fn verify_seal_with_options(
    bytes: &[u8],
    expected: SealFormat,
    options: &SealVerifyOptions<'_>,
) -> Result<SealVerification, SealError> {
    if expected == SealFormat::Unknown {
        return Err(SealError::UnsupportedFormat);
    }
    let layout = parse_layout(bytes)?;
    let actual = layout.fingerprint.format();
    if actual != expected {
        return Err(SealError::FormatMismatch { expected, actual });
    }

    let algorithm = layout
        .envelope
        .algorithm
        .and_then(|tlv| der::oid(tlv).ok())
        .ok_or(SealError::Invalid("印章签名算法标识无效"))?;
    if algorithm != SM2_SM3_SIGNATURE_OID {
        return Err(SealError::UnsupportedAlgorithm(algorithm));
    }

    let certificate = layout
        .envelope
        .certificate
        .ok_or(SealError::MissingCertificate)?;
    let public_key = public_key_from_certificate(certificate.value)
        .map_err(|_| SealError::InvalidCertificate)?;
    let signature = layout
        .envelope
        .signature
        .ok_or(SealError::Invalid("印章签名值编码无效"))?;
    let message = signed_message(&layout, expected)?;
    verify_signature(&public_key, &message, signature, options.dist_id)
        .map_err(|_| SealError::SignatureInvalid)?;

    Ok(SealVerification {
        format: expected,
        signature_algorithm: algorithm,
    })
}

/// 厂商实现同时存在 DER `SEQUENCE { r, s }` 与 `r || s` 两种签名编码。
fn verify_signature(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
    dist_id: &str,
) -> Result<(), ofdbox_crypto::CryptoError> {
    if signature.len() >= 2
        && signature[0] == 0x30
        && usize::from(signature[1]) == signature.len() - 2
    {
        verify_sm3(public_key, message, signature, dist_id)
    } else {
        verify_sm3_raw(public_key, message, signature, dist_id)
    }
}

fn signed_message(layout: &Layout<'_>, format: SealFormat) -> Result<Vec<u8>, SealError> {
    match format {
        // GB/T 38540 的签名对象是 SES_SealInfo 的 DER 编码。
        SealFormat::GbT38540 => Ok(layout.seal_info.full.to_vec()),
        // GM/T 0031 的签名对象是 SEQUENCE { 印章信息, 制章人证书, 签名算法标识 }。
        SealFormat::GmT0031 => {
            let certificate = layout
                .envelope
                .certificate
                .ok_or(SealError::MissingCertificate)?;
            let algorithm = layout
                .envelope
                .algorithm
                .ok_or(SealError::Invalid("印章签名算法标识无效"))?;
            let mut content = Vec::with_capacity(
                layout.seal_info.full.len() + certificate.full.len() + algorithm.full.len(),
            );
            content.extend_from_slice(layout.seal_info.full);
            content.extend_from_slice(certificate.full);
            content.extend_from_slice(algorithm.full);
            Ok(der::wrap(der::TAG_SEQUENCE, &content))
        }
        SealFormat::Unknown => Err(SealError::UnsupportedFormat),
    }
}
