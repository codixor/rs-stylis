#![allow(clippy::needless_raw_string_hashes)]

use pretty_assertions::assert_eq;
use rs_stylis::printer::PrinterOptions;
use rs_stylis::stylesheet::{MinifyOptions, NormalizationProfile, ParserOptions, StyleSheet};

fn render(code: &str, minify_stylesheet: bool, minify_output: bool) -> String {
    render_with_profiles(
        code,
        minify_stylesheet,
        minify_output,
        false,
        NormalizationProfile::Standards,
    )
}

fn render_with_profiles(
    code: &str,
    minify_stylesheet: bool,
    minify_output: bool,
    react_compat: bool,
    normalization_profile: NormalizationProfile,
) -> String {
    let mut stylesheet = StyleSheet::parse(code, ParserOptions::default()).unwrap();
    if minify_stylesheet {
        stylesheet
            .minify(MinifyOptions {
                react_compat,
                ..MinifyOptions::default()
            })
            .unwrap();
    }

    stylesheet
        .to_css(PrinterOptions {
            minify: minify_output,
            normalization_profile,
            ..PrinterOptions::default()
        })
        .unwrap()
        .code
}

#[test]
fn nonadjacent_rules_preserve_declaration_importance() {
    let input = ".a{color:red!important}.b{width:1px}.a{color:blue}";
    let output = render(input, true, true);
    assert!(output.contains("color:red!important"), "{output}");

    let input = ".a{color:red!important}.b{width:1px}.a{color:blue!important}";
    let output = render(input, true, true);
    assert!(!output.contains("color:red"), "{output}");
    assert!(output.contains("#00f!important"), "{output}");
}

#[test]
fn unparsed_value_keeps_earlier_nonadjacent_fallback() {
    let output = render(
        ".a{color:red}.b{width:1px}.a{color:future-keyword}",
        true,
        true,
    );
    assert!(output.contains("color:red"), "{output}");
    assert!(output.contains("color:future-keyword"), "{output}");
}

#[test]
fn property_location_uses_the_rules_column() {
    use rs_stylis::rules::CssRule;

    let input = ".a{color:red}.b{width:1px}";
    let sheet = StyleSheet::parse(input, ParserOptions::default()).unwrap();
    let CssRule::Style(rule) = &sheet.rules.0[1] else {
        panic!("expected style rule")
    };
    let (name, value) = rule.property_location(input, 0).unwrap();
    assert_eq!(
        &input[name.start.column as usize - 1..name.end.column as usize - 1],
        "width"
    );
    assert_eq!(
        &input[value.start.column as usize - 1..value.end.column as usize - 1],
        "1px"
    );
}

#[test]
fn value_parser_rejects_trailing_tokens_with_default_options() {
    use rs_stylis::{
        properties::position::ZIndex,
        traits::{Parse, ParseWithOptions},
    };

    assert!(ZIndex::parse_string_with_options("1 2", Default::default()).is_err());
    assert_eq!(
        ZIndex::parse_string_with_options("1 ", Default::default()).unwrap(),
        ZIndex::parse_string("1").unwrap()
    );
}

#[test]
fn pretty_prints_basic_rule_from_golden() {
    let input = ".foo{color:red}";
    let output = render(input, false, false);
    assert_eq!(output, include_str!("golden/basic_pretty.css"));
}

#[test]
fn minifies_and_merges_adjacent_rules_from_goldens() {
    let input = r#"
      .foo {
        color: red;
      }

      .bar {
        color: red;
      }
    "#;

    let pretty_output = render(input, true, false);
    assert_eq!(pretty_output, include_str!("golden/merged_pretty.css"));

    let minified_output = render(input, true, true).trim().replace('\n', "");
    assert_eq!(
        minified_output,
        include_str!("golden/merged_minified.css").trim()
    );
}

#[test]
fn pretty_prints_selector_heavy_rule_from_golden() {
    let input = r#"
      .card:is(.primary, .secondary) > .title,
      button[data-kind="cta"]:hover::before {
        color: red;
        content: "go";
      }
    "#;

    let output = render(input, false, false);
    assert_eq!(output, include_str!("golden/selector_heavy_pretty.css"));
}

#[test]
fn round_trips_custom_properties_from_goldens() {
    let input = r#"
      :root {
        --brand: #0af;
        --space: 1rem;
      }

      .button {
        color: var(--brand);
        padding: var(--space);
      }
    "#;

    let pretty_output = render(input, false, false);
    assert_eq!(
        pretty_output,
        include_str!("golden/custom_properties_pretty.css")
    );

    let minified_output = render(input, true, true).trim().replace('\n', "");
    assert_eq!(
        minified_output,
        include_str!("golden/custom_properties_minified.css").trim()
    );
}

#[test]
fn pretty_prints_nested_at_rules_from_goldens() {
    let input = r#"
      @media screen and (min-width: 600px) {
        @supports (display: grid) {
          @container card (width > 30rem) {
            .card {
              display: grid;
              gap: 1rem;
            }
          }
        }
      }
    "#;

    let pretty_output = render(input, false, false);
    assert_eq!(pretty_output, include_str!("golden/at_rules_pretty.css"));

    let minified_output = render(input, true, true).trim().replace('\n', "");
    assert_eq!(
        minified_output,
        include_str!("golden/at_rules_minified.css").trim()
    );
}

#[test]
fn react_compat_preserves_trailing_semicolon_in_minified_output() {
    let input = ".button{color:red}";

    let standards = render_with_profiles(input, true, true, false, NormalizationProfile::Standards);
    let react = render_with_profiles(input, true, true, true, NormalizationProfile::ReactCompat);

    assert_eq!(standards.trim(), ".button{color:red}");
    assert_eq!(react.trim(), ".button{color:red;}");
}

#[test]
fn react_compat_preserves_authored_transform_shape() {
    let input = ".button{transform:translate(1px,0)}";

    let standards = render_with_profiles(input, true, true, false, NormalizationProfile::Standards);
    let react = render_with_profiles(input, true, true, true, NormalizationProfile::ReactCompat);

    assert_eq!(standards.trim(), ".button{transform:translate(1px)}");
    assert_eq!(react.trim(), ".button{transform:translate(1px,0);}");
}
