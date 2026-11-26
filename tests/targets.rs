use pretty_assertions::assert_eq;
use rs_stylis::targets::{Browsers, Features, Targets};

#[test]
fn builds_targets_from_browser_struct() {
    let browsers = Browsers {
        chrome: Some(120 << 16),
        safari: Some((17 << 16) | (4 << 8)),
        ..Browsers::default()
    };

    let targets: Targets = browsers.into();
    assert_eq!(targets.browsers, Some(browsers));
    assert_eq!(targets.include, Features::empty());
    assert_eq!(targets.exclude, Features::empty());
}

#[test]
fn exposes_feature_groups() {
    assert!(Features::Selectors.contains(Features::Nesting));
    assert!(Features::MediaQueries.contains(Features::MediaRangeSyntax));
    assert!(Features::Colors.contains(Features::ColorFunction));
}

#[cfg(feature = "browserslist")]
#[test]
fn parses_browserslist_queries() {
    let browsers = Browsers::from_browserslist(["chrome 120", "firefox 115"])
        .unwrap()
        .expect("browserslist targets");

    assert_eq!(browsers.chrome, Some(120 << 16));
    assert_eq!(browsers.firefox, Some(115 << 16));
}
