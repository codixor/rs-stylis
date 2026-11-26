use rs_stylis::{CssModulesConfig, ParserOptions, PrinterOptions, StyleSheet};

#[test]
fn css_modules_exports_are_available_from_root_surface() {
    let options = ParserOptions {
        filename: "/workspace/components/button.module.css".into(),
        css_modules: Some(CssModulesConfig::default()),
        ..ParserOptions::default()
    };

    let stylesheet =
        StyleSheet::parse(".button{color:red}.buttonPrimary{composes:button}", options).unwrap();
    let result = stylesheet
        .to_css(PrinterOptions {
            project_root: Some("/workspace"),
            ..PrinterOptions::default()
        })
        .unwrap();

    let exports = result.exports.expect("css modules exports");
    let button = exports.get("button").expect("button export");
    let primary = exports.get("buttonPrimary").expect("buttonPrimary export");

    assert!(button.name.ends_with("_button"));
    assert!(primary.name.ends_with("_buttonPrimary"));
    assert!(result.code.contains(&button.name));
    assert!(result.code.contains(&primary.name));
    assert_eq!(primary.composes.len(), 1);
}
