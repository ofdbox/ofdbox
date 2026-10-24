//! 制章：按 GB/T 38540—2020 或 GM/T 0031—2014 组装并签名电子印章数据。

use ofdbox_crypto::sign_sm3;
use zeroize::Zeroizing;

use crate::der;
use crate::error::SealError;

/// SM2 with SM3 的 DER OID 内容（1.2.156.10197.1.501）。
const OID_SM2_SM3: &[u8] = &[0x2A, 0x81, 0x1C, 0xCF, 0x55, 0x01, 0x83, 0x75];

/// 印章标准。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SealStandard {
    /// GB/T 38540—2020：属性含证书信息类型与列表，时间用 GeneralizedTime，外层平铺。
    GbT38540,
    /// GM/T 0031—2014：属性只含证书列表，时间用 UTCTime，签章信息单独一层。
    GmT0031,
}

/// 印章图片。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealPictureInput {
    /// 图片类型，如 `PNG`、`GIF`、`JPG`、`BMP`、`SVG`。
    pub media_type: String,
    /// 图片原始字节。
    pub data: Vec<u8>,
    /// 显示宽度，单位毫米。
    pub width: i64,
    /// 显示高度，单位毫米。
    pub height: i64,
}

/// 制章参数。
#[derive(Clone, Debug)]
pub struct SealSpec {
    /// 印章标准。
    pub standard: SealStandard,
    /// 印章数据版本号。
    pub version: i64,
    /// 厂商标识（`SES_Header.Vid`）。
    pub vendor: String,
    /// 印章标识（`esID`）。
    pub seal_id: String,
    /// 印章类型。
    pub seal_type: i64,
    /// 印章名称。
    pub name: String,
    /// 印章图片。
    pub picture: SealPictureInput,
    /// 制作时间，14 位 `YYYYMMDDHHMMSS`。
    pub created_at: String,
    /// 有效期起始，14 位 `YYYYMMDDHHMMSS`。
    pub valid_start: String,
    /// 有效期终止，14 位 `YYYYMMDDHHMMSS`。
    pub valid_end: String,
    /// 签章者证书（DER）。
    pub certificate_list: Vec<Vec<u8>>,
    /// 38540 的证书信息类型：1 证书，2 杂凑值。
    pub certificate_list_type: i64,
    /// 38540 类型 2 的证书杂凑值：`(类型标识, 32 字节杂凑值)`。
    pub certificate_digests: Vec<(String, Vec<u8>)>,
}

impl SealSpec {
    /// 用标准、印章标识、名称与图片创建参数。
    pub fn new(
        standard: SealStandard,
        seal_id: impl Into<String>,
        name: impl Into<String>,
        picture: SealPictureInput,
    ) -> Self {
        Self {
            standard,
            version: match standard {
                SealStandard::GbT38540 => 4,
                SealStandard::GmT0031 => 1,
            },
            vendor: "ofdbox".to_owned(),
            seal_id: seal_id.into(),
            seal_type: 1,
            name: name.into(),
            picture,
            created_at: String::new(),
            valid_start: String::new(),
            valid_end: String::new(),
            certificate_list: Vec::new(),
            certificate_list_type: 1,
            certificate_digests: Vec::new(),
        }
    }

    /// 指定版本号。
    pub fn with_version(mut self, version: i64) -> Self {
        self.version = version;
        self
    }

    /// 指定厂商标识。
    pub fn with_vendor(mut self, vendor: impl Into<String>) -> Self {
        self.vendor = vendor.into();
        self
    }

    /// 指定印章类型。
    pub fn with_seal_type(mut self, seal_type: i64) -> Self {
        self.seal_type = seal_type;
        self
    }

    /// 指定制作时间与有效期，均为 14 位 `YYYYMMDDHHMMSS`。
    pub fn with_validity(
        mut self,
        created_at: impl Into<String>,
        valid_start: impl Into<String>,
        valid_end: impl Into<String>,
    ) -> Self {
        self.created_at = created_at.into();
        self.valid_start = valid_start.into();
        self.valid_end = valid_end.into();
        self
    }

