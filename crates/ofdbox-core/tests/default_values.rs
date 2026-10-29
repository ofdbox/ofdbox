use ofdbox_core::model::document::{OutlineElem, Permissions, VPreferences};
use ofdbox_core::model::ofd::DocInfo;
use ofdbox_core::model::page::TextObject;
use ofdbox_core::model_xml;

#[test]
fn doc_usage_defaults_to_normal() {
    let info: DocInfo =
        model_xml::from_str(r#"<DocInfo><DocID>id</DocID></DocInfo>"#).expect("parse DocInfo");
    assert_eq!(info.doc_usage, "Normal");
}

#[test]
fn permission_flags_default_to_true() {
    let permissions: Permissions =
        model_xml::from_str(r#"<Permissions/>"#).expect("parse Permissions");
    assert!(permissions.edit);
    assert!(permissions.annot);
    assert!(permissions.export);
    assert!(permissions.signature);
    assert!(permissions.watermark);
    assert!(permissions.print_screen);
}

#[test]
fn view_preferences_use_standard_defaults() {
    let preferences: VPreferences =
        model_xml::from_str(r#"<VPreferences/>"#).expect("parse VPreferences");
    assert_eq!(preferences.page_mode, Default::default());
    assert_eq!(preferences.page_layout, Default::default());
    assert_eq!(preferences.tab_display, Default::default());
    assert!(!preferences.hide_toolbar);
    assert!(!preferences.hide_menubar);
    assert!(!preferences.hide_window_ui);
    assert_eq!(preferences.zoom_mode, Default::default());
}

#[test]
fn outline_and_graphic_defaults() {
    let outline: OutlineElem =
        model_xml::from_str(r#"<OutlineElem Title="t"/>"#).expect("parse OutlineElem");
    assert_eq!(outline.count, 0);
    assert!(outline.expanded);

    let text: TextObject =
        model_xml::from_str(r#"<TextObject ID="1" Boundary="0 0 10 10" Font="1" Size="10"/>"#)
            .expect("parse TextObject");
    assert_eq!(text.common.alpha, 0);
    assert!(text.common.name.is_empty());
}
