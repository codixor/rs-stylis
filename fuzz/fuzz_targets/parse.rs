#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &str| {
    if let Ok(sheet) = rs_stylis::StyleSheet::parse(input, rs_stylis::ParserOptions::default()) {
        let _ = sheet.to_css(rs_stylis::PrinterOptions::default());
    }
});
