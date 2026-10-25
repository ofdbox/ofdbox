//! 全量签名验证报告。
//!
//! 一次遍历包内所有签名/签章，汇总每个签名的加密验证、引用摘要、证书与
//! 时间戳状态，给出整体结论。

use ofdbox_core::{OfdError, OfdPackage, SignedValueFormat, detect_signed_value_format};
use ofdbox_crypto::{PasswordBackend, signed_data_signer_certificate, sm3_digest};
use x509_cert::Certificate;
use x509_cert::der::{Decode, Encode};

use crate::certificate::CertificatePolicy;
use crate::error::SealError;
use crate::gbt38540::signature::parse_ses_signature;
use crate::gmt0031::sign::parse_gmt0031_signature;
use crate::timestamp::{TimeStampVerifyOptions, verify_time_stamp_token};

/// 证书角色。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificateRole {
    /// 签名/签章者证书。
    Signer,
    /// 制章者证书。
    SealMaker,
    /// 时间戳机构证书。
    Tsa,
}

/// 证书状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificateState {
    /// 已验证且可信。
    Valid,
    /// 未提供证书链策略，只提取了证书信息。
    NotChecked,
    /// 证书链未到信任锚。
    Untrusted,
    /// 证书已过期。
    Expired,
    /// 证书尚未生效。
    NotYetValid,
    /// 证书已吊销。
    Revoked,
    /// 要求吊销检查但没有可用 CRL。
    RevocationUnknown,
    /// 密钥用法不符合要求。
    KeyUsage,
    /// 路径长度超限。
    PathLength,
    /// 证书自身签名无效或结构非法。
    Invalid,
}

/// 证书状态摘要。
#[derive(Clone, Debug)]
pub struct CertificateStatus {
    /// 角色。
    pub role: CertificateRole,
    /// 主题。
    pub subject: String,
    /// 签发者。
    pub issuer: String,
    /// 序列号。
    pub serial_number: String,
    /// DER 的 SM3 指纹（十六进制）。
    pub fingerprint_sm3: String,
    /// 生效时间（Unix 秒）。
    pub not_before: u64,
    /// 失效时间（Unix 秒）。
    pub not_after: u64,
    /// 状态。
    pub state: CertificateState,
    /// 证书链长度。
    pub chain_length: Option<usize>,
    /// 是否完成吊销检查。
    pub revocation_checked: bool,
    /// 失败原因。
    pub error: Option<String>,
}

/// 单个签名中的引用摘要状态。
#[derive(Clone, Debug)]
pub struct ReferenceStatus {
    /// 被引用的包内路径。
    pub file_ref: String,
    /// 摘要算法。
    pub check_method: String,
    /// 期望摘要。
    pub expected: Vec<u8>,
    /// 实际摘要。
    pub actual: Vec<u8>,
    /// 是否一致。
    pub matches: bool,
}

/// 时间戳状态。
#[derive(Clone, Debug)]
pub struct TimestampStatus {
    /// 是否携带时间戳。
    pub present: bool,
    /// 时间戳是否有效；未提供 TSA 策略时为 `None`。
    pub valid: Option<bool>,
    /// 时间戳时间。
    pub gen_time: Option<String>,
    /// 错误。
    pub error: Option<String>,
}

/// 签名状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignatureState {
    /// 完全通过。
    Valid,
    /// 加密验证失败。
    Invalid,
    /// 算法或格式不支持。
    Unsupported,
    /// 证书链不可信。
    Untrusted,
    /// 要求时间戳但缺失。
    TimestampMissing,
    /// 时间戳无效。
    TimestampInvalid,
}

/// 单个签名/签章的状态。
#[derive(Clone, Debug)]
pub struct SignatureStatus {
    /// 签名 ID。
    pub id: String,
    /// 类型：Sign / Seal。
    pub signature_type: Option<String>,
    /// 容器格式。
    pub format: SignedValueFormat,
    /// 提供者名称。
    pub provider: Option<String>,
    /// 签名时间。
    pub signed_at: Option<String>,
    /// 状态。
    pub state: SignatureState,
    /// 错误。
    pub error: Option<String>,
    /// 引用摘要。
    pub references: Vec<ReferenceStatus>,
    /// 证书状态。
    pub certificate: Option<CertificateStatus>,
    /// 时间戳状态。
    pub timestamp: TimestampStatus,
}

/// 汇总。
#[derive(Clone, Debug, Default)]
pub struct SignatureSummary {
    /// 总数。
    pub total: usize,
    /// 通过。
    pub valid: usize,
    /// 无效。
    pub invalid: usize,
    /// 不支持。
    pub unsupported: usize,
    /// 不可信。
    pub untrusted: usize,
    /// 缺时间戳。
    pub timestamp_missing: usize,
}

