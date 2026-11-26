use cssparser::{ParseError, Parser};
use rs_stylis::error::{ParserError, PrinterError};
use rs_stylis::printer::Printer;
use rs_stylis::traits::{Parse, ToCss};
use stylis_derive::{Parse, ToCss};

#[derive(Debug, PartialEq, Parse)]
enum Value {
    Pair(i32, i32),
    Single(i32),
    End,
}

#[derive(ToCss)]
enum Word {
    One,
    Two,
}

#[derive(ToCss)]
enum Words {
    Middle(Word, Option<Word>, Word),
    Optional(Option<Word>, std::option::Option<Word>),
}

#[derive(Debug, PartialEq, Parse, ToCss)]
enum Named {
    Fields {
        input: i32,
        separator: Option<i32>,
        dest: i32,
    },
}

#[test]
fn named_fields_cannot_shadow_macro_locals() {
    let value = Named::parse_string("1 2 3").unwrap();
    assert_eq!(
        value,
        Named::Fields {
            input: 1,
            separator: Some(2),
            dest: 3
        }
    );
    assert_eq!(value.to_css_string(Default::default()).unwrap(), "1 2 3");
}

#[test]
fn partial_variants_restore_the_input_cursor() {
    assert_eq!(Value::parse_string("3").unwrap(), Value::Single(3));
    assert_eq!(Value::parse_string("3 4").unwrap(), Value::Pair(3, 4));
    assert_eq!(Value::parse_string("end").unwrap(), Value::End);
    assert!(Value::parse_string("3 end").is_err());
}

#[test]
fn present_fields_always_have_one_separator() {
    for (value, expected) in [
        (Words::Middle(Word::One, None, Word::Two), "one two"),
        (Words::Optional(Some(Word::One), Some(Word::Two)), "one two"),
        (Words::Optional(None, Some(Word::Two)), "two"),
        (Words::Optional(Some(Word::One), None), "one"),
        (Words::Optional(None, None), ""),
    ] {
        assert_eq!(value.to_css_string(Default::default()).unwrap(), expected);
    }
}
