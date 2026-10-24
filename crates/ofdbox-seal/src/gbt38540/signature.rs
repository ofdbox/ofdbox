//! GB/T 38540 电子签章数据（`SES_Signature`）的解析与验证。
//!
//! 结构见 38540 7.1：
//!
//! ```text
//! SES_Signature ::= SEQUENCE {
//!     toSign         TBS_Sign,
//!     cert           OCTET STRING,          -- 签章者证书
//!     signatureAlgID OBJECT IDENTIFIER,
//!     signature      BIT STRING,            -- 对 TBS_Sign 的签名
//!     timeStamp  [0] BIT STRING OPTIONAL
//! }
//! TBS_Sign ::= SEQUENCE {
//!     version     INTEGER,
//!     eseal       SESeal,
//!     timeInfo    GeneralizedTime,
//!     dataHash    BIT STRING,               -- 原文杂凑值
//!     propertyInfo IA5String,               -- 原文属性，含签名保护范围
//!     extDatas   [0] OPTIONAL
//! }
//! ```

use ofdbox_core::{OfdError, SignatureVerifier, SignedValueFormat};
use ofdbox_crypto::{DEFAULT_DIST_ID, public_key_from_certificate, sm3_digest};

use crate::certificate::CertificatePolicy;
use crate::der::{self, TAG_SEQUENCE};
use crate::error::SealError;
use crate::eseal::parse::identify_seal_format;
use crate::eseal::verify::{SealVerifyOptions, verify_seal_with_options};
use crate::fingerprint::SealFormat;
use crate::oids::SM2_SM3_SIGNATURE_OID;
use crate::signature::verify_sm2;
use crate::timestamp::{TimeStampVerification, TimeStampVerifyOptions, verify_time_stamp_token};

/// `SES_Signature` 的类型化视图。
#[derive(Clone, Debug)]
pub struct SesSignature<'a> {
    /// 签章版本号，与电子印章版本一致。
    pub version: i64,
    /// 内嵌的电子印章数据（`SESeal` 完整 TLV）。
    pub seal: &'a [u8],
    /// 签章时间。
    pub time_info: &'a str,
    /// 原文杂凑值。
    pub data_hash: &'a [u8],
    /// 原文属性，通常是被签文件的包内路径。
    pub property_info: &'a str,
    /// 签章者证书（DER）。
    pub certificate: &'a [u8],
    /// 签名算法标识。
    pub algorithm: String,
    /// 对 `TBS_Sign` 的签名值。
    pub signature: &'a [u8],
    /// `TBS_Sign` 的完整 DER，验签对象。
    pub tbs: &'a [u8],
    /// `timeStamp [0]` 中的时间戳令牌 DER；未携带时为 `None`。
    pub time_stamp: Option<&'a [u8]>,
}

/// 解析 `SES_Signature`。
pub fn parse_ses_signature(bytes: &[u8]) -> Result<SesSignature<'_>, SealError> {
    let (root, end) = der::parse_tlv(bytes, 0, bytes.len())?;
    // 真实文件会在签名值末尾补零；只容忍全零尾随字节。
    if end != bytes.len() && !bytes[end..].iter().all(|byte| *byte == 0) {
        return Err(SealError::Invalid("签章数据末尾存在多余字节"));
    }
    der::expect_tag(root, TAG_SEQUENCE, "签章数据根节点不是 SEQUENCE")?;
    let items = der::items(root)?;
    if items.len() < 4 || items.len() > 5 {
        return Err(SealError::Invalid("签章数据子节点数量不正确"));
    }

    let tbs = items[0];
    der::expect_tag(tbs, TAG_SEQUENCE, "签章信息不是 SEQUENCE")?;
    let certificate = der::octet_string(items[1], "签章者证书无效")?;
    let algorithm = der::oid(items[2])?;
    let signature =
        der::bit_string_content(items[3]).ok_or(SealError::Invalid("签章签名值无效"))?;
    let time_stamp = match items.get(4) {
        Some(item) => {
            der::expect_tag(*item, 0xA0, "签章时间戳不是 [0] 显式标签")?;
            let mut children = item.children()?;
            let value = children
                .next()
                .ok_or(SealError::Invalid("签章时间戳为空"))??;
            let token = der::bit_string_content(value)
                .ok_or(SealError::Invalid("签章时间戳不是 BIT STRING"))?;
            if children.next().is_some() {
                return Err(SealError::Invalid("签章时间戳包含多余字段"));
            }
            Some(token)
        }
        None => None,
    };

    let fields = der::items(tbs)?;
    if fields.len() < 5 {
        return Err(SealError::Invalid("签章信息字段不足"));
    }
    let version = der::integer(fields[0])?;
    der::expect_tag(fields[1], TAG_SEQUENCE, "内嵌电子印章无效")?;
    let (time_info, _) = der::time(fields[2])?;
    let data_hash =
        der::bit_string_content(fields[3]).ok_or(SealError::Invalid("原文杂凑值无效"))?;
    let property_info = der::ia5(fields[4], "原文属性无效")?;

    Ok(SesSignature {
        version,
        seal: fields[1].full,
        time_info,
        data_hash,
        property_info,
        certificate,
        algorithm,
        signature,
        tbs: tbs.full,
        time_stamp,
    })
}

