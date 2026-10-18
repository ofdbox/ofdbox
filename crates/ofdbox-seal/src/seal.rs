use crate::der::{
    self, TAG_BIT_STRING, TAG_INTEGER, TAG_OCTET_STRING, TAG_OID, TAG_SEQUENCE, TimeKind, Tlv,
};
use crate::error::SealError;
use crate::fingerprint::{OuterSignatureShape, SealFingerprint, SealFormat, TimeType};

/// 印章头。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SealHeader<'a> {
    /// 头标识，标准值为 `ES`。
    pub id: &'a str,
    /// 印章数据版本号。
    pub version: i64,
    /// 厂商标识。
    pub vendor: &'a str,
}

/// 印章属性。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SealProperty<'a> {
    /// 印章类型。
    pub seal_type: i64,
    /// 印章名称。
    pub name: &'a str,
    /// 签章者证书信息类型；仅 GB/T 38540 系列结构包含该字段。
    pub certificate_list_type: Option<i64>,
    /// 签章者证书信息列表中的条目数。
    pub certificate_count: usize,
    /// 印章制作时间。
    pub create_date: &'a str,
    /// 有效期起始时间。
    pub valid_start: &'a str,
    /// 有效期终止时间。
    pub valid_end: &'a str,
}

/// 印章内嵌图片。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SealPicture<'a> {
    /// 图片类型，如 `PNG`、`GIF`、`JPG`、`SVG`。
    pub media_type: &'a str,
    /// 图片原始数据。
    pub data: &'a [u8],
    /// 显示宽度，单位毫米。
    pub width: i64,
    /// 显示高度，单位毫米。
    pub height: i64,
}

/// 供显示使用的印章信息。
///
/// 解析过程只读取印章头、属性和内嵌图片，不验证签名、证书链或有效期。
#[derive(Clone, Debug)]
pub struct SealDisplay<'a> {
    /// 结构指纹及其格式判断。
    pub fingerprint: SealFingerprint,
    /// 印章头。
    pub header: SealHeader<'a>,
    /// 印章标识。
    pub seal_id: &'a str,
    /// 印章属性。
    pub property: SealProperty<'a>,
    /// 内嵌图片。
    pub picture: SealPicture<'a>,
}

impl SealDisplay<'_> {
    /// 结构指纹给出的格式判断。
    pub fn format(&self) -> SealFormat {
        self.fingerprint.format()
    }
}

/// 解析印章并提取显示所需的信息。
///
/// 该函数不执行任何密码学验证。未知或厂商变体结构只要字段完整也会成功返回，
/// 由 [`SealDisplay::format`] 给出格式判断。
pub fn parse_seal_for_display(bytes: &[u8]) -> Result<SealDisplay<'_>, SealError> {
    let layout = parse_layout(bytes)?;
    let picture = parse_picture(layout.picture)?;
    Ok(SealDisplay {
        fingerprint: layout.fingerprint,
        header: layout.header,
        seal_id: layout.seal_id,
        property: layout.property,
        picture,
    })
}

/// 只解析结构指纹，不读取图片内容。
pub fn fingerprint_seal(bytes: &[u8]) -> Result<SealFingerprint, SealError> {
    parse_layout(bytes).map(|layout| layout.fingerprint)
}

/// 只判断印章格式，不读取图片内容。
pub fn identify_seal_format(bytes: &[u8]) -> Result<SealFormat, SealError> {
    fingerprint_seal(bytes).map(|fingerprint| fingerprint.format())
}

struct Layout<'a> {
    header: SealHeader<'a>,
    seal_id: &'a str,
    property: SealProperty<'a>,
    picture: Tlv<'a>,
    fingerprint: SealFingerprint,
}

fn parse_layout(bytes: &[u8]) -> Result<Layout<'_>, SealError> {
    let root = der::parse_root(bytes)?;
    der::expect_tag(root, TAG_SEQUENCE, "印章外层必须是 SEQUENCE")?;
    let top = der::items(root)?;
    let seal_info = *top.first().ok_or(SealError::Invalid("印章数据为空"))?;
    der::expect_tag(seal_info, TAG_SEQUENCE, "印章信息必须是 SEQUENCE")?;
    let (outer, signature_algorithm) = classify_outer(&top[1..]);

    let fields = der::items(seal_info)?;
    if fields.len() < 4 {
        return Err(SealError::Invalid("印章信息字段不足"));
    }
    let header = parse_header(fields[0])?;
    let seal_id = der::ia5(fields[1], "印章标识必须是 IA5String")?;
    let property = parse_property(fields[2])?;
    let picture = fields[3];
    der::expect_tag(picture, TAG_SEQUENCE, "印章图片信息必须是 SEQUENCE")?;

    let fingerprint = SealFingerprint {
        header_id: header.id.to_owned(),
        version: header.version,
        vendor: header.vendor.to_owned(),
        certificate_list_type: property.certificate_list_type,
        time_type: property.time_type,
        outer,
        signature_algorithm,
        has_extensions: fields.len() > 4,
    };
    Ok(Layout {
        header,
        seal_id,
        property: property.into_property(),
        picture,
        fingerprint,
    })
}

