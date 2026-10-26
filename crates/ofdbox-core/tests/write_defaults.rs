use ofdbox_core::ReadOptions;
use ofdbox_core::model::annotation::PageAnnot;
use ofdbox_core::model::attachment::Attachments;
use ofdbox_core::model::document::Document;
use ofdbox_core::model::page::Page;
use ofdbox_core::model::res::Res;
use ofdbox_core::model::signature::{Signature, Signatures};

const PAGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Template TemplateID="1" ZOrder="Background"/>
  <ofd:Content>
    <ofd:Layer ID="1" Type="Body">
      <ofd:TextObject ID="2" Boundary="0 0 1 1" Font="3" Size="10"
        Visible="true" LineWidth="0.353" Cap="Butt" Join="Miter" MiterLimit="4.234"
        DashOffset="0" Alpha="255" Stroke="false" Fill="true" HScale="1.0"
        ReadDirection="0" CharDirection="0" Weight="400" Italic="false">
        <ofd:CGTransform CodePosition="0" CodeCount="1" GlyphCount="1"/>
        <ofd:TextCode X="0" Y="0">A</ofd:TextCode>
      </ofd:TextObject>
      <ofd:PathObject ID="4" Boundary="0 0 1 1" Visible="true" Stroke="true" Fill="false" Rule="NonZero">
        <ofd:AbbreviatedData>M 0 0</ofd:AbbreviatedData>
      </ofd:PathObject>
      <ofd:ImageObject ID="5" Boundary="0 0 1 1" ResourceID="6" Visible="true">
        <ofd:Border LineWidth="0.353" HorizonalCornerRadius="0" VerticalCornerRadius="0" DashOffset="0"/>
      </ofd:ImageObject>
    </ofd:Layer>
  </ofd:Content>
</ofd:Page>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>20</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="10" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
  <ofd:Outlines><ofd:OutlineElem Title="t" Expanded="true"/></ofd:Outlines>
  <ofd:Permissions>
    <ofd:Edit>true</ofd:Edit>
    <ofd:Annot>true</ofd:Annot>
    <ofd:Export>true</ofd:Export>
    <ofd:Signature>true</ofd:Signature>
    <ofd:Watermark>true</ofd:Watermark>
    <ofd:PrintScreen>true</ofd:PrintScreen>
    <ofd:Print Printable="true" Copies="-1"/>
  </ofd:Permissions>
  <ofd:Actions>
    <ofd:Action Event="CLICK"><ofd:GotoA AttachID="a1" NewWindow="true"/></ofd:Action>
    <ofd:Action Event="DO"><ofd:Movie ResourceID="7" Operator="Play"/></ofd:Action>
  </ofd:Actions>
  <ofd:VPreferences>
    <ofd:PageMode>None</ofd:PageMode>
    <ofd:PageLayout>OneColumn</ofd:PageLayout>
    <ofd:TabDisplay>DocTitle</ofd:TabDisplay>
    <ofd:HideToolbar>false</ofd:HideToolbar>
    <ofd:HideMenubar>false</ofd:HideMenubar>
    <ofd:HideWindowUI>false</ofd:HideWindowUI>
  </ofd:VPreferences>
</ofd:Document>"#;

const RES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="./">
  <ofd:ColorSpaces>
    <ofd:ColorSpace ID="1" Type="RGB" BitsPerComponent="8"/>
  </ofd:ColorSpaces>
  <ofd:DrawParams>
    <ofd:DrawParam ID="2" LineWidth="0.353" Join="Miter" Cap="Butt" DashOffset="0" MiterLimit="4.234">
      <ofd:FillColor Value="0 0 0" Alpha="255"/>
    </ofd:DrawParam>
  </ofd:DrawParams>
  <ofd:Fonts>
    <ofd:Font ID="3" FontName="F" Charset="unicode" Italic="false" Bold="false" Serif="false" FixedWidth="false"/>
  </ofd:Fonts>
</ofd:Res>"#;

const PAGE_ANNOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:PageAnnot xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Annot ID="20" Type="Link" Creator="u" LastModDate="2024-01-02"
             Visible="true" Print="true" NoZoom="false" NoRotate="false" ReadOnly="true">
    <ofd:Appearance Boundary="0 0 1 1"/>
  </ofd:Annot>
</ofd:PageAnnot>"#;

const ATTACHMENTS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Attachments xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Attachment ID="a1" Name="f.txt" Visible="true" Usage="none">
    <ofd:FileLoc>Attachments/f.txt</ofd:FileLoc>
  </ofd:Attachment>
</ofd:Attachments>"#;

