use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesText, Event};

use crate::error::OfdError;
use crate::model::decrypt_seed::{DecryptSeedView, DecryptUserView};
use crate::write::ofd::{missing, write_start, write_text_element};
use crate::xml::namespace::OFD_NAMESPACE;

pub(crate) fn write_decrypt_seed(seed: &DecryptSeedView) -> Result<String, OfdError> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    let id = seed
        .id()
        .ok_or_else(|| missing("/DecryptSeed/@ID", "缺少 DecryptSeed ID"))?;
    let case_id = seed
        .encrypt_case_id()
        .ok_or_else(|| missing("/DecryptSeed/@EncryptCaseId", "缺少 EncryptCaseId"))?;
    write_start(
        &mut writer,
        "ofd:DecryptSeed",
        &[
            ("xmlns:ofd", OFD_NAMESPACE),
            ("ID", id),
            ("EncryptCaseId", case_id),
        ],
    )?;

    let users: Vec<_> = seed.users().collect();
    if users.is_empty() {
        return Err(missing("/DecryptSeed/UserInfo", "缺少 UserInfo"));
    }
    for user in users {
        write_user(&mut writer, &user)?;
    }

    let extend_params = seed
        .extend_params()
        .ok_or_else(|| missing("/DecryptSeed/ExtendParams", "缺少 ExtendParams"))?;
    write_start(&mut writer, "ofd:ExtendParams", &[])?;
    for parameter in extend_params.parameters() {
        let name = parameter
            .name()
            .ok_or_else(|| missing("/ExtendParams/Parameter/@Name", "缺少 Name"))?;
        let mut attributes = vec![("Name", name)];
        if let Some(value) = parameter.value_attr() {
            attributes.push(("Value", value));
        }
        write_start(&mut writer, "ofd:Parameter", &attributes)?;
        writer.write_event(Event::Text(BytesText::new(parameter.text())))?;
        writer.write_event(Event::End(BytesEnd::new("ofd:Parameter")))?;
    }
    writer.write_event(Event::End(BytesEnd::new("ofd:ExtendParams")))?;

    writer.write_event(Event::End(BytesEnd::new("ofd:DecryptSeed")))?;
    String::from_utf8(writer.into_inner())
        .map_err(|error| OfdError::InvalidStructure(format!("序列化结果不是 UTF-8: {error}")))
}

fn write_user(writer: &mut Writer<Vec<u8>>, user: &DecryptUserView) -> Result<(), OfdError> {
    let name = user
        .user_name()
        .ok_or_else(|| missing("/DecryptSeed/UserInfo/@UserName", "缺少 UserName"))?;
    let mut attributes = vec![("UserName", name)];
    if let Some(user_type) = user.user_type() {
        attributes.push(("UserType", user_type));
    }
    write_start(writer, "ofd:UserInfo", &attributes)?;

    if let Some(cert) = user.user_cert() {
        write_text_element(writer, "ofd:UserCert", cert)?;
    }
    let key = user
        .encrypted_work_key()
        .ok_or_else(|| missing("/UserInfo/EncryptedWK", "缺少 EncryptedWK"))?;
    write_text_element(writer, "ofd:EncryptedWK", key)?;
    if let Some(iv) = user.iv_value() {
        write_text_element(writer, "ofd:IVValue", iv)?;
    }

    writer.write_event(Event::End(BytesEnd::new("ofd:UserInfo")))?;
    Ok(())
}
