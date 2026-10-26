mod fixtures;

use ofdbox_core::model::attachment::Attachments;
use ofdbox_core::{OfdError, OfdPackage, ReadOptions};

const ATTACHMENTS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Attachments xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Attachment ID="a1" Name="说明.txt" Format="txt" CreationDate="2024-01-02T03:04:05Z"
                  ModDate="2024-01-03T03:04:05Z" Size="12.5" Visible="true" Usage="none">
    <ofd:FileLoc>Attachments/a1.txt</ofd:FileLoc>
  </ofd:Attachment>
  <ofd:Attachment ID="a2" Name="附件.pdf" Visible="false">
    <ofd:FileLoc>Attachments/a2.pdf</ofd:FileLoc>
  </ofd:Attachment>
</ofd:Attachments>"#;

const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>doc-1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>20</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="10" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
  <ofd:Attachments>Attachments.xml</ofd:Attachments>
</ofd:Document>"#;

fn parse(xml: &str, options: ReadOptions) -> Result<Attachments, OfdError> {
    Attachments::from_xml(xml.as_bytes(), options)
}

#[test]
fn canonical_attachments_parse_in_both_modes() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse(ATTACHMENTS, options).expect("canonical Attachments");
        let attachments: Vec<_> = view.attachments().collect();
        assert_eq!(attachments.len(), 2);

        let first = &attachments[0];
        assert_eq!(first.id(), Some("a1"));
        assert_eq!(first.name(), Some("说明.txt"));
        assert_eq!(first.format(), Some("txt"));
        assert_eq!(first.creation_date(), Some("2024-01-02T03:04:05Z"));
        assert_eq!(first.mod_date(), Some("2024-01-03T03:04:05Z"));
        assert_eq!(first.size(), Some(12.5));
        assert_eq!(first.visible(), Some(true));
        assert_eq!(first.usage(), Some("none"));
        assert_eq!(first.file_loc(), Some("Attachments/a1.txt"));

        assert_eq!(attachments[1].visible(), Some(false));
    }
}

#[test]
fn writer_emits_canonical_attachments() {
    let view = parse(ATTACHMENTS, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:Attachments"));
    assert!(xml.contains(r#"<ofd:Attachment ID="a1" Name="说明.txt""#));
    let reparsed =
        Attachments::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    assert_eq!(reparsed.attachments().count(), 2);
}

#[test]
fn missing_name_is_strict_error() {
    let missing = ATTACHMENTS.replace(r#" Name="说明.txt""#, "");
    let view = parse(&missing, ReadOptions::LENIENT).expect("lenient");
    assert_eq!(view.attachments().next().unwrap().name(), None);
    assert!(matches!(
        parse(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn invalid_size_is_strict_error() {
    let invalid = ATTACHMENTS.replace(r#" Size="12.5""#, r#" Size="large""#);
    let view = parse(&invalid, ReadOptions::LENIENT).expect("lenient");
    assert_eq!(view.attachments().next().unwrap().size(), None);
    assert!(matches!(
        parse(&invalid, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn package_resolves_attachments_and_entry() {
    let bytes = fixtures::zip_with_entries(&[
        ("OFD.xml", ROOT.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Attachments.xml", ATTACHMENTS.as_bytes()),
        ("Doc_0/Attachments/a1.txt", b"hello"),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");
    let attachments = package.parse_attachments().expect("Attachments");
    let first = attachments.attachments().next().expect("Attachment");
    assert_eq!(first.name(), Some("说明.txt"));

    let content = package
        .read_entry("Doc_0/Attachments/a1.txt")
        .expect("attachment bytes");
    assert_eq!(&*content, b"hello");
}