const SIGNATURES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Signatures xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Signature ID="s1" Type="Seal" BaseLoc="Signs/Sign_0/Signature.xml"/>
</ofd:Signatures>"#;

#[test]
fn page_writer_omits_default_attributes() {
    let view = Page::from_xml(PAGE.as_bytes(), ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    for attribute in [
        "ZOrder=",
        "Type=\"Body\"",
        "Visible=",
        "LineWidth=",
        "Cap=",
        "Join=",
        "MiterLimit=",
        "DashOffset=",
        "Alpha=",
        "Stroke=",
        "Fill=",
        "HScale=",
        "ReadDirection=",
        "CharDirection=",
        "Weight=",
        "Italic=",
        "Rule=",
        "CodeCount=",
        "GlyphCount=",
        "HorizonalCornerRadius=",
        "VerticalCornerRadius=",
    ] {
        assert!(
            !xml.contains(attribute),
            "default attribute kept: {attribute}"
        );
    }
    assert!(xml.contains(r#"TemplateID="1""#));
    assert!(xml.contains(r#"Font="3""#));
    assert!(xml.contains(r#"CodePosition="0""#));
    assert!(xml.contains(r#"ResourceID="6""#));
    Page::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
}

#[test]
fn document_writer_omits_default_elements_and_attributes() {
    let view = Document::from_xml(DOCUMENT.as_bytes(), ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    for text in [
        "<ofd:Edit>",
        "<ofd:Annot>",
        "<ofd:Export>",
        "<ofd:Signature>",
        "<ofd:Watermark>",
        "<ofd:PrintScreen>",
        "<ofd:PageMode>",
        "<ofd:PageLayout>",
        "<ofd:TabDisplay>",
        "<ofd:HideToolbar>",
        "<ofd:HideMenubar>",
        "<ofd:HideWindowUI>",
    ] {
        assert!(!xml.contains(text), "default element kept: {text}");
    }
    for attribute in ["Copies=", "Expanded=", "NewWindow=", "Operator="] {
        assert!(
            !xml.contains(attribute),
            "default attribute kept: {attribute}"
        );
    }
    assert!(xml.contains(r#"<ofd:Print Printable="true""#));
    assert!(xml.contains(r#"AttachID="a1""#));
    assert!(xml.contains(r#"ResourceID="7""#));
    Document::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
}

#[test]
fn res_writer_omits_default_attributes() {
    let view = Res::from_xml(RES.as_bytes(), ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    for attribute in [
        "BitsPerComponent=",
        "LineWidth=",
        "Join=",
        "Cap=",
        "DashOffset=",
        "MiterLimit=",
        "Alpha=",
        "Charset=",
        "Italic=",
        "Bold=",
        "Serif=",
        "FixedWidth=",
    ] {
        assert!(
            !xml.contains(attribute),
            "default attribute kept: {attribute}"
        );
    }
    Res::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
}

#[test]
fn annotation_attachment_and_signature_omit_defaults() {
    let annot = PageAnnot::from_xml(PAGE_ANNOT.as_bytes(), ReadOptions::LENIENT).unwrap();
    let xml = annot.to_xml().expect("strict writer");
    for attribute in ["Visible=", "Print=", "NoZoom=", "NoRotate=", "ReadOnly="] {
        assert!(
            !xml.contains(attribute),
            "default attribute kept: {attribute}"
        );
    }
    PageAnnot::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");

    let attachments = Attachments::from_xml(ATTACHMENTS.as_bytes(), ReadOptions::LENIENT).unwrap();
    let xml = attachments.to_xml().expect("strict writer");
    assert!(!xml.contains("Visible="));
    assert!(!xml.contains("Usage="));
    Attachments::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");

    let signatures = Signatures::from_xml(SIGNATURES.as_bytes(), ReadOptions::LENIENT).unwrap();
    let xml = signatures.to_xml().expect("strict writer");
    assert!(!xml.contains(r#"Type="Seal""#));
    Signatures::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
}

const SIGNATURE_DESCRIPTION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Signature xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:SignedInfo>
    <ofd:Provider ProviderName="P"/>
    <ofd:References CheckMethod="MD5">
      <ofd:Reference FileRef="/OFD.xml"><ofd:CheckValue>AAAA</ofd:CheckValue></ofd:Reference>
    </ofd:References>
  </ofd:SignedInfo>
  <ofd:SignedValue>Signs/SignedValue.dat</ofd:SignedValue>
</ofd:Signature>"#;

#[test]
fn signature_writer_omits_default_check_method() {
    let view = Signature::from_xml(SIGNATURE_DESCRIPTION.as_bytes(), ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(!xml.contains("CheckMethod="));
    Signature::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
}
