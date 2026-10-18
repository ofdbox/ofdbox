use quick_xml::Writer;
use quick_xml::events::{BytesEnd, Event};

use crate::error::OfdError;
use crate::model::color::{
    AxialShdView, ColorKind, ColorView, GouraudShdView, LaGouraudShdView, PatternView, PointView,
    RadialShdView, SegmentView,
};
use crate::write::ofd::{missing, write_start};
use crate::write::page::write_graphic_object;

/// 写出一个颜色元素，`name` 为 `ofd:FillColor`、`ofd:StrokeColor` 或 `ofd:Color`。
pub(crate) fn write_color(
    writer: &mut Writer<Vec<u8>>,
    name: &str,
    color: &ColorView,
) -> Result<(), OfdError> {
    let index = color.index().map(|value| value.to_string());
    let alpha = color.alpha().map(|value| value.to_string());
    let mut attributes = Vec::new();
    if let Some(value) = color.value() {
        attributes.push(("Value", value));
    }
    if let Some(index) = index.as_deref() {
        attributes.push(("Index", index));
    }
    if let Some(color_space) = color.color_space() {
        attributes.push(("ColorSpace", color_space));
    }
    if let Some(alpha) = alpha.as_deref() {
        attributes.push(("Alpha", alpha));
    }
    write_start(writer, name, &attributes)?;

    match color.kind() {
        ColorKind::Solid => {}
        ColorKind::Pattern => {
            let pattern = color.pattern().expect("kind reports Pattern");
            write_pattern(writer, &pattern)?;
        }
        ColorKind::AxialShd => {
            let shd = color.axial_shd().expect("kind reports AxialShd");
            write_axial_shd(writer, &shd)?;
        }
        ColorKind::RadialShd => {
            let shd = color.radial_shd().expect("kind reports RadialShd");
            write_radial_shd(writer, &shd)?;
        }
        ColorKind::GouraudShd => {
            let shd = color.gouraud_shd().expect("kind reports GouraudShd");
            write_gouraud_shd(writer, &shd)?;
        }
        ColorKind::LaGouraudShd => {
            let shd = color.la_gouraud_shd().expect("kind reports LaGouraudShd");
            write_la_gouraud_shd(writer, &shd)?;
        }
        ColorKind::Other(other) => {
            return Err(OfdError::InvalidStructure(format!(
                "严格写出不支持颜色形态: {other}"
            )));
        }
    }

    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

fn write_pattern(writer: &mut Writer<Vec<u8>>, pattern: &PatternView) -> Result<(), OfdError> {
    let width = pattern
        .width()
        .ok_or_else(|| missing("/Pattern/@Width", "缺少 Pattern Width"))?
        .to_string();
    let height = pattern
        .height()
        .ok_or_else(|| missing("/Pattern/@Height", "缺少 Pattern Height"))?
        .to_string();
    let x_step = pattern.x_step().map(|value| value.to_string());
    let y_step = pattern.y_step().map(|value| value.to_string());
    let mut attributes = vec![("Width", width.as_str()), ("Height", height.as_str())];
    if let Some(x_step) = x_step.as_deref() {
        attributes.push(("XStep", x_step));
    }
    if let Some(y_step) = y_step.as_deref() {
        attributes.push(("YStep", y_step));
    }
    if let Some(method) = pattern.reflect_method() {
        attributes.push(("ReflectMethod", method));
    }
    if let Some(relative_to) = pattern.relative_to() {
        attributes.push(("RelativeTo", relative_to));
    }
    if let Some(ctm) = pattern.ctm() {
        attributes.push(("CTM", ctm));
    }
    write_start(writer, "ofd:Pattern", &attributes)?;

    let mut content_attributes = Vec::new();
    if let Some(thumbnail) = pattern.thumbnail() {
        content_attributes.push(("Thumbnail", thumbnail));
    }
    write_start(writer, "ofd:CellContent", &content_attributes)?;
    for object in pattern.objects() {
        write_graphic_object(writer, &object)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:CellContent")))?;

    writer.write_event(Event::End(BytesEnd::new("ofd:Pattern")))?;
    Ok(())
}

fn write_axial_shd(writer: &mut Writer<Vec<u8>>, shd: &AxialShdView) -> Result<(), OfdError> {
    let start = shd
        .start_point()
        .ok_or_else(|| missing("/AxialShd/@StartPoint", "缺少 StartPoint"))?;
    let end = shd
        .end_point()
        .ok_or_else(|| missing("/AxialShd/@EndPoint", "缺少 EndPoint"))?;
    let map_unit = shd.map_unit().map(|value| value.to_string());
    let extend = shd.extend().map(|value| value.to_string());
    let mut attributes = Vec::new();
    if let Some(map_type) = shd.map_type() {
        attributes.push(("MapType", map_type));
    }
    if let Some(map_unit) = map_unit.as_deref() {
        attributes.push(("MapUnit", map_unit));
    }
    if let Some(extend) = extend.as_deref() {
        attributes.push(("Extend", extend));
    }
    attributes.push(("StartPoint", start));
    attributes.push(("EndPoint", end));
    write_start(writer, "ofd:AxialShd", &attributes)?;
    write_segments(writer, shd.segments())?;
    writer.write_event(Event::End(BytesEnd::new("ofd:AxialShd")))?;
    Ok(())
}

fn write_radial_shd(writer: &mut Writer<Vec<u8>>, shd: &RadialShdView) -> Result<(), OfdError> {
    let start = shd
        .start_point()
        .ok_or_else(|| missing("/RadialShd/@StartPoint", "缺少 StartPoint"))?;
    let end = shd
        .end_point()
        .ok_or_else(|| missing("/RadialShd/@EndPoint", "缺少 EndPoint"))?;
    let end_radius = shd
        .end_radius()
        .ok_or_else(|| missing("/RadialShd/@EndRadius", "缺少 EndRadius"))?;
    let map_unit = shd.map_unit().map(|value| value.to_string());
    let eccentricity = shd.eccentricity().map(|value| value.to_string());
    let angle = shd.angle().map(|value| value.to_string());
    let start_radius = shd.start_radius().map(|value| value.to_string());
    let end_radius = end_radius.to_string();
    let extend = shd.extend().map(|value| value.to_string());
    let mut attributes = Vec::new();
    if let Some(map_type) = shd.map_type() {
        attributes.push(("MapType", map_type));
    }
    if let Some(map_unit) = map_unit.as_deref() {
        attributes.push(("MapUnit", map_unit));
    }
    if let Some(eccentricity) = eccentricity.as_deref() {
        attributes.push(("Eccentricity", eccentricity));
    }
    if let Some(angle) = angle.as_deref() {
        attributes.push(("Angle", angle));
    }
    attributes.push(("StartPoint", start));
    if let Some(start_radius) = start_radius.as_deref() {
        attributes.push(("StartRadius", start_radius));
    }
    attributes.push(("EndPoint", end));
    attributes.push(("EndRadius", end_radius.as_str()));
    if let Some(extend) = extend.as_deref() {
        attributes.push(("Extend", extend));
    }
    write_start(writer, "ofd:RadialShd", &attributes)?;
    write_segments(writer, shd.segments())?;
    writer.write_event(Event::End(BytesEnd::new("ofd:RadialShd")))?;
    Ok(())
}

fn write_segments(
    writer: &mut Writer<Vec<u8>>,
    segments: impl Iterator<Item = SegmentView>,
) -> Result<(), OfdError> {
    for segment in segments {
        let position = segment.position().map(|value| value.to_string());
        let mut attributes = Vec::new();
        if let Some(position) = position.as_deref() {
            attributes.push(("Position", position));
        }
        write_start(writer, "ofd:Segment", &attributes)?;
        let color = segment
            .color()
            .ok_or_else(|| missing("/Segment/Color", "缺少 Segment Color"))?;
        write_color(writer, "ofd:Color", &color)?;
        writer.write_event(Event::End(BytesEnd::new("ofd:Segment")))?;
    }
    Ok(())
}

fn write_gouraud_shd(writer: &mut Writer<Vec<u8>>, shd: &GouraudShdView) -> Result<(), OfdError> {
    let extend = shd.extend().map(|value| value.to_string());
    let mut attributes = Vec::new();
    if let Some(extend) = extend.as_deref() {
        attributes.push(("Extend", extend));
    }
    write_start(writer, "ofd:GouraudShd", &attributes)?;
    for point in shd.points() {
        write_point(writer, &point)?;
    }
    if let Some(back) = shd.back_color() {
        write_color(writer, "ofd:BackColor", &back)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:GouraudShd")))?;
    Ok(())
}

fn write_la_gouraud_shd(
    writer: &mut Writer<Vec<u8>>,
    shd: &LaGouraudShdView,
) -> Result<(), OfdError> {
    let vertices = shd
        .vertices_per_row()
        .ok_or_else(|| missing("/LaGourandShd/@VerticesPerRow", "缺少 VerticesPerRow"))?;
    let vertices = vertices.to_string();
    let extend = shd.extend().map(|value| value.to_string());
    let mut attributes = vec![("VerticesPerRow", vertices.as_str())];
    if let Some(extend) = extend.as_deref() {
        attributes.push(("Extend", extend));
    }
    write_start(writer, "ofd:LaGourandShd", &attributes)?;
    for point in shd.points() {
        write_point(writer, &point)?;
    }
    if let Some(back) = shd.back_color() {
        write_color(writer, "ofd:BackColor", &back)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:LaGourandShd")))?;
    Ok(())
}

fn write_point(writer: &mut Writer<Vec<u8>>, point: &PointView) -> Result<(), OfdError> {
    let x = point.x().map(|value| value.to_string());
    let y = point.y().map(|value| value.to_string());
    let edge_flag = point.edge_flag().map(|value| value.to_string());
    let mut attributes = Vec::new();
    if let Some(x) = x.as_deref() {
        attributes.push(("X", x));
    }
    if let Some(y) = y.as_deref() {
        attributes.push(("Y", y));
    }
    if let Some(edge_flag) = edge_flag.as_deref() {
        attributes.push(("EdgeFlag", edge_flag));
    }
    write_start(writer, "ofd:Point", &attributes)?;
    let color = point
        .color()
        .ok_or_else(|| missing("/Point/Color", "缺少 Point Color"))?;
    write_color(writer, "ofd:Color", &color)?;
    writer.write_event(Event::End(BytesEnd::new("ofd:Point")))?;
    Ok(())
}
