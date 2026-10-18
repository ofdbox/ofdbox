use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, Event};

use crate::error::OfdError;
use crate::model::integrity::DocEntriesView;
use crate::write::ofd::{missing, write_start, write_text_element};
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_doc_entries(entries: &DocEntriesView) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    let id = entries
        .id()
        .ok_or_else(|| missing("/DocEntries/@ID", "缺少 ID"))?;
    let mut attributes = vec![("xmlns:ofd", OFD_NAMESPACE), ("ID", id)];
    if let Some(value) = entries.creator_name() {
        attributes.push(("CreatorName", value));
    }
    if let Some(value) = entries.version() {
        attributes.push(("Version", value));
    }
    if let Some(value) = entries.creation_date() {
        attributes.push(("CreationDate", value));
    }
    write_start(&mut writer, "ofd:DocEntries", &attributes)?;

    write_start(&mut writer, "ofd:FileList", &[])?;
    for file in entries.files() {
        let id = file
            .id()
            .ok_or_else(|| missing("/DocEntries/FileList/File/@ID", "缺少 File ID"))?;
        let loc = file
            .file_loc()
            .ok_or_else(|| missing("/DocEntries/FileList/File/FileLoc", "缺少 FileLoc"))?;
        write_start(&mut writer, "ofd:File", &[("ID", id)])?;
        write_text_element(&mut writer, "ofd:FileLoc", loc)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:File")))?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:FileList")))?;

    if let Some(value) = entries.signed_value_loc() {
        write_text_element(&mut writer, "ofd:SignedValueLoc", value)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:DocEntries")))?;
    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}
