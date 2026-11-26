#![allow(clippy::field_reassign_with_default)]

use pretty_assertions::assert_eq;
use rs_stylis::{CssModulesConfig, ParserOptions, PrinterOptions, StyleSheet};

#[test]
fn invalid_css_error_matches_golden_snapshot() {
    let error = StyleSheet::parse(
        ".foo[ { color: red; }",
        ParserOptions {
            filename: "invalid.css".into(),
            ..ParserOptions::default()
        },
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        include_str!("golden/invalid_parse_error.txt").trim()
    );
}

#[test]
fn deprecated_css_modules_value_rule_matches_golden_snapshot() {
    let error = StyleSheet::parse(
        "@value primary: red;",
        ParserOptions {
            filename: "tokens.module.css".into(),
            css_modules: Some(CssModulesConfig::default()),
            ..ParserOptions::default()
        },
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        include_str!("golden/deprecated_css_modules_value_error.txt").trim()
    );
}

#[test]
fn invalid_css_modules_grid_pattern_matches_error_surface() {
    let mut config = CssModulesConfig::default();
    config.grid = true;
    config.pattern = rs_stylis::css_modules::Pattern::parse("[hash]").unwrap();

    let stylesheet = StyleSheet::parse(
        ".grid { grid-template-columns: [header] 1fr [content] 2fr; }",
        ParserOptions {
            filename: "grid.module.css".into(),
            css_modules: Some(config),
            ..ParserOptions::default()
        },
    )
    .unwrap();

    let error = stylesheet.to_css(PrinterOptions::default()).unwrap_err();

    assert_eq!(
        error.to_string(),
        "The CSS modules `pattern` config must end with `[local]` for use in CSS grid line names. at unknown.css:0:1"
    );
}
