use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, Event};

use crate::error::OfdError;
use crate::model::attachment::AttachmentsView;
use crate::write::ofd::{missing, write_start, write_text_element};
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_attachments(attachments: &AttachmentsView) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_start(
        &mut writer,
        "ofd:Attachments",
        &[("xmlns:ofd", OFD_NAMESPACE)],
    )?;

    for attachment in attachments.attachments() {
        let id = attachment
            .id()
            .ok_or_else(|| missing("/Attachments/Attachment/@ID", "缺少附件 ID"))?;
        let name = attachment
            .name()
            .ok_or_else(|| missing("/Attachments/Attachment/@Name", "缺少附件 Name"))?;
        let mut attributes = vec![("ID", id), ("Name", name)];
        if let Some(format) = attachment.format() {
            attributes.push(("Format", format));
        }
        if let Some(date) = attachment.creation_date() {
            attributes.push(("CreationDate", date));
        }
        if let Some(date) = attachment.mod_date() {
            attributes.push(("ModDate", date));
        }
        let size = attachment.size().map(|value| value.to_string());
        if let Some(size) = size.as_deref() {
            attributes.push(("Size", size));
        }
        if let Some(visible) = attachment.visible() {
            attributes.push(("Visible", bool_text(visible)));
        }
        if let Some(usage) = attachment.usage() {
            attributes.push(("Usage", usage));
        }
        write_start(&mut writer, "ofd:Attachment", &attributes)?;

        let file_loc = attachment
            .file_loc()
            .ok_or_else(|| missing("/Attachments/Attachment/FileLoc", "缺少 FileLoc"))?;
        write_text_element(&mut writer, "ofd:FileLoc", file_loc)?;

        writer.write_event(Event::End(BytesEnd::new("ofd:Attachment")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:Attachments")))?;
    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}