/// 整体结论。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportVerdict {
    /// 全部有效。
    AllValid,
    /// 部分有效。
    PartiallyValid {
        /// 有效数量。
        valid: usize,
        /// 总数。
        total: usize,
    },
    /// 全部无效。
    Invalid,
}

/// 报告。
#[derive(Clone, Debug)]
pub struct SignatureReport {
    /// 每个签名的状态。
    pub signatures: Vec<SignatureStatus>,
    /// 汇总。
    pub summary: SignatureSummary,
    /// 整体结论。
    pub verdict: ReportVerdict,
}

/// 报告选项。
#[derive(Clone, Debug, Default)]
pub struct ReportOptions {
    /// 要求引用摘要全部一致。
    pub require_references: bool,
    /// 要求每个签名都带有效时间戳。
    pub require_timestamp: bool,
    /// 要求证书链可信。
    pub require_trusted_chain: bool,
    /// 要求的最少有效签名数量。
    pub minimum_valid: usize,
    /// 证书链策略。
    pub certificate_policy: Option<CertificatePolicy>,
    /// TSA 证书链策略。
    pub timestamp_policy: Option<CertificatePolicy>,
    /// 验证时间（Unix 秒）。
    pub verification_time: Option<u64>,
}

/// 生成全量签名验证报告。
pub fn verify_signature_report(
    package: &mut OfdPackage,
    verifiers: &ofdbox_core::SignatureVerifiers,
    options: &ReportOptions,
) -> Result<SignatureReport, OfdError> {
    let index = package.parse_signatures()?;
    let mut signatures = Vec::new();
    for entry in index.signatures() {
        let Some(base_loc) = entry.base_loc() else {
            continue;
        };
        let signature = package.parse_signature(base_loc)?;
        let id = entry.id().unwrap_or_default().to_owned();
        let signature_type = entry.signature_type().map(str::to_owned);
        let provider = signature
            .signed_info()
            .and_then(|info| info.provider())
            .and_then(|provider| provider.provider_name().map(str::to_owned));
        let signed_at = signature
            .signed_info()
            .and_then(|info| info.signature_date_time().map(str::to_owned));

        let signed_value = signature
            .signed_value()
            .map(|loc| package.read_entry(loc))
            .transpose()?;
        let format = signed_value
            .as_deref()
            .map(detect_signed_value_format)
            .unwrap_or(SignedValueFormat::Other);

        let mut references = Vec::new();
        match package.verify_references(&signature, &PasswordBackend::new("")) {
            Ok(checks) => {
                for check in checks {
                    let matches = check.expected == check.actual;
                    references.push(ReferenceStatus {
                        file_ref: check.file_ref,
                        check_method: check.check_method,
                        expected: check.expected,
                        actual: check.actual,
                        matches,
                    });
                }
            }
            Err(error) => {
                references.push(ReferenceStatus {
                    file_ref: String::new(),
                    check_method: String::new(),
                    expected: Vec::new(),
                    actual: Vec::new(),
                    matches: false,
                });
                let _ = error;
            }
        }

        let mut state = SignatureState::Valid;
        let mut error = None;
        if let Err(verify_error) = package.verify_signature(&signature, verifiers) {
            state = match verify_error {
                OfdError::UnsupportedSignature(_) => SignatureState::Unsupported,
                _ => SignatureState::Invalid,
            };
            error = Some(verify_error.to_string());
        }
        if state == SignatureState::Valid
            && options.require_references
            && references.iter().any(|reference| !reference.matches)
        {
            state = SignatureState::Invalid;
            error = Some("引用摘要不一致".to_owned());
        }

        let certificate = signed_value
            .as_deref()
            .and_then(|value| certificate_status(value, format, options));
        if state == SignatureState::Valid
            && options.require_trusted_chain
            && certificate
                .as_ref()
                .is_some_and(|certificate| certificate.state != CertificateState::Valid)
        {
            state = SignatureState::Untrusted;
            error = certificate
                .as_ref()
                .and_then(|certificate| certificate.error.clone());
        }

        let timestamp = timestamp_status(signed_value.as_deref(), format, options);
        if state == SignatureState::Valid && options.require_timestamp && !timestamp.present {
            state = SignatureState::TimestampMissing;
            error = Some("缺少时间戳".to_owned());
        }
        if state == SignatureState::Valid && timestamp.valid == Some(false) {
            state = SignatureState::TimestampInvalid;
            error = timestamp.error.clone();
        }

        signatures.push(SignatureStatus {
            id,
            signature_type,
            format,
            provider,
            signed_at,
            state,
            error,
            references,
            certificate,
            timestamp,
        });
    }

    let mut summary = SignatureSummary {
        total: signatures.len(),
        ..Default::default()
    };
    for signature in &signatures {
        match signature.state {
            SignatureState::Valid => summary.valid += 1,
            SignatureState::Invalid => summary.invalid += 1,
            SignatureState::Unsupported => summary.unsupported += 1,
            SignatureState::Untrusted => summary.untrusted += 1,
            SignatureState::TimestampMissing => summary.timestamp_missing += 1,
            SignatureState::TimestampInvalid => summary.invalid += 1,
        }
    }
    let verdict =
        if summary.total == 0 || summary.valid == 0 || summary.valid < options.minimum_valid {
            ReportVerdict::Invalid
        } else if summary.valid == summary.total {
            ReportVerdict::AllValid
        } else {
            ReportVerdict::PartiallyValid {
                valid: summary.valid,
                total: summary.total,
            }
        };

    Ok(SignatureReport {
        signatures,
        summary,
        verdict,
    })
}

