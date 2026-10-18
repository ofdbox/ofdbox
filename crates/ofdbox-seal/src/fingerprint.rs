/// 印章属性中的时间编码。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeType {
    /// `UTCTime`。
    Utc,
    /// `GeneralizedTime`。
    Generalized,
    /// 三个时间字段使用了不同的编码。
    Mixed,
}

/// 印章外层签名容器的结构。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OuterSignatureShape {
    /// `SEQUENCE { OCTET STRING, OID, BIT STRING }`，对应 GM/T 0031 的签章信息。
    SignerInfo,
    /// `OCTET STRING, OID, BIT STRING`，对应 GB/T 38540 的证书与签名值。
    CertificateAndSignature,
    /// 只有 `OID, BIT STRING`，缺少制章者证书的厂商变体。
    SignatureOnly,
    /// 其他结构。
    Unknown,
}

/// 结构指纹给出的印章格式判断。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SealFormat {
    /// GM/T 0031—2014。
    GmT0031,
    /// GB/T 38540—2020。
    GbT38540,
    /// 结构不符合已知标准，或不同格式的特征互相冲突。
    Unknown,
}

/// 印章的结构指纹。
///
/// 指纹只记录结构特征，不哈希原始字节，也不代表签名已经通过验证。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealFingerprint {
    /// 印章头标识，标准值为 `ES`。
    pub header_id: String,
    /// 印章数据版本号。
    pub version: i64,
    /// 厂商标识。
    pub vendor: String,
    /// 签章者证书信息类型；仅 GB/T 38540 系列结构包含该字段。
    pub certificate_list_type: Option<i64>,
    /// 属性中时间字段的编码。
    pub time_type: TimeType,
    /// 外层签名容器的结构。
    pub outer: OuterSignatureShape,
    /// 外层签名算法 OID。
    pub signature_algorithm: Option<String>,
    /// 印章信息是否带有自定义扩展数据。
    pub has_extensions: bool,
}

impl SealFingerprint {
    /// 根据结构特征判断印章所用格式。
    ///
    /// 版本号与厂商标识只作为指纹的一部分，不单独用于判断格式。
    pub fn format(&self) -> SealFormat {
        if self.header_id != "ES" {
            return SealFormat::Unknown;
        }
        match (self.outer, self.certificate_list_type, self.time_type) {
            (OuterSignatureShape::SignerInfo, None, TimeType::Utc) => SealFormat::GmT0031,
            (OuterSignatureShape::CertificateAndSignature, Some(_), TimeType::Generalized) => {
                SealFormat::GbT38540
            }
            _ => SealFormat::Unknown,
        }
    }
}
