use ofdbox_crypto::{DEFAULT_DIST_ID, public_key_from_certificate, sm3_digest};

use crate::certificate::{CertificatePolicy, validate_certificate_at};
use crate::der;
use crate::error::SealError;
use crate::eseal::parse::{Layout, SignerList, parse_layout};
use crate::fingerprint::SealFormat;
use crate::oids::SM2_SM3_SIGNATURE_OID;
use crate::signature::verify_sm2;
use crate::time::{parse_seal_time, resolve_time};

/// 印章验签参数。
#[derive(Clone, Copy, Debug)]
pub struct SealVerifyOptions<'a> {
    /// SM2 签名者标识，默认使用 GB/T 32918 的规定值。
    pub dist_id: &'a str,
    /// 证书链验证策略；不提供时只验证印章签名。
    pub certificate: Option<&'a CertificatePolicy>,
    /// 是否校验印章有效期。
    pub check_seal_validity: bool,
    /// 验证时间（Unix 秒）；缺省使用当前系统时间，WASM 下必须显式提供。
    pub verification_time: Option<u64>,
    /// 需要确认是否在印章证书列表中的签章人证书。
    pub signer_certificate: Option<&'a [u8]>,
}

impl<'a> Default for SealVerifyOptions<'a> {
    fn default() -> Self {
        Self {
            dist_id: DEFAULT_DIST_ID,
            certificate: None,
            check_seal_validity: false,
            verification_time: None,
            signer_certificate: None,
        }
    }
}

impl<'a> SealVerifyOptions<'a> {
    /// 使用默认参数。
    pub fn new() -> Self {
        Self::default()
    }

    /// 指定 SM2 签名者标识。
    pub fn with_dist_id(mut self, dist_id: &'a str) -> Self {
        self.dist_id = dist_id;
        self
    }

    /// 附加证书链验证策略。
    pub fn with_certificate_policy(mut self, policy: &'a CertificatePolicy) -> Self {
        self.certificate = Some(policy);
        self
    }

    /// 校验印章有效期。
    pub fn with_seal_validity(mut self, verification_time: Option<u64>) -> Self {
        self.check_seal_validity = true;
        self.verification_time = verification_time;
        self
    }

    /// 要求签名证书出现在印章的签章者证书列表中。
    pub fn with_signer_certificate(self, certificate: &'a [u8]) -> Self {
        Self {
            signer_certificate: Some(certificate),
            ..self
        }
    }
}

/// 印章验签结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealVerification {
    /// 已验证的印章格式。
    pub format: SealFormat,
    /// 签名算法 OID。
    pub signature_algorithm: String,
    /// 证书链长度；未提供证书策略时为 `None`。
    pub certificate_chain_length: Option<usize>,
    /// 是否完成了全部证书的 CRL 吊销检查。
    pub revocation_checked: bool,
}

/// 按调用方指定的格式验证印章签名。
///
/// 默认只验证制章者签名。证书链、证书有效期、CRL 与印章有效期需要调用方
/// 通过 [`SealVerifyOptions`] 显式启用。格式未知或与 `expected` 不一致时
/// 不做尝试性验证，直接返回错误。
pub fn verify_seal(bytes: &[u8], expected: SealFormat) -> Result<SealVerification, SealError> {
    verify_seal_with_options(bytes, expected, &SealVerifyOptions::default())
}

/// 带参数地验证印章签名与可选的信任信息。
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
    verify_sm2(&public_key, &message, signature, options.dist_id)
        .map_err(|_| SealError::SignatureInvalid)?;

    let certificate_validation = match options.certificate {
        Some(policy) => Some(validate_certificate_at(
            certificate.value,
            policy,
            options.verification_time,
        )?),
        None => None,
    };

    if options.check_seal_validity {
        let now = resolve_time(options.verification_time)?;
        let start = parse_seal_time(layout.validity[1].0, layout.validity[1].1)?;
        let end = parse_seal_time(layout.validity[2].0, layout.validity[2].1)?;
        if now < start {
            return Err(SealError::SealNotYetValid);
        }
        if now > end {
            return Err(SealError::SealExpired);
        }
    }

    if let Some(signer) = options.signer_certificate
        && !signer_in_list(&layout.signer_list, signer)?
    {
        return Err(SealError::SignerNotInSeal);
    }

    Ok(SealVerification {
        format: expected,
        signature_algorithm: algorithm,
        certificate_chain_length: certificate_validation.map(|validation| validation.chain_length),
        revocation_checked: certificate_validation
            .map(|validation| validation.revocation_checked)
            .unwrap_or(false),
    })
}

/// 判断证书是否出现在印章的签章者证书信息列表中。
///
/// 类型 1 直接比较证书 DER，类型 2 比较证书的 SM3 杂凑值。
pub fn signer_in_seal(bytes: &[u8], certificate_der: &[u8]) -> Result<bool, SealError> {
    let layout = parse_layout(bytes)?;
    signer_in_list(&layout.signer_list, certificate_der)
}

fn signer_in_list(list: &SignerList<'_>, certificate_der: &[u8]) -> Result<bool, SealError> {
    match list {
        SignerList::Certificates(certificates) => Ok(certificates.contains(&certificate_der)),
        SignerList::Digests(digests) => {
            let digest = sm3_digest(certificate_der);
            Ok(digests.contains(&digest.as_slice()))
        }
        SignerList::Unsupported(list_type) => {
            Err(SealError::UnsupportedCertificateListType(*list_type))
        }
        SignerList::Malformed => Err(SealError::Invalid("签章者证书信息列表结构无效")),
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
