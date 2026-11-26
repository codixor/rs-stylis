#![allow(
    clippy::format_push_string,
    clippy::needless_raw_string_hashes,
    clippy::semicolon_if_nothing_returned
)]

use std::env;
use std::hint::black_box;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use rs_stylis::{
    Features, MinifyOptions, ParserFlags, ParserOptions, PrinterOptions, StyleSheet, Targets,
};

const SMALL_CSS: &str = ".foo{color:red}.bar{margin:0;padding:0}";
const MERGE_CSS: &str = ".foo { color: red; } .bar { color: red; }";
const SELECTOR_ROW: &str = r#"
.card:is(.primary, .secondary) > .title,
.layout main > .card[data-kind="feature"] .copy a:hover::before,
button[data-kind="cta"]:hover::before {
  color: red;
  content: "go";
}
"#;
const CUSTOM_ROW: &str = r#"
:root {
  --brand: #0af;
  --space: 1rem;
  --radius: 12px;
}

.button {
  color: var(--brand);
  padding: var(--space);
  border-radius: var(--radius);
}
"#;
const AT_RULE_ROW: &str = r#"
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
const NESTING_ROW: &str = r#"
.card {
  color: red;

  & .title {
    color: blue;

    &:hover {
      color: green;
    }
  }
}
"#;
const PREFIX_ROW: &str = r#"
.field {
  appearance: none;
  user-select: none;
  backdrop-filter: blur(4px);
}
"#;

fn selector_heavy_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| SELECTOR_ROW.repeat(120)).as_str()
}

fn custom_properties_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| CUSTOM_ROW.repeat(160)).as_str()
}

fn nested_at_rules_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| AT_RULE_ROW.repeat(90)).as_str()
}

fn nesting_heavy_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| NESTING_ROW.repeat(160)).as_str()
}

fn prefix_lowering_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| PREFIX_ROW.repeat(180)).as_str()
}

fn prefix_targets() -> Targets {
    Targets {
        include: Features::VendorPrefixes,
        ..Targets::default()
    }
}

fn large_app_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let mut css = String::from(
            r#"
:root {
  --surface: #fff;
  --surface-muted: #f7f8fb;
  --text: #18212f;
  --accent: #0af;
  --radius: 14px;
  --space-2: 0.5rem;
  --space-4: 1rem;
  --space-6: 1.5rem;
}

*,
*::before,
*::after {
  box-sizing: border-box;
}

body {
  margin: 0;
  color: var(--text);
  background: var(--surface-muted);
  font-family: system-ui, sans-serif;
}

.layout {
  display: grid;
  grid-template-columns: 18rem minmax(0, 1fr);
  min-height: 100vh;
}

.sidebar {
  position: sticky;
  top: 0;
  padding: var(--space-6);
}

.content {
  padding: var(--space-6);
}
"#,
        );

        for idx in 0..120 {
            let accent = if idx % 3 == 0 { "#0af" } else { "#f43f5e" };
            css.push_str(&format!(
                r#"
.card-{idx} {{
  display: grid;
  gap: var(--space-4);
  padding: var(--space-6);
  background: var(--surface);
  border: 1px solid color-mix(in srgb, {accent} 14%, white);
  border-radius: var(--radius);
}}

.card-{idx}:is(.primary, .featured) > .title,
.card-{idx} .copy a:hover::before {{
  color: {accent};
  content: "go";
}}

@media screen and (min-width: 720px) {{
  @supports (display: grid) {{
    @container card-{idx} (width > 30rem) {{
      .card-{idx} {{
        grid-template-columns: 1.2fr .8fr;
      }}
    }}
  }}
}}
"#
            ));
        }

        css
    })
    .as_str()
}

#[derive(Clone, Copy)]
enum Workload {
    ParseSmall,
    NormalizeSmall,
    MinifySmall,
    NormalizeLargeApp,
    ParseLargeApp,
    MinifyLargeApp,
    ParseSelectorHeavy,
    MinifySelectorHeavy,
    PrintSelectorHeavy,
    MinifyCustomProperties,
    MinifyNestedAtRules,
    MinifyNestingHeavy,
    MinifyPrefixLowering,
}

impl Workload {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "parse_small" => Some(Self::ParseSmall),
            "normalize_small" => Some(Self::NormalizeSmall),
            "minify_small" => Some(Self::MinifySmall),
            "normalize_large_app" => Some(Self::NormalizeLargeApp),
            "parse_large_app" => Some(Self::ParseLargeApp),
            "minify_large_app" => Some(Self::MinifyLargeApp),
            "parse_selector_heavy" => Some(Self::ParseSelectorHeavy),
            "minify_selector_heavy" => Some(Self::MinifySelectorHeavy),
            "print_selector_heavy" => Some(Self::PrintSelectorHeavy),
            "minify_custom_properties" => Some(Self::MinifyCustomProperties),
            "minify_nested_at_rules" => Some(Self::MinifyNestedAtRules),
            "minify_nesting_heavy" => Some(Self::MinifyNestingHeavy),
            "minify_prefix_lowering" => Some(Self::MinifyPrefixLowering),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::ParseSmall => "parse_small",
            Self::NormalizeSmall => "normalize_small",
            Self::MinifySmall => "minify_small",
            Self::NormalizeLargeApp => "normalize_large_app",
            Self::ParseLargeApp => "parse_large_app",
            Self::MinifyLargeApp => "minify_large_app",
            Self::ParseSelectorHeavy => "parse_selector_heavy",
            Self::MinifySelectorHeavy => "minify_selector_heavy",
            Self::PrintSelectorHeavy => "print_selector_heavy",
            Self::MinifyCustomProperties => "minify_custom_properties",
            Self::MinifyNestedAtRules => "minify_nested_at_rules",
            Self::MinifyNestingHeavy => "minify_nesting_heavy",
            Self::MinifyPrefixLowering => "minify_prefix_lowering",
        }
    }
}

