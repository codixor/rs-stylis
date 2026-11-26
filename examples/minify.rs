use rs_stylis::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sheet = StyleSheet::parse(
        ".foo { color: red; } .bar { color: red; }",
        ParserOptions::default(),
    )?;
    sheet.minify(MinifyOptions::default())?;
    let css = sheet.to_css(PrinterOptions {
        minify: true,
        ..PrinterOptions::default()
    })?;

    println!("{}", css.code);
    Ok(())
}