/// 签章验证参数。
#[derive(Clone, Copy, Debug)]
pub struct SesVerifyOptions<'a> {
    /// SM2 签名者标识。
    pub dist_id: &'a str,
    /// 证书链验证策略；不提供时只验证签名。
    pub certificate: Option<&'a CertificatePolicy>,
    /// 验证时间（Unix 秒）。
    pub verification_time: Option<u64>,
    /// 是否验证内嵌电子印章的制章者签名。
    pub check_seal: bool,
    /// 是否要求签章者证书出现在印章的签章者证书列表中。
    pub check_signer: bool,
    /// 是否验证 `timeStamp` 时间戳令牌。
    pub check_timestamp: bool,
    /// TSA 证书链验证策略；提供时会校验 `timeStamping` EKU。
    pub timestamp_certificate: Option<&'a CertificatePolicy>,
}

impl<'a> Default for SesVerifyOptions<'a> {
    fn default() -> Self {
        Self {
            dist_id: DEFAULT_DIST_ID,
            certificate: None,
            verification_time: None,
            check_seal: true,
            check_signer: true,
            check_timestamp: false,
            timestamp_certificate: None,
        }
    }
}

impl<'a> SesVerifyOptions<'a> {
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

    /// 关闭内嵌印章验证。
    pub fn without_seal_check(mut self) -> Self {
        self.check_seal = false;
        self
    }

    /// 关闭签章者证书匹配。
    pub fn without_signer_check(mut self) -> Self {
        self.check_signer = false;
        self
    }

    /// 启用 `timeStamp` 时间戳令牌验证。
    pub fn with_timestamp_check(mut self) -> Self {
        self.check_timestamp = true;
        self
    }

    /// 启用时间戳验证，并指定 TSA 证书链策略。
    pub fn with_timestamp_certificate_policy(mut self, policy: &'a CertificatePolicy) -> Self {
        self.check_timestamp = true;
        self.timestamp_certificate = Some(policy);
        self
    }
}

/// 签章验证结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SesVerification {
    /// 签章时间。
    pub time_info: String,
    /// 内嵌印章的格式；未启用印章验证时为 `None`。
    pub seal_format: Option<SealFormat>,
    /// 是否已确认签章者证书出现在印章列表中。
    pub signer_in_seal: bool,
    /// 时间戳验证结果；未启用或未携带时间戳时为 `None`。
    pub timestamp: Option<TimeStampVerification>,
}

