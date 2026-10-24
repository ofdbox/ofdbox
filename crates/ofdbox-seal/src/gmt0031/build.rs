//! GM/T 0031—2014 的印章信息与外壳编码。

use crate::der;
use crate::error::SealError;
use crate::eseal::encoding::{ia5, integer, utc, utf8};
use crate::eseal::spec::SealSpec;

/// 组装 `SES_SealInfo`。
pub(crate) fn seal_info(spec: &SealSpec) -> Result<Vec<u8>, SealError> {
    if spec.name.is_empty() || spec.seal_id.is_empty() {
        return Err(SealError::Invalid("印章名称与标识不能为空"));
    }
    if spec.picture.data.is_empty() || spec.picture.media_type.is_empty() {
        return Err(SealError::Invalid("印章图片不能为空"));
    }
    if spec.picture.width <= 0 || spec.picture.height <= 0 {
        return Err(SealError::Invalid("印章图片宽高必须为正数"));
    }
    if !spec.certificate_digests.is_empty() {
        return Err(SealError::UnsupportedCertificateListType(2));
    }
    if spec.certificate_list.is_empty() {
        return Err(SealError::Invalid("签章者证书列表不能为空"));
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
    let certificates: Vec<u8> = spec
        .certificate_list
        .iter()
        .flat_map(|certificate| der::wrap(0x04, certificate))
        .collect();
    let property = [
        integer(spec.seal_type, "印章类型")?,
        utf8(&spec.name),
        der::wrap(0x30, &certificates),
        utc(&spec.created_at)?,
        utc(&spec.valid_start)?,
        utc(&spec.valid_end)?,
    ]
    .concat();
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

/// 0031 的签名对象：`SEQUENCE(sealInfo, cert, alg)`。
pub(crate) fn signed_content(seal_info: &[u8], certificate: &[u8], algorithm: &[u8]) -> Vec<u8> {
    let mut content = seal_info.to_vec();
    content.extend_from_slice(certificate);
    content.extend_from_slice(algorithm);
    der::wrap(0x30, &content)
}

/// 组装外层 `SESeal`：`{ sealInfo, SES_SignInfo }`。
pub(crate) fn assemble(
    seal_info: Vec<u8>,
    certificate: Vec<u8>,
    algorithm: Vec<u8>,
    signature: Vec<u8>,
) -> Vec<u8> {
    let mut sign_info = certificate;
    sign_info.extend(algorithm);
    sign_info.extend(signature);
    let mut content = seal_info;
    content.extend(der::wrap(0x30, &sign_info));
    der::wrap(0x30, &content)
}
