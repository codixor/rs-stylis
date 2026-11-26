use rs_stylis::{
    dependencies::{Dependency, DependencyOptions, Location, SourceRange},
    rules::CssRule,
    stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet},
};

fn options(remove_imports: bool) -> PrinterOptions<'static> {
    PrinterOptions {
        minify: true,
        analyze_dependencies: Some(DependencyOptions { remove_imports }),
        ..PrinterOptions::default()
    }
}

fn parse<'i>(css: &'i str, filename: &str, source_index: u32) -> StyleSheet<'i, 'static> {
    StyleSheet::parse(
        css,
        ParserOptions {
            filename: filename.into(),
            source_index,
            ..ParserOptions::default()
        },
    )
    .unwrap()
}

// Convert the public one-based UTF-16 coordinates into a replacement slice.
fn source_slice<'a>(source: &'a str, range: &SourceRange) -> &'a str {
    let byte_offset = |loc: Location, inclusive: bool| {
        let mut lines = source.split_inclusive('\n');
        let line_start: usize = lines
            .by_ref()
            .take(loc.line as usize - 1)
            .map(str::len)
            .sum();
        let column = loc.column as usize - usize::from(!inclusive);
        let line = lines.next().unwrap_or_default();
        let mut offset = line_start;
        let mut units = 0;
        for ch in line.chars() {
            if units == column {
                break;
            }
            units += ch.len_utf16();
            offset += ch.len_utf8();
        }
        assert_eq!(units, column);
        offset
    };
    &source[byte_offset(range.start, false)..byte_offset(range.end, true)]
}

#[test]
fn files_and_merged_rule_lists_keep_distinct_relative_assets() {
    let mut first = parse(".a{background-image:url(icon.svg)}", "one/site.css", 0);
    let second = parse(".b{background-image:url(icon.svg)}", "two/site.css", 1);
    for (sheet, path) in [(&first, "one/site.css"), (&second, "two/site.css")] {
        let output = sheet.to_css(options(false)).unwrap();
        let dependencies = output.dependencies.unwrap();
        let [Dependency::Url(url)] = &dependencies[..] else {
            panic!("{dependencies:?}")
        };
        assert_eq!(url.loc.file_path, path);
    }

    first.rules.0.extend(second.rules.0);
    let mut combined = StyleSheet::new(
        vec!["one/site.css".into(), "two/site.css".into()],
        first.rules,
        ParserOptions::default(),
    );
    combined.minify(MinifyOptions::default()).unwrap();
    let output = combined.to_css(options(false)).unwrap();
    let dependencies = output.dependencies.unwrap();
    let [Dependency::Url(first), Dependency::Url(second)] = &dependencies[..] else {
        panic!("{dependencies:?}");
    };
    assert_eq!(first.loc.file_path, "one/site.css");
    assert_eq!(second.loc.file_path, "two/site.css");
    assert_ne!(first.placeholder, second.placeholder);
    assert!(output.code.contains(&first.placeholder));
    assert!(output.code.contains(&second.placeholder));
}

#[test]
fn removed_and_retained_imports_report_layers_and_original_spelling() {
    let source = "@import /* first */ 'plain.css';\n\
                  @import url( anon.css ) layer;\n\
                  @import \nurl( \"nested\\2e css\" ) layer(theme.widgets) supports(display: grid) screen;";
    let sheet = StyleSheet::new(
        vec!["unused.css".into(), "imports.css".into()],
        parse(source, "imports.css", 1).rules,
        ParserOptions::default(),
    );
    for remove in [false, true] {
        let output = sheet.to_css(options(remove)).unwrap();
        assert_eq!(output.code.contains("@import"), !remove);
        let dependencies = output.dependencies.unwrap();
        let [
            Dependency::Import(plain),
            Dependency::Import(anon),
            Dependency::Import(named),
        ] = &dependencies[..]
        else {
            panic!("{dependencies:?}")
        };
        assert_eq!(plain.layer, None);
        assert_eq!(anon.layer, Some(None));
        assert_eq!(named.layer, Some(Some("theme.widgets".into())));
        assert!(named.supports.as_deref().unwrap().contains("display"));
        assert_eq!(named.media.as_deref(), Some("screen"));
        for (import, spelling) in [
            (plain, "plain.css"),
            (anon, "anon.css"),
            (named, "nested\\2e css"),
        ] {
            assert_eq!(import.loc.file_path, "imports.css");
            assert_eq!(source_slice(source, &import.loc), spelling);
            assert_eq!(output.code.contains(&import.placeholder), !remove);
        }
        assert_eq!(named.url, "nested.css");
    }
}

