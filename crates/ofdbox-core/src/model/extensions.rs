//! `Extensions.xsd`：扩展索引。

use std::io::{Read, Write};

use ofdbox_yaserde::de::Deserializer;
use ofdbox_yaserde::ser::Serializer;
use ofdbox_yaserde::{YaDeserialize, YaSerialize};

use super::definition::{StLoc, StRefId};

/// `Extensions.xml` 根对象。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
#[yaserde(
  namespace = "http://www.ofdspec.org/2016",
  xmlns = {
    "ofd" = "http://www.ofdspec.org/2016"
  },
  namespace_aliases = {
    "http://www.ofdspec.org" = "http://www.ofdspec.org/2016"
  }
)]
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
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        let ref_id = self.ref_id.to_string();
        let mut event = ofdbox_yaserde::__xml::writer::XmlEvent::start_element("Extension")
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
        writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
        for item in &self.items {
            writer.set_start_event_name(None);
            item.serialize(writer)?;
        }
        writer
            .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
            .map_err(ofdbox_yaserde::Error::custom)?;
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

impl YaDeserialize for Extension {
    fn deserialize<R: Read>(reader: &mut Deserializer<R>) -> ofdbox_yaserde::Result<Self> {
        let event = reader.next_event()?;
        let ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { attributes, .. } = event else {
            return Err(ofdbox_yaserde::Error::custom("Extension 缺少起始元素"));
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
                ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { name, .. } => {
                    let item = match name.local_name.as_str() {
                        "Property" => ExtensionItem::Property(Property::deserialize(reader)?),
                        "Data" => ExtensionItem::Data(RawXml::read(reader, "Data")?),
                        "ExtendData" => ExtensionItem::ExtendData(StLoc(read_text_content(
                            reader,
                            "ExtendData",
                        )?)),
                        other => {
                            return Err(ofdbox_yaserde::Error::custom(format!(
                                "未知 Extension 子元素: {other}"
                            )));
                        }
                    };
                    items.push(item);
                }
                ofdbox_yaserde::__xml::reader::XmlEvent::EndElement { name } => {
                    if name.local_name == "Extension" {
                        break;
                    }
                    let _ = reader.next_event()?;
                }
                ofdbox_yaserde::__xml::reader::XmlEvent::EndDocument => break,
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
                .map(|value| value.parse().map_err(ofdbox_yaserde::Error::custom))
                .transpose()?
                .unwrap_or_default(),
            items,
        })
    }
}

