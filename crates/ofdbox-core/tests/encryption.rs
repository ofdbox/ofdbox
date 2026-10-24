mod fixtures;

use ofdbox_core::{
    DecryptSeedView, EncryptedEntriesView, EncryptionsView, OfdError, OfdPackage, ReadOptions,
};

const ENCRYPTIONS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Encryptions xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:EncryptInfo ID="1" Relative="0">
    <ofd:Provider Name="ExampleCrypto" Company="Example" Version="1.0" ProtocolVer="2.0">
      <ofd:ExtendData>AAAA</ofd:ExtendData>
    </ofd:Provider>
    <ofd:EncryptScope>All</ofd:EncryptScope>
    <ofd:Parameters>
      <ofd:Parameter Name="role" Type="string">owner</ofd:Parameter>
    </ofd:Parameters>
    <ofd:EncryptDate>2024-01-02T03:04:05Z</ofd:EncryptDate>
    <ofd:DecryptSeedLoc>/decryptseed.xml</ofd:DecryptSeedLoc>
    <ofd:EntriesMapLoc>/EntriesMap.xml</ofd:EntriesMapLoc>
  </ofd:EncryptInfo>
</ofd:Encryptions>"#;

const DECRYPT_SEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:DecryptSeed xmlns:ofd="http://www.ofdspec.org/2016" ID="1" EncryptCaseId="1.1.1">
  <ofd:UserInfo UserName="alice" UserType="User">
    <ofd:UserCert>AAAA</ofd:UserCert>
    <ofd:EncryptedWK>BBBB</ofd:EncryptedWK>
    <ofd:IVValue>CCCC</ofd:IVValue>
  </ofd:UserInfo>
  <ofd:ExtendParams>
    <ofd:Parameter Name="p" Value="v">text</ofd:Parameter>
  </ofd:ExtendParams>
</ofd:DecryptSeed>"#;

const ENTRIES_MAP: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:EncryptedEntries xmlns:ofd="http://www.ofdspec.org/2016" ID="1">
  <ofd:EncryptEntry Path="/OFD.xml" EPath="/ofd.dat"/>
  <ofd:EncryptEntry Path="/Doc_0/Document.xml" EPath="/Doc_0/document.dat">
    <ofd:DecryptSeedLoc>/decryptseed.xml</ofd:DecryptSeedLoc>
  </ofd:EncryptEntry>
</ofd:EncryptedEntries>"#;

fn parse_encryptions(xml: &str, options: ReadOptions) -> Result<EncryptionsView, OfdError> {
    EncryptionsView::from_xml(xml.as_bytes(), options)
}

