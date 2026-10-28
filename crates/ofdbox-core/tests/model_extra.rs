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
    let parsed: model::page::Page = model_xml::from_bytes(xml.as_bytes()).expect("parse");
    let written = model_xml::to_bytes(&parsed).expect("serialize");
    let page: model::page::Page = model_xml::from_bytes(&written).expect("reparse");
    assert_eq!(parsed, page);
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

#[test]
fn page_annot_text_roundtrip() {
    let xml = r#"<PageAnnot>
  <Annot ID="1" Type="Highlight" Creator="tester" LastModDate="20261028">
    <Appearance Boundary="10 10 100 30">
      <TextObject ID="2" Boundary="10 10 100 30" Font="1" Size="12">
        <TextCode X="0" Y="0">hello</TextCode>
      </TextObject>
    </Appearance>
  </Annot>
</PageAnnot>"#;
    let parsed: model::annotation::PageAnnot =
        model_xml::from_bytes(xml.as_bytes()).expect("parse");
    let written = model_xml::to_bytes(&parsed).expect("serialize");
    let reparsed: model::annotation::PageAnnot = model_xml::from_bytes(&written).expect("reparse");
    assert_eq!(parsed, reparsed);
}

#[test]
fn signatures_index_roundtrip() {
    let xml = r#"<Signatures>
  <MaxSignId>2</MaxSignId>
  <Signature ID="1" Type="Sign" BaseLoc="Signs/Sign_0/Signature.xml"/>
  <Signature ID="2" Type="Seal" BaseLoc="Signs/Sign_1/Signature.xml"/>
</Signatures>"#;
    let parsed: model::signatures::Signatures =
        model_xml::from_bytes(xml.as_bytes()).expect("parse");
    let written = model_xml::to_bytes(&parsed).expect("serialize");
    let reparsed: model::signatures::Signatures = model_xml::from_bytes(&written).expect("reparse");
    assert_eq!(parsed, reparsed);
}

#[test]
fn page_with_axial_gradient() {
    let xml = r#"<Page>
  <Content>
    <Layer ID="1">
      <PathObject ID="2" Boundary="0 0 100 100" Fill="true">
        <FillColor>
          <AxialShd StartPoint="0 0" EndPoint="100 0">
            <Segment Position="0"><Color Value="255 0 0"/></Segment>
            <Segment Position="1"><Color Value="0 0 255"/></Segment>
          </AxialShd>
        </FillColor>
        <AbbreviatedData>M 0 0 L 100 0</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"#;
    let parsed: model::page::Page = model_xml::from_bytes(xml.as_bytes()).expect("parse");
    let written = model_xml::to_bytes(&parsed).expect("serialize");
    let reparsed: model::page::Page = model_xml::from_bytes(&written).expect("reparse");
    assert_eq!(parsed, reparsed);
}

#[test]
fn page_with_radial_gradient() {
    let xml = r#"<Page><Content><Layer ID="1"><PathObject ID="2" Boundary="0 0 100 100" Fill="true">
      <FillColor><RadialShd StartPoint="50 50" EndPoint="50 50" EndRadius="50">
        <Segment Position="0"><Color Value="255 0 0"/></Segment>
        <Segment Position="1"><Color Value="0 0 255"/></Segment>
      </RadialShd></FillColor>
      <AbbreviatedData>M 0 0 L 100 0</AbbreviatedData>
    </PathObject></Layer></Content></Page>"#;
    let parsed: model::page::Page = model_xml::from_bytes(xml.as_bytes()).expect("parse");
    let written = model_xml::to_bytes(&parsed).expect("serialize");
    let reparsed: model::page::Page = model_xml::from_bytes(&written).expect("reparse");
    assert_eq!(parsed, reparsed);
}

#[test]
fn page_with_gouraud_gradient() {
    let xml = r#"<Page><Content><Layer ID="1"><PathObject ID="2" Boundary="0 0 100 100" Fill="true">
      <FillColor><GouraudShd>
        <Point X="0" Y="0"><Color Value="255 0 0"/></Point>
        <Point X="100" Y="0"><Color Value="0 0 255"/></Point>
      </GouraudShd></FillColor>
      <AbbreviatedData>M 0 0 L 100 0</AbbreviatedData>
    </PathObject></Layer></Content></Page>"#;
    let parsed: model::page::Page = model_xml::from_bytes(xml.as_bytes()).expect("parse");
    let written = model_xml::to_bytes(&parsed).expect("serialize");
    let reparsed: model::page::Page = model_xml::from_bytes(&written).expect("reparse");
    assert_eq!(parsed, reparsed);
}

