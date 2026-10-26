//! 读取 OFD 的文档、页面、图元与资源。
//!
//! `cargo run -p ofdbox --example read_ofd -- target/created.ofd`

use ofdbox::core::OfdPackage;
use ofdbox::model::page::GraphicObjectKind;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/created.ofd".to_owned());
    let mut package = OfdPackage::open(&path)?;

    for document_entry in package.documents()? {
        let index = document_entry.index;
        let document = package.parse_document_at(index)?;
        let pages = document.pages().ok_or("document has no pages")?;
        println!("document {}: {} pages", index, pages.pages().count());

        for page_entry in pages.pages() {
            let page = package.parse_page_for_document(index, &page_entry)?;
            println!("  page {:?}", page_entry.id());
            if let Some(content) = page.content() {
                for layer in content.layers() {
                    for object in layer.objects() {
                        println!(
                            "    {:?} id={:?} boundary={:?}",
                            object.kind(),
                            object.id(),
                            object.boundary()
                        );
                        if object.kind() == GraphicObjectKind::Text {
                            for code in object.text_codes() {
                                println!("      text={:?}", code.text());
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
