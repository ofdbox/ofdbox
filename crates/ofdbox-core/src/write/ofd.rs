use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};

use crate::error::{Diagnostic, DiagnosticCode, OfdError};
use crate::model::ofd::{DocInfo, OfdRoot};
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_root(root: &OfdRoot) -> Result<String, OfdError> {
    let version = root
        .version()
        .ok_or_else(|| missing("/OFD/@Version", "缺少 Version 属性"))?;
    let doc_type = root
        .doc_type()
        .ok_or_else(|| missing("/OFD/@DocType", "缺少 DocType 属性"))?;

    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_start(
        &mut writer,
        "ofd:OFD",
        &[
            ("xmlns:ofd", OFD_NAMESPACE),
            ("Version", version),
            ("DocType", doc_type),
        ],
    )?;

    for (index, body) in root.doc_bodies().enumerate() {
        let body_path = format!("/OFD/DocBody[{index}]");
        write_start(&mut writer, "ofd:DocBody", &[])?;

        let info = body
            .doc_info()
            .ok_or_else(|| missing(&format!("{body_path}/DocInfo"), "缺少 DocInfo"))?;
        write_doc_info(&mut writer, &info, &format!("{body_path}/DocInfo"))?;

        let doc_root = body
            .doc_root()
            .ok_or_else(|| missing(&format!("{body_path}/DocRoot"), "缺少 DocRoot"))?;
        write_text_element(&mut writer, "ofd:DocRoot", doc_root)?;

        if let Some(versions) = body.versions() {
            write_start(&mut writer, "ofd:Versions", &[])?;
            for (version_index, version) in versions.versions().enumerate() {
                let path = format!("{body_path}/Versions/Version[{version_index}]");
                let id = version
                    .id()
                    .ok_or_else(|| missing(&format!("{path}/@ID"), "缺少 Version ID"))?;
                let number = version
                    .index()
                    .ok_or_else(|| missing(&format!("{path}/@Index"), "缺少 Version Index"))?;
                let base_loc = version
                    .base_loc()
                    .ok_or_else(|| missing(&format!("{path}/@BaseLoc"), "缺少 Version BaseLoc"))?;
                let index_text = number.to_string();
                let mut attributes = vec![
                    ("ID", id),
                    ("Index", index_text.as_str()),
                    ("BaseLoc", base_loc),
                ];
                if version.current() {
                    attributes.insert(2, ("Current", "true"));
                }
                write_start(&mut writer, "ofd:Version", &attributes)?;
                writer.write_event(Event::End(BytesEnd::new("ofd:Version")))?;
            }
            writer.write_event(Event::End(BytesEnd::new("ofd:Versions")))?;
        }

        if let Some(signatures) = body.signatures() {
            write_text_element(&mut writer, "ofd:Signatures", signatures)?;
        }

        writer.write_event(Event::End(BytesEnd::new("ofd:DocBody")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:OFD")))?;
    let bytes = writer.into_inner();
    String::from_utf8(bytes)
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

fn write_doc_info(
    writer: &mut Writer<Vec<u8>>,
    info: &DocInfo,
    path: &str,
) -> Result<(), OfdError> {
    write_start(writer, "ofd:DocInfo", &[])?;

    let doc_id = info
        .doc_id()
        .ok_or_else(|| missing(&format!("{path}/DocID"), "缺少 DocID"))?;
    write_text_element(writer, "ofd:DocID", doc_id)?;

    for (name, value) in [
        ("ofd:Title", info.title()),
        ("ofd:Author", info.author()),
        ("ofd:Subject", info.subject()),
        ("ofd:Abstract", info.abstract_()),
        ("ofd:CreationDate", info.creation_date()),
        ("ofd:ModDate", info.mod_date()),
        ("ofd:DocUsage", info.doc_usage()),
        ("ofd:Cover", info.cover()),
    ] {
        if let Some(value) = value {
            write_text_element(writer, name, value)?;
        }
    }

    let keywords: Vec<&str> = info.keywords().collect();
    if !keywords.is_empty() {
        write_start(writer, "ofd:Keywords", &[])?;
        for keyword in keywords {
            write_text_element(writer, "ofd:Keyword", keyword)?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:Keywords")))?;
    }

    for (name, value) in [
        ("ofd:Creator", info.creator()),
        ("ofd:CreatorVersion", info.creator_version()),
    ] {
        if let Some(value) = value {
            write_text_element(writer, name, value)?;
        }
    }

    let custom_datas: Vec<_> = info.custom_datas().collect();
    if !custom_datas.is_empty() {
        write_start(writer, "ofd:CustomDatas", &[])?;
        for custom in custom_datas {
            let name = custom.name().ok_or_else(|| {
                missing(
                    &format!("{path}/CustomDatas/CustomData/@Name"),
                    "缺少 CustomData Name",
                )
            })?;
            write_start(writer, "ofd:CustomData", &[("Name", name)])?;
            writer.write_event(Event::Text(BytesText::new(custom.value())))?;
            writer.write_event(Event::End(BytesEnd::new("ofd:CustomData")))?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:CustomDatas")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:DocInfo")))?;
    Ok(())
}

pub(crate) fn write_start(
    writer: &mut Writer<Vec<u8>>,
    name: &str,
    attributes: &[(&str, &str)],
) -> Result<(), OfdError> {
    let element = local_name(name);
    let escaped: Vec<(String, String)> = attributes
        .iter()
        .filter(|(key, value)| !is_default_attribute(element, key, value))
        .map(|(key, value)| ((*key).to_owned(), escape_attribute(value)))
        .collect();
    let mut start = BytesStart::new(name);
    for (key, value) in &escaped {
        start.push_attribute((key.as_str(), value.as_str()));
    }
    writer.write_event(Event::Start(start))?;
    Ok(())
}

pub(crate) fn write_text_element(
    writer: &mut Writer<Vec<u8>>,
    name: &str,
    value: &str,
) -> Result<(), OfdError> {
    if is_default_element(local_name(name), value) {
        return Ok(());
    }
    writer.write_event(Event::Start(BytesStart::new(name)))?;
    writer.write_event(Event::Text(BytesText::new(value)))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn is_default_attribute(element: &str, attribute: &str, value: &str) -> bool {
    let common_graphic = matches!(
        element,
        "TextObject" | "Text" | "PathObject" | "Path" | "ImageObject" | "CompositeObject"
    );
    if common_graphic {
        match attribute {
            "Visible" => return bool_is(value, true),
            "LineWidth" => return float_is(value, 0.353),
            "Cap" => return value == "Butt",
            "Join" => return value == "Miter",
            "MiterLimit" => return float_is(value, 4.234),
            "DashOffset" => return float_is(value, 0.0),
            "Alpha" => return int_is(value, 255),
            _ => {}
        }
    }
    match (element, attribute) {
        ("TextObject" | "Text", "Stroke") => bool_is(value, false),
        ("TextObject" | "Text", "Fill") => bool_is(value, true),
        ("TextObject" | "Text", "HScale") => float_is(value, 1.0),
        ("TextObject" | "Text", "ReadDirection") => int_is(value, 0),
        ("TextObject" | "Text", "CharDirection") => int_is(value, 0),
        ("TextObject" | "Text", "Weight") => int_is(value, 400),
        ("TextObject" | "Text", "Italic") => bool_is(value, false),
        ("PathObject" | "Path", "Stroke") => bool_is(value, true),
        ("PathObject" | "Path", "Fill") => bool_is(value, false),
        ("PathObject" | "Path", "Rule") => value == "NonZero",
        ("Border", "LineWidth") => float_is(value, 0.353),
        ("Border", "HorizonalCornerRadius" | "VerticalCornerRadius") => float_is(value, 0.0),
        ("Border", "DashOffset") => float_is(value, 0.0),
        ("Layer", "Type") => value == "Body",
        ("Template", "ZOrder") => value == "Background",
        ("Pattern", "ReflectMethod") => value == "Normal",
        ("Pattern", "RelativeTo") => value == "Object",
        ("AxialShd" | "RadialShd", "MapType") => value == "Direct",
        ("AxialShd", "Extend") => int_is(value, 0),
        ("RadialShd", "Eccentricity" | "Angle" | "StartRadius") => float_is(value, 0.0),
        ("RadialShd", "Extend") => int_is(value, 0),
        ("Color" | "FillColor" | "StrokeColor" | "BackColor" | "BorderColor", "Alpha") => {
            int_is(value, 255)
        }
        ("Print", "Copies") => int_is(value, -1),
        ("DrawParam", "LineWidth") => float_is(value, 0.353),
        ("DrawParam", "Join") => value == "Miter",
        ("DrawParam", "Cap") => value == "Butt",
        ("DrawParam", "DashOffset") => float_is(value, 0.0),
        ("DrawParam", "MiterLimit") => float_is(value, 4.234),
        ("ColorSpace", "BitsPerComponent") => int_is(value, 8),
        ("Font", "Charset") => value == "unicode",
        ("Font", "Italic" | "Bold" | "Serif" | "FixedWidth") => bool_is(value, false),
        ("CGTransform", "CodeCount" | "GlyphCount") => int_is(value, 1),
        ("Signature", "Type") => value == "Seal",
        ("Attachment", "Visible") => bool_is(value, true),
        ("Attachment", "Usage") => value == "none",
        ("Annot", "Visible" | "Print") => bool_is(value, true),
        ("Annot", "NoZoom" | "NoRotate") => bool_is(value, false),
        ("Annot", "ReadOnly") => bool_is(value, true),
        ("Version", "Current") => bool_is(value, false),
        ("OutlineElem", "Expanded") => bool_is(value, true),
        ("GotoA", "NewWindow") => bool_is(value, true),
        ("Movie", "Operator") => value == "Play",
        ("References", "CheckMethod") => value == "MD5",
        _ => false,
    }
}

fn is_default_element(element: &str, value: &str) -> bool {
    match element {
        "Edit" | "Annot" | "Export" | "Signature" | "Watermark" | "PrintScreen" => {
            bool_is(value, true)
        }
        "PageMode" => value == "None",
        "PageLayout" => value == "OneColumn",
        "TabDisplay" => value == "DocTitle",
        "HideToolbar" | "HideMenubar" | "HideWindowUI" => bool_is(value, false),
        _ => false,
    }
}

fn bool_is(value: &str, expected: bool) -> bool {
    match value {
        "true" | "1" => expected,
        "false" | "0" => !expected,
        _ => false,
    }
}

fn float_is(value: &str, expected: f64) -> bool {
    value
        .parse::<f64>()
        .map(|value| value == expected)
        .unwrap_or(false)
}

fn int_is(value: &str, expected: i64) -> bool {
    value
        .parse::<i64>()
        .map(|value| value == expected)
        .unwrap_or(false)
}

pub(crate) fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(crate) fn missing(path: &str, message: &str) -> OfdError {
    OfdError::Conformance(vec![Diagnostic::new(
        DiagnosticCode::MissingRequired,
        path,
        message,
    )])
}
