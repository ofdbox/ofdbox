//! GM/T 0031—2014 电子签章值（`SES_sign`）的解析、验证与生成。

use std::sync::OnceLock;

use ofdbox_algo::{
    AlgorithmError, AlgorithmRegistry, PublicKeyRef, RawSignatureAlgorithm, SM2_CURVE_OID,
    SM2_SM3_SIGN_OID, SecretKeyRef, SignatureParams,
};
use ofdbox_core::{
    OfdError, SignatureVerifier, SignedValueFormat, SignedValueProducer, SigningContext,
};
use ofdbox_crypto::{DEFAULT_DIST_ID, public_key_from_certificate, sm3_digest};
use zeroize::Zeroizing;

use crate::certificate::CertificatePolicy;
use crate::der;
use crate::error::SealError;
use crate::eseal::encoding::{bit_string, ia5, integer};
use crate::eseal::parse::{identify_seal_format, parse_seal_for_display};
use crate::eseal::verify::{SealVerifyOptions, signer_in_seal, verify_seal_with_options};
use crate::fingerprint::SealFormat;

/// 解析后的 0031 签章值。
#[derive(Clone, Debug)]
pub struct GmT0031Signature<'a> {
    /// 签章版本。
    pub version: i64,
    /// 内嵌 `SESeal` 完整 DER。
    pub seal: &'a [u8],
    /// 签章时间文本。
    pub time_info: &'a str,
    /// 原文杂凑值。
    pub data_hash: &'a [u8],
    /// 原文属性。
    pub property_info: &'a str,
    /// 签章者证书 DER。
    pub certificate: &'a [u8],
    /// 签名算法 OID。
    pub algorithm: String,
    /// 签名值。
    pub signature: &'a [u8],
    /// `SES_SignInfo` 完整 DER，验签对象。
    pub tbs: &'a [u8],
}

/// 验证参数。
#[derive(Clone, Copy, Debug)]
pub struct GmT0031VerifyOptions<'a> {
    /// SM2 签名者标识。
    pub dist_id: &'a str,
    /// 证书链验证策略。
    pub certificate: Option<&'a CertificatePolicy>,
    /// 是否验证内嵌印章。
    pub check_seal: bool,
    /// 是否要求签章者证书在印章列表中。
    pub check_signer: bool,
    /// 算法注册表。
    pub algorithms: Option<&'a AlgorithmRegistry>,
}

impl<'a> Default for GmT0031VerifyOptions<'a> {
    fn default() -> Self {
        Self {
            dist_id: DEFAULT_DIST_ID,
            certificate: None,
            check_seal: true,
            check_signer: true,
            algorithms: None,
        }
    }
}

/// 验证结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GmT0031Verification {
    /// 签章时间。
    pub time_info: String,
    /// 内嵌印章格式。
    pub seal_format: Option<SealFormat>,
    /// 签章者证书是否在印章列表中。
    pub signer_in_seal: bool,
}

/// 解析 0031 签章值。
pub fn parse_gmt0031_signature(bytes: &[u8]) -> Result<GmT0031Signature<'_>, SealError> {
    let (root, end) = der::parse_tlv(bytes, 0, bytes.len())?;
    if !bytes[end..].iter().all(|byte| *byte == 0) {
        return Err(SealError::Invalid("签章数据末尾存在多余字节"));
    }
    der::expect_tag(root, der::TAG_SEQUENCE, "签章数据根节点不是 SEQUENCE")?;
    let outer = der::items(root)?;
    if outer.len() != 2 {
        return Err(SealError::Invalid("0031 签章值应为 signInfo + signature"));
    }
    der::expect_tag(outer[0], der::TAG_SEQUENCE, "signInfo 不是 SEQUENCE")?;
    let fields = der::items(outer[0])?;
    if fields.len() < 7 {
        return Err(SealError::Invalid("SES_SignInfo 字段不足"));
    }

    let version = der::integer(fields[0])?;
    der::expect_tag(fields[1], der::TAG_SEQUENCE, "印章信息不是 SEQUENCE")?;
    let time_info = der::bit_string_content(fields[2])
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .ok_or(SealError::Invalid("签章时间无效"))?;
    let data_hash =
        der::bit_string_content(fields[3]).ok_or(SealError::Invalid("原文杂凑值无效"))?;
    if fields[4].tag != der::TAG_IA5_STRING {
        return Err(SealError::Invalid("原文属性不是 IA5String"));
    }
    let property_info =
        std::str::from_utf8(fields[4].value).map_err(|_| SealError::Invalid("原文属性无效"))?;
    let certificate = der::octet_string(fields[5], "签章者证书无效")?;
    let algorithm = der::oid(fields[6])?;
    let signature =
        der::bit_string_content(outer[1]).ok_or(SealError::Invalid("签章签名值无效"))?;

    Ok(GmT0031Signature {
        version,
        seal: fields[1].full,
        time_info,
        data_hash,
        property_info,
        certificate,
        algorithm,
        signature,
        tbs: outer[0].full,
    })
}

