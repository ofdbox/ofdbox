use ofdbox_crypto::DEFAULT_DIST_ID;
use x509_cert::Certificate;
use x509_cert::crl::CertificateList;
use x509_cert::der::{Decode, Encode};
use x509_cert::ext::pkix::{BasicConstraints, KeyUsage};

use crate::error::SealError;
use crate::oids::SM2_SM3_SIGNATURE_OID;
use crate::signature::verify_sm2;
use crate::time::resolve_time;

/// 证书链最长长度，包含终端证书与信任锚。
const MAX_CHAIN_LENGTH: usize = 8;

/// 证书验证策略。
///
/// 信任完全由调用方提供：没有信任锚时任何证书链都会被拒绝。
/// 吊销检查只使用调用方提供的 CRL，不访问网络。
#[derive(Clone, Debug)]
pub struct CertificatePolicy {
    /// 信任锚证书（DER）。
    pub trust_anchors: Vec<Vec<u8>>,
    /// 可选的中间证书（DER）。
    pub intermediates: Vec<Vec<u8>>,
    /// 可选的 CRL（DER）。
    pub crls: Vec<Vec<u8>>,
    /// 验证时间（Unix 秒）；缺省使用当前系统时间，WASM 下必须显式提供。
    pub verification_time: Option<u64>,
    /// 是否要求每个非信任锚证书都有适用的 CRL。
    pub require_revocation: bool,
    /// 是否要求终端证书带 `digitalSignature` 密钥用法。
    pub require_digital_signature: bool,
    /// 证书签名使用的 SM2 签名者标识；缺省使用 GB/T 32918 规定值。
    pub dist_id: Option<String>,
}

impl Default for CertificatePolicy {
    fn default() -> Self {
        Self {
            trust_anchors: Vec::new(),
            intermediates: Vec::new(),
            crls: Vec::new(),
            verification_time: None,
            require_revocation: false,
            require_digital_signature: true,
            dist_id: None,
        }
    }
}

impl CertificatePolicy {
    /// 创建空策略。
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加信任锚证书。
    pub fn with_trust_anchor(mut self, certificate_der: impl Into<Vec<u8>>) -> Self {
        self.trust_anchors.push(certificate_der.into());
        self
    }

    /// 添加中间证书。
    pub fn with_intermediate(mut self, certificate_der: impl Into<Vec<u8>>) -> Self {
        self.intermediates.push(certificate_der.into());
        self
    }

    /// 添加 CRL。
    pub fn with_crl(mut self, crl_der: impl Into<Vec<u8>>) -> Self {
        self.crls.push(crl_der.into());
        self
    }

    /// 指定验证时间（Unix 秒）。
    pub fn at_time(mut self, unix_seconds: u64) -> Self {
        self.verification_time = Some(unix_seconds);
        self
    }

    /// 要求提供可用的吊销信息。
    pub fn with_revocation_required(mut self) -> Self {
        self.require_revocation = true;
        self
    }

    /// 指定证书签名的 SM2 签名者标识。
    pub fn with_dist_id(mut self, dist_id: impl Into<String>) -> Self {
        self.dist_id = Some(dist_id.into());
        self
    }
}

/// 证书验证结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CertificateValidation {
    /// 证书链长度，包含终端证书与信任锚。
    pub chain_length: usize,
    /// 是否对所有非信任锚证书完成了 CRL 吊销检查。
    pub revocation_checked: bool,
}

/// 使用策略中的验证时间验证证书链。
pub fn validate_certificate(
    certificate_der: &[u8],
    policy: &CertificatePolicy,
) -> Result<CertificateValidation, SealError> {
    validate_certificate_at(certificate_der, policy, policy.verification_time)
}

/// 在指定时间验证证书链。
///
/// 校验签名链、有效期、BasicConstraints、KeyUsage、路径长度以及可用的 CRL。
/// 只支持 SM2+SM3 的证书与 CRL 签名，不处理名称约束与证书策略扩展。
pub fn validate_certificate_at(
    certificate_der: &[u8],
    policy: &CertificatePolicy,
    verification_time: Option<u64>,
) -> Result<CertificateValidation, SealError> {
    let now = resolve_time(verification_time)?;
    let leaf = (certificate_der.to_vec(), decode(certificate_der)?);
    let anchors = parse_certificates(&policy.trust_anchors)?;
    let intermediates = parse_certificates(&policy.intermediates)?;
    let crls = policy
        .crls
        .iter()
        .map(|der| CertificateList::from_der(der).map_err(|_| SealError::InvalidCertificate))
        .collect::<Result<Vec<_>, _>>()?;
    let dist_id = policy.dist_id.as_deref().unwrap_or(DEFAULT_DIST_ID);

    let chain = build_chain(leaf, &anchors, &intermediates, dist_id)?;
    check_path(&chain, &anchors, now, policy)?;
    let revocation_checked = check_revocation(&chain, &crls, now, policy, dist_id)?;

    Ok(CertificateValidation {
        chain_length: chain.len(),
        revocation_checked,
    })
}

