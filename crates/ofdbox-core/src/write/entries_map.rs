use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, Event};

use crate::error::OfdError;
use crate::model::entries_map::EncryptedEntriesView;
use crate::write::ofd::{missing, write_start};
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_entries_map(entries: &EncryptedEntriesView) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    let id = entries
        .id()
        .ok_or_else(|| missing("/EncryptedEntries/@ID", "缺少 ID"))?;
    write_start(
        &mut writer,
        "ofd:EncryptedEntries",
        &[("xmlns:ofd", OFD_NAMESPACE), ("ID", id)],
    )?;

    for entry in entries.entries() {
        let path = entry
            .path()
            .ok_or_else(|| missing("/EncryptEntry/@Path", "缺少 Path"))?;
        let mut attributes = vec![("Path", path)];
        if let Some(epath) = entry.epath() {
            attributes.push(("EPath", epath));
        }
        // 0099 附录 C.4 与图 C.3 把它定义为属性。
        if let Some(seed) = entry.decrypt_seed_loc() {
            attributes.push(("DecryptSeedLoc", seed));
        }
        write_start(&mut writer, "ofd:EncryptEntry", &attributes)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:EncryptEntry")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:EncryptedEntries")))?;
    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}
