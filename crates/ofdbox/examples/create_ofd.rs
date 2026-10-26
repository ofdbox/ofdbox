//! 从零创建 OFD。
//!
//! `cargo run -p ofdbox --example create_ofd`

use ofdbox::{OfdWriter, PageSize};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = OfdWriter::new()
        .doc_id("demo")
        .title("OFDBox 示例")
        .creator("OFDBox");

    let font = writer.add_font("SimSun");
    let page = writer.add_page(PageSize::a4());
    writer.add_text(
        page,
        "10 10 100 20",
        &font,
        12.0,
        &[(10.0, 20.0, "Hello OFD")],
    )?;

    std::fs::create_dir_all("target")?;
    writer.save("target/created.ofd")?;
    println!("created target/created.ofd");
    Ok(())
}
