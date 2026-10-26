use ofdbox_core::model::encryption::Encryptions;
use ofdbox_core::model::entries_map::EncryptedEntries;
use ofdbox_core::{OfdError, ReadOptions};

const ENCRYPTIONS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Encryptions xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:EncryptInfo ID="1">
    <ofd:Provider Name="ExampleCrypto"/>
    <ofd:EncryptScope>All</ofd:EncryptScope>
    <ofd:DecryptSeedLoc>/decryptseed.xml</ofd:DecryptSeedLoc>
    <ofd:EntriesMapLoc>/EntriesMap.xml</ofd:EntriesMapLoc>
  </ofd:EncryptInfo>
</ofd:Encryptions>"#;

const ENTRIES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:EncryptedEntries xmlns:ofd="http://www.ofdspec.org/2016" ID="1">
  <ofd:EncryptEntry Path="/OFD.xml" EPath="/ofd.dat"/>
</ofd:EncryptedEntries>"#;

#[test]
fn duplicate_encrypt_info_id_is_strict_error() {
    let duplicated = ENCRYPTIONS.replace(
        "</ofd:Encryptions>",
        r#"<ofd:EncryptInfo ID="1"><ofd:Provider Name="Second"/><ofd:EncryptScope>All</ofd:EncryptScope><ofd:DecryptSeedLoc>/decryptseed.xml</ofd:DecryptSeedLoc><ofd:EntriesMapLoc>/EntriesMap.xml</ofd:EntriesMapLoc></ofd:EncryptInfo></ofd:Encryptions>"#,
    );
    assert!(Encryptions::from_xml(duplicated.as_bytes(), ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        Encryptions::from_xml(duplicated.as_bytes(), ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}

#[test]
fn missing_epath_is_strict_error() {
    let missing = ENTRIES.replace(r#" EPath="/ofd.dat""#, "");
    assert!(EncryptedEntries::from_xml(missing.as_bytes(), ReadOptions::LENIENT).is_ok());
    assert!(matches!(
        EncryptedEntries::from_xml(missing.as_bytes(), ReadOptions::STRICT),
        Err(OfdError::Conformance(_))
    ));
}
