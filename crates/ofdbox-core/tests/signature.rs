mod fixtures;

use ofdbox_core::{
    DiagnosticCode, OfdError, OfdPackage, ReadOptions, SignatureView, SignaturesView,
};

const SIGNATURES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Signatures xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:MaxSignId>s2</ofd:MaxSignId>
  <ofd:Signature ID="s1" Type="Seal" BaseLoc="Signs/Sign_0/Signature.xml"/>
</ofd:Signatures>"#;

const SIGNATURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Signature xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:SignedInfo>
    <ofd:Provider ProviderName="ExampleProvider" Version="1.0" Company="Example"/>
    <ofd:SignatureMethod>SM2-SM3</ofd:SignatureMethod>
    <ofd:SignatureDateTime>2024-01-02T03:04:05Z</ofd:SignatureDateTime>
    <ofd:References CheckMethod="SHA1">
      <ofd:Reference FileRef="/Doc_0/Document.xml"><ofd:CheckValue>AAAA</ofd:CheckValue></ofd:Reference>
      <ofd:Reference FileRef="/Doc_0/Pages/Page_0/Content.xml"><ofd:CheckValue>BBBB</ofd:CheckValue></ofd:Reference>
    </ofd:References>
    <ofd:StampAnnot ID="s2" PageRef="10" Boundary="0 0 100 50" Clip="0 0 100 50"/>
    <ofd:Seal><ofd:BaseLoc>Signs/Seal.esl</ofd:BaseLoc></ofd:Seal>
  </ofd:SignedInfo>
  <ofd:SignedValue>Signs/Sign_0/SignedValue.dat</ofd:SignedValue>
</ofd:Signature>"#;

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>doc-1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
    <ofd:Signatures>Doc_0/Signatures.xml</ofd:Signatures>
  </ofd:DocBody>
</ofd:OFD>"#;

fn parse_signatures(xml: &str, options: ReadOptions) -> Result<SignaturesView, OfdError> {
    SignaturesView::from_xml(xml.as_bytes(), options)
}

fn parse_signature(xml: &str, options: ReadOptions) -> Result<SignatureView, OfdError> {
    SignatureView::from_xml(xml.as_bytes(), options)
}

#[test]
fn canonical_signatures_index_parses_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse_signatures(SIGNATURES, options).expect("canonical index");
        assert_eq!(view.max_sign_id(), Some("s2"));
        let signature = view.signatures().next().expect("Signature");
        assert_eq!(signature.id(), Some("s1"));
        assert_eq!(signature.signature_type(), Some("Seal"));
        assert_eq!(signature.base_loc(), Some("Signs/Sign_0/Signature.xml"));
    }
}

