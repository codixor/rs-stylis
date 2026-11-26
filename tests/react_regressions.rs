use rs_stylis::dependencies::{Dependency, DependencyOptions};
use rs_stylis::properties::transform::Transform;
use rs_stylis::properties::{Property, PropertyId};
use rs_stylis::traits::{Parse, ToCss};
use rs_stylis::values::string::CSSString;
use rs_stylis::values::{color::CssColor, length::LengthPercentage};
use rs_stylis::{
    CssModulesConfig, Features, MinifyOptions, NormalizationProfile, ParserOptions, PrinterOptions,
    StyleAttribute, StyleSheet, Targets,
};

fn react_printer() -> PrinterOptions<'static> {
    PrinterOptions {
        minify: true,
        normalization_profile: NormalizationProfile::ReactCompat,
        ..PrinterOptions::default()
    }
}

fn react_minify() -> MinifyOptions {
    MinifyOptions {
        react_compat: true,
        targets: Targets {
            include: Features::VendorPrefixes,
            ..Targets::default()
        },
        ..MinifyOptions::default()
    }
}

#[test]
fn prefixed_declarations_use_module_names_and_url_placeholders() {
    let mut sheet = StyleSheet::parse(
        "@keyframes fade{to{opacity:1}}.base{width:1px}.card{composes:base;\
         animation:fade 1s!important;background-image:image-set(url(icon.svg) 1x);--tone:red;color:var(--tone)}\
         .timed{animation:fade var(--delay);animation-name:var(--name),fade}",
        ParserOptions {
            filename: "components/card.css".into(),
            source_index: 4,
            css_modules: Some(CssModulesConfig {
                dashed_idents: true,
                ..CssModulesConfig::default()
            }),
            ..ParserOptions::default()
        },
    )
    .unwrap();
    sheet.minify(react_minify()).unwrap();
    let result = sheet
        .to_css(PrinterOptions {
            analyze_dependencies: Some(DependencyOptions::default()),
            ..react_printer()
        })
        .unwrap();
    let exports = result.exports.unwrap();
    let animation = &exports["fade"];
    assert!(animation.is_referenced);
    assert!(
        result
            .code
            .contains(&format!("animation:{} var(", animation.name)),
        "{}",
        result.code
    );
    assert!(
        result.code.contains(&format!("),{};", animation.name)),
        "{}",
        result.code
    );
    assert!(
        result
            .code
            .contains(&format!("@keyframes {}", animation.name))
    );
    let card = result
        .code
        .split_once(&format!(".{}{{", exports["card"].name))
        .unwrap()
        .1
        .split('}')
        .next()
        .unwrap();
    for name in ["-webkit-animation", "animation"] {
        let value = card
            .split(';')
            .find_map(|decl| decl.strip_prefix(&format!("{name}:")))
            .unwrap();
        assert!(value.contains(&animation.name), "{}", result.code);
        assert!(value.ends_with("!important"), "{value}");
    }
    assert!(!result.code.contains("composes:"));
    assert_eq!(exports["card"].composes.len(), 1);
    let dashed_name = &exports["--tone"].name;
    assert!(
        result.code.contains(&format!("{dashed_name}:red;")),
        "{}",
        result.code
    );
    assert!(
        result.code.contains(&format!("var({dashed_name})")),
        "{}",
        result.code
    );
    let dependencies = result.dependencies.unwrap();
    let [Dependency::Url(icon)] = dependencies.as_slice() else {
        panic!("expected a single dependency, got {dependencies:?}");
    };
    assert_eq!(icon.url, "icon.svg");
    assert!(!result.code.contains("icon.svg"));
    assert_eq!(
        result.code.matches(&icon.placeholder).count(),
        2,
        "{}",
        result.code
    );
}

#[test]
fn style_attributes_use_current_properties_and_importance_at_print_time() {
    let mut style = StyleAttribute::parse(
        "color:red;transform:translate(1px,0)!important;z-index:1",
        ParserOptions::default(),
    )
    .unwrap();
    style.declarations.declarations[1] =
        Property::parse_string(PropertyId::ZIndex, "2", ParserOptions::default()).unwrap();
    style.minify(react_minify());
    let color =
        Property::parse_string(PropertyId::Color, "blue", ParserOptions::default()).unwrap();
    style.declarations.set(color, true);
    let css = style.to_css(react_printer()).unwrap().code;
    assert!(css.contains("z-index:2;"), "{css}");
    assert!(!css.contains("red"), "{css}");
    assert!(
        css.contains("-webkit-transform:translate(1px,0)!important;"),
        "{css}"
    );
    let reparsed = StyleAttribute::parse(&css, ParserOptions::default()).unwrap();
    let (color, important) = reparsed.declarations.get(&PropertyId::Color).unwrap();
    assert!(important);
    assert_eq!(
        color
            .value_to_css_string(PrinterOptions::default())
            .unwrap(),
        "#00f"
    );
}

#[test]
fn editing_typed_values_invalidates_their_authored_spelling() {
    let mut style = StyleAttribute::parse(
        "color:#f00;transform:translate(1px,0);width:calc(2px + 3px)",
        ParserOptions::default(),
    )
    .unwrap();
    let Property::Color(CssColor::RGBA(color)) = &mut style.declarations.declarations[0] else {
        panic!("expected an RGB color")
    };
    color.red = 0;
    color.blue = 255;
    style.minify(react_minify());
    let Property::Transform(list, _) = &mut style.declarations.declarations[1] else {
        panic!("expected a transform")
    };
    let Transform::Translate(x, _, _) = &mut list.0[0] else {
        panic!("expected translate")
    };
    *x = LengthPercentage::parse_string("2px").unwrap();
    let css = style.to_css(react_printer()).unwrap().code;
    assert!(css.contains("color:#00f;"), "{css}");
    assert!(css.contains("transform:translate(2px,0);"), "{css}");
    assert!(css.contains("width:calc(2px + 3px);"), "{css}");
}

#[test]
fn strings_round_trip_through_react_printing() {
    for text in [
        "\u{a0}B",
        "\u{a0}0",
        "\u{a0} ",
        "\u{a0}\t",
        "\u{a0}\u{a0}",
        "a\"b",
        "\\\"\\\\",
        "one\ntwo",
        "\u{0001}f",
        "é雪",
    ] {
        let value = CSSString::from(text);
        let css = value.to_css_string(react_printer()).unwrap();
        let decoded = CSSString::parse_string(&css).unwrap();
        assert_eq!(decoded.as_ref(), text, "{css}");
    }
}