/// 验证一个 `SES_Signature` 对 `content`（通常是 `Signature.xml` 字节）的签章。
pub fn verify_ses_signature(
    signed_value: &[u8],
    content: &[u8],
    options: &SesVerifyOptions<'_>,
) -> Result<SesVerification, SealError> {
    let signature = parse_ses_signature(signed_value)?;
    if signature.algorithm != SM2_SM3_SIGNATURE_OID
        && signature.algorithm != "1.2.156.10197.1.301.1"
    {
        return Err(SealError::UnsupportedAlgorithm(signature.algorithm.clone()));
    }

    // 原文杂凑值应等于签名对象的摘要（38540 7.1.2）。
    if sm3_digest(content).as_slice() != signature.data_hash {
        return Err(SealError::SignatureInvalid);
    }

    let public_key = public_key_from_certificate(signature.certificate)
        .map_err(|_| SealError::InvalidCertificate)?;
    verify_sm2(
        &public_key,
        signature.tbs,
        signature.signature,
        options.dist_id,
    )
    .map_err(|_| SealError::SignatureInvalid)?;

    let mut result = SesVerification {
        time_info: signature.time_info.to_owned(),
        seal_format: None,
        signer_in_seal: false,
        timestamp: None,
    };
    if options.check_seal {
        let format = identify_seal_format(signature.seal)?;
        if format == SealFormat::Unknown {
            return Err(SealError::UnsupportedFormat);
        }
        // 38540 只定义类型 1（证书）与 2（杂凑值）；厂商扩展类型无法比对时
        // 跳过“签章者证书在列表中”的检查，但仍验证印章签名。
        let check_signer = if options.check_signer {
            match crate::eseal::verify::signer_in_seal(signature.seal, signature.certificate) {
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
            verification_time: options.verification_time,
            signer_certificate: check_signer.then_some(signature.certificate),
        };
        let verification = verify_seal_with_options(signature.seal, format, &seal_options)?;
        result.seal_format = Some(verification.format);
        result.signer_in_seal = check_signer;
    }
    if options.check_timestamp
        && let Some(token) = signature.time_stamp
    {
        result.timestamp = Some(verify_time_stamp_token(
            token,
            signature.signature,
            &TimeStampVerifyOptions {
                dist_id: options.dist_id,
                certificate: options.timestamp_certificate,
                require_time_stamping_eku: options.timestamp_certificate.is_some(),
            },
        )?);
    }
    Ok(result)
}

/// 按 GB/T 38540 验证 `SES_Signature` 的验签器。
pub struct SesSignatureVerifier {
    dist_id: String,
    certificate_policy: Option<CertificatePolicy>,
    check_seal: bool,
    check_signer: bool,
}

impl SesSignatureVerifier {
    /// 使用默认参数创建验签器。
    pub fn new() -> Self {
        Self {
            dist_id: DEFAULT_DIST_ID.to_owned(),
            certificate_policy: None,
            check_seal: true,
            check_signer: true,
        }
    }

    /// 指定 SM2 签名者标识。
    pub fn with_dist_id(mut self, dist_id: impl Into<String>) -> Self {
        self.dist_id = dist_id.into();
        self
    }

    /// 附加证书链验证策略。
    pub fn with_certificate_policy(mut self, policy: CertificatePolicy) -> Self {
        self.certificate_policy = Some(policy);
        self
    }

    /// 关闭内嵌印章验证。
    pub fn without_seal_check(mut self) -> Self {
        self.check_seal = false;
        self
    }

    /// 关闭签章者证书匹配。
    pub fn without_signer_check(mut self) -> Self {
        self.check_signer = false;
        self
    }
}

impl Default for SesSignatureVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl SignatureVerifier for SesSignatureVerifier {
    fn format(&self) -> SignedValueFormat {
        SignedValueFormat::SesSignature
    }

    fn supports(&self, signature_method: &str) -> bool {
        signature_method == SM2_SM3_SIGNATURE_OID
            || signature_method == "1.2.156.10197.1.301.1"
            || signature_method.contains("SM2")
    }

    fn verify(
        &self,
        _signature_method: &str,
        content: &[u8],
        signed_value: &[u8],
    ) -> Result<(), OfdError> {
        let options = SesVerifyOptions {
            dist_id: &self.dist_id,
            certificate: self.certificate_policy.as_ref(),
            verification_time: None,
            check_seal: self.check_seal,
            check_signer: self.check_signer,
            check_timestamp: false,
            timestamp_certificate: None,
        };
        verify_ses_signature(signed_value, content, &options)
            .map(|_| ())
            .map_err(|error| OfdError::Crypto(error.to_string()))
    }
}
