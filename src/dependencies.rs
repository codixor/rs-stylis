//! Dependency analysis.
//!
//! Dependencies in CSS can be analyzed using the `analyze_dependencies` option
//! when printing a style sheet. These include other style sheets referenced via
//! the `@import` rule, as well as `url()` references. See [PrinterOptions](PrinterOptions).
//!
//! Dependency analysis replaces URLs with hashed placeholders that can be substituted
//! with the final URLs after bundling. Set [`DependencyOptions::remove_imports`] to
//! remove `@import` rules and recover their conditions from the dependency records.

use crate::css_modules::hash;
use crate::printer::PrinterOptions;
use crate::rules::import::ImportRule;
use crate::traits::ToCss;
use crate::values::url::Url;
use cssparser::SourceLocation;
/// Options for `analyze_dependencies` in `PrinterOptions`.
#[derive(Default)]
pub struct DependencyOptions {
    /// Whether to remove `@import` rules.
    pub remove_imports: bool,
}

/// A dependency.
#[derive(Debug)]
pub enum Dependency {
    /// An `@import` dependency.
    Import(ImportDependency),
    /// A `url()` dependency.
    Url(UrlDependency),
}

/// An `@import` dependency.
#[derive(Debug)]
pub struct ImportDependency {
    /// The url to import.
    pub url: String,
    /// The placeholder that the URL was replaced with.
    pub placeholder: String,
    /// The cascade layer. `None` means unlayered, `Some(None)` means anonymous,
    /// and `Some(Some(name))` contains a serialized CSS layer name.
    pub layer: Option<Option<String>>,
    /// An optional `supports()` condition.
    pub supports: Option<String>,
    /// A media query.
    pub media: Option<String>,
    /// The location of the dependency in the source file.
    pub loc: SourceRange,
}

impl ImportDependency {
    /// Creates a new dependency from an `@import` rule.
    pub fn new(rule: &ImportRule, filename: &str) -> ImportDependency {
        let supports = if let Some(supports) = &rule.supports {
            let s = supports.to_css_string(PrinterOptions::default()).unwrap();
            Some(s)
        } else {
            None
        };

        let media = if !rule.media.media_queries.is_empty() {
            let s = rule.media.to_css_string(PrinterOptions::default()).unwrap();
            Some(s)
        } else {
            None
        };

        let placeholder = hash(&format!("{filename}_{}", rule.url), false);

        ImportDependency {
            url: rule.url.as_ref().to_owned(),
            placeholder,
            layer: rule.layer.as_ref().map(|layer| {
                layer
                    .as_ref()
                    .map(|name| name.to_css_string(PrinterOptions::default()).unwrap())
            }),
            supports,
            media,
            loc: SourceRange::new(filename, rule.url_span),
        }
    }
}

/// A `url()` dependency.
#[derive(Debug)]
pub struct UrlDependency {
    /// The url of the dependency.
    pub url: String,
    /// The placeholder that the URL was replaced with.
    pub placeholder: String,
    /// The location of the dependency in the source file.
    pub loc: SourceRange,
}

impl UrlDependency {
    /// Creates a new url dependency.
    pub fn new(url: &Url, filename: &str) -> UrlDependency {
        let placeholder = hash(&format!("{filename}_{}", url.url), false);
        UrlDependency {
            url: url.url.to_string(),
            placeholder,
            loc: SourceRange::new(filename, url.span),
        }
    }
}

/// Represents the range of source code where a dependency was found.
#[derive(Debug)]
pub struct SourceRange {
    /// The filename in which the dependency was found.
    pub file_path: String,
    /// The first source code unit of the URL contents (excluding surrounding quotes).
    pub start: Location,
    /// The last source code unit, inclusive. For an empty URL, this is one column before `start`.
    pub end: Location,
}

/// A line and column position within a source file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Location {
    /// The line number, starting from 1.
    pub line: u32,
    /// The column number in UTF-16 code units, starting from 1.
    pub column: u32,
}

impl From<SourceLocation> for Location {
    fn from(loc: SourceLocation) -> Location {
        Location {
            line: loc.line + 1,
            column: loc.column,
        }
    }
}

impl SourceRange {
    fn new(filename: &str, span: SourceSpan) -> SourceRange {
        SourceRange {
            file_path: filename.into(),
            start: span.start,
            end: span.end,
        }
    }
}

/// Positions of the original URL contents, independent of the stylesheet filename.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceSpan {
    /// The first source code unit, or the insertion point for an empty URL.
    pub start: Location,
    /// The last source code unit, inclusive. For an empty URL this is one column before `start`.
    pub end: Location,
}
