//! 用 SM2 生成 GB/T 38540 `SES_Signature` 的签名值生产者。

use ofdbox_core::{OfdError, SignedValueProducer, SigningContext};
use ofdbox_crypto::{sign_sm3, sm3_digest};
use zeroize::Zeroizing;

use crate::der;
use crate::error::SealError;
use crate::eseal::parse::parse_seal_for_display;
use crate::fingerprint::SealFormat;
use crate::oids::SM2_SM3_SIGNATURE_OID;

/// SM2 with SM3 的 DER OID 内容（1.2.156.10197.1.501）。
const OID_SM2_SM3: &[u8] = &[0x2A, 0x81, 0x1C, 0xCF, 0x55, 0x01, 0x83, 0x75];

/// 用已有电子印章（`SESeal`）与签章者证书/私钥生成 `SES_Signature`。
pub struct Sm2SesSignatureProducer {
    seal: Vec<u8>,
    version: i64,
    certificate: Vec<u8>,
    private_key: Zeroizing<Vec<u8>>,
    dist_id: String,
}

impl Sm2SesSignatureProducer {
    /// 从印章数据与签章者证书、私钥创建生产者。
    ///
    /// 印章数据的版本号会写入 `TBS_Sign.version`。
    pub fn new(
        seal: impl Into<Vec<u8>>,
        certificate: impl Into<Vec<u8>>,
        private_key: impl Into<Vec<u8>>,
    ) -> Result<Self, SealError> {
        let seal = seal.into();
        let display = parse_seal_for_display(&seal)?;
        if display.format() == SealFormat::Unknown {
            return Err(SealError::UnsupportedFormat);
        }
        Ok(Self {
            version: display.header.version,
            seal,
            certificate: certificate.into(),
            private_key: Zeroizing::new(private_key.into()),
            dist_id: ofdbox_crypto::DEFAULT_DIST_ID.to_owned(),
        })
    }

    /// 指定 SM2 签名者标识。
    pub fn with_dist_id(mut self, dist_id: impl Into<String>) -> Self {
        self.dist_id = dist_id.into();
        self
    }
}

impl SignedValueProducer for Sm2SesSignatureProducer {
    fn supports(&self, signature_method: &str) -> bool {
        signature_method == SM2_SM3_SIGNATURE_OID
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
        let time = generalized_time(context.signed_at).ok_or_else(|| {
            OfdError::InvalidStructure("签章时间需要 14 位 YYYYMMDDHHMMSS".to_owned())
        })?;
        if !(0..=127).contains(&self.version) {
            return Err(OfdError::InvalidStructure(
                "印章版本号超出可编码范围".to_owned(),
            ));
        }

        let version = der::wrap(0x02, &[self.version as u8]);
        let mut hash = vec![0u8];
        hash.extend_from_slice(&sm3_digest(context.content));
        let hash = der::wrap(0x03, &hash);
        let property = der::wrap(0x16, context.signed_entry.as_bytes());
        let time = der::wrap(0x18, time.as_bytes());

        let mut tbs_content = Vec::new();
        tbs_content.extend(version);
        tbs_content.extend_from_slice(&self.seal);
        tbs_content.extend(time);
        tbs_content.extend(hash);
        tbs_content.extend(property);
        let tbs = der::wrap(0x30, &tbs_content);

        let signature = sign_sm3(&self.private_key, &tbs, &self.dist_id)
            .map_err(|error| OfdError::Crypto(error.to_string()))?;
        let mut signature_value = vec![0u8];
        signature_value.extend(signature);

        let mut content = Vec::new();
        content.extend(tbs);
        content.extend(der::wrap(0x04, &self.certificate));
        content.extend(der::wrap(0x06, OID_SM2_SM3));
        content.extend(der::wrap(0x03, &signature_value));
        Ok(der::wrap(0x30, &content))
    }
}

/// 把 14 位 `YYYYMMDDHHMMSS` 转成 GeneralizedTime 的 `YYYYMMDDHHMMSSZ`。
fn generalized_time(signed_at: &str) -> Option<String> {
    let digits: String = signed_at
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
    if digits.len() == 14 {
        Some(format!("{digits}Z"))
    } else {
        None
    }
}