fn certificate_status(
    signed_value: &[u8],
    format: SignedValueFormat,
    options: &ReportOptions,
) -> Option<CertificateStatus> {
    let certificate = match format {
        SignedValueFormat::SignedData => signed_data_signer_certificate(signed_value).ok(),
        SignedValueFormat::SesSignature => parse_ses_signature(signed_value)
            .ok()
            .map(|signature| signature.certificate.to_vec()),
        SignedValueFormat::GmT0031Signature => parse_gmt0031_signature(signed_value)
            .ok()
            .map(|signature| signature.certificate.to_vec()),
        _ => None,
    }?;
    let certificate = Certificate::from_der(&certificate).ok()?;
    let tbs = certificate.tbs_certificate();
    let not_before = tbs.validity().not_before.to_unix_duration().as_secs();
    let not_after = tbs.validity().not_after.to_unix_duration().as_secs();
    let fingerprint = hex(&sm3_digest(&certificate.to_der().ok()?));
    let mut status = CertificateStatus {
        role: CertificateRole::Signer,
        subject: tbs.subject().to_string(),
        issuer: tbs.issuer().to_string(),
        serial_number: format!("{:?}", tbs.serial_number()),
        fingerprint_sm3: fingerprint,
        not_before,
        not_after,
        state: CertificateState::NotChecked,
        chain_length: None,
        revocation_checked: false,
        error: None,
    };
    let Some(policy) = &options.certificate_policy else {
        return Some(status);
    };
    match crate::certificate::validate_certificate_at(
        &certificate.to_der().ok()?,
        policy,
        options.verification_time,
    ) {
        Ok(validation) => {
            status.state = CertificateState::Valid;
            status.chain_length = Some(validation.chain_length);
            status.revocation_checked = validation.revocation_checked;
        }
        Err(error) => {
            status.state = certificate_state(&error);
            status.error = Some(error.to_string());
        }
    }
    Some(status)
}

fn certificate_state(error: &SealError) -> CertificateState {
    match error {
        SealError::UntrustedCertificate | SealError::InvalidCertificateChain => {
            CertificateState::Untrusted
        }
        SealError::CertificateExpired => CertificateState::Expired,
        SealError::CertificateNotYetValid => CertificateState::NotYetValid,
        SealError::CertificateRevoked => CertificateState::Revoked,
        SealError::RevocationUnknown => CertificateState::RevocationUnknown,
        SealError::CertificateKeyUsage => CertificateState::KeyUsage,
        SealError::CertificatePathLength => CertificateState::PathLength,
        _ => CertificateState::Invalid,
    }
}

fn timestamp_status(
    signed_value: Option<&[u8]>,
    format: SignedValueFormat,
    options: &ReportOptions,
) -> TimestampStatus {
    if format != SignedValueFormat::SesSignature {
        return TimestampStatus {
            present: false,
            valid: None,
            gen_time: None,
            error: None,
        };
    }
    let Some(value) = signed_value else {
        return TimestampStatus {
            present: false,
            valid: None,
            gen_time: None,
            error: None,
        };
    };
    let Ok(signature) = parse_ses_signature(value) else {
        return TimestampStatus {
            present: false,
            valid: None,
            gen_time: None,
            error: None,
        };
    };
    let Some(token) = signature.time_stamp else {
        return TimestampStatus {
            present: false,
            valid: None,
            gen_time: None,
            error: None,
        };
    };
    let Some(policy) = &options.timestamp_policy else {
        return TimestampStatus {
            present: true,
            valid: None,
            gen_time: None,
            error: None,
        };
    };
    match verify_time_stamp_token(
        token,
        signature.signature,
        &TimeStampVerifyOptions {
            certificate: Some(policy),
            ..Default::default()
        },
    ) {
        Ok(verification) => TimestampStatus {
            present: true,
            valid: Some(true),
            gen_time: Some(verification.gen_time),
            error: None,
        },
        Err(error) => TimestampStatus {
            present: true,
            valid: Some(false),
            gen_time: None,
            error: Some(error.to_string()),
        },
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
