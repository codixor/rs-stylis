use rs_stylis::{CssModulesConfig, ParserOptions, PrinterOptions, StyleSheet};

fn main() {
    let options = ParserOptions {
        filename: "/workspace/components/button.module.css".into(),
        css_modules: Some(CssModulesConfig::default()),
        ..ParserOptions::default()
    };

    let stylesheet =
        StyleSheet::parse(".button{color:red}.buttonPrimary{composes:button}", options).unwrap();
    let css = stylesheet.to_css(PrinterOptions::default()).unwrap();
    let exports = css.exports.expect("css modules exports");

    println!("{}", css.code);
    for (name, export) in exports {
        println!("{name} => {}", export.name);
    }
}