fn decode(der: &[u8]) -> Result<Certificate, SealError> {
    Certificate::from_der(der).map_err(|_| SealError::InvalidCertificate)
}

fn parse_certificates(certificates: &[Vec<u8>]) -> Result<Vec<(Vec<u8>, Certificate)>, SealError> {
    certificates
        .iter()
        .map(|der| Ok((der.clone(), decode(der)?)))
        .collect()
}

fn build_chain(
    leaf: (Vec<u8>, Certificate),
    anchors: &[(Vec<u8>, Certificate)],
    intermediates: &[(Vec<u8>, Certificate)],
    dist_id: &str,
) -> Result<Vec<(Vec<u8>, Certificate)>, SealError> {
    let mut chain = vec![leaf];
    if is_anchor(&chain[0].0, anchors) {
        return Ok(chain);
    }
    for _ in 0..MAX_CHAIN_LENGTH {
        let current = chain
            .last()
            .cloned()
            .ok_or(SealError::InvalidCertificateChain)?;
        let mut matched_name = false;
        let mut next = None;
        for candidate in intermediates.iter().chain(anchors.iter()) {
            if candidate.1.tbs_certificate().subject() != current.1.tbs_certificate().issuer() {
                continue;
            }
            matched_name = true;
            match verify_certificate_signature(&current.1, &candidate.1, dist_id) {
                Ok(()) => {
                    if chain.iter().any(|(der, _)| der == &candidate.0) {
                        return Err(SealError::InvalidCertificateChain);
                    }
                    next = Some(candidate.clone());
                    break;
                }
                Err(error @ SealError::UnsupportedCertificateAlgorithm(_)) => return Err(error),
                Err(_) => {}
            }
        }
        match next {
            Some(candidate) => {
                let anchored = is_anchor(&candidate.0, anchors);
                chain.push(candidate);
                if anchored {
                    return Ok(chain);
                }
            }
            None if matched_name => return Err(SealError::CertificateSignatureInvalid),
            None => return Err(SealError::UntrustedCertificate),
        }
    }
    Err(SealError::InvalidCertificateChain)
}

fn is_anchor(der: &[u8], anchors: &[(Vec<u8>, Certificate)]) -> bool {
    anchors.iter().any(|(anchor, _)| anchor == der)
}

fn verify_certificate_signature(
    child: &Certificate,
    issuer: &Certificate,
    dist_id: &str,
) -> Result<(), SealError> {
    let algorithm = child.signature_algorithm().oid.to_string();
    if algorithm != SM2_SM3_SIGNATURE_OID {
        return Err(SealError::UnsupportedCertificateAlgorithm(algorithm));
    }
    if child.tbs_certificate().signature().oid.to_string() != algorithm {
        return Err(SealError::InvalidCertificate);
    }
    let message = child
        .tbs_certificate()
        .to_der()
        .map_err(|_| SealError::InvalidCertificate)?;
    let public_key = certificate_public_key(issuer)?;
    verify_sm2(public_key, &message, child.signature().raw_bytes(), dist_id)
        .map_err(|_| SealError::CertificateSignatureInvalid)
}

fn certificate_public_key(certificate: &Certificate) -> Result<&[u8], SealError> {
    let key = certificate
        .tbs_certificate()
        .subject_public_key_info()
        .subject_public_key
        .raw_bytes();
    if key.is_empty() {
        return Err(SealError::InvalidCertificate);
    }
    Ok(key)
}

