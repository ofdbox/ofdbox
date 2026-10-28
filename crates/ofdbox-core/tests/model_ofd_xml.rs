use ofdbox_core::model::definition::StLoc;
use ofdbox_core::model::ofd::{DocBody, DocInfo, Ofd, VersionRef, Versions};
use ofdbox_core::model_xml as xml;

#[test]
fn parses_and_writes_ofd_root() {
    let xml_text = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo>
      <ofd:DocID>demo</ofd:DocID>
      <ofd:Title>示例</ofd:Title>
      <ofd:Keywords>
        <ofd:Keyword>OFD</ofd:Keyword>
        <ofd:Keyword>Demo</ofd:Keyword>
      </ofd:Keywords>
    </ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
    <ofd:Versions>
      <ofd:Version ID="v1" Index="1" Current="true" BaseLoc="Doc_0/Versions/v1.xml"/>
    </ofd:Versions>
  </ofd:DocBody>
</ofd:OFD>"#;

    let parsed: Ofd = xml::from_str(xml_text).expect("parse OFD");
    assert_eq!(parsed.version, "1.0");
    assert_eq!(parsed.doc_type, "OFD");
    assert_eq!(parsed.doc_bodies.len(), 1);
    let body = &parsed.doc_bodies[0];
    assert_eq!(body.doc_info.doc_id, "demo");
    assert_eq!(body.doc_info.title.as_deref(), Some("示例"));
    assert_eq!(body.doc_root.as_str(), "Doc_0/Document.xml");
    assert_eq!(
        body.versions.as_ref().unwrap().versions[0]
            .base_loc
            .as_str(),
        "Doc_0/Versions/v1.xml"
    );

    let written = xml::to_string(&parsed).expect("write OFD");
    assert!(written.contains(r#"xmlns:ofd="http://www.ofdspec.org/2016""#));
    assert!(written.contains("<ofd:OFD"));
    let reparsed: Ofd = xml::from_str(&written).expect("reparse OFD");
    assert_eq!(reparsed, parsed);
}

#[test]
fn builds_ofd_xml() {
    let ofd = Ofd {
        version: "1.0".to_owned(),
        doc_type: "OFD".to_owned(),
        doc_bodies: vec![DocBody {
            doc_info: DocInfo {
                doc_id: "demo".to_owned(),
                title: Some("示例".to_owned()),
                ..Default::default()
            },
            doc_root: StLoc::new("Doc_0/Document.xml"),
            versions: Some(Versions {
                versions: vec![VersionRef {
                    id: "v1".to_owned(),
                    index: 1,
                    current: true,
                    base_loc: StLoc::new("Doc_0/Versions/v1.xml"),
                }],
            }),
            signatures: None,
        }],
    };

    let written = xml::to_string(&ofd).expect("serialize");
    assert!(written.contains("<ofd:DocID>demo</ofd:DocID>"), "{written}");
    assert!(
        written.contains("<ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>"),
        "{written}"
    );
    let reparsed: Ofd = xml::from_str(&written).expect("reparse");
    assert_eq!(reparsed, ofd);
}