fn render_normalized(code: &str) {
    let stylesheet = StyleSheet::parse(code, ParserOptions::default()).unwrap();
    let _ = stylesheet.to_css(PrinterOptions::default()).unwrap();
}

fn render_minified(code: &str) {
    let mut stylesheet = StyleSheet::parse(code, ParserOptions::default()).unwrap();
    stylesheet.minify(MinifyOptions::default()).unwrap();
    let _ = stylesheet
        .to_css(PrinterOptions {
            minify: true,
            ..PrinterOptions::default()
        })
        .unwrap();
}

fn render_minified_with_parser_options<'a>(code: &'a str, options: ParserOptions<'a, 'a>) {
    let mut stylesheet = StyleSheet::parse(code, options).unwrap();
    stylesheet.minify(MinifyOptions::default()).unwrap();
    let _ = stylesheet
        .to_css(PrinterOptions {
            minify: true,
            ..PrinterOptions::default()
        })
        .unwrap();
}

fn render_minified_with_targets(code: &str, targets: Targets) {
    let mut stylesheet = StyleSheet::parse(code, ParserOptions::default()).unwrap();
    stylesheet
        .minify(MinifyOptions {
            targets,
            ..MinifyOptions::default()
        })
        .unwrap();
    let _ = stylesheet
        .to_css(PrinterOptions {
            minify: true,
            targets,
            ..PrinterOptions::default()
        })
        .unwrap();
}

fn render_preminified(stylesheet: &StyleSheet<'static, 'static>) {
    let _ = stylesheet
        .to_css(PrinterOptions {
            minify: true,
            ..PrinterOptions::default()
        })
        .unwrap();
}

fn minified_selector_heavy_stylesheet() -> &'static StyleSheet<'static, 'static> {
    static STYLESHEET: OnceLock<StyleSheet<'static, 'static>> = OnceLock::new();
    STYLESHEET.get_or_init(|| {
        let mut stylesheet =
            StyleSheet::parse(selector_heavy_css(), ParserOptions::default()).unwrap();
        stylesheet.minify(MinifyOptions::default()).unwrap();
        stylesheet
    })
}

fn run_once(workload: Workload) {
    match workload {
        Workload::ParseSmall => {
            let _ = black_box(
                StyleSheet::parse(black_box(SMALL_CSS), ParserOptions::default()).unwrap(),
            );
        }
        Workload::NormalizeSmall => render_normalized(black_box(SMALL_CSS)),
        Workload::MinifySmall => render_minified(black_box(MERGE_CSS)),
        Workload::NormalizeLargeApp => render_normalized(black_box(large_app_css())),
        Workload::ParseLargeApp => {
            let _ = black_box(
                StyleSheet::parse(black_box(large_app_css()), ParserOptions::default()).unwrap(),
            );
        }
        Workload::MinifyLargeApp => render_minified(black_box(large_app_css())),
        Workload::ParseSelectorHeavy => {
            let _ = black_box(
                StyleSheet::parse(black_box(selector_heavy_css()), ParserOptions::default())
                    .unwrap(),
            );
        }
        Workload::MinifySelectorHeavy => render_minified(black_box(selector_heavy_css())),
        Workload::PrintSelectorHeavy => {
            render_preminified(black_box(minified_selector_heavy_stylesheet()))
        }
        Workload::MinifyCustomProperties => render_minified(black_box(custom_properties_css())),
        Workload::MinifyNestedAtRules => render_minified(black_box(nested_at_rules_css())),
        Workload::MinifyNestingHeavy => render_minified_with_parser_options(
            black_box(nesting_heavy_css()),
            ParserOptions {
                flags: ParserFlags::NESTING,
                ..ParserOptions::default()
            },
        ),
        Workload::MinifyPrefixLowering => {
            render_minified_with_targets(black_box(prefix_lowering_css()), prefix_targets())
        }
    }
}

fn main() {
    let workload_name = env::args().nth(1).unwrap_or_else(|| {
        eprintln!(
            "usage: cargo run -p rs-stylis --example profile_workload --release -- <workload> [seconds]"
        );
        std::process::exit(2);
    });

    let workload = Workload::parse(&workload_name).unwrap_or_else(|| {
        eprintln!("unknown workload: {workload_name}");
        std::process::exit(2);
    });

    let seconds = env::args()
        .nth(2)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(15);
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut iterations = 0u64;

    while Instant::now() < deadline {
        run_once(workload);
        iterations += 1;
    }

    eprintln!(
        "completed {} iterations of {} in {}s",
        iterations,
        workload.name(),
        seconds
    );
}
