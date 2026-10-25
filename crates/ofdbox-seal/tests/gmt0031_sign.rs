mod common;

use std::io::{Cursor, Write};

use ofdbox_core::{
    DEFAULT_SIGNATURE_METHOD, OfdPackage, SignatureVerifier, SignatureVerifiers, SignedValueFormat,
    SignedValueProducer, SigningContext, SigningRequest, detect_signed_value_format,
};
use ofdbox_seal::{
    GmT0031SignatureVerifier, GmT0031VerifyOptions, SealBuilder, SealPictureInput, SealSpec,
    SealStandard, Sm2GmT0031SignatureProducer, parse_gmt0031_signature, parse_time_info,
    verify_gmt0031_signature,
};

const CONTENT: &[u8] = b"<ofd:Signature>0031 content</ofd:Signature>";

fn make_seal(certificate: &[u8], secret: &sm2::SecretKey) -> Vec<u8> {
    let spec = SealSpec::new(
        SealStandard::GmT0031,
        "SEAL-0031",
        "0031 测试印章",
        SealPictureInput {
            media_type: "PNG".to_owned(),
            data: common::PNG.to_vec(),
            width: 40,
            height: 40,
        },
    )
    .with_validity("20260101000000", "20260101000000", "20360101000000")
    .with_certificates(vec![certificate.to_vec()]);
    SealBuilder::new(spec, certificate.to_vec(), secret.to_bytes().to_vec())
        .build()
        .expect("seal")
}

fn make_signature() -> (Vec<u8>, Vec<u8>) {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let seal = make_seal(&certificate, &secret);
    let producer =
        Sm2GmT0031SignatureProducer::new(seal, certificate.clone(), secret.to_bytes().to_vec())
            .expect("producer");
    let value = producer
        .produce(&SigningContext {
            content: CONTENT,
            signed_entry: "/Doc_0/Signs/Sign_0/Signature.xml",
            signature_method: DEFAULT_SIGNATURE_METHOD,
            signed_at: "20261024050000",
            references: &[],
        })
        .expect("produce");
    (value, certificate)
}

#[test]
fn produces_and_verifies_0031_signature() {
    let (value, _) = make_signature();
    assert_eq!(
        detect_signed_value_format(&value),
        SignedValueFormat::GmT0031Signature
    );

    let parsed = parse_gmt0031_signature(&value).expect("parse");
    assert_eq!(parsed.property_info, "/Doc_0/Signs/Sign_0/Signature.xml");
    assert_eq!(parsed.time_info, "2026-10-24T05:00:00.000Z");
    assert_eq!(parsed.algorithm, "1.2.156.10197.1.501");
    assert!(!parsed.signature.is_empty());

    let verification = verify_gmt0031_signature(&value, CONTENT, &GmT0031VerifyOptions::default())
        .expect("verify");
    assert_eq!(verification.time_info, "2026-10-24T05:00:00.000Z");
}

#[test]
fn rejects_tampered_content() {
    let (value, _) = make_signature();
    assert!(
        verify_gmt0031_signature(&value, b"tampered", &GmT0031VerifyOptions::default()).is_err()
    );
}

#[test]
fn rejects_tampered_signature() {
    let (mut value, _) = make_signature();
    let last = value.len() - 1;
    value[last] ^= 1;
    assert!(parse_gmt0031_signature(&value).is_ok());
    assert!(verify_gmt0031_signature(&value, CONTENT, &GmT0031VerifyOptions::default()).is_err());
}

#[test]
fn verifier_is_registered_for_gmt0031() {
    let (value, _) = make_signature();
    let verifier = GmT0031SignatureVerifier::new();
    assert_eq!(verifier.format(), SignedValueFormat::GmT0031Signature);
    let _ = value;
}

const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>0031-e2e</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>1</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;

const PAGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:Area>
  <ofd:Content/>
</ofd:Page>"#;

fn package() -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in [
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", PAGE.as_bytes()),
    ] {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .expect("start");
        writer.write_all(data).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

#[test]
fn signs_package_with_0031() {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let seal = make_seal(&certificate, &secret);
    let producer = Sm2GmT0031SignatureProducer::new(seal, certificate, secret.to_bytes().to_vec())
        .expect("producer");

    let mut package = OfdPackage::from_bytes(package()).expect("open");
    let request = SigningRequest::new(DEFAULT_SIGNATURE_METHOD, "20261024050000");
    let mut output = Vec::new();
    package
        .sign_package(&request, &producer, &mut Cursor::new(&mut output))
        .expect("sign package");

    let mut signed = OfdPackage::from_bytes(output).expect("open signed");
    let index = signed.parse_signatures().expect("signatures");
    let entry = index.signatures().next().expect("entry");
    let signature = signed
        .parse_signature(entry.base_loc().expect("loc"))
        .expect("signature");
    let signed_value = signed
        .read_entry(signature.signed_value().expect("signed value"))
        .expect("read value");
    assert_eq!(
        detect_signed_value_format(&signed_value),
        SignedValueFormat::GmT0031Signature
    );

    let verifiers = SignatureVerifiers::new().with(Box::new(GmT0031SignatureVerifier::new()));
    signed
        .verify_signature(&signature, &verifiers)
        .expect("verify 0031 package");
}

#[test]
fn time_info_accepts_common_formats() {
    assert!(parse_time_info("2026-10-24T05:00:00Z").is_some());
    assert!(parse_time_info("2026-10-24T05:00:00.123Z").is_some());
    assert!(parse_time_info("2026-10-24T05:00:00+08:00").is_some());
    assert!(parse_time_info("2026-10-24 05:00:00Z").is_some());
    assert!(parse_time_info("2026/10/24T05:00:00Z").is_none());
}
