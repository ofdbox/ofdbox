mod common;

use std::io::{Cursor, Write};

use ofdbox_core::{
    DEFAULT_SIGNATURE_METHOD, OfdPackage, PackageEdits, SignatureVerifiers, SigningRequest,
};
use ofdbox_seal::{
    CertificatePolicy, CertificateState, ReportOptions, ReportVerdict, SealBuilder,
    SealPictureInput, SealSpec, SealStandard, SesSignatureVerifier, SignatureState,
    Sm2SesSignatureProducer, verify_signature_report,
};

const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>report</ofd:DocID></ofd:DocInfo>
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

fn signed_package() -> (Vec<u8>, Vec<u8>) {
    let secret = common::test_secret();
    let certificate = common::test_certificate(&secret);
    let spec = SealSpec::new(
        SealStandard::GbT38540,
        "SEAL-REPORT",
        "报告测试印章",
        SealPictureInput {
            media_type: "PNG".to_owned(),
            data: common::PNG.to_vec(),
            width: 40,
            height: 40,
        },
    )
    .with_validity("20260101000000", "20260101000000", "20360101000000")
    .with_certificates(vec![certificate.clone()]);
    let seal = SealBuilder::new(spec, certificate.clone(), secret.to_bytes().to_vec())
        .build()
        .expect("seal");
    let producer =
        Sm2SesSignatureProducer::new(seal, certificate.clone(), secret.to_bytes().to_vec())
            .expect("producer");
    let mut package = OfdPackage::from_bytes(package()).expect("open");
    let request = SigningRequest::new(DEFAULT_SIGNATURE_METHOD, "20261024050000");
    let mut output = Vec::new();
    package
        .sign_package(&request, &producer, &mut Cursor::new(&mut output))
        .expect("sign");
    (output, certificate)
}

#[test]
fn reports_valid_signature() {
    let (signed, _) = signed_package();
    let mut package = OfdPackage::from_bytes(signed).expect("open");
    let verifiers = SignatureVerifiers::new().with(Box::new(SesSignatureVerifier::new()));
    let report = verify_signature_report(
        &mut package,
        &verifiers,
        &ReportOptions {
            require_references: true,
            ..Default::default()
        },
    )
    .expect("report");

    assert_eq!(report.summary.total, 1);
    assert_eq!(report.summary.valid, 1);
    assert_eq!(report.verdict, ReportVerdict::AllValid);
    let status = &report.signatures[0];
    assert_eq!(status.state, SignatureState::Valid);
    assert!(status.references.iter().all(|reference| reference.matches));
    assert_eq!(
        status
            .certificate
            .as_ref()
            .map(|certificate| certificate.state),
        Some(CertificateState::NotChecked)
    );
    assert!(!status.timestamp.present);
}

#[test]
fn reports_tampered_references() {
    let (signed, _) = signed_package();
    let mut package = OfdPackage::from_bytes(signed).expect("open");
    let mut edits = PackageEdits::new();
    edits.replace(
        "Doc_0/Document.xml",
        DOCUMENT.replace("MaxUnitID>1", "MaxUnitID>2"),
    );
    let mut tampered = Vec::new();
    package
        .write_with(&mut Cursor::new(&mut tampered), &edits, &Default::default())
        .expect("write");

    let mut package = OfdPackage::from_bytes(tampered).expect("open");
    let verifiers = SignatureVerifiers::new().with(Box::new(SesSignatureVerifier::new()));
    let report = verify_signature_report(
        &mut package,
        &verifiers,
        &ReportOptions {
            require_references: true,
            ..Default::default()
        },
    )
    .expect("report");
    assert_eq!(report.signatures[0].state, SignatureState::Invalid);
    assert_eq!(report.verdict, ReportVerdict::Invalid);
}

#[test]
fn reports_trusted_certificate() {
    let (signed, certificate) = signed_package();
    let mut policy = CertificatePolicy::new().with_trust_anchor(certificate);
    policy.require_digital_signature = false;
    let mut package = OfdPackage::from_bytes(signed).expect("open");
    let verifiers = SignatureVerifiers::new().with(Box::new(SesSignatureVerifier::new()));
    let report = verify_signature_report(
        &mut package,
        &verifiers,
        &ReportOptions {
            require_references: true,
            require_trusted_chain: true,
            certificate_policy: Some(policy),
            ..Default::default()
        },
    )
    .expect("report");
    assert_eq!(
        report.signatures[0]
            .certificate
            .as_ref()
            .map(|certificate| certificate.state),
        Some(CertificateState::Valid)
    );
    assert_eq!(report.verdict, ReportVerdict::AllValid);
}

#[test]
fn reports_missing_timestamp() {
    let (signed, _) = signed_package();
    let mut package = OfdPackage::from_bytes(signed).expect("open");
    let verifiers = SignatureVerifiers::new().with(Box::new(SesSignatureVerifier::new()));
    let report = verify_signature_report(
        &mut package,
        &verifiers,
        &ReportOptions {
            require_timestamp: true,
            ..Default::default()
        },
    )
    .expect("report");
    assert_eq!(report.signatures[0].state, SignatureState::TimestampMissing);
    assert_eq!(report.verdict, ReportVerdict::Invalid);
}
