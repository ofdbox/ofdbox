use ofdbox_core::{OfdPackage, ReadOptions};
use ofdbox_seal::{
    OuterSignatureShape, SealError, SealFormat, TimeType, fingerprint_seal, identify_seal_format,
    parse_seal_for_display,
};

mod common;

use common::*;

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
