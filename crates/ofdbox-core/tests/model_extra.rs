use ofdbox_core::model;
use ofdbox_core::model_xml;

fn roundtrip<T>(xml: &str) -> T
where
    T: yaserde::YaSerialize + yaserde::YaDeserialize + PartialEq + std::fmt::Debug,
{
    let parsed: T = model_xml::from_bytes(xml.as_bytes()).expect("parse");
    let written = model_xml::to_bytes(&parsed).expect("serialize");
    let reparsed: T = model_xml::from_bytes(&written).expect("reparse");
    assert_eq!(parsed, reparsed);
    reparsed
}

#[test]
fn res_with_colors_fonts_and_media() {
    let xml = r#"<Res BaseLoc="Res">
  <ColorSpaces>
    <ColorSpace ID="1" Type="RGB" BitsPerComponent="8">
      <Palette>0 0 0 255 255 255</Palette>
    </ColorSpace>
  </ColorSpaces>
  <DrawParams>
    <DrawParam ID="1" LineWidth="0.5" Join="Round" Cap="Round" MiterLimit="4" DashPattern="1 2"/>
  </DrawParams>
  <Fonts>
    <Font ID="1" FontName="SimSun" Charset="Unicode"/>
  </Fonts>
  <MultiMedias>
    <MultiMedia ID="2" Type="Image" Format="PNG"><MediaFile>img/a.png</MediaFile></MultiMedia>
  </MultiMedias>
</Res>"#;
    let res: model::res::Res = roundtrip(xml);
    assert_eq!(res.base_loc.as_str(), "Res");
    assert!(res.fonts.is_some());
    assert!(res.multi_medias.is_some());
}

#[test]
fn page_with_path_color_and_gradient() {
    let xml = r#"<Page>
  <Area><PhysicalBox>0 0 210 297</PhysicalBox></Area>
  <Content>
    <Layer ID="1" Type="Body">
      <PathObject ID="2" Boundary="0 0 100 100" Stroke="true" Fill="true" Rule="NonZero">
        <StrokeColor Value="255 0 0" Alpha="255"/>
        <FillColor Value="0 0 255" Alpha="128"/>
        <AbbreviatedData>M 0 0 L 100 100</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"#;
    let page: model::page::Page = model_xml::from_bytes(xml.as_bytes()).expect("parse");
    let written = model_xml::to_bytes(&page).expect("serialize");
    let page: model::page::Page = model_xml::from_bytes(&written).expect("reparse");
    let objects = &page.content.as_ref().unwrap().layers[0].objects;
    assert_eq!(objects.len(), 1);
    assert!(matches!(objects[0], model::page::GraphicObject::Path(_)));
}

#[test]
fn annotations_attachments_extensions_roundtrip() {
    let annotations = r#"<Annotations>
  <Page PageID="1"><FileLoc>Annots/Page_1.xml</FileLoc></Page>
</Annotations>"#;
    roundtrip::<model::annotations::Annotations>(annotations);

    let attachments = r#"<Attachments>
  <Attachment ID="1" Name="a.txt" Visible="true" Usage="none"><FileLoc>Attach/a.txt</FileLoc></Attachment>
</Attachments>"#;
    roundtrip::<model::attachments::Attachments>(attachments);

    let extensions = r#"<Extensions>
  <Extension AppName="demo" RefId="1">
    <Property Name="k" Type="String">v</Property>
    <Data><vendor:Foo xmlns:vendor="urn:foo" a="1">bar</vendor:Foo></Data>
    <ExtendData>Ext/data.bin</ExtendData>
  </Extension>
</Extensions>"#;
    roundtrip::<model::extensions::Extensions>(extensions);
}

#[test]
fn signature_roundtrip() {
    let xml = r#"<Signature>
  <SignedInfo>
    <Provider ProviderName="demo" Version="1.0"/>
    <SignatureMethod>1.2.156.10197.1.501</SignatureMethod>
    <SignatureDateTime>20261028000000</SignatureDateTime>
    <References CheckMethod="MD5">
      <Reference FileRef="/Doc_0/Document.xml"><CheckValue>aGVsbG8=</CheckValue></Reference>
    </References>
    <StampAnnot ID="1" PageRef="1" Boundary="10 10 20 20"/>
    <Seal><BaseLoc>Seal.esl</BaseLoc></Seal>
  </SignedInfo>
  <SignedValue>SignedValue.dat</SignedValue>
</Signature>"#;
    let signature: model::signature::Signature = roundtrip(xml);
    assert_eq!(signature.signed_info.references.references.len(), 1);
    assert!(signature.signed_info.seal.is_some());
}

#[test]
fn doc_entries_roundtrip() {
    let xml = r#"<DocEntries ID="1" CreatorName="ofdbox" Version="1.0" CreationDate="20261028">
  <FileList>
    <File ID="f1" FileLoc="/OFD.xml"/>
    <File ID="f2" FileLoc="/Doc_0/Document.xml"/>
  </FileList>
  <SignedValueLoc>/Integrity/SignedValue.dat</SignedValueLoc>
</DocEntries>"#;
    let entries: model::doc_entries::DocEntries = roundtrip(xml);
    assert_eq!(entries.file_list.files.len(), 2);
}
