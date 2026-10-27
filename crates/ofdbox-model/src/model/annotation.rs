//! `Annotation.xsd`：单页注释内容。

use std::io::{Read, Write};

use yaserde::de::Deserializer;
use yaserde::ser::Serializer;
use yaserde::{YaDeserialize, YaSerialize};

use super::definition::{StBox, StId};
use super::page::{GraphicObject, PageBlock};

/// `PageAnnot` 根对象。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "PageAnnot")]
pub struct PageAnnot {
    /// 注释。
    #[yaserde(rename = "Annot")]
    pub annots: Vec<Annot>,
}

/// 注释。
#[derive(Clone, Debug, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Annot")]
pub struct Annot {
    /// 注释 ID。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: StId,
    /// 注释类型。
    #[yaserde(attribute = true, rename = "Type")]
    pub annot_type: AnnotationType,
    /// 创建者。
    #[yaserde(attribute = true, rename = "Creator")]
    pub creator: String,
    /// 最后修改日期。
    #[yaserde(attribute = true, rename = "LastModDate")]
    pub last_mod_date: String,
    /// 是否可见。
    #[yaserde(attribute = true, rename = "Visible", default = "default_true")]
    pub visible: bool,
    /// 子类型。
    #[yaserde(attribute = true, rename = "Subtype")]
    pub subtype: Option<String>,
    /// 是否打印。
    #[yaserde(attribute = true, rename = "Print", default = "default_true")]
    pub print: bool,
    /// 是否随缩放。
    #[yaserde(attribute = true, rename = "NoZoom", default = "default_false")]
    pub no_zoom: bool,
    /// 是否随旋转。
    #[yaserde(attribute = true, rename = "NoRotate", default = "default_false")]
    pub no_rotate: bool,
    /// 是否只读。
    #[yaserde(attribute = true, rename = "ReadOnly", default = "default_true")]
    pub read_only: bool,
    /// 备注。
    #[yaserde(rename = "Remark")]
    pub remark: Option<String>,
    /// 参数。
    #[yaserde(rename = "Parameters")]
    pub parameters: Option<Parameters>,
    /// 外观。
    #[yaserde(rename = "Appearance")]
    pub appearance: Appearance,
}

/// 注释类型。
#[derive(Clone, Debug, Default, PartialEq, Eq, yaserde::DefaultYaSerde)]
pub enum AnnotationType {
    /// 链接。
    #[default]
    Link,
    /// 路径。
    Path,
    /// 高亮。
    Highlight,
    /// 印章。
    Stamp,
    /// 水印。
    Watermark,
    /// 未知值。
    Other(String),
}

crate::impl_xml_enum_codec!(AnnotationType, Other, {
    Link => "Link",
    Path => "Path",
    Highlight => "Highlight",
    Stamp => "Stamp",
    Watermark => "Watermark",
});

/// 注释参数集合。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Parameters")]
pub struct Parameters {
    /// 参数。
    #[yaserde(rename = "Parameter")]
    pub parameters: Vec<Parameter>,
}

/// 注释参数。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "Parameter")]
pub struct Parameter {
    /// 参数名。
    #[yaserde(attribute = true, rename = "Name")]
    pub name: String,
    /// 参数值。
    #[yaserde(text = true)]
    pub value: String,
}

/// 注释外观。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Appearance {
    /// 边界。
    pub boundary: Option<StBox>,
    /// 页块内容。
    pub page_block: PageBlock,
}

impl YaSerialize for Appearance {
    fn serialize<W: Write>(&self, writer: &mut Serializer<W>) -> yaserde::Result<()> {
        let boundary = self.boundary.map(|value| value.to_string());
        let mut event = yaserde::__xml::writer::XmlEvent::start_element("Appearance");
        if let Some(boundary) = &boundary {
            event = event.attr("Boundary", boundary.as_str());
        }
        writer.write(event).map_err(yaserde::Error::custom)?;
        for object in &self.page_block.objects {
            writer.set_start_event_name(None);
            object.serialize(writer)?;
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

impl YaDeserialize for Appearance {
    fn deserialize<R: Read>(reader: &mut Deserializer<R>) -> yaserde::Result<Self> {
        let event = reader.next_event()?;
        let yaserde::__xml::reader::XmlEvent::StartElement { attributes, .. } = event else {
            return Err(yaserde::Error::custom("Appearance 缺少起始元素"));
        };
        let boundary = attributes
            .iter()
            .find(|attribute| attribute.name.local_name == "Boundary")
            .map(|attribute| attribute.value.parse().map_err(yaserde::Error::custom))
            .transpose()?;
        let mut objects = Vec::new();
        loop {
            let event = reader.peek()?.to_owned();
            match event {
                yaserde::__xml::reader::XmlEvent::StartElement { .. } => {
                    objects.push(GraphicObject::deserialize(reader)?);
                }
                yaserde::__xml::reader::XmlEvent::EndElement { name } => {
                    if name.local_name == "Appearance" {
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
            boundary,
            page_block: PageBlock { objects },
        })
    }
}

impl Default for Annot {
    fn default() -> Self {
        Self {
            id: StId::default(),
            annot_type: AnnotationType::Link,
            creator: String::new(),
            last_mod_date: String::new(),
            visible: true,
            subtype: None,
            print: true,
            no_zoom: false,
            no_rotate: false,
            read_only: true,
            remark: None,
            parameters: None,
            appearance: Appearance::default(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}
