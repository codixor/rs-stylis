use rs_stylis::{Features, MinifyOptions, ParserOptions, PrinterOptions, StyleSheet, Targets};

fn main() {
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

    let css = stylesheet
        .to_css(PrinterOptions {
            minify: true,
            targets,
            ..PrinterOptions::default()
        })
        .unwrap();

    println!("{}", css.code);
}
