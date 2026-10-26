use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesText, Event};

use crate::error::OfdError;
use crate::model::page::{GraphicObject, GraphicObjectKind, Page};
use crate::write::action::write_actions;
use crate::write::document::write_page_area;
use crate::write::ofd::{missing, write_start, write_text_element};
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_page(page: &Page) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_start(&mut writer, "ofd:Page", &[("xmlns:ofd", OFD_NAMESPACE)])?;

    for template in page.templates() {
        let template_id = template
            .template_id()
            .ok_or_else(|| missing("/Page/Template/@TemplateID", "缺少 TemplateID"))?;
        let mut attributes = vec![("TemplateID", template_id)];
        if let Some(z_order) = template.z_order() {
            attributes.push(("ZOrder", z_order));
        }
        write_start(&mut writer, "ofd:Template", &attributes)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:Template")))?;
    }
    for path in page.page_res() {
        write_text_element(&mut writer, "ofd:PageRes", path)?;
    }
    if let Some(area) = page.area() {
        write_page_area(&mut writer, "ofd:Area", &area)?;
    }
    if let Some(content) = page.content() {
        write_start(&mut writer, "ofd:Content", &[])?;
        for layer in content.layers() {
            let id = layer
                .id()
                .ok_or_else(|| missing("/Page/Content/Layer/@ID", "缺少 Layer ID"))?;
            let mut attributes = vec![("ID", id)];
            if let Some(layer_type) = layer.layer_type() {
                attributes.push(("Type", layer_type));
            }
            if let Some(draw_param) = layer.draw_param() {
                attributes.push(("DrawParam", draw_param));
            }
            write_start(&mut writer, "ofd:Layer", &attributes)?;
            for object in layer.objects() {
                write_graphic_object(&mut writer, &object)?;
            }
            writer.write_event(Event::End(BytesEnd::new("ofd:Layer")))?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:Content")))?;
    }
    if let Some(actions) = page.actions() {
        write_actions(&mut writer, &actions)?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:Page")))?;
    let bytes = writer.into_inner();
    String::from_utf8(bytes)
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

pub(crate) fn write_graphic_object(
    writer: &mut Writer<Vec<u8>>,
    object: &GraphicObject,
) -> Result<(), OfdError> {
    let name = match object.kind() {
        GraphicObjectKind::Text => "ofd:TextObject",
        GraphicObjectKind::Path => "ofd:PathObject",
        GraphicObjectKind::Image => "ofd:ImageObject",
        GraphicObjectKind::Composite => "ofd:CompositeObject",
        GraphicObjectKind::PageBlock => "ofd:PageBlock",
        GraphicObjectKind::Other(_) => {
            return Err(OfdError::InvalidStructure(
                "严格写出不支持该图元类型".to_owned(),
            ));
        }
    };
    let doc = object.document();
    let node = object.node();
    let attributes: Vec<(&str, &str)> = doc
        .node(node)
        .attributes
        .iter()
        .map(|attribute| (attribute.name.local.as_str(), attribute.value.as_str()))
        .collect();
    write_start(writer, name, &attributes)?;

    if object.kind() == GraphicObjectKind::PageBlock {
        for child in object.children() {
            write_graphic_object(writer, &child)?;
        }
    } else {
        for child in doc.element_children(node) {
            write_subtree(writer, doc, child)?;
        }
    }

    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

pub(crate) fn write_subtree(
    writer: &mut Writer<Vec<u8>>,
    doc: &crate::xml::tree::XmlDocument,
    node: crate::xml::tree::NodeId,
) -> Result<(), OfdError> {
    let local = doc.name(node).local.as_str();
    let name = format!("ofd:{local}");
    let attributes: Vec<(&str, &str)> = doc
        .node(node)
        .attributes
        .iter()
        .map(|attribute| (attribute.name.local.as_str(), attribute.value.as_str()))
        .collect();
    write_start(writer, &name, &attributes)?;

    let has_children = doc.element_children(node).next().is_some();
    if !has_children {
        let text = doc.text(node);
        if !text.is_empty() {
            writer.write_event(Event::Text(BytesText::new(text)))?;
        }
    }
    for child in doc.element_children(node) {
        write_subtree(writer, doc, child)?;
    }
    writer.write_event(Event::End(BytesEnd::new(&name)))?;
    Ok(())
}
