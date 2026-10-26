use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesText, Event};

use crate::error::OfdError;
use crate::model::version::Version;
use crate::write::ofd::{missing, write_start, write_text_element};
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_doc_version(version: &Version) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    let id = version
        .id()
        .ok_or_else(|| missing("/DocVersion/@ID", "缺少 DocVersion ID"))?;
    let mut attributes = vec![("xmlns:ofd", OFD_NAMESPACE), ("ID", id)];
    if let Some(value) = version.version() {
        attributes.push(("Version", value));
    }
    if let Some(name) = version.name() {
        attributes.push(("Name", name));
    }
    if let Some(date) = version.creation_date() {
        attributes.push(("CreationDate", date));
    }
    write_start(&mut writer, "ofd:DocVersion", &attributes)?;

    write_start(&mut writer, "ofd:FileList", &[])?;
    for file in version.files() {
        let file_id = file
            .id()
            .ok_or_else(|| missing("/DocVersion/FileList/File/@ID", "缺少 File ID"))?;
        write_start(&mut writer, "ofd:File", &[("ID", file_id)])?;
        writer.write_event(Event::Text(BytesText::new(file.location())))?;
        writer.write_event(Event::End(BytesEnd::new("ofd:File")))?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:FileList")))?;

    let doc_root = version
        .doc_root()
        .ok_or_else(|| missing("/DocVersion/DocRoot", "缺少 DocRoot"))?;
    write_text_element(&mut writer, "ofd:DocRoot", doc_root)?;

    writer.write_event(Event::End(BytesEnd::new("ofd:DocVersion")))?;
    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}
