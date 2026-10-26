use quick_xml::Writer;
use quick_xml::events::{BytesEnd, Event};

use crate::error::OfdError;
use crate::model::action::{Action, Actions};
use crate::write::document::write_dest;
use crate::write::ofd::{missing, write_start};
use crate::write::region::write_region;

pub(crate) fn write_actions(
    writer: &mut Writer<Vec<u8>>,
    actions: &Actions,
) -> Result<(), OfdError> {
    write_start(writer, "ofd:Actions", &[])?;
    for action in actions.actions() {
        write_action(writer, &action)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:Actions")))?;
    Ok(())
}

fn write_action(writer: &mut Writer<Vec<u8>>, action: &Action) -> Result<(), OfdError> {
    let event = action
        .event()
        .ok_or_else(|| missing("/Action/@Event", "缺少 Action Event"))?;
    write_start(writer, "ofd:Action", &[("Event", event)])?;
    if let Some(region) = action.region() {
        write_region(writer, &region)?;
    }

    let mut kinds = 0;
    if let Some(goto) = action.goto() {
        kinds += 1;
        write_start(writer, "ofd:Goto", &[])?;
        if let Some(dest) = goto.dest() {
            write_dest(writer, &dest)?;
        } else if let Some(name) = goto.bookmark() {
            write_start(writer, "ofd:Bookmark", &[("Name", name)])?;
            writer.write_event(Event::End(BytesEnd::new("ofd:Bookmark")))?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:Goto")))?;
    }
    if let Some(uri) = action.uri() {
        kinds += 1;
        let mut attributes = vec![("URI", uri.uri().unwrap_or_default())];
        if let Some(base) = uri.base() {
            attributes.push(("Base", base));
        }
        if let Some(target) = uri.target() {
            attributes.push(("Target", target));
        }
        write_start(writer, "ofd:URI", &attributes)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:URI")))?;
    }
    if let Some(goto_a) = action.goto_a() {
        kinds += 1;
        let attach_id = goto_a
            .attach_id()
            .ok_or_else(|| missing("/Action/GotoA/@AttachID", "缺少 AttachID"))?;
        let new_window = goto_a.new_window().map(bool_text);
        let mut attributes = vec![("AttachID", attach_id)];
        if let Some(new_window) = new_window {
            attributes.push(("NewWindow", new_window));
        }
        write_start(writer, "ofd:GotoA", &attributes)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:GotoA")))?;
    }
    if let Some(sound) = action.sound() {
        kinds += 1;
        let resource_id = sound
            .resource_id()
            .ok_or_else(|| missing("/Action/Sound/@ResourceID", "缺少 ResourceID"))?;
        let volume = sound.volume().map(|value| value.to_string());
        let repeat = sound.repeat().map(bool_text);
        let synchronous = sound.synchronous().map(bool_text);
        let mut attributes = vec![("ResourceID", resource_id)];
        if let Some(volume) = volume.as_deref() {
            attributes.push(("Volume", volume));
        }
        if let Some(repeat) = repeat {
            attributes.push(("Repeat", repeat));
        }
        if let Some(synchronous) = synchronous {
            attributes.push(("Synchronous", synchronous));
        }
        write_start(writer, "ofd:Sound", &attributes)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:Sound")))?;
    }
    if let Some(movie) = action.movie() {
        kinds += 1;
        let resource_id = movie
            .resource_id()
            .ok_or_else(|| missing("/Action/Movie/@ResourceID", "缺少 ResourceID"))?;
        let operator = movie.operator();
        let mut attributes = vec![("ResourceID", resource_id)];
        if let Some(operator) = operator {
            attributes.push(("Operator", operator));
        }
        write_start(writer, "ofd:Movie", &attributes)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:Movie")))?;
    }

    if kinds != 1 {
        return Err(OfdError::InvalidStructure(
            "Action 必须且只能包含一种动作类型".to_owned(),
        ));
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:Action")))?;
    Ok(())
}

fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}
