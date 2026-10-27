use ofdbox_core::schema::annotation::PageAnnot;
use ofdbox_core::schema::annotations::Annotations;
use ofdbox_core::schema::attachments::Attachments;
use ofdbox_core::schema::custom_tags::CustomTags;
use ofdbox_core::schema::document::Document;
use ofdbox_core::schema::extensions::Extensions;
use ofdbox_core::schema::res::Res;
use ofdbox_core::schema::signature::Signature;
use ofdbox_core::schema::signatures::Signatures;
use ofdbox_core::schema::version::DocVersion;
use ofdbox_core::schema_xml as xml;

fn roundtrip<T>(xml_text: &str)
where
    T: yaserde::YaDeserialize + yaserde::YaSerialize + PartialEq + std::fmt::Debug,
{
    let parsed: T = xml::from_str(xml_text).expect("parse");
    let written = xml::to_string(&parsed).expect("write");
    let reparsed: T = xml::from_str(&written).expect("reparse");
    assert_eq!(parsed, reparsed, "{written}");
}

#[test]
fn document_roundtrip() {
    roundtrip::<Document>(
        r#"<Document xmlns="http://www.ofdspec.org/2016"><CommonData><MaxUnitID>10</MaxUnitID><PageArea><PhysicalBox>0 0 210 297</PhysicalBox></PageArea><DocumentRes>Res/DocumentRes.xml</DocumentRes></CommonData><Pages><Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></Pages></Document>"#,
    );
}

#[test]
fn res_roundtrip() {
    roundtrip::<Res>(
        r#"<Res xmlns="http://www.ofdspec.org/2016" BaseLoc="."><Fonts><Font ID="1" FontName="SimSun" Charset="unicode" Italic="false" Bold="false" Serif="false" FixedWidth="false"/></Fonts><MultiMedias><MultiMedia ID="2" Type="Image" Format="PNG"><MediaFile>Image_2.png</MediaFile></MultiMedia></MultiMedias></Res>"#,
    );
}

#[test]
fn annotations_roundtrip() {
    roundtrip::<Annotations>(
        r#"<Annotations xmlns="http://www.ofdspec.org/2016"><Page PageID="1"><FileLoc>Pages/Page_0/Annotation.xml</FileLoc></Page></Annotations>"#,
    );
}

#[test]
fn attachments_roundtrip() {
    roundtrip::<Attachments>(
        r#"<Attachments xmlns="http://www.ofdspec.org/2016"><Attachment ID="a1" Name="readme" Format="txt" CreationDate="2020-01-01T00:00:00" ModDate="2020-01-01T00:00:00" Size="3.0" Visible="true" Usage="none"><FileLoc>Attachments/a1.txt</FileLoc></Attachment></Attachments>"#,
    );
}

#[test]
fn custom_tags_roundtrip() {
    roundtrip::<CustomTags>(
        r#"<CustomTags xmlns="http://www.ofdspec.org/2016"><CustomTag NameSpace="urn:demo"><SchemaLoc>Tags/demo.xsd</SchemaLoc><FileLoc>Tags/demo.xml</FileLoc></CustomTag></CustomTags>"#,
    );
}

#[test]
fn extensions_roundtrip() {
    roundtrip::<Extensions>(
        r#"<Extensions xmlns="http://www.ofdspec.org/2016"><Extension AppName="demo" RefId="1"><Property Name="p" Type="string">value</Property><Data>raw</Data><ExtendData>Ext/binary.dat</ExtendData></Extension></Extensions>"#,
    );
}

#[test]
fn signature_roundtrip() {
    roundtrip::<Signature>(
        r#"<Signature xmlns="http://www.ofdspec.org/2016"><SignedInfo><Provider ProviderName="demo"/><References CheckMethod="MD5"><Reference FileRef="Doc_0/Document.xml"><CheckValue>YWJj</CheckValue></Reference></References><StampAnnot ID="s1" PageRef="1" Boundary="0 0 10 10"/></SignedInfo><SignedValue>Signs/Sign_0/SignedValue.dat</SignedValue></Signature>"#,
    );
}

#[test]
fn signatures_roundtrip() {
    roundtrip::<Signatures>(
        r#"<Signatures xmlns="http://www.ofdspec.org/2016"><MaxSignId>s1</MaxSignId><Signature ID="s1" Type="Seal" BaseLoc="Signs/Sign_0/Signature.xml"/></Signatures>"#,
    );
}

#[test]
fn version_roundtrip() {
    roundtrip::<DocVersion>(
        r#"<DocVersion xmlns="http://www.ofdspec.org/2016" ID="v1" Version="1" Name="v1" CreationDate="2020-01-01"><FileList><File ID="f1">Doc_0/Document.xml</File></FileList><DocRoot>Doc_0/Document.xml</DocRoot></DocVersion>"#,
    );
}

#[test]
fn page_annot_roundtrip() {
    roundtrip::<PageAnnot>(
        r#"<PageAnnot xmlns="http://www.ofdspec.org/2016"><Annot ID="1" Type="Stamp" Creator="demo" LastModDate="2020-01-01" Visible="true" Print="true" NoZoom="false" NoRotate="false" ReadOnly="true"><Appearance Boundary="0 0 10 10"/></Annot></PageAnnot>"#,
    );
}

#[test]
fn extension_data_raw_xml() {
    let input = r#"<Extensions xmlns="http://www.ofdspec.org/2016"><Extension AppName="demo" RefId="1"><Data><vendor:Foo xmlns:vendor="urn:foo" a="1">bar</vendor:Foo></Data></Extension></Extensions>"#;
    let parsed: Extensions = xml::from_str(input).expect("parse");
    let written = xml::to_string(&parsed).expect("write");
    println!("{written}");
    let reparsed: Extensions = xml::from_str(&written).expect("reparse");
    assert_eq!(parsed, reparsed);
    assert!(written.contains("<vendor:Foo"), "{written}");
    assert!(written.contains("xmlns:vendor=\"urn:foo\""), "{written}");
}
