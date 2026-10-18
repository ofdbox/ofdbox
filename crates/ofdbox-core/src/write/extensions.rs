use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesText, Event};

use crate::error::OfdError;
use crate::model::extensions::{ExtensionView, ExtensionsView};
use crate::write::ofd::{missing, write_start, write_text_element};
use crate::write::page::write_subtree;
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_extensions(extensions: &ExtensionsView) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_start(
        &mut writer,
        "ofd:Extensions",
        &[("xmlns:ofd", OFD_NAMESPACE)],
    )?;

    for extension in extensions.extensions() {
        write_extension(&mut writer, &extension)?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:Extensions")))?;
    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

fn write_extension(
    writer: &mut Writer<Vec<u8>>,
    extension: &ExtensionView,
) -> Result<(), OfdError> {
    let app_name = extension
        .app_name()
        .ok_or_else(|| missing("/Extensions/Extension/@AppName", "缺少 AppName"))?;
    let ref_id = extension
        .ref_id()
        .ok_or_else(|| missing("/Extensions/Extension/@RefId", "缺少 RefId"))?;
    let mut attributes = vec![("AppName", app_name)];
    if let Some(company) = extension.company() {
        attributes.push(("Company", company));
    }
    if let Some(version) = extension.app_version() {
        attributes.push(("AppVersion", version));
    }
    if let Some(date) = extension.date() {
        attributes.push(("Date", date));
    }
    attributes.push(("RefId", ref_id));
    write_start(writer, "ofd:Extension", &attributes)?;

    let doc = extension.document();
    for child in doc.element_children(extension.node()) {
        match doc.name(child).local.as_str() {
            "Property" => {
                let mut attributes = Vec::new();
                let name = doc
                    .attribute(child, "Name")
                    .ok_or_else(|| missing("/Extension/Property/@Name", "缺少属性 Name"))?;
                attributes.push(("Name", name));
                if let Some(value_type) = doc.attribute(child, "Type") {
                    attributes.push(("Type", value_type));
                }
                write_start(writer, "ofd:Property", &attributes)?;
                writer.write_event(Event::Text(BytesText::new(doc.text(child).trim())))?;
                writer.write_event(Event::End(BytesEnd::new("ofd:Property")))?;
            }
            "Data" => write_subtree(writer, doc, child)?,
            "ExtendData" => write_text_element(writer, "ofd:ExtendData", doc.text(child).trim())?,
            other => {
                return Err(OfdError::InvalidStructure(format!(
                    "严格写出不支持 Extension 子元素: {other}"
                )));
            }
        }
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:Extension")))?;
    Ok(())
}
