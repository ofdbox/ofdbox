use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, Event};

use crate::error::OfdError;
use crate::model::custom_tags::CustomTags;
use crate::write::ofd::{missing, write_start, write_text_element};
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_custom_tags(custom_tags: &CustomTags) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_start(
        &mut writer,
        "ofd:CustomTags",
        &[("xmlns:ofd", OFD_NAMESPACE)],
    )?;

    for tag in custom_tags.custom_tags() {
        let type_id = tag
            .type_id()
            .ok_or_else(|| missing("/CustomTags/CustomTag/@TypeID", "缺少类型标识"))?;
        write_start(&mut writer, "ofd:CustomTag", &[("TypeID", type_id)])?;
        if let Some(schema_loc) = tag.schema_loc() {
            write_text_element(&mut writer, "ofd:SchemaLoc", schema_loc)?;
        }
        let file_loc = tag
            .file_loc()
            .ok_or_else(|| missing("/CustomTags/CustomTag/FileLoc", "缺少 FileLoc"))?;
        write_text_element(&mut writer, "ofd:FileLoc", file_loc)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:CustomTag")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:CustomTags")))?;
    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}
