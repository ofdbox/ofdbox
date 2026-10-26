use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesText, Event};

use crate::error::OfdError;
use crate::model::encryption::{EncryptInfo, Encryptions};
use crate::write::ofd::{missing, write_start, write_text_element};
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_encryptions(encryptions: &Encryptions) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_start(
        &mut writer,
        "ofd:Encryptions",
        &[("xmlns:ofd", OFD_NAMESPACE)],
    )?;

    for info in encryptions.encrypt_infos() {
        write_encrypt_info(&mut writer, &info)?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:Encryptions")))?;
    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

fn write_encrypt_info(writer: &mut Writer<Vec<u8>>, info: &EncryptInfo) -> Result<(), OfdError> {
    let id = info
        .id()
        .ok_or_else(|| missing("/Encryptions/EncryptInfo/@ID", "缺少 EncryptInfo ID"))?;
    let mut attributes = vec![("ID", id)];
    if let Some(relative) = info.relative() {
        attributes.push(("Relative", relative));
    }
    write_start(writer, "ofd:EncryptInfo", &attributes)?;

    let provider = info
        .provider()
        .ok_or_else(|| missing("/EncryptInfo/Provider", "缺少 Provider"))?;
    let name = provider
        .name()
        .ok_or_else(|| missing("/EncryptInfo/Provider/@Name", "缺少 Provider Name"))?;
    let mut attributes = vec![("Name", name)];
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

    let scope = info
        .encrypt_scope()
        .ok_or_else(|| missing("/EncryptInfo/EncryptScope", "缺少 EncryptScope"))?;
    write_text_element(writer, "ofd:EncryptScope", scope)?;

    let parameters: Vec<_> = info.parameters().collect();
    if !parameters.is_empty() {
        write_start(writer, "ofd:Parameters", &[])?;
        for parameter in parameters {
            let name = parameter
                .name()
                .ok_or_else(|| missing("/EncryptInfo/Parameters/Parameter/@Name", "缺少 Name"))?;
            let mut attributes = vec![("Name", name)];
            if let Some(value_type) = parameter.value_type() {
                attributes.push(("Type", value_type));
            }
            write_start(writer, "ofd:Parameter", &attributes)?;
            if !parameter.value().is_empty() {
                writer.write_event(Event::Text(BytesText::new(parameter.value())))?;
            }
            writer.write_event(Event::End(BytesEnd::new("ofd:Parameter")))?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:Parameters")))?;
    }

    if let Some(date) = info.encrypt_date() {
        write_text_element(writer, "ofd:EncryptDate", date)?;
    }
    let seed = info
        .decrypt_seed_loc()
        .ok_or_else(|| missing("/EncryptInfo/DecryptSeedLoc", "缺少 DecryptSeedLoc"))?;
    write_text_element(writer, "ofd:DecryptSeedLoc", seed)?;
    let map = info
        .entries_map_loc()
        .ok_or_else(|| missing("/EncryptInfo/EntriesMapLoc", "缺少 EntriesMapLoc"))?;
    write_text_element(writer, "ofd:EntriesMapLoc", map)?;

    writer.write_event(Event::End(BytesEnd::new("ofd:EncryptInfo")))?;
    Ok(())
}