fn check_path(
    chain: &[(Vec<u8>, Certificate)],
    anchors: &[(Vec<u8>, Certificate)],
    now: u64,
    policy: &CertificatePolicy,
) -> Result<(), SealError> {
    for (index, (der, certificate)) in chain.iter().enumerate() {
        let tbs = certificate.tbs_certificate();
        let anchored = is_anchor(der, anchors);
        if !anchored {
            let not_before = tbs.validity().not_before.to_unix_duration().as_secs();
            let not_after = tbs.validity().not_after.to_unix_duration().as_secs();
            if now < not_before {
                return Err(SealError::CertificateNotYetValid);
            }
            if now > not_after {
                return Err(SealError::CertificateExpired);
            }
        }
        let basic = tbs
            .get_extension::<BasicConstraints>()
            .map_err(|_| SealError::InvalidCertificate)?;
        let usage = tbs
            .get_extension::<KeyUsage>()
            .map_err(|_| SealError::InvalidCertificate)?;

        if index == 0 {
            if let Some((_, basic)) = &basic
                && basic.ca
            {
                return Err(SealError::InvalidCertificateChain);
            }
            if policy.require_digital_signature {
                let Some((_, usage)) = &usage else {
                    return Err(SealError::CertificateKeyUsage);
                };
                if !usage.digital_signature() {
                    return Err(SealError::CertificateKeyUsage);
                }
            }
            continue;
        }

        match &basic {
            Some((_, basic)) if !basic.ca => return Err(SealError::InvalidCertificateChain),
            None if !anchored => return Err(SealError::InvalidCertificateChain),
            _ => {}
        }
        if let Some((_, usage)) = &usage
            && !usage.key_cert_sign()
        {
            return Err(SealError::CertificateKeyUsage);
        }
        if let Some((_, basic)) = &basic
            && let Some(limit) = basic.path_len_constraint
        {
            let below = (index - 1) as u16;
            if below > u16::from(limit) {
                return Err(SealError::CertificatePathLength);
            }
        }
    }
    Ok(())
}

fn check_revocation(
    chain: &[(Vec<u8>, Certificate)],
    crls: &[CertificateList],
    now: u64,
    policy: &CertificatePolicy,
    dist_id: &str,
) -> Result<bool, SealError> {
    let mut checked = true;
    for index in 0..chain.len().saturating_sub(1) {
        let certificate = &chain[index].1;
        let issuer = &chain[index + 1].1;
        let applicable = crls.iter().find(|crl| {
            &crl.tbs_cert_list.issuer == certificate.tbs_certificate().issuer()
                && verify_crl(crl, issuer, now, dist_id).is_ok()
        });
        let Some(crl) = applicable else {
            checked = false;
            if policy.require_revocation {
                return Err(SealError::RevocationUnknown);
            }
            continue;
        };
        if let Some(revoked) = &crl.tbs_cert_list.revoked_certificates {
            let serial = certificate.tbs_certificate().serial_number();
            for entry in revoked {
                if &entry.serial_number == serial
                    && entry.revocation_date.to_unix_duration().as_secs() <= now
                {
                    return Err(SealError::CertificateRevoked);
                }
            }
        }
    }
    Ok(checked)
}

fn verify_crl(
    crl: &CertificateList,
    issuer: &Certificate,
    now: u64,
    dist_id: &str,
) -> Result<(), SealError> {
    let this_update = crl.tbs_cert_list.this_update.to_unix_duration().as_secs();
    if now < this_update {
        return Err(SealError::RevocationUnknown);
    }
    if let Some(next_update) = crl.tbs_cert_list.next_update
        && now > next_update.to_unix_duration().as_secs()
    {
        return Err(SealError::RevocationUnknown);
    }
    let algorithm = crl.signature_algorithm.oid.to_string();
    if algorithm != SM2_SM3_SIGNATURE_OID {
        return Err(SealError::UnsupportedCertificateAlgorithm(algorithm));
    }
    if crl.tbs_cert_list.signature.oid.to_string() != algorithm {
        return Err(SealError::RevocationUnknown);
    }
    let usage = issuer
        .tbs_certificate()
        .get_extension::<KeyUsage>()
        .map_err(|_| SealError::InvalidCertificate)?;
    if let Some((_, usage)) = usage
        && !usage.crl_sign()
    {
        return Err(SealError::CertificateKeyUsage);
    }
    let message = crl
        .tbs_cert_list
        .to_der()
        .map_err(|_| SealError::RevocationUnknown)?;
    verify_sm2(
        certificate_public_key(issuer)?,
        &message,
        crl.signature.raw_bytes(),
        dist_id,
    )
    .map_err(|_| SealError::RevocationUnknown)
}
