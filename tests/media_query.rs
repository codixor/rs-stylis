use cssparser::{Parser, ParserInput};
use pretty_assertions::assert_eq;
use rs_stylis::media_query::MediaList;
use rs_stylis::printer::{Printer, PrinterOptions};
use rs_stylis::stylesheet::ParserOptions;
use rs_stylis::traits::ToCss;

#[test]
fn parses_and_prints_media_queries() {
    let input = "screen and (min-width: 600px), print";
    let mut parser_input = ParserInput::new(input);
    let mut parser = Parser::new(&mut parser_input);
    let media = MediaList::parse(&mut parser, &ParserOptions::default()).unwrap();

    assert_eq!(media.media_queries.len(), 2);
    assert!(!media.always_matches());
    assert!(!media.never_matches());

    let mut output = String::new();
    let mut printer = Printer::new(&mut output, PrinterOptions::default());
    media.to_css(&mut printer).unwrap();
    assert_eq!(output, "screen and (width >= 600px), print");
}
