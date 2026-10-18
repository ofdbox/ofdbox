use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, Event};

use crate::error::OfdError;
use crate::model::res::{
    ColorSpaceView, CompositeGraphicUnitView, DrawParamView, FontView, MultiMediaView, ResView,
};
use crate::write::color::write_color;
use crate::write::ofd::{missing, write_start, write_text_element};
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_res(res: &ResView) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    let base_loc = res
        .base_loc()
        .ok_or_else(|| missing("/Res/@BaseLoc", "缺少 BaseLoc"))?;
    write_start(
        &mut writer,
        "ofd:Res",
        &[("xmlns:ofd", OFD_NAMESPACE), ("BaseLoc", base_loc)],
    )?;

    let color_spaces: Vec<_> = res.color_spaces().collect();
    if !color_spaces.is_empty() {
        write_start(&mut writer, "ofd:ColorSpaces", &[])?;
        for color_space in color_spaces {
            write_color_space(&mut writer, &color_space)?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:ColorSpaces")))?;
    }

    let draw_params: Vec<_> = res.draw_params().collect();
    if !draw_params.is_empty() {
        write_start(&mut writer, "ofd:DrawParams", &[])?;
        for draw_param in draw_params {
            write_draw_param(&mut writer, &draw_param)?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:DrawParams")))?;
    }

    let fonts: Vec<_> = res.fonts().collect();
    if !fonts.is_empty() {
        write_start(&mut writer, "ofd:Fonts", &[])?;
        for font in fonts {
            write_font(&mut writer, &font)?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:Fonts")))?;
    }

    let multi_medias: Vec<_> = res.multi_medias().collect();
    if !multi_medias.is_empty() {
        write_start(&mut writer, "ofd:MultiMedias", &[])?;
        for media in multi_medias {
            write_multi_media(&mut writer, &media)?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:MultiMedias")))?;
    }

    let composite_units: Vec<_> = res.composite_graphic_units().collect();
    if !composite_units.is_empty() {
        write_start(&mut writer, "ofd:CompositeGraphicUnits", &[])?;
        for unit in composite_units {
            write_composite_graphic_unit(&mut writer, &unit)?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:CompositeGraphicUnits")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:Res")))?;
    let bytes = writer.into_inner();
    String::from_utf8(bytes)
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

fn write_color_space(
    writer: &mut Writer<Vec<u8>>,
    color_space: &ColorSpaceView,
) -> Result<(), OfdError> {
    let id = color_space
        .id()
        .ok_or_else(|| missing("/Res/ColorSpaces/ColorSpace/@ID", "缺少 ID"))?;
    let color_type = color_space
        .color_space_type()
        .ok_or_else(|| missing("/Res/ColorSpaces/ColorSpace/@Type", "缺少 Type"))?;
    let bits = color_space
        .bits_per_component()
        .map(|value| value.to_string());
    let mut attributes = vec![("ID", id), ("Type", color_type)];
    if let Some(bits) = bits.as_deref() {
        attributes.push(("BitsPerComponent", bits));
    }
    if let Some(profile) = color_space.profile() {
        attributes.push(("Profile", profile));
    }
    write_start(writer, "ofd:ColorSpace", &attributes)?;
    let palette: Vec<&str> = color_space.palette().collect();
    if !palette.is_empty() {
        write_start(writer, "ofd:Palette", &[])?;
        for cv in palette {
            write_text_element(writer, "ofd:CV", cv)?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:Palette")))?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:ColorSpace")))?;
    Ok(())
}

fn write_draw_param(
    writer: &mut Writer<Vec<u8>>,
    draw_param: &DrawParamView,
) -> Result<(), OfdError> {
    let id = draw_param
        .id()
        .ok_or_else(|| missing("/Res/DrawParams/DrawParam/@ID", "缺少 ID"))?;
    let line_width = draw_param.line_width().map(|value| value.to_string());
    let dash_offset = draw_param.dash_offset().map(|value| value.to_string());
    let miter_limit = draw_param.miter_limit().map(|value| value.to_string());
    let mut attributes = vec![("ID", id)];
    if let Some(relative) = draw_param.relative() {
        attributes.push(("Relative", relative));
    }
    if let Some(line_width) = line_width.as_deref() {
        attributes.push(("LineWidth", line_width));
    }
    if let Some(join) = draw_param.join() {
        attributes.push(("Join", join));
    }
    if let Some(cap) = draw_param.cap() {
        attributes.push(("Cap", cap));
    }
    if let Some(dash_offset) = dash_offset.as_deref() {
        attributes.push(("DashOffset", dash_offset));
    }
    if let Some(dash_pattern) = draw_param.dash_pattern() {
        attributes.push(("DashPattern", dash_pattern));
    }
    if let Some(miter_limit) = miter_limit.as_deref() {
        attributes.push(("MiterLimit", miter_limit));
    }
    write_start(writer, "ofd:DrawParam", &attributes)?;
    if let Some(color) = draw_param.fill_color() {
        write_color(writer, "ofd:FillColor", &color)?;
    }
    if let Some(color) = draw_param.stroke_color() {
        write_color(writer, "ofd:StrokeColor", &color)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:DrawParam")))?;
    Ok(())
}

fn write_font(writer: &mut Writer<Vec<u8>>, font: &FontView) -> Result<(), OfdError> {
    let id = font
        .id()
        .ok_or_else(|| missing("/Res/Fonts/Font/@ID", "缺少 ID"))?;
    let font_name = font
        .font_name()
        .ok_or_else(|| missing("/Res/Fonts/Font/@FontName", "缺少 FontName"))?;
    let mut attributes = vec![("ID", id), ("FontName", font_name)];
    if let Some(family_name) = font.family_name() {
        attributes.push(("FamilyName", family_name));
    }
    if let Some(charset) = font.charset() {
        attributes.push(("Charset", charset));
    }
    for (name, value) in [
        ("Italic", font.italic()),
        ("Bold", font.bold()),
        ("Serif", font.serif()),
        ("FixedWidth", font.fixed_width()),
    ] {
        if let Some(value) = value {
            attributes.push((name, bool_text(value)));
        }
    }
    write_start(writer, "ofd:Font", &attributes)?;
    if let Some(font_file) = font.font_file() {
        write_text_element(writer, "ofd:FontFile", font_file)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:Font")))?;
    Ok(())
}

fn write_multi_media(writer: &mut Writer<Vec<u8>>, media: &MultiMediaView) -> Result<(), OfdError> {
    let id = media
        .id()
        .ok_or_else(|| missing("/Res/MultiMedias/MultiMedia/@ID", "缺少 ID"))?;
    let media_type = media
        .media_type()
        .ok_or_else(|| missing("/Res/MultiMedias/MultiMedia/@Type", "缺少 Type"))?;
    let mut attributes = vec![("ID", id), ("Type", media_type)];
    if let Some(format) = media.format() {
        attributes.push(("Format", format));
    }
    write_start(writer, "ofd:MultiMedia", &attributes)?;
    let media_file = media
        .media_file()
        .ok_or_else(|| missing("/Res/MultiMedias/MultiMedia/MediaFile", "缺少 MediaFile"))?;
    write_text_element(writer, "ofd:MediaFile", media_file)?;
    writer.write_event(Event::End(BytesEnd::new("ofd:MultiMedia")))?;
    Ok(())
}

fn write_composite_graphic_unit(
    writer: &mut Writer<Vec<u8>>,
    unit: &CompositeGraphicUnitView,
) -> Result<(), OfdError> {
    let id = unit.id().ok_or_else(|| {
        missing(
            "/Res/CompositeGraphicUnits/CompositeGraphicUnit/@ID",
            "缺少 ID",
        )
    })?;
    let width = unit.width().ok_or_else(|| {
        missing(
            "/Res/CompositeGraphicUnits/CompositeGraphicUnit/@Width",
            "缺少 Width",
        )
    })?;
    let height = unit.height().ok_or_else(|| {
        missing(
            "/Res/CompositeGraphicUnits/CompositeGraphicUnit/@Height",
            "缺少 Height",
        )
    })?;
    let width = width.to_string();
    let height = height.to_string();
    write_start(
        writer,
        "ofd:CompositeGraphicUnit",
        &[
            ("ID", id),
            ("Width", width.as_str()),
            ("Height", height.as_str()),
        ],
    )?;
    if let Some(thumbnail) = unit.thumbnail() {
        write_text_element(writer, "ofd:Thumbnail", thumbnail)?;
    }
    if let Some(substitution) = unit.substitution() {
        write_text_element(writer, "ofd:Substitution", substitution)?;
    }
    if let Some(content) = unit.content() {
        write_start(writer, "ofd:Content", &[])?;
        for object in content.objects() {
            crate::write::page::write_graphic_object(writer, &object)?;
        }
        writer.write_event(Event::End(BytesEnd::new("ofd:Content")))?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:CompositeGraphicUnit")))?;
    Ok(())
}

fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}