    /// 指定签章者证书列表（38540 类型 1，或 0031 的证书列表）。
    pub fn with_certificates(mut self, certificates: Vec<Vec<u8>>) -> Self {
        self.certificate_list = certificates;
        self.certificate_list_type = 1;
        self.certificate_digests.clear();
        self
    }

    /// 指定签章者证书杂凑值列表（仅 38540，类型 2）。
    pub fn with_certificate_digests(mut self, digests: Vec<(String, Vec<u8>)>) -> Self {
        self.certificate_digests = digests;
        self.certificate_list_type = 2;
        self.certificate_list.clear();
        self
    }
}

/// 电子印章构造器。
pub struct SealBuilder {
    spec: SealSpec,
    maker_certificate: Vec<u8>,
    private_key: Zeroizing<Vec<u8>>,
    dist_id: String,
}

impl SealBuilder {
    /// 用制章参数、制章者证书与私钥创建构造器。
    pub fn new(
        spec: SealSpec,
        maker_certificate: impl Into<Vec<u8>>,
        private_key: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            spec,
            maker_certificate: maker_certificate.into(),
            private_key: Zeroizing::new(private_key.into()),
            dist_id: ofdbox_crypto::DEFAULT_DIST_ID.to_owned(),
        }
    }

    /// 指定 SM2 签名者标识。
    pub fn with_dist_id(mut self, dist_id: impl Into<String>) -> Self {
        self.dist_id = dist_id.into();
        self
    }

    /// 生成电子印章数据（`SESeal`）。
    pub fn build(self) -> Result<Vec<u8>, SealError> {
        let seal_info = encode_seal_info(&self.spec)?;
        let algorithm = der::wrap(0x06, OID_SM2_SM3);
        let certificate = der::wrap(0x04, &self.maker_certificate);

        // 0031 的签名对象是 SEQUENCE(印章信息, 制章者证书, 算法标识)；
        // 38540 只对印章信息签名。
        let signed = match self.spec.standard {
            SealStandard::GbT38540 => seal_info.clone(),
            SealStandard::GmT0031 => {
                let mut content = seal_info.clone();
                content.extend_from_slice(&certificate);
                content.extend_from_slice(&algorithm);
                der::wrap(0x30, &content)
            }
        };
        let signature = sign_sm3(&self.private_key, &signed, &self.dist_id)
            .map_err(|error| SealError::Crypto(error.to_string()))?;
        let signature = bit_string(&signature);

        let mut content = seal_info;
        match self.spec.standard {
            SealStandard::GbT38540 => {
                content.extend(certificate);
                content.extend(algorithm);
                content.extend(signature);
            }
            SealStandard::GmT0031 => {
                let mut sign_info = certificate;
                sign_info.extend(algorithm);
                sign_info.extend(signature);
                content.extend(der::wrap(0x30, &sign_info));
            }
        }
        Ok(der::wrap(0x30, &content))
    }
}

