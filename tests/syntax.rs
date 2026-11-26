use rs_stylis::values::syntax::{ParsedComponent, SyntaxString};

#[test]
fn parses_syntax_string_and_value() {
    let syntax = SyntaxString::parse_string("<length> | <color>").unwrap();
    let value = syntax.parse_value_from_string("12px").unwrap();
    assert!(matches!(value, ParsedComponent::Length(_)));
}

#[test]
fn parses_universal_syntax_as_token_list() {
    let syntax = SyntaxString::parse_string("*").unwrap();
    let value = syntax.parse_value_from_string("var(--brand)").unwrap();
    assert!(matches!(value, ParsedComponent::TokenList(_)));
}
