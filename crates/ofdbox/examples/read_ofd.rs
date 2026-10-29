//! 读取 OFD 的文档、页面、图元与资源。
//!
//! `cargo run -p ofdbox --example read_ofd -- target/created.ofd`

use ofdbox::core::OfdFile;
use ofdbox::model::page::GraphicObject;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/created.ofd".to_owned());
    let file = OfdFile::open(&path)?;
    let ofd = file.ofd()?;

    for (document_index, _) in ofd.data.doc_bodies.iter().enumerate() {
        let document = ofd.document(document_index)?;
        println!(
            "document {}: {} pages",
            document_index,
            document.data.pages.pages.len()
        );

        for page_entry in document.pages() {
            let page = page_entry.load()?;
            println!("  page {:?}", page_entry.id());
            let Some(content) = &page.data.content else {
                continue;
            };
            for layer in &content.layers {
                for object in &layer.objects {
                    match object {
                        GraphicObject::Text(text) => {
                            println!(
                                "    Text id={} boundary={}",
                                text.id, text.content.graphic.boundary
                            );
                            for code in &text.content.text_codes {
                                println!("      text={:?}", code.text);
                            }
                        }
                        GraphicObject::Path(path) => {
                            println!(
                                "    Path id={} boundary={}",
                                path.id, path.content.graphic.boundary
                            );
                        }
                        GraphicObject::Image(image) => {
                            println!(
                                "    Image id={} resource={}",
                                image.id, image.content.resource_id
                            );
                        }
                        GraphicObject::Composite(composite) => {
                            println!(
                                "    Composite id={} resource={}",
                                composite.id, composite.content.resource_id
                            );
                        }
                        GraphicObject::PageBlock(block) => {
                            println!("    PageBlock id={}", block.id);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
