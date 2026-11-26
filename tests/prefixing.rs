use pretty_assertions::assert_eq;
use rs_stylis::{Features, MinifyOptions, ParserOptions, PrinterOptions, StyleSheet, Targets};

#[test]
fn emits_prefix_lowering_output_from_golden() {
    let targets = Targets {
        include: Features::VendorPrefixes,
        ..Targets::default()
    };

    let mut stylesheet = StyleSheet::parse(
        ".field{appearance:none;user-select:none;backdrop-filter:blur(4px)}",
        ParserOptions::default(),
    )
    .unwrap();

    stylesheet
        .minify(MinifyOptions {
            targets,
            ..MinifyOptions::default()
        })
        .unwrap();

    let output = stylesheet
        .to_css(PrinterOptions {
            minify: true,
            targets,
            ..PrinterOptions::default()
        })
        .unwrap()
        .code;

    assert_eq!(
        output.trim(),
        include_str!("golden/prefix_lowering_minified.css").trim()
    );
}
