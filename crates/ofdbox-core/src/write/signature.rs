use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, Event};

use crate::error::OfdError;
use crate::model::signature::{Reference, Signature, Signatures, SignedInfo, StampAnnot};
use crate::write::ofd::{missing, write_start, write_text_element};
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_signatures(signatures: &Signatures) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_start(
        &mut writer,
        "ofd:Signatures",
        &[("xmlns:ofd", OFD_NAMESPACE)],
    )?;

    if let Some(max_sign_id) = signatures.max_sign_id() {
        write_text_element(&mut writer, "ofd:MaxSignId", max_sign_id)?;
    }
    for signature in signatures.signatures() {
        let id = signature
            .id()
            .ok_or_else(|| missing("/Signatures/Signature/@ID", "缺少 Signature ID"))?;
        let base_loc = signature
            .base_loc()
            .ok_or_else(|| missing("/Signatures/Signature/@BaseLoc", "缺少 BaseLoc"))?;
        let mut attributes = vec![("ID", id)];
        if let Some(signature_type) = signature.signature_type() {
            attributes.push(("Type", signature_type));
        }
        if let Some(relative) = signature.relative() {
            attributes.push(("Relative", relative));
        }
        attributes.push(("BaseLoc", base_loc));
        write_start(&mut writer, "ofd:Signature", &attributes)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:Signature")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:Signatures")))?;
    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

pub(crate) fn write_signature(signature: &Signature) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_start(
        &mut writer,
        "ofd:Signature",
        &[("xmlns:ofd", OFD_NAMESPACE)],
    )?;

    let info = signature
        .signed_info()
        .ok_or_else(|| missing("/Signature/SignedInfo", "缺少 SignedInfo"))?;
    write_signed_info(&mut writer, &info)?;
    let signed_value = signature
        .signed_value()
        .ok_or_else(|| missing("/Signature/SignedValue", "缺少 SignedValue"))?;
    write_text_element(&mut writer, "ofd:SignedValue", signed_value)?;

    writer.write_event(Event::End(BytesEnd::new("ofd:Signature")))?;
    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

fn write_signed_info(writer: &mut Writer<Vec<u8>>, info: &SignedInfo) -> Result<(), OfdError> {
    write_start(writer, "ofd:SignedInfo", &[])?;

    let provider = info
        .provider()
        .ok_or_else(|| missing("/Signature/SignedInfo/Provider", "缺少 Provider"))?;
    let provider_name = provider
        .provider_name()
        .ok_or_else(|| missing("/SignedInfo/Provider/@ProviderName", "缺少 ProviderName"))?;
    let mut attributes = vec![("ProviderName", provider_name)];
    if let Some(company) = provider.company() {
        attributes.push(("Company", company));
    }
    if let Some(version) = provider.version() {
        attributes.push(("Version", version));
    }
    if let Some(protocol_version) = provider.protocol_version() {
        attributes.push(("ProtocolVer", protocol_version));
    }
    write_start(writer, "ofd:Provider", &attributes)?;
    if let Some(extend_data) = provider.extend_data() {
        write_text_element(writer, "ofd:ExtendData", extend_data)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:Provider")))?;

    if let Some(method) = info.signature_method() {
        write_text_element(writer, "ofd:SignatureMethod", method)?;
    }
    if let Some(date_time) = info.signature_date_time() {
        write_text_element(writer, "ofd:SignatureDateTime", date_time)?;
    }
    let parameters: Vec<_> = info.parameters().collect();
    if !parameters.is_empty() {
        write_start(writer, "ofd:Parameters", &[])?;
        for parameter in parameters {
            let name = parameter
                .name()
                .ok_or_else(|| missing("/SignedInfo/Parameters/Parameter/@Name", "缺少 Name"))?;
            let mut attributes = vec![("Name", name)];
            if let Some(value_type) = parameter.value_type() {
                attributes.push(("Type", value_type));
            }
            write_start(writer, "ofd:Parameter", &attributes)?;
            if !parameter.value().is_empty() {
                writer.write_event(Event::Text(quick_xml::events::BytesText::new(
                    parameter.value(),
                )))?;
            }
            writer.write_event(Event::End(BytesEnd::new("ofd:Parameter")))?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:Parameters")))?;
    }

    let references = info
        .references()
        .ok_or_else(|| missing("/Signature/SignedInfo/References", "缺少 References"))?;
    let mut attributes = Vec::new();
    if let Some(check_method) = references.check_method() {
        attributes.push(("CheckMethod", check_method));
    }
    write_start(writer, "ofd:References", &attributes)?;
    for reference in references.references() {
        write_reference(writer, &reference)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:References")))?;

    for stamp_annot in info.stamp_annots() {
        write_stamp_annot(writer, &stamp_annot)?;
    }
    if let Some(seal) = info.seal() {
        write_start(writer, "ofd:Seal", &[])?;
        if let Some(base_loc) = seal.base_loc() {
            write_text_element(writer, "ofd:BaseLoc", base_loc)?;
        }
        if let Some(image_loc) = seal.image_loc() {
            write_text_element(writer, "ofd:ImageLoc", image_loc)?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:Seal")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:SignedInfo")))?;
    Ok(())
}

fn write_reference(writer: &mut Writer<Vec<u8>>, reference: &Reference) -> Result<(), OfdError> {
    let file_ref = reference
        .file_ref()
        .ok_or_else(|| missing("/References/Reference/@FileRef", "缺少 FileRef"))?;
    write_start(writer, "ofd:Reference", &[("FileRef", file_ref)])?;
    write_text_element(writer, "ofd:CheckValue", reference.check_value())?;
    writer.write_event(Event::End(BytesEnd::new("ofd:Reference")))?;
    Ok(())
}

fn write_stamp_annot(
    writer: &mut Writer<Vec<u8>>,
    stamp_annot: &StampAnnot,
) -> Result<(), OfdError> {
    let id = stamp_annot
        .id()
        .ok_or_else(|| missing("/SignedInfo/StampAnnot/@ID", "缺少 StampAnnot ID"))?;
    let page_ref = stamp_annot
        .page_ref()
        .ok_or_else(|| missing("/SignedInfo/StampAnnot/@PageRef", "缺少 PageRef"))?;
    let boundary = stamp_annot
        .boundary()
        .ok_or_else(|| missing("/SignedInfo/StampAnnot/@Boundary", "缺少 Boundary"))?;
    let mut attributes = vec![("ID", id), ("PageRef", page_ref), ("Boundary", boundary)];
    if let Some(clip) = stamp_annot.clip() {
        attributes.push(("Clip", clip));
    }
    write_start(writer, "ofd:StampAnnot", &attributes)?;
    writer.write_event(Event::End(BytesEnd::new("ofd:StampAnnot")))?;
    Ok(())
}