#[test]
fn nested_rules_use_their_own_source_index() {
    let mut sheet = parse(
        "@media screen { .a { cursor: url(a.cur), auto } }",
        "one/site.css",
        0,
    );
    let CssRule::Media(media) = &mut sheet.rules.0[0] else {
        unreachable!()
    };
    media.rules.0.extend(
        parse(".b { cursor: url(b.cur), auto }", "two/site.css", 1)
            .rules
            .0,
    );
    sheet.sources.push("two/site.css".into());
    let dependencies = sheet.to_css(options(false)).unwrap().dependencies.unwrap();
    let [Dependency::Url(first), Dependency::Url(second)] = &dependencies[..] else {
        panic!("{dependencies:?}");
    };
    assert_eq!(
        (&*first.url, &*first.loc.file_path),
        ("a.cur", "one/site.css")
    );
    assert_eq!(
        (&*second.url, &*second.loc.file_path),
        ("b.cur", "two/site.css")
    );
}

#[test]
fn minification_keeps_file_relative_urls_in_at_rules() {
    for source in [
        "@keyframes pulse { to { background-image:url(icon.svg) } }",
        "@container style(background-image:url(icon.svg)) { .a { color: red } }",
    ] {
        let mut first = parse(source, "one/site.css", 0);
        first
            .rules
            .0
            .extend(parse(source, "two/site.css", 1).rules.0);
        first.sources.push("two/site.css".into());
        first.minify(MinifyOptions::default()).unwrap();
        let dependencies = first.to_css(options(false)).unwrap().dependencies.unwrap();
        let [Dependency::Url(one), Dependency::Url(two)] = &dependencies[..] else {
            panic!("{source}: {dependencies:?}");
        };
        assert_eq!(one.loc.file_path, "one/site.css");
        assert_eq!(two.loc.file_path, "two/site.css");
    }
}

#[test]
fn dependency_ranges_select_exact_url_contents() {
    let cases = [
        ("url(\"icon.svg\")", "icon.svg", "icon.svg"),
        ("url(no-quotes.svg)", "no-quotes.svg", "no-quotes.svg"),
        (
            " /* before */ URL( \t'face🙂.svg' \n)",
            "face🙂.svg",
            "face🙂.svg",
        ),
        ("u\\72 l( \r\n a\\)b.svg \t)", "a\\)b.svg", "a)b.svg"),
        ("url(\\31 23\\  )", "\\31 23\\ ", "123 "),
        ("url(x\\20  )", "x\\20 ", "x "),
        (
            "url(\"multi\\\r\nline\\\"name\")",
            "multi\\\r\nline\\\"name",
            "multiline\"name",
        ),
        ("url('')", "", ""),
        ("url(  )", "", ""),
        ("image-set( 'alt\\2e png' 1x)", "alt\\2e png", "alt.png"),
    ];
    for (syntax, spelling, decoded) in cases {
        let source = format!(".🙂{{background-image:{syntax}}}");
        let sheet = parse(&source, "styles/site.css", 0);
        let output = sheet.to_css(options(false)).unwrap();
        let dependencies = output.dependencies.unwrap();
        let [Dependency::Url(url)] = &dependencies[..] else {
            panic!("{syntax}: {dependencies:?}")
        };
        assert_eq!(url.url, decoded, "{syntax}");
        assert_eq!(source_slice(&source, &url.loc), spelling, "{syntax}");
        assert!(output.code.contains(&url.placeholder));
    }
}
