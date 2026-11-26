use pretty_assertions::assert_eq;
use rs_stylis::stylesheet::PrinterOptions;
use rs_stylis::traits::ToCss;
use rs_stylis::values::percentage::Percentage;

#[test]
fn percentage_to_css_keeps_percent_scale() {
    let mut out = Percentage(1.0)
        .to_css_string(PrinterOptions::default())
        .expect("serialize 100%");
    assert_eq!(out, "100%");

    out = Percentage(-0.5)
        .to_css_string(PrinterOptions::default())
        .expect("serialize -50%");
    assert_eq!(out, "-50%");
}
