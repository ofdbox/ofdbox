//! 手动创建 OFD 结构。
//!
//! 这个示例不使用高层 writer，而是直接操作 [`OfdFile`]、模型和 `Stored<T>`：
//!
//! - 创建文档
//! - 创建页面
//! - 创建页面资源并登记非内嵌系统字体
//! - 创建图层
//! - 添加文本对象
//!
//! 这里只演示结构。复杂版式、字体度量、换行和渲染不在本库职责内。
//!
//! `cargo run -p ofdbox --example create_ofd`

use ofdbox::model;
use ofdbox::model::definition::{ColorArray, DeltaArray, StBox, StId, StLoc, StRefId};
use ofdbox::model::page::{
    Color, GraphicObject, GraphicUnit, Layer, LayerType, Page, PageContent, TextCode, TextContent,
    TextObject,
};
use ofdbox::model::res::{Charset, Font, Fonts, Res};
use ofdbox::{ObjectContext, OfdFile, Stored};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = OfdFile::create()?;

    // 1. 文档元数据。
    let mut ofd = file.ofd()?;
    let body = ofd.data.doc_bodies.first_mut().ok_or("OFD 缺少 DocBody")?;
    body.doc_info.doc_id = "ofdbox-structure-demo".to_owned();
    body.doc_info.title = Some("OFD 结构示例".to_owned());
    body.doc_info.creator = Some("OFDBox".to_owned());
    body.doc_info.creation_date = Some("2026-10-28".to_owned());
    body.doc_info.mod_date = Some("2026-10-28".to_owned());
    ofd.save()?;

    // 2. 文档级页面区域。
    let mut document = file.document(0)?;
    let page_area = model::definition::PageArea {
        physical_box: StBox::new(0.0, 0.0, 210.0, 297.0),
        ..Default::default()
    };
    document.data.common_data.page_area = page_area.clone();
    document.data.common_data.max_unit_id = StId::new(100);
    document.save()?;

    // 3. 手动构造页面和图层。
    let body_layer = Layer {
        id: StId::new(20),
        layer_type: LayerType::Body,
        draw_param: None,
        objects: vec![
            text_object(TextSpec {
                id: 21,
                boundary: StBox::new(20.0, 36.0, 170.0, 26.0),
                font: 1,
                size: 16.0,
                text: "OFD 结构示例",
            }),
            text_object(TextSpec {
                id: 22,
                boundary: StBox::new(20.0, 76.0, 170.0, 18.0),
                font: 1,
                size: 9.0,
                text: "这个文件由 OfdFile + 模型",
            }),
            text_object(TextSpec {
                id: 23,
                boundary: StBox::new(20.0, 98.0, 170.0, 18.0),
                font: 1,
                size: 9.0,
                text: "手动创建。",
            }),
            text_object(TextSpec {
                id: 24,
                boundary: StBox::new(20.0, 120.0, 170.0, 18.0),
                font: 1,
                size: 9.0,
                text: "字体是非内嵌系统字体：SimSun。",
            }),
            text_object(TextSpec {
                id: 25,
                boundary: StBox::new(20.0, 270.0, 170.0, 14.0),
                font: 1,
                size: 7.0,
                text: "OFDBox / structure only",
            }),
        ],
    };

    let page = Page {
        templates: Vec::new(),
        page_res: vec![StLoc::new("Res.xml")],
        area: Some(page_area),
        content: Some(PageContent {
            layers: vec![body_layer],
        }),
        actions: None,
    };

    // add_page 会写入 Content.xml，并更新 Document.Pages。
    let stored_page = document.add_page(page)?;

    // 4. 页面资源：非内嵌系统字体。
    //
    // 不写 FontFile，表示字体不嵌入，由阅读器按 FontName/FamilyName 使用系统字体。
    let resource = Res {
        base_loc: StLoc::new("."),
        color_spaces: None,
        draw_params: None,
        fonts: Some(Fonts {
            fonts: vec![Font {
                id: StId::new(1),
                font_name: "SimSun".to_owned(),
                family_name: Some("SimSun".to_owned()),
                charset: Charset::Unicode,
                ..Default::default()
            }],
        }),
        multi_medias: None,
        composite_graphic_units: None,
    };
    let resource_location = stored_page.context().resolve(&StLoc::new("Res.xml"))?;
    let resource_context = ObjectContext::child(stored_page.context(), resource_location.clone())?;
    Stored::new(resource, resource_context, resource_location).save()?;

    // 再写一次页面，确保 PageRes 与页面内容都落盘。
    stored_page.save()?;
    document.save()?;

    // 5. 输出。
    std::fs::create_dir_all("target")?;
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/created.ofd".to_owned());
    file.save(&output)?;
    println!("created {output}");
    Ok(())
}

struct TextSpec<'a> {
    id: u32,
    boundary: StBox,
    font: u32,
    size: f64,
    text: &'a str,
}

fn text_object(spec: TextSpec<'_>) -> GraphicObject {
    GraphicObject::Text(TextObject {
        id: StId::new(spec.id),
        content: TextContent {
            graphic: GraphicUnit {
                boundary: spec.boundary,
                ..Default::default()
            },
            font: StRefId::new(spec.font),
            size: spec.size,
            fill_color: Some(Color {
                value: Some(ColorArray::new(vec![32, 36, 44])),
                ..Default::default()
            }),
            text_codes: vec![TextCode {
                x: Some(0.0),
                y: Some(spec.size),
                delta_x: Some(DeltaArray::new(vec![
                    spec.size;
                    spec.text.chars().count().max(1)
                ])),
                text: spec.text.to_owned(),
                ..Default::default()
            }],
            ..Default::default()
        },
    })
}