#[test]
fn page_with_la_gouraud_gradient() {
    let xml = r#"<Page><Content><Layer ID="1"><PathObject ID="2" Boundary="0 0 100 100" Fill="true">
      <FillColor><LaGourandShd VerticesPerRow="2">
        <Point X="0" Y="0"><Color Value="255 0 0"/></Point>
        <Point X="100" Y="0"><Color Value="0 0 255"/></Point>
      </LaGourandShd></FillColor>
      <AbbreviatedData>M 0 0 L 100 0</AbbreviatedData>
    </PathObject></Layer></Content></Page>"#;
    let parsed: model::page::Page = model_xml::from_bytes(xml.as_bytes()).expect("parse");
    let written = model_xml::to_bytes(&parsed).expect("serialize");
    let reparsed: model::page::Page = model_xml::from_bytes(&written).expect("reparse");
    assert_eq!(parsed, reparsed);
}

#[test]
fn document_outlines_permissions_preferences() {
    let xml = r#"<Document>
  <CommonData>
    <MaxUnitID>1</MaxUnitID>
    <PageArea><PhysicalBox>0 0 210 297</PhysicalBox></PageArea>
  </CommonData>
  <Pages><Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></Pages>
  <Outlines>
    <OutlineElem Title="root" Expanded="true">
      <OutlineElem Title="child"/>
    </OutlineElem>
  </Outlines>
  <Permissions>
    <Edit>true</Edit>
    <Print Printable="true" Copies="2"/>
  </Permissions>
  <VPreferences>
    <PageMode>FullScreen</PageMode>
    <PageLayout>SinglePage</PageLayout>
  </VPreferences>
</Document>"#;
    let parsed: model::document::Document = model_xml::from_bytes(xml.as_bytes()).expect("parse");
    let written = model_xml::to_bytes(&parsed).expect("serialize");
    let reparsed: model::document::Document = model_xml::from_bytes(&written).expect("reparse");
    assert_eq!(parsed, reparsed);
}

#[test]
fn page_with_clip_and_composite() {
    let xml = r#"<Page>
  <Content>
    <Layer ID="1">
      <CompositeObject ID="2" Boundary="0 0 100 100" ResourceID="10"/>
      <PathObject ID="3" Boundary="0 0 100 100">
        <AbbreviatedData>M 0 0 L 100 100</AbbreviatedData>
        <Clips>
          <Clip>
            <Area><Path><AbbreviatedData>M 0 0 L 50 50</AbbreviatedData><PathObject/></Path></Area>
          </Clip>
        </Clips>
      </PathObject>
    </Layer>
  </Content>
</Page>"#;
    let parsed: model::page::Page = model_xml::from_bytes(xml.as_bytes()).expect("parse");
    let written = model_xml::to_bytes(&parsed).expect("serialize");
    let reparsed: model::page::Page = model_xml::from_bytes(&written).expect("reparse");
    assert_eq!(parsed, reparsed);
}

#[test]
fn gouraud_gradient_back_color() {
    let xml = r#"<Page><Content><Layer ID="1"><PathObject ID="2" Boundary="0 0 100 100" Fill="true">
      <FillColor><GouraudShd>
        <BackColor Value="255 255 255"/>
        <Point X="0" Y="0"><Color Value="255 0 0"/></Point>
        <Point X="100" Y="0"><Color Value="0 0 255"/></Point>
      </GouraudShd></FillColor>
      <AbbreviatedData>M 0 0 L 100 0</AbbreviatedData>
    </PathObject></Layer></Content></Page>"#;
    let parsed: model::page::Page = model_xml::from_bytes(xml.as_bytes()).expect("parse");
    let written = model_xml::to_bytes(&parsed).expect("serialize");
    let reparsed: model::page::Page = model_xml::from_bytes(&written).expect("reparse");
    assert_eq!(parsed, reparsed);
}

#[test]
fn empty_color_matches_default() {
    let parsed: model::page::Color = model_xml::from_str("<Color/>").expect("parse");
    assert_eq!(parsed, model::page::Color::default());
    assert!(matches!(parsed.kind, model::page::ColorKind::None));
}
