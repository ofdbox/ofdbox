use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use ofdbox_core::{OfdPackage, ReadOptions};
use ofdbox_seal::{
    OuterSignatureShape, SealError, SealFormat, TimeType, fingerprint_seal, identify_seal_format,
    parse_seal_for_display,
};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfictional-seal-image";
const OID_SM2_SM3: &[u8] = &[42, 0x81, 0x1c, 0xcf, 0x55, 0x01, 0x83, 0x75];

fn tlv(tag: u8, value: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let length = value.len();
    if length < 0x80 {
        out.push(length as u8);
    } else {
        let bytes = (length as u32).to_be_bytes();
        let start = bytes.iter().position(|&byte| byte != 0).unwrap();
        out.push(0x80 | (bytes.len() - start) as u8);
        out.extend_from_slice(&bytes[start..]);
    }
    out.extend_from_slice(value);
    out
}

fn seq(items: &[Vec<u8>]) -> Vec<u8> {
    tlv(0x30, &items.concat())
}

fn integer(value: i64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let mut start = 0;
    while start + 1 < bytes.len()
        && ((bytes[start] == 0 && bytes[start + 1] & 0x80 == 0)
            || (bytes[start] == 0xff && bytes[start + 1] & 0x80 != 0))
    {
        start += 1;
    }
    tlv(0x02, &bytes[start..])
}

fn ia5(value: &str) -> Vec<u8> {
    tlv(0x16, value.as_bytes())
}

fn utf8(value: &str) -> Vec<u8> {
    tlv(0x0c, value.as_bytes())
}

fn octets(value: &[u8]) -> Vec<u8> {
    tlv(0x04, value)
}

fn oid(value: &[u8]) -> Vec<u8> {
    tlv(0x06, value)
}

fn bit_string(value: &[u8]) -> Vec<u8> {
    let mut content = vec![0];
    content.extend_from_slice(value);
    tlv(0x03, &content)
}

fn utc(value: &str) -> Vec<u8> {
    tlv(0x17, value.as_bytes())
}

fn generalized(value: &str) -> Vec<u8> {
    tlv(0x18, value.as_bytes())
}

fn header(version: i64, vendor: &str) -> Vec<u8> {
    seq(&[ia5("ES"), integer(version), ia5(vendor)])
}

fn picture() -> Vec<u8> {
    seq(&[ia5("PNG"), octets(PNG), integer(40), integer(40)])
}

fn gm_property() -> Vec<u8> {
    seq(&[
        integer(1),
        utf8("测试公章"),
        seq(&[octets(b"fictional-certificate")]),
        utc("230117071431Z"),
        utc("230109071043Z"),
        utc("280109071043Z"),
    ])
}

fn gb_property() -> Vec<u8> {
    seq(&[
        integer(1),
        utf8("测试公章"),
        integer(1),
        seq(&[
            octets(b"fictional-certificate"),
            octets(b"fictional-certificate-2"),
        ]),
        generalized("20211229175736Z"),
        generalized("20211229180422Z"),
        generalized("20241231180430Z"),
    ])
}

fn seal_info(property: Vec<u8>, extensions: bool) -> Vec<u8> {
    let mut fields = vec![
        header(1, "OFDBOX"),
        ia5("seal-id-0001"),
        property,
        picture(),
    ];
    if extensions {
        fields.push(seq(&[seq(&[
            oid(&[42, 1]),
            tlv(0x01, &[0xff]),
            octets(b"extension"),
        ])]));
    }
    seq(&fields)
}

fn gm_seal() -> Vec<u8> {
    let signer = seq(&[
        octets(b"fictional-certificate"),
        oid(OID_SM2_SM3),
        bit_string(b"fictional-signature"),
    ]);
    seq(&[seal_info(gm_property(), false), signer])
}

fn gb_seal() -> Vec<u8> {
    seq(&[
        seal_info(gb_property(), false),
        octets(b"fictional-certificate"),
        oid(OID_SM2_SM3),
        bit_string(b"fictional-signature"),
    ])
}