#[test]
fn encryptions_parse_and_roundtrip() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = parse_encryptions(ENCRYPTIONS, options).expect("canonical Encryptions");
        let info = view.encrypt_infos().next().expect("EncryptInfo");
        assert_eq!(info.id(), Some("1"));
        assert_eq!(info.relative(), Some("0"));
        assert_eq!(info.encrypt_scope(), Some("All"));
        assert_eq!(info.encrypt_date(), Some("2024-01-02T03:04:05Z"));
        assert_eq!(info.decrypt_seed_loc(), Some("/decryptseed.xml"));
        assert_eq!(info.entries_map_loc(), Some("/EntriesMap.xml"));

        let provider = info.provider().expect("Provider");
        assert_eq!(provider.name(), Some("ExampleCrypto"));
        assert_eq!(provider.company(), Some("Example"));
        assert_eq!(provider.version(), Some("1.0"));
        assert_eq!(provider.protocol_version(), Some("2.0"));
        assert_eq!(provider.extend_data(), Some("AAAA"));

        let parameter = info.parameters().next().expect("Parameter");
        assert_eq!(parameter.name(), Some("role"));
        assert_eq!(parameter.value_type(), Some("string"));
        assert_eq!(parameter.value(), "owner");
    }

    let view = parse_encryptions(ENCRYPTIONS, ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:Encryptions"));
    assert!(xml.contains(r#"ProtocolVer="2.0""#));
    assert!(xml.contains("<ofd:ExtendData>AAAA</ofd:ExtendData>"));
    let reparsed =
        EncryptionsView::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    assert_eq!(reparsed.encrypt_infos().next().unwrap().id(), Some("1"));
}

#[test]
fn encrypt_list_alias_is_accepted_and_normalized() {
    let alias = ENCRYPTIONS
        .replace("<ofd:Encryptions", "<ofd:EncryptList")
        .replace("</ofd:Encryptions>", "</ofd:EncryptList>");
    let view = EncryptionsView::from_xml(alias.as_bytes(), ReadOptions::LENIENT).expect("alias");
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:Encryptions"));
    assert!(!xml.contains("EncryptList"));
}

#[test]
fn missing_provider_name_is_strict_error() {
    let missing = ENCRYPTIONS.replace(r#" Name="ExampleCrypto""#, "");
    assert!(parse_encryptions(&missing, ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        parse_encryptions(&missing, ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn decrypt_seed_parses_and_roundtrips() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view =
            DecryptSeedView::from_xml(DECRYPT_SEED.as_bytes(), options).expect("DecryptSeed");
        assert_eq!(view.id(), Some("1"));
        assert_eq!(view.encrypt_case_id(), Some("1.1.1"));
        let user = view.users().next().expect("UserInfo");
        assert_eq!(user.user_name(), Some("alice"));
        assert_eq!(user.user_type(), Some("User"));
        assert_eq!(user.user_cert(), Some("AAAA"));
        assert_eq!(user.encrypted_work_key(), Some("BBBB"));
        assert_eq!(user.iv_value(), Some("CCCC"));
        let parameter = view
            .extend_params()
            .expect("ExtendParams")
            .parameters()
            .next()
            .expect("Parameter");
        assert_eq!(parameter.name(), Some("p"));
        assert_eq!(parameter.value_attr(), Some("v"));
        assert_eq!(parameter.text(), "text");
        assert_eq!(parameter.value(), "text");
    }

    let view = DecryptSeedView::from_xml(DECRYPT_SEED.as_bytes(), ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:DecryptSeed"));
    assert!(xml.contains(r#"EncryptCaseId="1.1.1""#));
    assert!(xml.contains("<ofd:ExtendParams>"));
    let reparsed =
        DecryptSeedView::from_xml(xml.as_bytes(), ReadOptions::STRICT).expect("strict reparse");
    assert_eq!(reparsed.users().count(), 1);
}

#[test]
fn encrypted_entries_parse_and_roundtrip() {
    for options in [ReadOptions::LENIENT, ReadOptions::STRICT] {
        let view = EncryptedEntriesView::from_xml(ENTRIES_MAP.as_bytes(), options)
            .expect("EncryptedEntries");
        assert_eq!(view.id(), Some("1"));
        let entries: Vec<_> = view.entries().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path(), Some("/OFD.xml"));
        assert_eq!(entries[0].epath(), Some("/ofd.dat"));
        assert_eq!(entries[1].decrypt_seed_loc(), Some("/decryptseed.xml"));
    }

    let view =
        EncryptedEntriesView::from_xml(ENTRIES_MAP.as_bytes(), ReadOptions::LENIENT).unwrap();
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains(r#"Path="/Doc_0/Document.xml""#));
    // 0099 图 C.3 把它定义为 EncryptEntry 的属性。
    assert!(xml.contains(r#"DecryptSeedLoc="/decryptseed.xml""#));
    assert!(!xml.contains("<ofd:DecryptSeedLoc>"));
    let reparsed = EncryptedEntriesView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
        .expect("strict reparse");
    assert_eq!(reparsed.entries().count(), 2);
}

#[test]
fn package_parses_encryption_descriptions() {
    let bytes = fixtures::zip_with_entries(&[
        ("Encryptions.xml", ENCRYPTIONS.as_bytes()),
        ("decryptseed.xml", DECRYPT_SEED.as_bytes()),
        ("EntriesMap.xml", ENTRIES_MAP.as_bytes()),
    ]);
    let mut package = OfdPackage::from_bytes(bytes).expect("open package");

    let encryptions = package.parse_encryptions().expect("Encryptions");
    assert_eq!(encryptions.encrypt_infos().next().unwrap().id(), Some("1"));

    let seed = package
        .parse_decrypt_seed("/decryptseed.xml")
        .expect("DecryptSeed");
    assert_eq!(seed.users().next().unwrap().user_name(), Some("alice"));

    let map = package
        .parse_entries_map("/EntriesMap.xml")
        .expect("EntriesMap");
    assert_eq!(map.entries().count(), 2);
}

#[test]
fn encrypt_entries_alias_is_accepted_and_normalized() {
    let alias = ENTRIES_MAP
        .replace("<ofd:EncryptedEntries", "<ofd:EncryptEntries")
        .replace("</ofd:EncryptedEntries>", "</ofd:EncryptEntries>");
    let view =
        EncryptedEntriesView::from_xml(alias.as_bytes(), ReadOptions::STRICT).expect("alias");
    let xml = view.to_xml().expect("strict writer");
    assert!(xml.contains("<ofd:EncryptedEntries"));
    assert!(!xml.contains("EncryptEntries "));
}
