use quick_xml::Writer;
use quick_xml::events::{BytesEnd, Event};

use crate::error::OfdError;
use crate::model::outline::{OutlineElemView, OutlinesView};
use crate::write::action::write_actions;
use crate::write::ofd::{missing, write_start};

pub(crate) fn write_outlines(
    writer: &mut Writer<Vec<u8>>,
    outlines: &OutlinesView,
) -> Result<(), OfdError> {
    write_start(writer, "ofd:Outlines", &[])?;
    for outline in outlines.outline_elems() {
        write_outline_elem(writer, &outline)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:Outlines")))?;
    Ok(())
}

fn write_outline_elem(
    writer: &mut Writer<Vec<u8>>,
    outline: &OutlineElemView,
) -> Result<(), OfdError> {
    let title = outline
        .title()
        .ok_or_else(|| missing("/Outlines/OutlineElem/@Title", "缺少 OutlineElem Title"))?;
    let count = outline.count().map(|value| value.to_string());
    let expanded = outline
        .expanded()
        .map(|value| if value { "true" } else { "false" });
    let mut attributes = vec![("Title", title)];
    if let Some(count) = count.as_deref() {
        attributes.push(("Count", count));
    }
    if let Some(expanded) = expanded {
        attributes.push(("Expanded", expanded));
    }
    write_start(writer, "ofd:OutlineElem", &attributes)?;
    if let Some(actions) = outline.actions() {
        write_actions(writer, &actions)?;
    }
    for child in outline.children() {
        write_outline_elem(writer, &child)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:OutlineElem")))?;
    Ok(())
}