fn parse_header(tlv: Tlv<'_>) -> Result<SealHeader<'_>, SealError> {
    der::expect_tag(tlv, TAG_SEQUENCE, "印章头必须是 SEQUENCE")?;
    let fields = der::items(tlv)?;
    if fields.len() < 3 {
        return Err(SealError::Invalid("印章头字段不足"));
    }
    Ok(SealHeader {
        id: der::ia5(fields[0], "印章头标识必须是 IA5String")?,
        version: der::integer(fields[1])?,
        vendor: der::ia5(fields[2], "厂商标识必须是 IA5String")?,
    })
}

fn parse_property(tlv: Tlv<'_>) -> Result<PropertyFields<'_>, SealError> {
    der::expect_tag(tlv, TAG_SEQUENCE, "印章属性必须是 SEQUENCE")?;
    let fields = der::items(tlv)?;
    if fields.len() < 3 {
        return Err(SealError::Invalid("印章属性字段不足"));
    }
    let seal_type = der::integer(fields[0])?;
    let name = der::utf8(fields[1], "印章名称必须是 UTF8String")?;
    let mut cursor = 2;
    let certificate_list_type = if fields[cursor].tag == TAG_INTEGER {
        let value = der::integer(fields[cursor])?;
        cursor += 1;
        Some(value)
    } else {
        None
    };
    let certificate_list = *fields
        .get(cursor)
        .ok_or(SealError::Invalid("印章属性缺少证书列表"))?;
    der::expect_tag(
        certificate_list,
        TAG_SEQUENCE,
        "签章者证书信息列表必须是 SEQUENCE",
    )?;
    let mut certificate_count = 0;
    for entry in certificate_list.children()? {
        entry?;
        certificate_count += 1;
    }
    cursor += 1;

    let mut dates = [""; 3];
    let mut kinds = [None; 3];
    for (index, date) in dates.iter_mut().enumerate() {
        let field = *fields
            .get(cursor + index)
            .ok_or(SealError::Invalid("印章属性缺少时间字段"))?;
        let (value, kind) = der::time(field)?;
        *date = value;
        kinds[index] = Some(kind);
    }
    let time_type = match kinds {
        [
            Some(TimeKind::Utc),
            Some(TimeKind::Utc),
            Some(TimeKind::Utc),
        ] => TimeType::Utc,
        [
            Some(TimeKind::Generalized),
            Some(TimeKind::Generalized),
            Some(TimeKind::Generalized),
        ] => TimeType::Generalized,
        _ => TimeType::Mixed,
    };

    Ok(PropertyFields {
        seal_type,
        name,
        certificate_list_type,
        certificate_count,
        dates,
        time_type,
    })
}

struct PropertyFields<'a> {
    seal_type: i64,
    name: &'a str,
    certificate_list_type: Option<i64>,
    certificate_count: usize,
    dates: [&'a str; 3],
    time_type: TimeType,
}

impl<'a> PropertyFields<'a> {
    fn into_property(self) -> SealProperty<'a> {
        SealProperty {
            seal_type: self.seal_type,
            name: self.name,
            certificate_list_type: self.certificate_list_type,
            certificate_count: self.certificate_count,
            create_date: self.dates[0],
            valid_start: self.dates[1],
            valid_end: self.dates[2],
        }
    }
}

fn parse_picture(tlv: Tlv<'_>) -> Result<SealPicture<'_>, SealError> {
    der::expect_tag(tlv, TAG_SEQUENCE, "印章图片信息必须是 SEQUENCE")?;
    let fields = der::items(tlv)?;
    if fields.len() < 4 {
        return Err(SealError::Invalid("印章图片信息字段不足"));
    }
    Ok(SealPicture {
        media_type: der::ia5(fields[0], "印章图片类型必须是 IA5String")?,
        data: der::octet_string(fields[1], "印章图片数据必须是 OCTET STRING")?,
        width: der::integer(fields[2])?,
        height: der::integer(fields[3])?,
    })
}

fn classify_outer(rest: &[Tlv<'_>]) -> (OuterSignatureShape, Option<String>) {
    if rest.len() == 1 && rest[0].tag == TAG_SEQUENCE {
        if let Ok(fields) = der::items(rest[0])
            && fields.len() == 3
            && fields[0].tag == TAG_OCTET_STRING
            && fields[1].tag == TAG_OID
            && fields[2].tag == TAG_BIT_STRING
        {
            return (OuterSignatureShape::SignerInfo, der::oid(fields[1]).ok());
        }
        return (OuterSignatureShape::Unknown, None);
    }
    if rest.len() == 3
        && rest[0].tag == TAG_OCTET_STRING
        && rest[1].tag == TAG_OID
        && rest[2].tag == TAG_BIT_STRING
    {
        return (
            OuterSignatureShape::CertificateAndSignature,
            der::oid(rest[1]).ok(),
        );
    }
    if rest.len() == 2 && rest[0].tag == TAG_OID && rest[1].tag == TAG_BIT_STRING {
        return (OuterSignatureShape::SignatureOnly, der::oid(rest[0]).ok());
    }
    (OuterSignatureShape::Unknown, None)
}
