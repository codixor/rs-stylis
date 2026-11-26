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
