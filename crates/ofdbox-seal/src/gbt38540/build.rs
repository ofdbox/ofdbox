//! GB/T 38540—2020 的印章信息与外壳编码。

use crate::der;
use crate::error::SealError;
use crate::eseal::encoding::{generalized, ia5, integer, utf8};
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

    let header = der::wrap(
        0x30,
        &[
            ia5("ES"),
            integer(spec.version, "印章版本号")?,
            ia5(&spec.vendor),
        ]
        .concat(),
    );
    let certificates = certificate_list(
        spec.certificate_list_type,
        &spec.certificate_list,
        &spec.certificate_digests,
    )?;
    let property = [
        integer(spec.seal_type, "印章类型")?,
        utf8(&spec.name),
        integer(spec.certificate_list_type, "证书信息类型")?,
        certificates,
        generalized(&spec.created_at)?,
        generalized(&spec.valid_start)?,
        generalized(&spec.valid_end)?,
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

/// 组装外层 `SESeal`：`{ sealInfo, cert, alg, signature }`。
pub(crate) fn assemble(
    seal_info: Vec<u8>,
    certificate: Vec<u8>,
    algorithm: Vec<u8>,
    signature: Vec<u8>,
) -> Vec<u8> {
    let mut content = seal_info;
    content.extend(certificate);
    content.extend(algorithm);
    content.extend(signature);
    der::wrap(0x30, &content)
}

fn certificate_list(
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
