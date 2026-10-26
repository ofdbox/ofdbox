use ofdbox_core::model::annotation::{Annotations, PageAnnot};
use ofdbox_core::model::attachment::Attachments;
use ofdbox_core::model::custom_tags::CustomTags;
use ofdbox_core::model::document::Document;
use ofdbox_core::model::ofd::OfdRoot;
use ofdbox_core::model::page::Page;
use ofdbox_core::model::res::Res;
use ofdbox_core::model::signature::{Signature, Signatures};
use ofdbox_core::model::version::Version;
use ofdbox_core::{OfdError, ReadOptions};

const OFD_INPUT: &str = include_str!("golden/33190/ofd_root.input.xml");
const OFD_EXPECTED: &str = include_str!("golden/33190/ofd_root.expected.xml");
const DOCUMENT_INPUT: &str = include_str!("golden/33190/document.input.xml");
const DOCUMENT_EXPECTED: &str = include_str!("golden/33190/document.expected.xml");
const RES_INPUT: &str = include_str!("golden/33190/res.input.xml");
const RES_EXPECTED: &str = include_str!("golden/33190/res.expected.xml");
const ANNOTATIONS_INPUT: &str = include_str!("golden/33190/annotations.input.xml");
const ANNOTATIONS_EXPECTED: &str = include_str!("golden/33190/annotations.expected.xml");
const PAGE_ANNOT_INPUT: &str = include_str!("golden/33190/page_annot.input.xml");
const PAGE_ANNOT_EXPECTED: &str = include_str!("golden/33190/page_annot.expected.xml");
const SIGNATURES_INPUT: &str = include_str!("golden/33190/signatures.input.xml");
const SIGNATURES_EXPECTED: &str = include_str!("golden/33190/signatures.expected.xml");
const SIGNATURE_INPUT: &str = include_str!("golden/33190/signature.input.xml");
const SIGNATURE_EXPECTED: &str = include_str!("golden/33190/signature.expected.xml");
const PAGE_INPUT: &str = include_str!("golden/33190/page.input.xml");
const PAGE_EXPECTED: &str = include_str!("golden/33190/page.expected.xml");
const ATTACHMENTS_INPUT: &str = include_str!("golden/33190/attachments.input.xml");
const ATTACHMENTS_EXPECTED: &str = include_str!("golden/33190/attachments.expected.xml");
const CUSTOM_TAGS_INPUT: &str = include_str!("golden/33190/custom_tags.input.xml");
const CUSTOM_TAGS_EXPECTED: &str = include_str!("golden/33190/custom_tags.expected.xml");
const DOC_VERSION_INPUT: &str = include_str!("golden/33190/doc_version.input.xml");
const DOC_VERSION_EXPECTED: &str = include_str!("golden/33190/doc_version.expected.xml");

fn parse<T>(input: &str, parse: impl Fn(&[u8], ReadOptions) -> Result<T, OfdError>) -> T {
    parse(input.as_bytes(), ReadOptions::STRICT).expect("strict parse")
}

#[test]
fn ofd_root_golden() {
    let view = parse(OFD_INPUT, OfdRoot::from_xml);
    let body = view.doc_bodies().next().expect("body");
    let doc_info = body.doc_info().expect("doc info");
    assert_eq!(doc_info.doc_id(), Some("doc-1"));
    assert_eq!(body.doc_root(), Some("Doc_0/Document.xml"));
    assert_eq!(body.signatures(), Some("Doc_0/Signs/Signatures.xml"));
    assert_eq!(view.to_xml().expect("xml"), OFD_EXPECTED);
    parse(OFD_EXPECTED, OfdRoot::from_xml);
}

#[test]
fn document_golden() {
    let view = parse(DOCUMENT_INPUT, Document::from_xml);
    assert_eq!(view.pages().expect("pages").pages().count(), 1);
    assert_eq!(view.annotations(), Some("Annots/Annotations.xml"));
    assert_eq!(view.to_xml().expect("xml"), DOCUMENT_EXPECTED);
    parse(DOCUMENT_EXPECTED, Document::from_xml);
}

#[test]
fn res_golden() {
    let view = parse(RES_INPUT, Res::from_xml);
    assert_eq!(view.fonts().count(), 1);
    assert_eq!(view.multi_medias().count(), 1);
    assert_eq!(view.to_xml().expect("xml"), RES_EXPECTED);
    parse(RES_EXPECTED, Res::from_xml);
}

#[test]
fn annotations_index_golden() {
    let view = parse(ANNOTATIONS_INPUT, Annotations::from_xml);
    let page = view.pages().next().expect("page");
    assert_eq!(page.page_id(), Some("1"));
    assert_eq!(page.file_loc(), Some("Page_0/Annotation.xml"));
    assert_eq!(view.to_xml().expect("xml"), ANNOTATIONS_EXPECTED);
    parse(ANNOTATIONS_EXPECTED, Annotations::from_xml);
}

