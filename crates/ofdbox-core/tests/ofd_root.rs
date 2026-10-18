use ofdbox_core::{DiagnosticCode, OfdError, OfdRootView, ReadOptions};

const CANONICAL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo>
      <ofd:DocID>doc-1</ofd:DocID>
      <ofd:Title>A &amp; B</ofd:Title>
      <ofd:Keywords>
        <ofd:Keyword>alpha</ofd:Keyword>
        <ofd:Keyword>beta</ofd:Keyword>
      </ofd:Keywords>
    </ofd:DocInfo>
    <ofd:DocRoot>Document.xml</ofd:DocRoot>
    <ofd:Versions>
      <ofd:Version ID="v1" Index="1" Current="true" BaseLoc="Versions/Version_1.xml"/>
    </ofd:Versions>
    <ofd:Signatures>Signs/Signatures.xml</ofd:Signatures>
  </ofd:DocBody>
</ofd:OFD>"#;

const MISSING_2016: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>doc-1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const UNORDERED_DOCINFO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo>
      <ofd:Title>标题</ofd:Title>
      <ofd:DocID>doc-1</ofd:DocID>
    </ofd:DocInfo>
    <ofd:DocRoot>Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const UNQUALIFIED_CHILDREN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <DocBody>
    <DocInfo><DocID>doc-1</DocID></DocInfo>
    <DocRoot>Document.xml</DocRoot>
  </DocBody>
</ofd:OFD>"#;

const DEFAULT_NAMESPACE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" xmlns="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <DocBody>
    <DocInfo><DocID>doc-1</DocID></DocInfo>
    <DocRoot>Document.xml</DocRoot>
  </DocBody>
</ofd:OFD>"#;

const UNKNOWN_ELEMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>doc-1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Document.xml</ofd:DocRoot>
    <ofd:VendorExtra>ignored</ofd:VendorExtra>
  </ofd:DocBody>
</ofd:OFD>"#;

const MALFORMED: &str =
    r#"<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD"><ofd:DocBody>"#;

const MISSING_DOCID: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:Title>无 DocID</ofd:Title></ofd:DocInfo>
    <ofd:DocRoot>Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

fn parse(xml: &str, options: ReadOptions) -> Result<OfdRootView, OfdError> {
    OfdRootView::from_xml(xml.as_bytes(), options)
}

#[test]
fn canonical_parses_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse(CANONICAL, options).expect("canonical XML should parse");
        assert_eq!(view.version(), Some("1.0"));
        assert_eq!(view.doc_type(), Some("OFD"));
        let body = view.doc_bodies().next().expect("one DocBody");
        let info = body.doc_info().expect("DocInfo");
        assert_eq!(info.doc_id(), Some("doc-1"));
        assert_eq!(info.title(), Some("A & B"));
        assert_eq!(info.keywords().collect::<Vec<_>>(), ["alpha", "beta"]);
        assert_eq!(body.doc_root(), Some("Document.xml"));
        assert_eq!(body.signatures(), Some("Signs/Signatures.xml"));
        let version = body
            .versions()
            .expect("Versions")
            .versions()
            .next()
            .unwrap();
        assert_eq!(version.id(), Some("v1"));
        assert_eq!(version.index(), Some(1));
        assert!(version.current());
        assert_eq!(version.base_loc(), Some("Versions/Version_1.xml"));
    }
}

#[test]
fn lenient_normalizes_missing_2016_but_strict_rejects() {
    let view = parse(MISSING_2016, ReadOptions::LENIENT).expect("lenient should accept");
    assert_eq!(view.version(), Some("1.0"));
    assert!(view.doc_bodies().next().is_some());

    match parse(MISSING_2016, ReadOptions::STRICT) {
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
fn lenient_accepts_unordered_docinfo_but_strict_rejects() {
    let view = parse(UNORDERED_DOCINFO, ReadOptions::LENIENT).expect("lenient should accept");
    let info = view.doc_bodies().next().unwrap().doc_info().unwrap();
    assert_eq!(info.doc_id(), Some("doc-1"));
    assert_eq!(info.title(), Some("标题"));

    match parse(UNORDERED_DOCINFO, ReadOptions::STRICT) {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::OutOfOrder)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn lenient_accepts_unqualified_children_but_strict_rejects() {
    let view = parse(UNQUALIFIED_CHILDREN, ReadOptions::LENIENT).expect("lenient should accept");
    assert_eq!(
        view.doc_bodies().next().unwrap().doc_root(),
        Some("Document.xml")
    );

    match parse(UNQUALIFIED_CHILDREN, ReadOptions::STRICT) {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::MissingNamespace)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn strict_accepts_default_namespace() {
    let view = parse(DEFAULT_NAMESPACE, ReadOptions::STRICT).expect("default namespace is valid");
    assert_eq!(view.version(), Some("1.0"));
}

#[test]
fn lenient_preserves_unknown_but_strict_rejects() {
    let view = parse(UNKNOWN_ELEMENT, ReadOptions::LENIENT).expect("lenient should accept");
    let body = view
        .document()
        .find_child(view.node(), "DocBody")
        .expect("DocBody");
    assert!(view.document().find_child(body, "VendorExtra").is_some());

    match parse(UNKNOWN_ELEMENT, ReadOptions::STRICT) {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::UnexpectedElement)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
    assert!(matches!(view.to_xml(), Err(OfdError::Conformance(_))));
}

#[test]
fn malformed_xml_is_an_xml_error() {
    assert!(matches!(
        parse(MALFORMED, ReadOptions::LENIENT),
        Err(OfdError::Xml(_))
    ));
    assert!(matches!(
        parse(MALFORMED, ReadOptions::STRICT),
        Err(OfdError::Xml(_))
    ));
}

#[test]
fn writer_emits_canonical_order_and_roundtrips() {
    let view = parse(CANONICAL, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer should succeed");
    assert!(xml.contains(r#"xmlns:ofd="http://www.ofdspec.org/2016""#));
    assert!(xml.contains("<ofd:OFD"));
    assert!(xml.contains("<ofd:DocInfo>"));
    assert!(xml.contains("<ofd:DocRoot>"));
    assert!(xml.contains("<ofd:Versions>"));
    assert!(xml.contains("<ofd:Signatures>"));

    let doc_info = xml.find("<ofd:DocInfo>").unwrap();
    let doc_root = xml.find("<ofd:DocRoot>").unwrap();
    let versions = xml.find("<ofd:Versions>").unwrap();
    let signatures = xml.find("<ofd:Signatures>").unwrap();
    assert!(doc_info < doc_root);
    assert!(doc_root < versions);
    assert!(versions < signatures);

    let reparsed = OfdRootView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
        .expect("canonical output should pass strict parsing");
    assert_eq!(reparsed.version(), Some("1.0"));
}

#[test]
fn writer_rejects_missing_required_fields() {
    let view = parse(MISSING_DOCID, ReadOptions::LENIENT).expect("lenient should accept");
    match view.to_xml() {
        Err(OfdError::Conformance(diagnostics)) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::MissingRequired)
            );
        }
        other => panic!("expected Conformance, got {other:?}"),
    }
}

#[test]
fn bom_is_accepted() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(CANONICAL.as_bytes());
    assert!(OfdRootView::from_xml(&bytes, ReadOptions::LENIENT).is_ok());
    assert!(OfdRootView::from_xml(&bytes, ReadOptions::STRICT).is_ok());
}
