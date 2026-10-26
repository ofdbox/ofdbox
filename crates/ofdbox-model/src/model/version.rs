//! `Version.xsd`：版本描述文件。

use super::definition::StLoc;

/// `DocVersion` 根对象。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "DocVersion")]
pub struct DocVersion {
    /// 版本 ID。
    #[yaserde(attribute = true, rename = "ID")]
    pub id: String,
    /// 版本号。
    #[yaserde(attribute = true, rename = "Version")]
    pub version: Option<String>,
    /// 版本名称。
    #[yaserde(attribute = true, rename = "Name")]
    pub name: Option<String>,
    /// 创建日期。
    #[yaserde(attribute = true, rename = "CreationDate")]
    pub creation_date: Option<String>,
    /// 文件列表。
    #[yaserde(rename = "FileList")]
    pub file_list: FileList,
    /// 文档根路径。
    #[yaserde(rename = "DocRoot")]
    pub doc_root: StLoc,
}

/// 版本文件列表。
#[derive(Clone, Debug, Default, PartialEq, yaserde::YaSerialize, yaserde::YaDeserialize)]
#[yaserde(rename = "FileList")]
pub struct FileList {
    /// 文件。
    #[yaserde(rename = "File")]
    pub files: Vec<File>,
}

/// 版本文件。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct File {
    /// 文件 ID。
    pub id: String,
    /// 文件路径。
    pub file_loc: StLoc,
}

impl yaserde::YaSerialize for File {
    fn serialize<W: std::io::Write>(
        &self,
        writer: &mut yaserde::ser::Serializer<W>,
    ) -> yaserde::Result<()> {
        let event =
            yaserde::__xml::writer::XmlEvent::start_element("File").attr("ID", self.id.as_str());
        writer.write(event).map_err(yaserde::Error::custom)?;
        writer
            .write(yaserde::__xml::writer::XmlEvent::characters(
                self.file_loc.as_str(),
            ))
            .map_err(yaserde::Error::custom)?;
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

impl yaserde::YaDeserialize for File {
    fn deserialize<R: std::io::Read>(
        reader: &mut yaserde::de::Deserializer<R>,
    ) -> yaserde::Result<Self> {
        let event = reader.next_event()?;
        let yaserde::__xml::reader::XmlEvent::StartElement { attributes, .. } = event else {
            return Err(yaserde::Error::custom("File 缺少起始元素"));
        };
        let id = attributes
            .iter()
            .find(|attribute| attribute.name.local_name == "ID")
            .map(|attribute| attribute.value.clone())
            .unwrap_or_default();
        let mut text = String::new();
        loop {
            let event = reader.peek()?.to_owned();
            match event {
                yaserde::__xml::reader::XmlEvent::Characters(value) => {
                    text.push_str(&value);
                    let _ = reader.next_event()?;
                }
                yaserde::__xml::reader::XmlEvent::EndElement { .. } => break,
                _ => {
                    let _ = reader.next_event()?;
                }
            }
        }
        Ok(Self {
            id,
            file_loc: StLoc(text),
        })
    }
}