#[test]
fn recognizes_gm_t_0031_and_extracts_picture() {
    let bytes = gm_seal();
    let fingerprint = fingerprint_seal(&bytes).expect("fingerprint");
    assert_eq!(fingerprint.format(), SealFormat::GmT0031);
    assert_eq!(fingerprint.header_id, "ES");
    assert_eq!(fingerprint.version, 1);
    assert_eq!(fingerprint.vendor, "OFDBOX");
    assert_eq!(fingerprint.time_type, TimeType::Utc);
    assert_eq!(fingerprint.outer, OuterSignatureShape::SignerInfo);
    assert_eq!(
        fingerprint.signature_algorithm.as_deref(),
        Some("1.2.156.10197.1.501")
    );
    assert!(!fingerprint.has_extensions);
    assert_eq!(
        identify_seal_format(&bytes).expect("format"),
        SealFormat::GmT0031
    );

    let display = parse_seal_for_display(&bytes).expect("display");
    assert_eq!(display.format(), SealFormat::GmT0031);
    assert_eq!(display.seal_id, "seal-id-0001");
    assert_eq!(display.property.name, "测试公章");
    assert_eq!(display.property.certificate_list_type, None);
    assert_eq!(display.property.certificate_count, 1);
    assert_eq!(display.property.valid_end, "280109071043Z");
    assert_eq!(display.picture.media_type, "PNG");
    assert_eq!(display.picture.data, PNG);
    assert_eq!(display.picture.width, 40);
    assert_eq!(display.picture.height, 40);
}

#[test]
fn recognizes_gb_t_38540_and_extracts_picture() {
    let bytes = gb_seal();
    let fingerprint = fingerprint_seal(&bytes).expect("fingerprint");
    assert_eq!(fingerprint.format(), SealFormat::GbT38540);
    assert_eq!(fingerprint.time_type, TimeType::Generalized);
    assert_eq!(
        fingerprint.outer,
        OuterSignatureShape::CertificateAndSignature
    );
    assert_eq!(fingerprint.certificate_list_type, Some(1));

    let display = parse_seal_for_display(&bytes).expect("display");
    assert_eq!(display.format(), SealFormat::GbT38540);
    assert_eq!(display.property.certificate_count, 2);
    assert_eq!(display.picture.data, PNG);
}

#[test]
fn vendor_variant_is_displayable_but_not_classified() {
    let variant = seq(&[
        seal_info(
            seq(&[
                integer(3),
                utf8("厂商变体章"),
                seq(&[octets(b"fictional-certificate")]),
                generalized("20200422020900Z"),
                generalized("20200422020900Z"),
                generalized("20220422020900Z"),
            ]),
            false,
        ),
        oid(OID_SM2_SM3),
        bit_string(b"fictional-signature"),
    ]);
    let fingerprint = fingerprint_seal(&variant).expect("fingerprint");
    assert_eq!(fingerprint.format(), SealFormat::Unknown);
    assert_eq!(fingerprint.outer, OuterSignatureShape::SignatureOnly);
    assert_eq!(fingerprint.certificate_list_type, None);

    let display = parse_seal_for_display(&variant).expect("display");
    assert_eq!(display.format(), SealFormat::Unknown);
    assert_eq!(display.picture.data, PNG);
}

#[test]
fn gb_property_without_certificate_is_not_classified() {
    let variant = seq(&[
        seal_info(gb_property(), false),
        oid(OID_SM2_SM3),
        bit_string(b"fictional-signature"),
    ]);
    assert_eq!(
        identify_seal_format(&variant).expect("format"),
        SealFormat::Unknown
    );
}

#[test]
fn extensions_are_recorded() {
    let bytes = seq(&[
        seal_info(gm_property(), true),
        seq(&[
            octets(b"fictional-certificate"),
            oid(OID_SM2_SM3),
            bit_string(b"fictional-signature"),
        ]),
    ]);
    let display = parse_seal_for_display(&bytes).expect("display");
    assert!(display.fingerprint.has_extensions);
    assert_eq!(display.format(), SealFormat::GmT0031);
}

