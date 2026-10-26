use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesText, Event};

use crate::error::OfdError;
use crate::model::annotation::{Annot, Annotations, PageAnnot, Parameter};
use crate::write::ofd::{missing, write_start, write_text_element};
use crate::write::page::write_graphic_object;
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_annotations(annotations: &Annotations) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_start(
        &mut writer,
        "ofd:Annotations",
        &[("xmlns:ofd", OFD_NAMESPACE)],
    )?;

    for page in annotations.pages() {
        let page_id = page
            .page_id()
            .ok_or_else(|| missing("/Annotations/Page/@PageID", "缺少 PageID"))?;
        write_start(&mut writer, "ofd:Page", &[("PageID", page_id)])?;
        let file_loc = page
            .file_loc()
            .ok_or_else(|| missing("/Annotations/Page/FileLoc", "缺少 FileLoc"))?;
        write_text_element(&mut writer, "ofd:FileLoc", file_loc)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:Page")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:Annotations")))?;
    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

pub(crate) fn write_page_annot(page_annot: &PageAnnot) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_start(
        &mut writer,
        "ofd:PageAnnot",
        &[("xmlns:ofd", OFD_NAMESPACE)],
    )?;

    for annot in page_annot.annots() {
        write_annot(&mut writer, &annot)?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:PageAnnot")))?;
    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

fn write_annot(writer: &mut Writer<Vec<u8>>, annot: &Annot) -> Result<(), OfdError> {
    let id = annot
        .id()
        .ok_or_else(|| missing("/PageAnnot/Annot/@ID", "缺少 Annot ID"))?;
    let annot_type = annot
        .annot_type()
        .ok_or_else(|| missing("/PageAnnot/Annot/@Type", "缺少 Annot Type"))?;
    let creator = annot
        .creator()
        .ok_or_else(|| missing("/PageAnnot/Annot/@Creator", "缺少 Annot Creator"))?;
    let last_mod_date = annot
        .last_mod_date()
        .ok_or_else(|| missing("/PageAnnot/Annot/@LastModDate", "缺少 LastModDate"))?;
    let mut attributes = vec![
        ("ID", id),
        ("Type", annot_type),
        ("Creator", creator),
        ("LastModDate", last_mod_date),
    ];
    if let Some(subtype) = annot.subtype() {
        attributes.push(("Subtype", subtype));
    }
    if let Some(visible) = annot.visible() {
        attributes.push(("Visible", bool_text(visible)));
    }
    if let Some(print) = annot.print() {
        attributes.push(("Print", bool_text(print)));
    }
    if let Some(no_zoom) = annot.no_zoom() {
        attributes.push(("NoZoom", bool_text(no_zoom)));
    }
    if let Some(no_rotate) = annot.no_rotate() {
        attributes.push(("NoRotate", bool_text(no_rotate)));
    }
    if let Some(read_only) = annot.read_only() {
        attributes.push(("ReadOnly", bool_text(read_only)));
    }
    write_start(writer, "ofd:Annot", &attributes)?;

    if let Some(remark) = annot.remark() {
        write_text_element(writer, "ofd:Remark", remark)?;
    }
    let parameters: Vec<_> = annot.parameters().collect();
    if !parameters.is_empty() {
        write_start(writer, "ofd:Parameters", &[])?;
        for parameter in parameters {
            write_parameter(writer, &parameter)?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:Parameters")))?;
    }
    if let Some(appearance) = annot.appearance() {
        let mut attributes = Vec::new();
        if let Some(boundary) = appearance.boundary() {
            attributes.push(("Boundary", boundary));
        }
        write_start(writer, "ofd:Appearance", &attributes)?;
        for object in appearance.objects() {
            write_graphic_object(writer, &object)?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:Appearance")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:Annot")))?;
    Ok(())
}

fn write_parameter(writer: &mut Writer<Vec<u8>>, parameter: &Parameter) -> Result<(), OfdError> {
    let name = parameter
        .name()
        .ok_or_else(|| missing("/PageAnnot/Annot/Parameters/Parameter/@Name", "缺少 Name"))?;
    write_start(writer, "ofd:Parameter", &[("Name", name)])?;
    writer.write_event(Event::Text(BytesText::new(parameter.value())))?;
    writer.write_event(Event::End(BytesEnd::new("ofd:Parameter")))?;
    Ok(())
}

fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}
