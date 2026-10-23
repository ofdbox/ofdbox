//! GB/T 35275 `SignedData` 签名值生成。

use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::{CmsVersion, ContentInfo};
use cms::signed_data::{
    CertificateSet, EncapsulatedContentInfo, SignedData, SignerIdentifier, SignerInfo, SignerInfos,
};
use der::asn1::{Any, AnyRef, ObjectIdentifier, OctetString, SetOfVec};
use der::{Decode, Encode, Tag};
use ofdbox_core::{DEFAULT_SIGNATURE_METHOD, OfdError, SignedValueProducer, SigningContext};
use sm3::{Digest, Sm3};
use x509_cert::Certificate;
use x509_cert::attr::{Attribute, Attributes};
use zeroize::Zeroizing;

use crate::signature::{DEFAULT_DIST_ID, sign_sm3};

const ID_DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.156.10197.6.1.4.2.1");
const ID_SIGNED_DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.156.10197.6.1.4.2.2");
const ID_CONTENT_TYPE: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.3");
const ID_MESSAGE_DIGEST: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.4");
const ID_SM3: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.156.10197.1.401");
const ID_SM2_SIGN_ALG: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.156.10197.1.301.1");

/// 用 SM2 + SM3 生成 GB/T 35275 `SignedData` 的签名值生产者。
///
/// 证书与私钥由调用方提供；私钥为 32 字节 SM2 标量。
pub struct Sm2SignedDataProducer {
    certificate_der: Vec<u8>,
    private_key: Zeroizing<Vec<u8>>,
    dist_id: String,
}

impl Sm2SignedDataProducer {
    /// 用签名者证书（DER）与私钥创建生产者。
    pub fn new(certificate_der: impl Into<Vec<u8>>, private_key: impl Into<Vec<u8>>) -> Self {
        Self {
            certificate_der: certificate_der.into(),
            private_key: Zeroizing::new(private_key.into()),
            dist_id: DEFAULT_DIST_ID.to_owned(),
        }
    }

    /// 指定 SM2 签名者标识。
    pub fn with_dist_id(mut self, dist_id: impl Into<String>) -> Self {
        self.dist_id = dist_id.into();
        self
    }
}

impl SignedValueProducer for Sm2SignedDataProducer {
    fn supports(&self, signature_method: &str) -> bool {
        matches!(
            signature_method,
            DEFAULT_SIGNATURE_METHOD | "1.2.156.10197.1.301.1" | "1.2.1"
        ) || signature_method.contains("SM2")
    }

    fn digest(&self, data: &[u8]) -> Result<Vec<u8>, OfdError> {
        Ok(Sm3::digest(data).to_vec())
    }

    fn produce(&self, context: &SigningContext<'_>) -> Result<Vec<u8>, OfdError> {
        let certificate = Certificate::from_der(&self.certificate_der)
            .map_err(|_| OfdError::Crypto("签名者证书无效".to_owned()))?;
        let message_digest = Sm3::digest(context.content);

        let content_type = Attribute {
            oid: ID_CONTENT_TYPE,
            values: SetOfVec::try_from(vec![Any::from(ID_DATA)])
                .map_err(|_| OfdError::Crypto("contentType 属性无效".to_owned()))?,
        };
        let digest = Attribute {
            oid: ID_MESSAGE_DIGEST,
            values: SetOfVec::try_from(vec![
                Any::new(Tag::OctetString, message_digest.as_slice())
                    .map_err(|_| OfdError::Crypto("messageDigest 属性无效".to_owned()))?,
            ])
            .map_err(|_| OfdError::Crypto("messageDigest 属性无效".to_owned()))?,
        };
        let mut attributes = SetOfVec::new();
        attributes
            .insert(content_type)
            .map_err(|_| OfdError::Crypto("签名属性排序失败".to_owned()))?;
        attributes
            .insert(digest)
            .map_err(|_| OfdError::Crypto("签名属性排序失败".to_owned()))?;
        let attributes: Attributes = attributes;

        // 35275 规定签名对象是 signedAttrs 的 DER（SET OF 编码）。
        let signed_bytes = attributes
            .to_der()
            .map_err(|_| OfdError::Crypto("签名属性编码失败".to_owned()))?;
        let signature = sign_sm3(&self.private_key, &signed_bytes, &self.dist_id)
            .map_err(|error| OfdError::Crypto(error.to_string()))?;

        let signer = SignerInfo {
            version: CmsVersion::V1,
            sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
                issuer: certificate.tbs_certificate().issuer().clone(),
                serial_number: certificate.tbs_certificate().serial_number().clone(),
            }),
            digest_alg: algorithm(ID_SM3),
            signed_attrs: Some(attributes),
            signature_algorithm: algorithm(ID_SM2_SIGN_ALG),
            signature: OctetString::new(signature)
                .map_err(|_| OfdError::Crypto("签名值编码失败".to_owned()))?,
            unsigned_attrs: None,
        };

        let signed_data = SignedData {
            version: CmsVersion::V1,
            digest_algorithms: SetOfVec::try_from(vec![algorithm(ID_SM3)])
                .map_err(|_| OfdError::Crypto("摘要算法编码失败".to_owned()))?,
            encap_content_info: EncapsulatedContentInfo {
                econtent_type: ID_DATA,
                econtent: None,
            },
            certificates: Some(
                CertificateSet::try_from(vec![CertificateChoices::Certificate(certificate)])
                    .map_err(|_| OfdError::Crypto("证书集合编码失败".to_owned()))?,
            ),
            crls: None,
            signer_infos: SignerInfos::try_from(vec![signer])
                .map_err(|_| OfdError::Crypto("签名者信息编码失败".to_owned()))?,
        };
        let signed_data_der = signed_data
            .to_der()
            .map_err(|_| OfdError::Crypto("SignedData 编码失败".to_owned()))?;

        let content_info = ContentInfo {
            content_type: ID_SIGNED_DATA,
            content: Any::from(
                AnyRef::try_from(signed_data_der.as_slice())
                    .map_err(|_| OfdError::Crypto("ContentInfo 编码失败".to_owned()))?,
            ),
        };
        content_info
            .to_der()
            .map_err(|_| OfdError::Crypto("ContentInfo 编码失败".to_owned()))
    }
}

fn algorithm(oid: ObjectIdentifier) -> x509_cert::spki::AlgorithmIdentifierOwned {
    x509_cert::spki::AlgorithmIdentifierOwned {
        oid,
        parameters: None,
    }
}
