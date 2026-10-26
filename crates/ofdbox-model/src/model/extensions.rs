//! `Extensions.xsd`：扩展索引。

use std::io::{Read, Write};

use yaserde::de::Deserializer;
use yaserde::ser::Serializer;
use yaserde::{YaDeserialize, YaSerialize};

use super::definition::{StLoc, StRefId};

/// `Extensions.xml` 根对象。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Extensions")]
pub struct Extensions {
    /// 扩展。
    #[yaserde(rename = "Extension")]
    pub extensions: Vec<Extension>,
}

/// 扩展。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extension {
    /// 应用名。
    pub app_name: String,
    /// 公司。
    pub company: Option<String>,
    /// 应用版本。
    pub app_version: Option<String>,
    /// 日期。
    pub date: Option<String>,
    /// 被扩展对象引用。
    pub ref_id: StRefId,
    /// 扩展内容。
    pub items: Vec<ExtensionItem>,
}

impl YaSerialize for Extension {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> yaserde::Result<()> {
        let ref_id = self.ref_id.to_string();
        let mut event = yaserde::__xml::writer::XmlEvent::start_element("Extension")
            .attr("AppName", self.app_name.as_str())
            .attr("RefId", ref_id.as_str());
        if let Some(value) = &self.company {
            event = event.attr("Company", value.as_str());
        }
        if let Some(value) = &self.app_version {
            event = event.attr("AppVersion", value.as_str());
        }
        if let Some(value) = &self.date {
            event = event.attr("Date", value.as_str());
        }
        writer.write(event).map_err(yaserde::Error::custom)?;
        for item in &self.items {
            writer.set_start_event_name(None);
            item.serialize(writer)?;
        }
        writer
            .write(yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<yaserde::__xml::attribute::OwnedAttribute>,
        namespace: yaserde::__xml::namespace::Namespace,
    ) -> yaserde::Result<(
        Vec<yaserde::__xml::attribute::OwnedAttribute>,
        yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaDeserialize for Extension {
    fn deserialize<R: Read>(reader: &mut Deserializer<R>) -> yaserde::Result<Self> {
        let event = reader.next_event()?;
        let yaserde::__xml::reader::XmlEvent::StartElement { attributes, .. } = event else {
            return Err(yaserde::Error::custom("Extension 缺少起始元素"));
        };
        let value = |key: &str| {
            attributes
                .iter()
                .find(|attribute| attribute.name.local_name == key)
                .map(|attribute| attribute.value.clone())
        };
        let mut items = Vec::new();
        loop {
            let event = reader.peek()?.to_owned();
            match event {
                yaserde::__xml::reader::XmlEvent::StartElement { name, .. } => {
                    let item = match name.local_name.as_str() {
                        "Property" => ExtensionItem::Property(Property::deserialize(reader)?),
                        "Data" => ExtensionItem::Data(read_text_content(reader, "Data")?),
                        "ExtendData" => ExtensionItem::ExtendData(StLoc(read_text_content(
                            reader,
                            "ExtendData",
                        )?)),
                        other => {
                            return Err(yaserde::Error::custom(format!(
                                "未知 Extension 子元素: {other}"
                            )));
                        }
                    };
                    items.push(item);
                }
                yaserde::__xml::reader::XmlEvent::EndElement { name } => {
                    if name.local_name == "Extension" {
                        break;
                    }
                    let _ = reader.next_event()?;
                }
                yaserde::__xml::reader::XmlEvent::EndDocument => break,
                _ => {
                    let _ = reader.next_event()?;
                }
            }
        }
        Ok(Self {
            app_name: value("AppName").unwrap_or_default(),
            company: value("Company"),
            app_version: value("AppVersion"),
            date: value("Date"),
            ref_id: value("RefId")
                .map(|value| value.parse().map_err(yaserde::Error::custom))
                .transpose()?
                .unwrap_or_default(),
            items,
        })
    }
}

fn read_text_content<R: Read>(
    reader: &mut Deserializer<R>,
    expected: &str,
) -> yaserde::Result<String> {
    let event = reader.next_event()?;
    let yaserde::__xml::reader::XmlEvent::StartElement { name, .. } = event else {
        return Err(yaserde::Error::custom("文本元素缺少起始标签"));
    };
    let mut text = String::new();
    loop {
        let event = reader.next_event()?;
        match event {
            yaserde::__xml::reader::XmlEvent::Characters(value) => text.push_str(&value),
            yaserde::__xml::reader::XmlEvent::EndElement { name } => {
                if name.local_name == expected {
                    break;
                }
            }
            yaserde::__xml::reader::XmlEvent::EndDocument => break,
            _ => {}
        }
    }
    let _ = name;
    Ok(text)
}

/// 扩展内容。
#[derive(Clone, Debug, PartialEq)]
pub enum ExtensionItem {
    /// 属性。
    Property(Property),
    /// 任意数据。
    Data(String),
    /// 外部数据路径。
    ExtendData(StLoc),
}

impl YaSerialize for ExtensionItem {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> yaserde::Result<()> {
        match self {
            Self::Property(value) => value.serialize(writer),
            Self::Data(value) => write_text_element(writer, "Data", value),
            Self::ExtendData(value) => write_text_element(writer, "ExtendData", value.as_str()),
        }
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<yaserde::__xml::attribute::OwnedAttribute>,
        namespace: yaserde::__xml::namespace::Namespace,
    ) -> yaserde::Result<(
        Vec<yaserde::__xml::attribute::OwnedAttribute>,
        yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

fn write_text_element<W: Write>(
    writer: &mut Serializer<W>,
    name: &str,
    value: &str,
) -> yaserde::Result<()> {
    writer
        .write(yaserde::__xml::writer::XmlEvent::start_element(name))
        .map_err(yaserde::Error::custom)?;
    writer
        .write(yaserde::__xml::writer::XmlEvent::characters(value))
        .map_err(yaserde::Error::custom)?;
    writer
        .write(yaserde::__xml::writer::XmlEvent::end_element())
        .map_err(yaserde::Error::custom)?;
    Ok(())
}

/// 扩展属性。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Property")]
pub struct Property {
    /// 属性名。
    #[yaserde(attribute = true, rename = "Name")]
    pub name: String,
    /// 属性类型。
    #[yaserde(attribute = true, rename = "Type")]
    pub property_type: Option<String>,
    /// 属性值。
    #[yaserde(text = true)]
    pub value: String,
}
