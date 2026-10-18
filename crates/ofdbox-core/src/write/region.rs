use quick_xml::Writer;
use quick_xml::events::{BytesEnd, Event};

use crate::error::OfdError;
use crate::model::region::{RegionAreaView, RegionCommandKind, RegionCommandView, RegionView};
use crate::write::ofd::{missing, write_start};

pub(crate) fn write_region(
    writer: &mut Writer<Vec<u8>>,
    region: &RegionView,
) -> Result<(), OfdError> {
    write_start(writer, "ofd:Region", &[])?;
    for area in region.areas() {
        write_area(writer, &area)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:Region")))?;
    Ok(())
}

fn write_area(writer: &mut Writer<Vec<u8>>, area: &RegionAreaView) -> Result<(), OfdError> {
    let start = area
        .start()
        .ok_or_else(|| missing("/Region/Area/@Start", "缺少 Area Start"))?;
    write_start(writer, "ofd:Area", &[("Start", start)])?;
    for command in area.commands() {
        write_command(writer, &command)?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:Area")))?;
    Ok(())
}

fn write_command(
    writer: &mut Writer<Vec<u8>>,
    command: &RegionCommandView,
) -> Result<(), OfdError> {
    match command.kind() {
        RegionCommandKind::Move => {
            write_points(writer, "ofd:Move", &[("Point1", command.point1())])
        }
        RegionCommandKind::Line => {
            write_points(writer, "ofd:Line", &[("Point1", command.point1())])
        }
        RegionCommandKind::QuadraticBezier => write_points(
            writer,
            "ofd:QuadraticBezier",
            &[("Point1", command.point1()), ("Point2", command.point2())],
        ),
        RegionCommandKind::CubicBezier => write_points(
            writer,
            "ofd:CubicBezier",
            &[
                ("Point1", command.point1()),
                ("Point2", command.point2()),
                ("Point3", command.point3()),
            ],
        ),
        RegionCommandKind::Arc => {
            let sweep = command.sweep_direction().ok_or_else(|| {
                missing("/Region/Area/Arc/@SweepDirection", "缺少 SweepDirection")
            })?;
            let large_arc = command
                .large_arc()
                .ok_or_else(|| missing("/Region/Area/Arc/@LargeArc", "缺少 LargeArc"))?;
            let rotation = command
                .rotation_angle()
                .ok_or_else(|| missing("/Region/Area/Arc/@RotationAngle", "缺少 RotationAngle"))?;
            let rotation = rotation.to_string();
            let ellipse = command
                .ellipse_size()
                .ok_or_else(|| missing("/Region/Area/Arc/@EllipseSize", "缺少 EllipseSize"))?;
            let end = command
                .end_point()
                .ok_or_else(|| missing("/Region/Area/Arc/@EndPoint", "缺少 EndPoint"))?;
            write_start(
                writer,
                "ofd:Arc",
                &[
                    ("SweepDirection", bool_text(sweep)),
                    ("LargeArc", bool_text(large_arc)),
                    ("RotationAngle", rotation.as_str()),
                    ("EllipseSize", ellipse),
                    ("EndPoint", end),
                ],
            )?;
            writer.write_event(Event::End(BytesEnd::new("ofd:Arc")))?;
            Ok(())
        }
        RegionCommandKind::Close => {
            write_start(writer, "ofd:Close", &[])?;
            writer.write_event(Event::End(BytesEnd::new("ofd:Close")))?;
            Ok(())
        }
        RegionCommandKind::Other(other) => Err(OfdError::InvalidStructure(format!(
            "严格写出不支持区域命令: {other}"
        ))),
    }
}

fn write_points(
    writer: &mut Writer<Vec<u8>>,
    name: &str,
    points: &[(&str, Option<&str>)],
) -> Result<(), OfdError> {
    let mut attributes = Vec::new();
    for &(attribute, value) in points {
        if let Some(value) = value {
            attributes.push((attribute, value));
        }
    }
    write_start(writer, name, &attributes)?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}
