use std::io::{Cursor, Write};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

#[allow(dead_code)]
pub fn zip_with_entry(name: &str, data: &[u8]) -> Vec<u8> {
    zip_with_entries(&[(name, data)])
}

#[allow(dead_code)]
pub fn zip_with_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .expect("start zip entry");
        writer.write_all(data).expect("write zip entry");
    }
    writer.finish().expect("finish zip").into_inner()
}