#[test]
fn trailing_bytes_are_rejected() {
    let mut bytes = gm_seal();
    bytes.push(0);
    assert_eq!(
        fingerprint_seal(&bytes),
        Err(SealError::Invalid("印章数据末尾存在多余字节"))
    );
}

#[test]
fn truncated_picture_is_rejected() {
    let mut bytes = gm_seal();
    let length = bytes.len();
    bytes.truncate(length - 4);
    assert!(matches!(
        parse_seal_for_display(&bytes),
        Err(SealError::Truncated(_))
    ));
}

#[test]
fn high_tag_number_is_rejected() {
    let bytes = tlv(0x3f, &[]);
    assert_eq!(
        fingerprint_seal(&bytes),
        Err(SealError::Invalid("不支持高标签号"))
    );
}

#[test]
fn excessive_children_are_rejected() {
    let mut fields = vec![
        header(1, "OFDBOX"),
        ia5("seal-id"),
        gm_property(),
        picture(),
    ];
    for _ in 0..1025 {
        fields.push(seq(&[]));
    }
    let bytes = seq(&[seq(&fields)]);
    assert!(matches!(
        fingerprint_seal(&bytes),
        Err(SealError::LimitExceeded(_))
    ));
}

fn zip_with_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .expect("start zip entry");
        writer.write_all(data).expect("write zip entry");
    }
    writer.finish().expect("finish zip").into_inner()
}

#[test]
fn reads_seal_only_on_demand() {
    let signature = br#"<ofd:Signature xmlns:ofd="http://www.ofdspec.org/2016">
        <ofd:SignedValue>SignedValue.dat</ofd:SignedValue>
        <ofd:SignedInfo>
          <ofd:Seal><ofd:BaseLoc>Seal.esl</ofd:BaseLoc></ofd:Seal>
        </ofd:SignedInfo>
      </ofd:Signature>"#;
    let bytes = zip_with_entries(&[
        ("Doc_0/Signs/Sign_0/Signature.xml", signature),
        ("Doc_0/Signs/Sign_0/Seal.esl", b"not-a-seal"),
    ]);
    let mut package =
        OfdPackage::from_bytes_with_options(bytes, ReadOptions::LENIENT, Default::default())
            .expect("open package");
    let view = package
        .parse_signature_file("Doc_0/Signs/Sign_0/Signature.xml")
        .expect("parse signature");
    assert_eq!(
        &package.read_seal_bytes(&view).expect("read seal")[..],
        b"not-a-seal"
    );
}

#[test]
fn parses_real_seal_samples() {
    let _ = dotenvy::dotenv();
    let Some(dir) = std::env::var_os("OFDBOX_TESTDATA_DIR").map(PathBuf::from) else {
        return;
    };
    if !dir.is_dir() {
        return;
    }
    let mut paths = Vec::new();
    collect_seals(&dir, &mut paths);

    let mut total = 0usize;
    let mut gm_t_0031 = 0usize;
    let mut gb_t_38540 = 0usize;
    let mut unknown = 0usize;
    let mut with_picture = 0usize;
    for path in &paths {
        let bytes = fs::read(path).expect("read seal sample");
        total += 1;
        let display = parse_seal_for_display(&bytes).expect("parse real seal");
        match display.format() {
            SealFormat::GmT0031 => gm_t_0031 += 1,
            SealFormat::GbT38540 => gb_t_38540 += 1,
            SealFormat::Unknown => unknown += 1,
        }
        if !display.picture.data.is_empty() {
            with_picture += 1;
        }
    }
    if total > 0 {
        assert_eq!(gm_t_0031 + gb_t_38540 + unknown, total);
        assert_eq!(with_picture, total);
    }
}

fn collect_seals(dir: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            collect_seals(&path, paths);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("esl"))
        {
            paths.push(path);
        }
    }
}
