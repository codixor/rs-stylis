# rs-stylis

Parse CSS into a typed Rust syntax tree, transform it for a set of target
browsers, and print the result as formatted or minified CSS. Use `StyleSheet`
for a complete stylesheet and `StyleAttribute` for an inline declaration list.

## Minify a stylesheet

Add `rs-stylis = "0.1"` to your Cargo dependencies. The crate requires Rust
1.100 or later.

```rust
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

    println!("{}", css.code); // .foo,.bar{color:red}
    Ok(())
}
```

`StyleSheet::minify` merges compatible rules and simplifies declarations.
`PrinterOptions::minify` controls whitespace and compact serialization. For
readable, normalized CSS, parse the input and call `to_css` with the default
printer options.

## Browser targets and CSS Modules

Pass the same `Targets` to the minifier and printer to lower CSS syntax and
emit vendor prefixes for your browsers. Set versions directly with `Browsers`,
or enable the optional `browserslist` feature to build targets from queries
with `Browsers::from_browserslist`.

For CSS Modules, set `ParserOptions::css_modules` and supply the input filename.
The printer returns both the generated CSS and a map of the local class names
to their exports. The CSS-in-JS compatibility options are
`MinifyOptions::react_compat` and `NormalizationProfile::ReactCompat`.

## Upgrading from 0.1

Dependency records now identify the originating stylesheet. URL ranges cover
the original URL contents, including CSS escapes. Lines and UTF-16 columns
start at one, and the end is inclusive. For an empty URL, insert at the start
position. Code that constructs AST nodes directly must supply `Url::span` or
`ImportRule::url_span`. Parsing CSS fills these fields automatically.

`ImportDependency::layer` distinguishes unlayered, anonymously layered and
named-layer imports. Use it with the recorded media and supports conditions
when inlining an import.

The DOM selector matcher supports filtered `:nth-child()` and
`:nth-last-child()`. Use `parcel_selectors::matching::try_matches_selector`
to distinguish a nonmatch from selectors that need a richer DOM or an
enclosing style rule, including `:has()`, table-column selectors and `&`.

## Examples

Run the examples from a checkout.

```sh
cargo run --example parse
cargo run --example normalize
cargo run --example minify
cargo run --example prefix_lowering
cargo run --example css_modules
```

The parser and transformation engine derive from Lightning CSS and use
`cssparser` and the Servo selector implementation. The [license](LICENSE)
includes attribution and terms for these components.