/// 验证 0031 签章值对 `content` 的签章。
pub fn verify_gmt0031_signature(
    signed_value: &[u8],
    content: &[u8],
    options: &GmT0031VerifyOptions<'_>,
) -> Result<GmT0031Verification, SealError> {
    let signature = parse_gmt0031_signature(signed_value)?;
    if sm3_digest(content).as_slice() != signature.data_hash {
        return Err(SealError::SignatureInvalid);
    }

    let algorithms = match options.algorithms {
        Some(algorithms) => algorithms,
        None => default_algorithms(),
    };
    let public_key = public_key_from_certificate(signature.certificate)
        .map_err(|_| SealError::InvalidCertificate)?;
    let key = PublicKeyRef::new(SM2_CURVE_OID, &public_key);
    let params = SignatureParams {
        dist_id: options.dist_id,
    };
    let verified = if algorithms.suite(&signature.algorithm).is_ok() {
        algorithms.verify_suite(
            &signature.algorithm,
            &key,
            signature.tbs,
            signature.signature,
            &params,
        )
    } else {
        algorithms
            .signature(&signature.algorithm)
            .and_then(|algorithm| {
                algorithm.verify(&key, signature.tbs, signature.signature, &params)
            })
    };
    verified.map_err(|error| map_algorithm_error(error, &signature.algorithm))?;

    let mut result = GmT0031Verification {
        time_info: signature.time_info.to_owned(),
        seal_format: None,
        signer_in_seal: false,
    };
    if options.check_seal {
        let format = identify_seal_format(signature.seal)?;
        if format != SealFormat::GmT0031 {
            return Err(SealError::FormatMismatch {
                expected: SealFormat::GmT0031,
                actual: format,
            });
        }
        let check_signer = if options.check_signer {
            match signer_in_seal(signature.seal, signature.certificate) {
                Ok(true) => true,
                Ok(false) => return Err(SealError::SignerNotInSeal),
                Err(SealError::UnsupportedCertificateListType(_)) => false,
                Err(error) => return Err(error),
            }
        } else {
            false
        };
        let seal_options = SealVerifyOptions {
            dist_id: options.dist_id,
            certificate: options.certificate,
            check_seal_validity: false,
            verification_time: None,
            signer_certificate: check_signer.then_some(signature.certificate),
        };
        let verification = verify_seal_with_options(signature.seal, format, &seal_options)?;
        result.seal_format = Some(verification.format);
        result.signer_in_seal = check_signer;
    }
    Ok(result)
}

fn default_algorithms() -> &'static AlgorithmRegistry {
    static DEFAULT: OnceLock<AlgorithmRegistry> = OnceLock::new();
    DEFAULT.get_or_init(AlgorithmRegistry::gm_default)
}

fn map_algorithm_error(error: AlgorithmError, oid: &str) -> SealError {
    match error {
        AlgorithmError::UnknownAlgorithm { .. } | AlgorithmError::RoleMismatch { .. } => {
            SealError::UnsupportedAlgorithm(oid.to_owned())
        }
        AlgorithmError::InvalidKey => SealError::InvalidCertificate,
        _ => SealError::SignatureInvalid,
    }
}

/// 用 SM2 + SM3 生成 0031 签章值。
pub struct Sm2GmT0031SignatureProducer {
    seal: Vec<u8>,
    version: i64,
    certificate: Vec<u8>,
    private_key: Zeroizing<Vec<u8>>,
    dist_id: String,
}

impl Sm2GmT0031SignatureProducer {
    /// 从印章、签章者证书与私钥创建生产者。
    pub fn new(
        seal: impl Into<Vec<u8>>,
        certificate: impl Into<Vec<u8>>,
        private_key: impl Into<Vec<u8>>,
    ) -> Result<Self, SealError> {
        let seal = seal.into();
        let display = parse_seal_for_display(&seal)?;
        if display.format() != SealFormat::GmT0031 {
            return Err(SealError::UnsupportedFormat);
        }
        Ok(Self {
            version: display.header.version,
            seal,
            certificate: certificate.into(),
            private_key: Zeroizing::new(private_key.into()),
            dist_id: DEFAULT_DIST_ID.to_owned(),
        })
    }