#[test]
fn signatures_writer_emits_canonical_index() {
    let view = parse_signatures(SIGNATURES, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:Signatures"));
    let max_id = xml.find("<ofd:MaxSignId>").unwrap();
    let signature = xml.find("<ofd:Signature ").unwrap();
    assert!(max_id < signature);
    let reparsed =
        SignaturesView::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    assert_eq!(reparsed.signatures().next().unwrap().id(), Some("s1"));
}

#[test]
fn missing_base_loc_is_strict_error() {
    let missing = SIGNATURES.replace(r#" BaseLoc="Signs/Sign_0/Signature.xml""#, "");
    let view = parse_signatures(&missing, ReadOptions::LENIENT).expect("lenient");
    assert_eq!(view.signatures().next().unwrap().base_loc(), None);
    assert!(matches!(
        parse_signatures(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn canonical_signature_parses_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse_signature(SIGNATURE, options).expect("canonical Signature");
        let info = view.signed_info().expect("SignedInfo");
        let provider = info.provider().expect("Provider");
        assert_eq!(provider.provider_name(), Some("ExampleProvider"));
        assert_eq!(provider.version(), Some("1.0"));
        assert_eq!(provider.company(), Some("Example"));
        assert_eq!(info.signature_method(), Some("SM2-SM3"));
        assert_eq!(info.signature_date_time(), Some("2024-01-02T03:04:05Z"));

        let references = info.references().expect("References");
        assert_eq!(references.check_method(), Some("SHA1"));
        let reference = references.references().next().expect("Reference");
        assert_eq!(reference.file_ref(), Some("/Doc_0/Document.xml"));
        assert_eq!(reference.check_value(), "AAAA");

        let stamp = info.stamp_annots().next().expect("StampAnnot");
        assert_eq!(stamp.id(), Some("s2"));
        assert_eq!(stamp.page_ref(), Some("10"));
        assert_eq!(stamp.boundary(), Some("0 0 100 50"));
        assert_eq!(stamp.clip(), Some("0 0 100 50"));

        assert_eq!(info.seal().unwrap().base_loc(), Some("Signs/Seal.esl"));
        assert_eq!(view.signed_value(), Some("Signs/Sign_0/SignedValue.dat"));
    }
}

#[test]
fn signature_writer_roundtrips() {
    let view = parse_signature(SIGNATURE, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    let provider = xml.find("<ofd:Provider").unwrap();
    let method = xml.find("<ofd:SignatureMethod>").unwrap();
    let date_time = xml.find("<ofd:SignatureDateTime>").unwrap();
    let references = xml.find("<ofd:References").unwrap();
    let stamp = xml.find("<ofd:StampAnnot").unwrap();
    let seal = xml.find("<ofd:Seal>").unwrap();
    let signed_value = xml.find("<ofd:SignedValue>").unwrap();
    assert!(provider < method);
    assert!(method < date_time);
    assert!(date_time < references);
    assert!(references < stamp);
    assert!(stamp < seal);
    assert!(seal < signed_value);

    let reparsed =
        SignatureView::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    assert_eq!(
        reparsed
            .signed_info()
            .unwrap()
            .references()
            .unwrap()
            .references()
            .count(),
        2
    );
}

#[test]
fn missing_provider_name_is_strict_error() {
    let missing = SIGNATURE.replace(r#" ProviderName="ExampleProvider""#, "");
    let view = parse_signature(&missing, ReadOptions::LENIENT).expect("lenient");
    assert_eq!(
        view.signed_info()
            .unwrap()
            .provider()
            .unwrap()
            .provider_name(),
        None
    );
    assert!(matches!(
        parse_signature(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn unknown_check_method_is_strict_error() {
    let unknown = SIGNATURE.replace(r#"CheckMethod="SHA1""#, r#"CheckMethod="SHA256""#);
    assert!(parse_signature(&unknown, ReadOptions::LENIENT).is_ok());
    match parse_signature(&unknown, ReadOptions::STRICT) {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::InvalidValue)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn lenient_normalizes_missing_2016_but_strict_rejects() {
    let dirty = SIGNATURE.replace("http://www.ofdspec.org/2016", "http://www.ofdspec.org/");
    assert!(parse_signature(&dirty, ReadOptions::LENIENT).is_ok());
    match parse_signature(&dirty, ReadOptions::STRICT) {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::NamespaceVariant)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn package_resolves_signatures_and_signature() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Signatures.xml", SIGNATURES.as_bytes()),
        ("Doc_0/Signs/Sign_0/Signature.xml", SIGNATURE.as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");

    let signatures = package.parse_signatures().expect("Signatures");
    let entry = signatures.signatures().next().expect("Signature entry");
    assert_eq!(entry.id(), Some("s1"));
    let base_loc = entry.base_loc().expect("BaseLoc").to_owned();

    let signature = package
        .parse_signature(&base_loc)
        .expect("Signature should resolve from signatures base");
    assert_eq!(
        signature
            .signed_info()
            .unwrap()
            .provider()
            .unwrap()
            .provider_name(),
        Some("ExampleProvider")
    );
}

const SIGNATURE_0099: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Signature xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:SignedValue>Signs/Sign_0/SignedValue.dat</ofd:SignedValue>
  <ofd:SignedInfo>
    <ofd:Provider ProviderName="Provider" Company="Example" Version="1.0" ProtocolVer="2.0">
      <ofd:ExtendData>AAAA</ofd:ExtendData>
    </ofd:Provider>
    <ofd:SignatureDateTime>2024-01-02T03:04:05Z</ofd:SignatureDateTime>
    <ofd:SignatureMethod>SM2-SM3</ofd:SignatureMethod>
    <ofd:Parameters>
      <ofd:Parameter Name="role" Type="string">owner</ofd:Parameter>
    </ofd:Parameters>
    <ofd:Seal>
      <ofd:BaseLoc>Signs/Seal.esl</ofd:BaseLoc>
      <ofd:ImageLoc>Signs/Seal.png</ofd:ImageLoc>
    </ofd:Seal>
    <ofd:StampAnnot ID="s2" PageRef="10" Boundary="0 0 100 50"/>
    <ofd:References CheckMethod="SM3">
      <ofd:Reference FileRef="/OFD.xml"><ofd:CheckValue>AAAA</ofd:CheckValue></ofd:Reference>
    </ofd:References>
  </ofd:SignedInfo>
</ofd:Signature>"#;

#[test]
fn signature_0099_extensions_parse_and_roundtrip() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse_signature(SIGNATURE_0099, options).expect("0099 signature");
        let info = view.signed_info().unwrap();
        let provider = info.provider().unwrap();
        assert_eq!(provider.protocol_version(), Some("2.0"));
        assert_eq!(provider.extend_data(), Some("AAAA"));
        let parameter = info.parameters().next().expect("Parameter");
        assert_eq!(parameter.name(), Some("role"));
        assert_eq!(parameter.value_type(), Some("string"));
        assert_eq!(parameter.value(), "owner");
        let seal = info.seal().unwrap();
        assert_eq!(seal.base_loc(), Some("Signs/Seal.esl"));
        assert_eq!(seal.image_loc(), Some("Signs/Seal.png"));
        assert_eq!(info.references().unwrap().check_method(), Some("SM3"));
    }

    let view = parse_signature(SIGNATURE_0099, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains(r#"ProtocolVer="2.0""#));
    assert!(xml.contains("<ofd:ExtendData>AAAA</ofd:ExtendData>"));
    assert!(xml.contains(r#"<ofd:Parameter Name="role" Type="string">owner</ofd:Parameter>"#));
    assert!(xml.contains("<ofd:ImageLoc>Signs/Seal.png</ofd:ImageLoc>"));
    assert!(xml.contains(r#"CheckMethod="SM3""#));
    let reparsed =
        SignatureView::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    assert_eq!(
        reparsed
            .signed_info()
            .unwrap()
            .provider()
            .unwrap()
            .protocol_version(),
        Some("2.0")
    );
}

#[test]
fn signatures_index_relative_is_parsed() {
    let with_relative = SIGNATURES.replace(r#"ID="s1""#, r#"ID="s1" Relative="s0""#);
    let view = parse_signatures(&with_relative, ReadOptions::STRICT).expect("Relative");
    assert_eq!(view.signatures().next().unwrap().relative(), Some("s0"));
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains(r#"Relative="s0""#));
}
