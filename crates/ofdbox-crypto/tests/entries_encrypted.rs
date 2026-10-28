use std::io::{Cursor, Write};

use ofdbox_core::{EntriesWriteOptions, OfdError, OfdPackage, SignedValueProducer};
use ofdbox_crypto::{
    EncryptOptions, PasswordBackend, encrypt_package, write_ofd_entries_encrypted,
};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>entries-encrypted</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>1</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;

const PAGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Area><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:Area>
  <ofd:Content/>
</ofd:Page>"#;

struct FakeProducer;

impl SignedValueProducer for FakeProducer {
    fn supports(&self, _signature_method: &str) -> bool {
        true
    }

    fn digest(&self, data: &[u8]) -> Result<Vec<u8>, OfdError> {
        Ok(data.to_vec())
    }

    fn signature_type(&self) -> &str {
        "Sign"
    }

    fn produce(&self, _context: &ofdbox_core::SigningContext<'_>) -> Result<Vec<u8>, OfdError> {
        Ok(b"fake-signature".to_vec())
    }
}

fn package() -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in [
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", PAGE.as_bytes()),
        ("Doc_0/extra.txt", b"extra"),
    ] {
        writer
            .start_file(name, SimpleFileOptions::default())
            .expect("start");
        writer.write_all(data).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

#[test]
fn generates_entries_for_encrypted_package() {
    let encrypted = encrypt_package(&package(), &EncryptOptions::password("alice", "old-secret"))
        .expect("encrypt");
    let mut source = OfdPackage::from_bytes(encrypted).expect("open encrypted");
    source
        .enable_decryption(Box::new(PasswordBackend::new("old-secret")))
        .expect("decrypt session");

    let mut output = Vec::new();
    let report = write_ofd_entries_encrypted(
        &mut source,
        Cursor::new(&mut output),
        &FakeProducer,
        &EntriesWriteOptions::default(),
        &EncryptOptions::password("alice", "new-secret"),
    )
    .expect("entries + reencrypt");
    assert_eq!(report.files, 1);

    let mut result = OfdPackage::from_bytes(output).expect("open result");
    assert!(matches!(
        result.parse_root(),
        Err(OfdError::EncryptionRequired)
    ));
    result
        .enable_decryption(Box::new(PasswordBackend::new("new-secret")))
        .expect("decrypt result");
    let entries = result.parse_ofd_entries().expect("entries");
    assert_eq!(entries.file_list.files.len(), 1);
}
