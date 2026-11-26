#![allow(
    clippy::format_push_string,
    clippy::needless_raw_string_hashes,
    clippy::semicolon_if_nothing_returned
)]

use std::hint::black_box;
use std::sync::OnceLock;

use criterion::{Criterion, criterion_group, criterion_main};
use rs_stylis::{
    Features, MinifyOptions, ParserFlags, ParserOptions, PrinterOptions, StyleSheet, Targets,
    declaration::DeclarationBlock, printer::Printer, properties::Property,
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
const DECLARATION_ROW: &str = r#"
color: red;
background: linear-gradient(45deg, #0af, #f43f5e);
padding: 1rem 1.5rem;
border: 1px solid color-mix(in srgb, #0af 14%, white);
border-radius: 14px;
transform: translate(1px,0) scale(1);
transition: transform .2s ease, opacity 120ms linear;
content: "go";
display: grid;
grid-template-columns: 1fr minmax(0, 2fr);
gap: var(--space);
filter: blur(4px);
--space: 1rem;
--radius: 12px;
"#;

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

fn render_normalized(code: &str) {
    let stylesheet = StyleSheet::parse(code, ParserOptions::default()).unwrap();
    let _ = stylesheet.to_css(PrinterOptions::default()).unwrap();
}

fn render_preminified(stylesheet: &StyleSheet<'static, 'static>) {
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

fn render_declaration_block(block: &DeclarationBlock<'static>, minify: bool) {
    let mut out = String::new();
    let mut printer = Printer::new(
        &mut out,
        PrinterOptions {
            minify,
            ..PrinterOptions::default()
        },
    );
    block.to_css_block(&mut printer).unwrap();
}

fn render_property_list(properties: &[Property<'static>], minify: bool) {
    let mut out = String::new();
    let mut printer = Printer::new(
        &mut out,
        PrinterOptions {
            minify,
            ..PrinterOptions::default()
        },
    );
    for (idx, property) in properties.iter().enumerate() {
        property.to_css(&mut printer, false).unwrap();
        if idx + 1 != properties.len() {
            printer.write_char(';').unwrap();
            printer.newline().unwrap();
        }
    }
}

fn selector_heavy_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| SELECTOR_ROW.repeat(120)).as_str()
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

fn declaration_heavy_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| DECLARATION_ROW.repeat(48)).as_str()
}

fn declaration_heavy_block() -> &'static DeclarationBlock<'static> {
    static BLOCK: OnceLock<DeclarationBlock<'static>> = OnceLock::new();
    BLOCK.get_or_init(|| {
        DeclarationBlock::parse_string(declaration_heavy_css(), ParserOptions::default()).unwrap()
    })
}

fn declaration_heavy_properties() -> &'static Vec<Property<'static>> {
    static PROPERTIES: OnceLock<Vec<Property<'static>>> = OnceLock::new();
    PROPERTIES.get_or_init(|| {
        let block = declaration_heavy_block();
        block
            .declarations
            .iter()
            .chain(block.important_declarations.iter())
            .cloned()
            .collect()
    })
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

fn bench_parse_small(c: &mut Criterion) {
    c.bench_function("rs-stylis/parse_small", |b| {
        b.iter(|| StyleSheet::parse(black_box(SMALL_CSS), ParserOptions::default()).unwrap())
    });
}

fn bench_minify_small(c: &mut Criterion) {
    c.bench_function("rs-stylis/minify_small", |b| {
        b.iter(|| render_minified(black_box(MERGE_CSS)))
    });
}

fn bench_normalize_small(c: &mut Criterion) {
    c.bench_function("rs-stylis/normalize_small", |b| {
        b.iter(|| render_normalized(black_box(SMALL_CSS)))
    });
}

fn bench_parse_large_app(c: &mut Criterion) {
    c.bench_function("rs-stylis/parse_large_app", |b| {
        let css = large_app_css();
        b.iter(|| StyleSheet::parse(black_box(css), ParserOptions::default()).unwrap())
    });
}

fn bench_minify_large_app(c: &mut Criterion) {
    c.bench_function("rs-stylis/minify_large_app", |b| {
        let css = large_app_css();
        b.iter(|| render_minified(black_box(css)))
    });
}

fn bench_normalize_large_app(c: &mut Criterion) {
    c.bench_function("rs-stylis/normalize_large_app", |b| {
        let css = large_app_css();
        b.iter(|| render_normalized(black_box(css)))
    });
}

fn bench_parse_selector_heavy(c: &mut Criterion) {
    c.bench_function("rs-stylis/parse_selector_heavy", |b| {
        let css = selector_heavy_css();
        b.iter(|| StyleSheet::parse(black_box(css), ParserOptions::default()).unwrap())
    });
}

fn bench_minify_selector_heavy(c: &mut Criterion) {
    c.bench_function("rs-stylis/minify_selector_heavy", |b| {
        let css = selector_heavy_css();
        b.iter(|| render_minified(black_box(css)))
    });
}

fn bench_print_selector_heavy(c: &mut Criterion) {
    c.bench_function("rs-stylis/print_selector_heavy", |b| {
        let stylesheet = minified_selector_heavy_stylesheet();
        b.iter(|| render_preminified(black_box(stylesheet)))
    });
}

fn bench_minify_custom_properties(c: &mut Criterion) {
    c.bench_function("rs-stylis/minify_custom_properties", |b| {
        let css = custom_properties_css();
        b.iter(|| render_minified(black_box(css)))
    });
}

fn bench_minify_nested_at_rules(c: &mut Criterion) {
    c.bench_function("rs-stylis/minify_nested_at_rules", |b| {
        let css = nested_at_rules_css();
        b.iter(|| render_minified(black_box(css)))
    });
}

fn bench_minify_nesting_heavy(c: &mut Criterion) {
    c.bench_function("rs-stylis/minify_nesting_heavy", |b| {
        let css = nesting_heavy_css();
        b.iter(|| {
            render_minified_with_parser_options(
                black_box(css),
                ParserOptions {
                    flags: ParserFlags::NESTING,
                    ..ParserOptions::default()
                },
            )
        })
    });
}

fn bench_minify_prefix_lowering(c: &mut Criterion) {
    c.bench_function("rs-stylis/minify_prefix_lowering", |b| {
        let css = prefix_lowering_css();
        let targets = prefix_targets();
        b.iter(|| render_minified_with_targets(black_box(css), targets))
    });
}

fn bench_print_declaration_block_large(c: &mut Criterion) {
    c.bench_function("rs-stylis/print_declaration_block_large", |b| {
        let block = declaration_heavy_block();
        b.iter(|| render_declaration_block(black_box(block), false))
    });
}

fn bench_minify_declaration_block_large(c: &mut Criterion) {
    c.bench_function("rs-stylis/minify_declaration_block_large", |b| {
        let block = declaration_heavy_block();
        b.iter(|| render_declaration_block(black_box(block), true))
    });
}

fn bench_minify_property_list_large(c: &mut Criterion) {
    c.bench_function("rs-stylis/minify_property_list_large", |b| {
        let properties = declaration_heavy_properties();
        b.iter(|| render_property_list(black_box(properties), true))
    });
}

criterion_group!(
    stylis_core,
    bench_parse_small,
    bench_normalize_small,
    bench_minify_small,
    bench_parse_large_app,
    bench_normalize_large_app,
    bench_minify_large_app,
    bench_parse_selector_heavy,
    bench_minify_selector_heavy,
    bench_print_selector_heavy,
    bench_minify_custom_properties,
    bench_minify_nested_at_rules,
    bench_minify_nesting_heavy,
    bench_minify_prefix_lowering,
    bench_print_declaration_block_large,
    bench_minify_declaration_block_large,
    bench_minify_property_list_large
);
criterion_main!(stylis_core);