#[test]
fn page_annot_golden() {
    let view = parse(PAGE_ANNOT_INPUT, PageAnnot::from_xml);
    let annot = view.annots().next().expect("annot");
    assert_eq!(annot.annot_type(), Some("Stamp"));
    assert_eq!(annot.id(), Some("1"));
    assert_eq!(view.to_xml().expect("xml"), PAGE_ANNOT_EXPECTED);
    parse(PAGE_ANNOT_EXPECTED, PageAnnot::from_xml);
}

#[test]
fn signatures_golden() {
    let view = parse(SIGNATURES_INPUT, Signatures::from_xml);
    assert_eq!(view.max_sign_id(), Some("s001"));
    let entry = view.signatures().next().expect("signature");
    assert_eq!(entry.signature_type(), Some("Sign"));
    assert_eq!(entry.base_loc(), Some("Sign_0/Signature.xml"));
    assert_eq!(view.to_xml().expect("xml"), SIGNATURES_EXPECTED);
    parse(SIGNATURES_EXPECTED, Signatures::from_xml);
}

#[test]
fn signature_golden() {
    let view = parse(SIGNATURE_INPUT, Signature::from_xml);
    let info = view.signed_info().expect("signed info");
    let provider = info.provider().expect("provider");
    assert_eq!(provider.provider_name(), Some("ofdbox"));
    assert_eq!(info.signature_method(), Some("1.2.156.10197.1.501"));
    let stamp = info.stamp_annots().next().expect("stamp");
    assert_eq!(stamp.id(), Some("1"));
    assert_eq!(stamp.page_ref(), Some("1"));
    assert_eq!(view.to_xml().expect("xml"), SIGNATURE_EXPECTED);
    parse(SIGNATURE_EXPECTED, Signature::from_xml);
}

#[test]
fn page_content_golden() {
    let view = parse(PAGE_INPUT, Page::from_xml);
    assert_eq!(view.content().expect("content").layers().count(), 1);
    assert_eq!(view.to_xml().expect("xml"), PAGE_EXPECTED);
    parse(PAGE_EXPECTED, Page::from_xml);
}

#[test]
fn attachments_golden() {
    let view = parse(ATTACHMENTS_INPUT, Attachments::from_xml);
    let attachment = view.attachments().next().expect("attachment");
    assert_eq!(attachment.id(), Some("1"));
    assert_eq!(attachment.name(), Some("a.txt"));
    assert_eq!(attachment.file_loc(), Some("Attachs/a.txt"));
    assert_eq!(view.to_xml().expect("xml"), ATTACHMENTS_EXPECTED);
    parse(ATTACHMENTS_EXPECTED, Attachments::from_xml);
}

#[test]
fn custom_tags_golden() {
    let view = parse(CUSTOM_TAGS_INPUT, CustomTags::from_xml);
    let tag = view.custom_tags().next().expect("tag");
    assert_eq!(tag.type_id(), Some("tag"));
    assert_eq!(tag.schema_loc(), Some("schema.xsd"));
    assert_eq!(tag.file_loc(), Some("tags.xml"));
    assert_eq!(view.to_xml().expect("xml"), CUSTOM_TAGS_EXPECTED);
    parse(CUSTOM_TAGS_EXPECTED, CustomTags::from_xml);
}

#[test]
fn doc_version_golden() {
    let view = parse(DOC_VERSION_INPUT, Version::from_xml);
    assert_eq!(view.id(), Some("1"));
    assert_eq!(view.version(), Some("1.0"));
    assert_eq!(view.files().count(), 1);
    assert_eq!(view.doc_root(), Some("/Doc_0/Document.xml"));
    assert_eq!(view.to_xml().expect("xml"), DOC_VERSION_EXPECTED);
    parse(DOC_VERSION_EXPECTED, Version::from_xml);
}

#[test]
fn missing_doc_root_is_strict_error() {
    let input = OFD_INPUT.replace("<ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>", "");
    assert!(OfdRoot::from_xml(input.as_bytes(), ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        OfdRoot::from_xml(input.as_bytes(), ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn missing_references_is_strict_error() {
    let input = SIGNATURE_INPUT.replace(
        r#"<ofd:References CheckMethod="1.2.156.10197.1.401"><ofd:Reference FileRef="/OFD.xml"><ofd:CheckValue>AAAA</ofd:CheckValue></ofd:Reference></ofd:References>"#,
        "",
    );
    assert!(Signature::from_xml(input.as_bytes(), ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        Signature::from_xml(input.as_bytes(), ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}
