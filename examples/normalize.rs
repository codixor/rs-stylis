use rs_stylis::{NormalizationProfile, ParserOptions, PrinterOptions, StyleSheet};

fn main() {
    let stylesheet = StyleSheet::parse(
        ".button{color:rgb(255 0 0 / 100%);margin:0 0 0 0}",
        ParserOptions::default(),
    )
    .unwrap();

    let css = stylesheet
        .to_css(PrinterOptions {
            normalization_profile: NormalizationProfile::Standards,
            ..PrinterOptions::default()
        })
        .unwrap();

    println!("{}", css.code);
}
