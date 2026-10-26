use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, Event};

use crate::error::OfdError;
use crate::model::document::{Dest, Document, PageArea, Permissions, VPreferences};
use crate::write::action::write_actions;
use crate::write::ofd::{missing, write_start, write_text_element};
use crate::write::outline::write_outlines;
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_document(document: &Document) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_start(&mut writer, "ofd:Document", &[("xmlns:ofd", OFD_NAMESPACE)])?;

    let common = document
        .common_data()
        .ok_or_else(|| missing("/Document/CommonData", "缺少 CommonData"))?;
    write_start(&mut writer, "ofd:CommonData", &[])?;
    let max_unit_id = common
        .max_unit_id()
        .ok_or_else(|| missing("/Document/CommonData/MaxUnitID", "缺少 MaxUnitID"))?;
    write_text_element(&mut writer, "ofd:MaxUnitID", max_unit_id)?;

    let area = common
        .page_area()
        .ok_or_else(|| missing("/Document/CommonData/PageArea", "缺少 PageArea"))?;
    write_start(&mut writer, "ofd:PageArea", &[])?;
    let physical = area.physical_box().ok_or_else(|| {
        missing(
            "/Document/CommonData/PageArea/PhysicalBox",
            "缺少 PhysicalBox",
        )
    })?;
    write_text_element(&mut writer, "ofd:PhysicalBox", physical)?;
    for (name, value) in [
        ("ofd:ApplicationBox", area.application_box()),
        ("ofd:ContentBox", area.content_box()),
        ("ofd:BleedBox", area.bleed_box()),
    ] {
        if let Some(value) = value {
            write_text_element(&mut writer, name, value)?;
        }
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:PageArea")))?;

    for path in common.public_res() {
        write_text_element(&mut writer, "ofd:PublicRes", path)?;
    }
    for path in common.document_res() {
        write_text_element(&mut writer, "ofd:DocumentRes", path)?;
    }
    for template in common.template_pages() {
        let id = template.id().ok_or_else(|| {
            missing(
                "/Document/CommonData/TemplatePage/@ID",
                "缺少 TemplatePage ID",
            )
        })?;
        let base_loc = template.base_loc().ok_or_else(|| {
            missing(
                "/Document/CommonData/TemplatePage/@BaseLoc",
                "缺少 TemplatePage BaseLoc",
            )
        })?;
        let mut attributes = vec![("ID", id)];
        if let Some(name) = template.name() {
            attributes.push(("Name", name));
        }
        if let Some(z_order) = template.z_order() {
            attributes.push(("ZOrder", z_order));
        }
        attributes.push(("BaseLoc", base_loc));
        write_start(&mut writer, "ofd:TemplatePage", &attributes)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:TemplatePage")))?;
    }
    if let Some(default_cs) = common.default_cs() {
        write_text_element(&mut writer, "ofd:DefaultCS", default_cs)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:CommonData")))?;

    let pages = document
        .pages()
        .ok_or_else(|| missing("/Document/Pages", "缺少 Pages"))?;
    write_start(&mut writer, "ofd:Pages", &[])?;
    for page in pages.pages() {
        let id = page
            .id()
            .ok_or_else(|| missing("/Document/Pages/Page/@ID", "缺少 Page ID"))?;
        let base_loc = page
            .base_loc()
            .ok_or_else(|| missing("/Document/Pages/Page/@BaseLoc", "缺少 Page BaseLoc"))?;
        write_start(
            &mut writer,
            "ofd:Page",
            &[("ID", id), ("BaseLoc", base_loc)],
        )?;
        writer.write_event(Event::End(BytesEnd::new("ofd:Page")))?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:Pages")))?;

    if let Some(outlines) = document.outlines() {
        write_outlines(&mut writer, &outlines)?;
    }

    if let Some(permissions) = document.permissions() {
        write_permissions(&mut writer, &permissions)?;
    }
    if let Some(actions) = document.actions() {
        write_actions(&mut writer, &actions)?;
    }
    if let Some(prefs) = document.v_preferences() {
        write_v_preferences(&mut writer, &prefs)?;
    }
    if let Some(bookmarks) = document.bookmarks() {
        write_start(&mut writer, "ofd:Bookmarks", &[])?;
        for bookmark in bookmarks.bookmarks() {
            let name = bookmark.name().ok_or_else(|| {
                missing("/Document/Bookmarks/Bookmark/@Name", "缺少 Bookmark Name")
            })?;
            write_start(&mut writer, "ofd:Bookmark", &[("Name", name)])?;
            if let Some(dest) = bookmark.dest() {
                write_dest(&mut writer, &dest)?;
            }
            writer.write_event(Event::End(BytesEnd::new("ofd:Bookmark")))?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:Bookmarks")))?;
    }

    if let Some(value) = document.annotations() {
        write_text_element(&mut writer, "ofd:Annotations", value)?;
    }
    if let Some(value) = document.custom_tags() {
        write_text_element(&mut writer, "ofd:CustomTags", value)?;
    }
    if let Some(value) = document.attachments() {
        write_text_element(&mut writer, "ofd:Attachments", value)?;
    }
    if let Some(value) = document.extensions() {
        write_text_element(&mut writer, "ofd:Extensions", value)?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:Document")))?;
    let bytes = writer.into_inner();
    String::from_utf8(bytes)
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

pub(crate) fn write_page_area(
    writer: &mut Writer<Vec<u8>>,
    name: &str,
    area: &PageArea,
) -> Result<(), OfdError> {
    write_start(writer, name, &[])?;
    let physical = area
        .physical_box()
        .ok_or_else(|| missing("/PageArea/PhysicalBox", "缺少 PhysicalBox"))?;
    write_text_element(writer, "ofd:PhysicalBox", physical)?;
    for (name, value) in [
        ("ofd:ApplicationBox", area.application_box()),
        ("ofd:ContentBox", area.content_box()),
        ("ofd:BleedBox", area.bleed_box()),
    ] {
        if let Some(value) = value {
            write_text_element(writer, name, value)?;
        }
    }
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

fn write_permissions(
    writer: &mut Writer<Vec<u8>>,
    permissions: &Permissions,
) -> Result<(), OfdError> {
    write_start(writer, "ofd:Permissions", &[])?;
    for (name, value) in [
        ("ofd:Edit", permissions.edit()),
        ("ofd:Annot", permissions.annot()),
        ("ofd:Export", permissions.export()),
        ("ofd:Signature", permissions.signature()),
        ("ofd:Watermark", permissions.watermark()),
        ("ofd:PrintScreen", permissions.print_screen()),
    ] {
        if let Some(value) = value {
            write_text_element(writer, name, bool_text(value))?;
        }
    }
    if let Some(print) = permissions.print() {
        let printable = print
            .printable()
            .ok_or_else(|| missing("/Document/Permissions/Print/@Printable", "缺少 Printable"))?;
        let copies = print.copies().map(|value| value.to_string());
        let mut attributes = vec![("Printable", bool_text(printable))];
        if let Some(copies) = copies.as_deref() {
            attributes.push(("Copies", copies));
        }
        write_start(writer, "ofd:Print", &attributes)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:Print")))?;
    }
    if let Some(period) = permissions.valid_period() {
        let mut attributes = Vec::new();
        if let Some(start) = period.start_date() {
            attributes.push(("StartDate", start));
        }
        if let Some(end) = period.end_date() {
            attributes.push(("EndDate", end));
        }
        write_start(writer, "ofd:ValidPeriod", &attributes)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:ValidPeriod")))?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:Permissions")))?;
    Ok(())
}

fn write_v_preferences(writer: &mut Writer<Vec<u8>>, prefs: &VPreferences) -> Result<(), OfdError> {
    write_start(writer, "ofd:VPreferences", &[])?;
    for (name, value) in [
        ("ofd:PageMode", prefs.page_mode()),
        ("ofd:PageLayout", prefs.page_layout()),
        ("ofd:TabDisplay", prefs.tab_display()),
    ] {
        if let Some(value) = value {
            write_text_element(writer, name, value)?;
        }
    }
    for (name, value) in [
        ("ofd:HideToolbar", prefs.hide_toolbar()),
        ("ofd:HideMenubar", prefs.hide_menubar()),
        ("ofd:HideWindowUI", prefs.hide_window_ui()),
    ] {
        if let Some(value) = value {
            write_text_element(writer, name, bool_text(value))?;
        }
    }
    if let Some(mode) = prefs.zoom_mode() {
        write_text_element(writer, "ofd:ZoomMode", mode)?;
    } else if let Some(zoom) = prefs.zoom() {
        write_text_element(writer, "ofd:Zoom", &zoom.to_string())?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:VPreferences")))?;
    Ok(())
}

pub(crate) fn write_dest(writer: &mut Writer<Vec<u8>>, dest: &Dest) -> Result<(), OfdError> {
    let dest_type = dest
        .dest_type()
        .ok_or_else(|| missing("/Document/Bookmarks/Bookmark/Dest/@Type", "缺少 Dest Type"))?;
    let page_id = dest.page_id().ok_or_else(|| {
        missing(
            "/Document/Bookmarks/Bookmark/Dest/@PageID",
            "缺少 Dest PageID",
        )
    })?;
    let left = dest.left().map(|value| value.to_string());
    let top = dest.top().map(|value| value.to_string());
    let right = dest.right().map(|value| value.to_string());
    let bottom = dest.bottom().map(|value| value.to_string());
    let zoom = dest.zoom().map(|value| value.to_string());
    let mut attributes = vec![("Type", dest_type), ("PageID", page_id)];
    if let Some(value) = left.as_deref() {
        attributes.push(("Left", value));
    }
    if let Some(value) = top.as_deref() {
        attributes.push(("Top", value));
    }
    if let Some(value) = right.as_deref() {
        attributes.push(("Right", value));
    }
    if let Some(value) = bottom.as_deref() {
        attributes.push(("Bottom", value));
    }
    if let Some(value) = zoom.as_deref() {
        attributes.push(("Zoom", value));
    }
    write_start(writer, "ofd:Dest", &attributes)?;
    writer.write_event(Event::End(BytesEnd::new("ofd:Dest")))?;
    Ok(())
}

fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}