fn encode_seal_info(spec: &SealSpec) -> Result<Vec<u8>, SealError> {
    if spec.name.is_empty() || spec.seal_id.is_empty() {
        return Err(SealError::Invalid("印章名称与标识不能为空"));
    }
    if spec.picture.data.is_empty() || spec.picture.media_type.is_empty() {
        return Err(SealError::Invalid("印章图片不能为空"));
    }
    if spec.picture.width <= 0 || spec.picture.height <= 0 {
        return Err(SealError::Invalid("印章图片宽高必须为正数"));
    }

    let header = der::wrap(
        0x30,
        &[
            ia5("ES"),
            integer(spec.version, "印章版本号")?,
            ia5(&spec.vendor),
        ]
        .concat(),
    );
    let property = match spec.standard {
        SealStandard::GbT38540 => {
            let certificates = encode_certificate_list(
                spec.certificate_list_type,
                &spec.certificate_list,
                &spec.certificate_digests,
            )?;
            [
                integer(spec.seal_type, "印章类型")?,
                utf8(&spec.name),
                integer(spec.certificate_list_type, "证书信息类型")?,
                certificates,
                generalized(&spec.created_at)?,
                generalized(&spec.valid_start)?,
                generalized(&spec.valid_end)?,
            ]
            .concat()
        }
        SealStandard::GmT0031 => {
            if !spec.certificate_digests.is_empty() {
                return Err(SealError::UnsupportedCertificateListType(2));
            }
            let certificates: Vec<u8> = spec
                .certificate_list
                .iter()
                .flat_map(|certificate| der::wrap(0x04, certificate))
                .collect();
            [
                integer(spec.seal_type, "印章类型")?,
                utf8(&spec.name),
                der::wrap(0x30, &certificates),
                utc(&spec.created_at)?,
                utc(&spec.valid_start)?,
                utc(&spec.valid_end)?,
            ]
            .concat()
        }
    };
    let property = der::wrap(0x30, &property);
    let picture = der::wrap(
        0x30,
        &[
            ia5(&spec.picture.media_type),
            der::wrap(0x04, &spec.picture.data),
            integer(spec.picture.width, "印章图片宽度")?,
            integer(spec.picture.height, "印章图片高度")?,
        ]
        .concat(),
    );

    Ok(der::wrap(
        0x30,
        &[header, ia5(&spec.seal_id), property, picture].concat(),
    ))
}

fn encode_certificate_list(
    list_type: i64,
    certificates: &[Vec<u8>],
    digests: &[(String, Vec<u8>)],
) -> Result<Vec<u8>, SealError> {
    match list_type {
        1 => {
            if certificates.is_empty() {
                return Err(SealError::Invalid("签章者证书列表不能为空"));
            }
            let items: Vec<u8> = certificates
                .iter()
                .flat_map(|certificate| der::wrap(0x04, certificate))
                .collect();
            Ok(der::wrap(0x30, &items))
        }
        2 => {
            if digests.is_empty() {
                return Err(SealError::Invalid("签章者证书杂凑值列表不能为空"));
            }
            let mut items = Vec::new();
            for (name, digest) in digests {
                let item = [der::wrap(0x13, name.as_bytes()), der::wrap(0x04, digest)].concat();
                items.extend(der::wrap(0x30, &item));
            }
            Ok(der::wrap(0x30, &items))
        }
        other => Err(SealError::UnsupportedCertificateListType(other)),
    }
}

fn ia5(value: &str) -> Vec<u8> {
    der::wrap(0x16, value.as_bytes())
}

fn utf8(value: &str) -> Vec<u8> {
    der::wrap(0x0c, value.as_bytes())
}

fn bit_string(value: &[u8]) -> Vec<u8> {
    let mut content = vec![0u8];
    content.extend_from_slice(value);
    der::wrap(0x03, &content)
}

fn integer(value: i64, what: &'static str) -> Result<Vec<u8>, SealError> {
    if value < 0 {
        return Err(SealError::Invalid(what));
    }
    let bytes = value.to_be_bytes();
    let start = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
    let mut content = bytes[start..].to_vec();
    if content[0] & 0x80 != 0 {
        content.insert(0, 0);
    }
    Ok(der::wrap(0x02, &content))
}

fn digits14(value: &str, what: &'static str) -> Result<String, SealError> {
    let digits: String = value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
    if digits.len() == 14 {
        Ok(digits)
    } else {
        Err(SealError::Invalid(what))
    }
}

fn generalized(value: &str) -> Result<Vec<u8>, SealError> {
    let digits = digits14(value, "时间需要 14 位 YYYYMMDDHHMMSS")?;
    Ok(der::wrap(0x18, format!("{digits}Z").as_bytes()))
}

fn utc(value: &str) -> Result<Vec<u8>, SealError> {
    let digits = digits14(value, "时间需要 14 位 YYYYMMDDHHMMSS")?;
    Ok(der::wrap(0x17, format!("{}Z", &digits[2..]).as_bytes()))
}
