use rs_stylis::{ParserOptions, PrinterOptions, StyleSheet};

fn main() {
    let stylesheet = StyleSheet::parse(".button{color:red}", ParserOptions::default()).unwrap();
    let css = stylesheet.to_css(PrinterOptions::default()).unwrap();
    println!("{}", css.code);
}