fn read_text_content<R: Read>(
    reader: &mut Deserializer<R>,
    expected: &str,
) -> ofdbox_yaserde::Result<String> {
    let event = reader.next_event()?;
    let ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { name, .. } = event else {
        return Err(ofdbox_yaserde::Error::custom("文本元素缺少起始标签"));
    };
    let mut text = String::new();
    loop {
        let event = reader.next_event()?;
        match event {
            ofdbox_yaserde::__xml::reader::XmlEvent::Characters(value) => text.push_str(&value),
            ofdbox_yaserde::__xml::reader::XmlEvent::EndElement { name } => {
                if name.local_name == expected {
                    break;
                }
            }
            ofdbox_yaserde::__xml::reader::XmlEvent::EndDocument => break,
            _ => {}
        }
    }
    let _ = name;
    Ok(text)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawXml {
    events: Vec<RawEvent>,
}

#[derive(Clone, Debug, PartialEq)]
enum RawEvent {
    Start {
        name: String,
        attributes: Vec<(String, String)>,
        namespaces: Vec<(String, String)>,
    },
    End,
    Text(String),
    CData(String),
}

impl RawXml {
    fn read<R: Read>(reader: &mut Deserializer<R>, expected: &str) -> ofdbox_yaserde::Result<Self> {
        let event = reader.next_event()?;
        let ofdbox_yaserde::__xml::reader::XmlEvent::StartElement { .. } = event else {
            return Err(ofdbox_yaserde::Error::custom("Data 缺少起始元素"));
        };

        let mut events = Vec::new();
        loop {
            let event = reader.next_event()?;
            match event {
                ofdbox_yaserde::__xml::reader::XmlEvent::StartElement {
                    name,
                    attributes,
                    namespace,
                } => {
                    events.push(RawEvent::Start {
                        name: qualified_name(&name),
                        attributes: attributes
                            .iter()
                            .map(|attribute| {
                                (qualified_name(&attribute.name), attribute.value.clone())
                            })
                            .collect(),
                        namespaces: namespace
                            .iter()
                            .filter(|(prefix, uri)| {
                                !prefix.is_empty()
                                    && *prefix != "xml"
                                    && *prefix != "xmlns"
                                    && *uri != crate::model_xml::OFD_NAMESPACE
                            })
                            .map(|(prefix, uri)| (prefix.to_owned(), uri.to_owned()))
                            .collect(),
                    });
                }
                ofdbox_yaserde::__xml::reader::XmlEvent::EndElement { name } => {
                    if name.local_name == expected {
                        break;
                    }
                    events.push(RawEvent::End);
                }
                ofdbox_yaserde::__xml::reader::XmlEvent::Characters(value) => {
                    events.push(RawEvent::Text(value));
                }
                ofdbox_yaserde::__xml::reader::XmlEvent::CData(value) => {
                    events.push(RawEvent::CData(value));
                }
                ofdbox_yaserde::__xml::reader::XmlEvent::EndDocument => break,
                _ => {}
            }
        }
        Ok(Self { events })
    }
}

fn qualified_name(name: &ofdbox_yaserde::__xml::name::OwnedName) -> String {
    match &name.prefix {
        Some(prefix) => format!("{prefix}:{}", name.local_name),
        None => name.local_name.clone(),
    }
}

impl YaSerialize for RawXml {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        for event in &self.events {
            match event {
                RawEvent::Start {
                    name,
                    attributes,
                    namespaces,
                } => {
                    let ns_attributes: Vec<(String, String)> = namespaces
                        .iter()
                        .map(|(prefix, uri)| {
                            let key = if prefix.is_empty() {
                                "xmlns".to_owned()
                            } else {
                                format!("xmlns:{prefix}")
                            };
                            (key, uri.clone())
                        })
                        .collect();
                    let mut event =
                        ofdbox_yaserde::__xml::writer::XmlEvent::start_element(name.as_str());
                    for (key, value) in attributes {
                        event = event.attr(key.as_str(), value.as_str());
                    }
                    for (key, value) in &ns_attributes {
                        event = event.attr(key.as_str(), value.as_str());
                    }
                    writer.write(event).map_err(ofdbox_yaserde::Error::custom)?;
                }
                RawEvent::End => {
                    writer
                        .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
                        .map_err(ofdbox_yaserde::Error::custom)?;
                }
                RawEvent::Text(value) => {
                    writer
                        .write(ofdbox_yaserde::__xml::writer::XmlEvent::characters(value))
                        .map_err(ofdbox_yaserde::Error::custom)?;
                }
                RawEvent::CData(value) => {
                    writer
                        .write(ofdbox_yaserde::__xml::writer::XmlEvent::cdata(value))
                        .map_err(ofdbox_yaserde::Error::custom)?;
                }
            }
        }
        Ok(())
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

fn write_raw_element<W: Write>(
    writer: &mut Serializer<W>,
    name: &str,
    value: &RawXml,
) -> ofdbox_yaserde::Result<()> {
    writer
        .write(ofdbox_yaserde::__xml::writer::XmlEvent::start_element(name))
        .map_err(ofdbox_yaserde::Error::custom)?;
    writer
        .write(ofdbox_yaserde::__xml::writer::XmlEvent::comment(
            "ofdbox-raw-start",
        ))
        .map_err(ofdbox_yaserde::Error::custom)?;
    value.serialize(writer)?;
    writer
        .write(ofdbox_yaserde::__xml::writer::XmlEvent::comment(
            "ofdbox-raw-end",
        ))
        .map_err(ofdbox_yaserde::Error::custom)?;
    writer
        .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
        .map_err(ofdbox_yaserde::Error::custom)?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExtensionItem {
    /// 属性。
    Property(Property),
    /// 任意数据。
    Data(RawXml),
    /// 外部数据路径。
    ExtendData(StLoc),
}

impl YaSerialize for ExtensionItem {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> ofdbox_yaserde::Result<()> {
        match self {
            Self::Property(value) => value.serialize(writer),
            Self::Data(value) => write_raw_element(writer, "Data", value),
            Self::ExtendData(value) => write_text_element(writer, "ExtendData", value.as_str()),
        }
    }

    fn serialize_attributes(
        &self,
        attributes: Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        namespace: ofdbox_yaserde::__xml::namespace::Namespace,
    ) -> ofdbox_yaserde::Result<(
        Vec<ofdbox_yaserde::__xml::attribute::OwnedAttribute>,
        ofdbox_yaserde::__xml::namespace::Namespace,
    )> {
        Ok((attributes, namespace))
    }
}

fn write_text_element<W: Write>(
    writer: &mut Serializer<W>,
    name: &str,
    value: &str,
) -> ofdbox_yaserde::Result<()> {
    writer
        .write(ofdbox_yaserde::__xml::writer::XmlEvent::start_element(name))
        .map_err(ofdbox_yaserde::Error::custom)?;
    writer
        .write(ofdbox_yaserde::__xml::writer::XmlEvent::characters(value))
        .map_err(ofdbox_yaserde::Error::custom)?;
    writer
        .write(ofdbox_yaserde::__xml::writer::XmlEvent::end_element())
        .map_err(ofdbox_yaserde::Error::custom)?;
    Ok(())
}

/// 扩展属性。
#[derive(
    Clone, Debug, Default, PartialEq, ofdbox_yaserde::YaSerialize, ofdbox_yaserde::YaDeserialize,
)]
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