    /// 指定 SM2 签名者标识。
    pub fn with_dist_id(mut self, dist_id: impl Into<String>) -> Self {
        self.dist_id = dist_id.into();
        self
    }
}

impl SignedValueProducer for Sm2GmT0031SignatureProducer {
    fn supports(&self, signature_method: &str) -> bool {
        signature_method == SM2_SM3_SIGN_OID
            || signature_method == "1.2.156.10197.1.301.1"
            || signature_method.contains("SM2")
    }

    fn digest(&self, data: &[u8]) -> Result<Vec<u8>, OfdError> {
        Ok(sm3_digest(data).to_vec())
    }

    fn signature_type(&self) -> &str {
        "Seal"
    }

    fn produce(&self, context: &SigningContext<'_>) -> Result<Vec<u8>, OfdError> {
        let time_info = time_info(context.signed_at).ok_or_else(|| {
            OfdError::InvalidStructure("签章时间需要 14 位 YYYYMMDDHHMMSS".to_owned())
        })?;
        let mut fields = Vec::new();
        fields.extend(
            integer(self.version, "签章版本号")
                .map_err(|error| OfdError::InvalidStructure(error.to_string()))?,
        );
        fields.extend_from_slice(&self.seal);
        fields.extend(bit_string(time_info.as_bytes()));
        fields.extend(bit_string(&sm3_digest(context.content)));
        fields.extend(ia5(context.signed_entry));
        fields.extend(der::wrap(0x04, &self.certificate));
        fields.extend(der::wrap(0x06, sm2_sm3_oid_content()));
        let tbs = der::wrap(der::TAG_SEQUENCE, &fields);

        let key = SecretKeyRef::new(SM2_CURVE_OID, &self.private_key);
        let params = SignatureParams {
            dist_id: &self.dist_id,
        };
        let signature = ofdbox_algo::Sm2RawSignature
            .sign(&key, &tbs, &params)
            .map_err(|error| OfdError::Crypto(error.to_string()))?;
        let mut content = tbs;
        content.extend(bit_string(&signature));
        Ok(der::wrap(der::TAG_SEQUENCE, &content))
    }
}

fn time_info(signed_at: &str) -> Option<String> {
    let digits: String = signed_at
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
    if digits.len() != 14 {
        return None;
    }
    Some(format!(
        "{}-{}-{}T{}:{}:{}.000Z",
        &digits[0..4],
        &digits[4..6],
        &digits[6..8],
        &digits[8..10],
        &digits[10..12],
        &digits[12..14]
    ))
}

fn sm2_sm3_oid_content() -> &'static [u8] {
    &[0x2A, 0x81, 0x1C, 0xCF, 0x55, 0x01, 0x83, 0x75]
}

/// 0031 签章值验签器。
pub struct GmT0031SignatureVerifier {
    dist_id: String,
    certificate_policy: Option<CertificatePolicy>,
    algorithms: AlgorithmRegistry,
    check_seal: bool,
    check_signer: bool,
}

impl GmT0031SignatureVerifier {
    /// 使用默认参数创建验签器。
    pub fn new() -> Self {
        Self {
            dist_id: DEFAULT_DIST_ID.to_owned(),
            certificate_policy: None,
            algorithms: AlgorithmRegistry::gm_default(),
            check_seal: true,
            check_signer: true,
        }
    }

    /// 指定算法注册表。
    pub fn with_algorithms(mut self, algorithms: AlgorithmRegistry) -> Self {
        self.algorithms = algorithms;
        self
    }

    /// 指定证书链策略。
    pub fn with_certificate_policy(mut self, policy: CertificatePolicy) -> Self {
        self.certificate_policy = Some(policy);
        self
    }
}

impl Default for GmT0031SignatureVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl SignatureVerifier for GmT0031SignatureVerifier {
    fn format(&self) -> SignedValueFormat {
        SignedValueFormat::GmT0031Signature
    }

    fn verify(
        &self,
        _signature_method: &str,
        content: &[u8],
        signed_value: &[u8],
        algorithms: &AlgorithmRegistry,
    ) -> Result<(), OfdError> {
        let options = GmT0031VerifyOptions {
            dist_id: &self.dist_id,
            certificate: self.certificate_policy.as_ref(),
            check_seal: self.check_seal,
            check_signer: self.check_signer,
            algorithms: Some(algorithms),
        };
        verify_gmt0031_signature(signed_value, content, &options)
            .map(|_| ())
            .map_err(|error| OfdError::Crypto(error.to_string()))
    }
}
