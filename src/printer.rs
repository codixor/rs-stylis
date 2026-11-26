//! CSS serialization.

use crate::css_modules::CssModule;
use crate::dependencies::{Dependency, DependencyOptions};
use crate::error::{Error, ErrorLocation, PrinterError, PrinterErrorKind};
use crate::react_compat::NormalizationProfile;
use crate::rules::Location;
use crate::targets::{Targets, TargetsWithSupportsScope};
use crate::vendor_prefix::VendorPrefix;
use cssparser::{serialize_identifier, serialize_name};

const INDENT_CHUNK: &str = "                                ";

/// Options that control how CSS is serialized to a string.
#[derive(Default)]
pub struct PrinterOptions<'a> {
    /// Whether to minify the CSS, i.e. remove white space.
    pub minify: bool,
    /// An optional project root path, used to generate relative paths for sources used in CSS module hashes.
    pub project_root: Option<&'a str>,
    /// Targets to output the CSS for.
    pub targets: Targets,
    /// Whether to analyze dependencies (i.e. `@import` and `url()`).
    /// If true, the dependencies are returned as part of the
    /// [ToCssResult](super::stylesheet::ToCssResult).
    ///
    /// When enabled, `@import` and `url()` dependencies
    /// are replaced with hashed placeholders that can be replaced with the final
    /// urls later (after bundling).
    pub analyze_dependencies: Option<DependencyOptions>,
    /// A mapping of pseudo classes to replace with class names that can be applied
    /// from JavaScript. Useful for polyfills, for example.
    pub pseudo_classes: Option<PseudoClasses<'a>>,
    /// Controls the normalization strategy used when multiple valid CSS serializations exist.
    pub normalization_profile: NormalizationProfile,
    /// Applies the JS stylis `namespace()` middleware selector rewrite during output.
    pub namespace_middleware: bool,
}

/// A mapping of user action pseudo classes to replace with class names.
///
/// See [PrinterOptions](PrinterOptions).
#[derive(Default, Debug)]
pub struct PseudoClasses<'a> {
    /// The class name to replace `:hover` with.
    pub hover: Option<&'a str>,
    /// The class name to replace `:active` with.
    pub active: Option<&'a str>,
    /// The class name to replace `:focus` with.
    pub focus: Option<&'a str>,
    /// The class name to replace `:focus-visible` with.
    pub focus_visible: Option<&'a str>,
    /// The class name to replace `:focus-within` with.
    pub focus_within: Option<&'a str>,
}

/// A `Printer` represents a destination to output serialized CSS, as used in
/// the [ToCss](super::traits::ToCss) trait. It can wrap any destination that
/// implements [std::fmt::Write](std::fmt::Write), such as a [String](String).
///
/// A `Printer` keeps track of the current line and column position.
///
/// `Printer` also includes helper functions that assist with writing output
/// that respects options such as `minify`, and `css_modules`.
pub struct Printer<'a, 'b, 'c, W> {
    pub(crate) sources: Option<&'c Vec<String>>,
    dest: &'a mut W,

    pub(crate) loc: Location,
    indent: u8,
    line: u32,
    col: u32,
    pub(crate) minify: bool,
    pub(crate) targets: TargetsWithSupportsScope,
    /// Vendor prefix override. When non-empty, it overrides
    /// the vendor prefix of whatever is being printed.
    pub(crate) vendor_prefix: VendorPrefix,
    pub(crate) in_calc: bool,
    pub(crate) css_module: Option<CssModule<'c, 'b, 'c>>,
    pub(crate) dependencies: Option<Vec<Dependency>>,
    pub(crate) remove_imports: bool,
    pub(crate) pseudo_classes: Option<PseudoClasses<'a>>,
    pub(crate) normalization_profile: NormalizationProfile,
    use_authored_values: bool,
    pub(crate) namespace_middleware: bool,
    last_char: Option<char>,
}

impl<'a, 'b, 'c, W: std::fmt::Write + Sized> Printer<'a, 'b, 'c, W> {
    /// Create a new Printer wrapping the given destination.
    pub fn new(dest: &'a mut W, options: PrinterOptions<'a>) -> Self {
        Printer {
            sources: None,
            dest,

            loc: Location {
                source_index: 0,
                line: 0,
                column: 1,
            },
            indent: 0,
            line: 0,
            col: 0,
            minify: options.minify,
            targets: TargetsWithSupportsScope::new(options.targets),
            vendor_prefix: VendorPrefix::empty(),
            in_calc: false,
            css_module: None,
            dependencies: if options.analyze_dependencies.is_some() {
                Some(Vec::new())
            } else {
                None
            },
            remove_imports: matches!(&options.analyze_dependencies, Some(d) if d.remove_imports),
            pseudo_classes: options.pseudo_classes,
            normalization_profile: options.normalization_profile,
            use_authored_values: true,
            namespace_middleware: options.namespace_middleware,
            last_char: None,
        }
    }

    /// Returns the current source filename that is being printed.
    pub fn filename(&self) -> &'c str {
        if let Some(sources) = self.sources {
            if let Some(f) = sources.get(self.loc.source_index as usize) {
                f
            } else {
                "unknown.css"
            }
        } else {
            "unknown.css"
        }
    }

    pub(crate) fn with_rule_location<T>(
        &mut self,
        mut loc: Location,
        print: impl FnOnce(&mut Self) -> T,
    ) -> T {
        // A separately parsed stylesheet can already carry its index in a future bundle.
        // Its own source and CSS module tables still have just one entry.
        if self.sources.is_some_and(|sources| sources.len() == 1) {
            loc.source_index = 0;
        }
        let previous = std::mem::replace(&mut self.loc, loc);
        let result = print(self);
        self.loc = previous;
        result
    }

    /// Writes a raw string to the underlying destination.
    ///
    /// NOTE: Is is assumed that the string does not contain any newline characters.
    /// If such a string is written, it will break source maps.
    pub fn write_str(&mut self, s: &str) -> Result<(), PrinterError> {
        self.col += s.len() as u32;
        self.dest.write_str(s)?;
        if let Some(last_char) = s.chars().next_back() {
            self.last_char = Some(last_char);
        }
        Ok(())
    }

    /// Writes a raw string which may contain newlines to the underlying destination.
    pub fn write_str_with_newlines(&mut self, s: &str) -> Result<(), PrinterError> {
        let mut last_line_start: usize = 0;
        let mut last_char = None;

        for (idx, n) in s.char_indices() {
            last_char = Some(n);
            if n == '\n' {
                self.line += 1;
                self.col = 0;

                // Keep track of where the *next* line starts
                last_line_start = idx + 1;
            }
        }

        self.col += (s.len() - last_line_start) as u32;
        self.dest.write_str(s)?;
        if let Some(last_char) = last_char {
            self.last_char = Some(last_char);
        }
        Ok(())
    }

    /// Write a single character to the underlying destination.
    pub fn write_char(&mut self, c: char) -> Result<(), PrinterError> {
        if c == '\n' {
            self.line += 1;
            self.col = 0;
        } else {
            self.col += 1;
        }
        self.dest.write_char(c)?;
        self.last_char = Some(c);
        Ok(())
    }

    /// Returns the most recently written character, if any.
    pub fn last_char(&self) -> Option<char> {
        self.last_char
    }

    /// Writes a single whitespace character, unless the `minify` option is enabled.
    ///
    /// Use `write_char` instead if you wish to force a space character to be written,
    /// regardless of the `minify` option.
    pub fn whitespace(&mut self) -> Result<(), PrinterError> {
        if self.minify {
            return Ok(());
        }

        self.write_char(' ')
    }

    /// Writes a delimiter character, followed by whitespace (depending on the `minify` option).
    /// If `ws_before` is true, then whitespace is also written before the delimiter.
    pub fn delim(&mut self, delim: char, ws_before: bool) -> Result<(), PrinterError> {
        if ws_before {
            self.whitespace()?;
        }
        self.write_char(delim)?;
        self.whitespace()
    }

    /// Writes a newline character followed by indentation.
    /// If the `minify` option is enabled, then nothing is printed.
    pub fn newline(&mut self) -> Result<(), PrinterError> {
        if self.minify {
            return Ok(());
        }

        self.write_char('\n')?;
        let mut remaining = self.indent as usize;
        while remaining > 0 {
            let chunk = remaining.min(INDENT_CHUNK.len());
            self.write_str(&INDENT_CHUNK[..chunk])?;
            remaining -= chunk;
        }

        Ok(())
    }

    /// Increases the current indent level.
    pub fn indent(&mut self) {
        self.indent += 2;
    }

    /// Decreases the current indent level.
    pub fn dedent(&mut self) {
        self.indent -= 2;
    }

    /// Increases the current indent level by the given number of characters.
    pub fn indent_by(&mut self, amt: u8) {
        self.indent += amt;
    }

    /// Decreases the current indent level by the given number of characters.
    pub fn dedent_by(&mut self, amt: u8) {
        self.indent -= amt;
    }

    pub(crate) fn declaration_printer<'d>(
        &self,
        text: &'d mut String,
    ) -> Printer<'d, 'b, 'c, String> {
        let mut printer = Printer::new(
            text,
            PrinterOptions {
                minify: true,
                normalization_profile: self.normalization_profile,
                targets: self.targets.current,
                ..PrinterOptions::default()
            },
        );
        printer.sources = self.sources;
        printer.loc = self.loc;
        printer.vendor_prefix = self.vendor_prefix;
        // The declaration printer validates authored spellings against these typed values.
        printer.use_authored_values = false;
        printer
    }

    /// Serialize a declaration component for prefix expansion, retaining the source and
    /// recording CSS Modules exports and URL dependencies once per authored value.
    pub(crate) fn declaration_text(
        &mut self,
        write: impl FnOnce(&mut Printer<String>) -> Result<(), PrinterError>,
    ) -> Result<String, PrinterError> {
        let mut text = String::new();
        let mut printer = self.declaration_printer(&mut text);
        printer.css_module = self.css_module.take();
        printer.dependencies = self.dependencies.take();
        let result = write(&mut printer);
        self.css_module = printer.css_module;
        self.dependencies = printer.dependencies;
        result.map(|()| text)
    }

    /// Returns whether the indent level is greater than one.
    pub fn is_nested(&self) -> bool {
        self.indent > 2
    }

    /// Writes a CSS identifier to the underlying destination, escaping it
    /// as appropriate. If the `css_modules` option was enabled, then a hash
    /// is added, and the mapping is added to the CSS module.
    pub fn write_ident(
        &mut self,
        ident: &str,
        handle_css_module: bool,
    ) -> Result<(), PrinterError> {
        if handle_css_module {
            if let Some(css_module) = &mut self.css_module {
                let dest = &mut self.dest;
                let mut first = true;
                css_module.config.pattern.write(
                    &css_module.hashes[self.loc.source_index as usize],
                    &css_module.sources[self.loc.source_index as usize],
                    ident,
                    if let Some(content_hashes) = &css_module.content_hashes {
                        &content_hashes[self.loc.source_index as usize]
                    } else {
                        ""
                    },
                    |s| {
                        self.col += s.len() as u32;
                        if first {
                            first = false;
                            serialize_identifier(s, dest)
                        } else {
                            serialize_name(s, dest)
                        }
                    },
                )?;

                css_module.add_local(&ident, &ident, self.loc.source_index);
                return Ok(());
            }
        }

        serialize_identifier(ident, self)?;
        Ok(())
    }

    pub(crate) fn write_dashed_ident(
        &mut self,
        ident: &str,
        is_declaration: bool,
    ) -> Result<(), PrinterError> {
        self.write_str("--")?;

        match &mut self.css_module {
            Some(css_module) if css_module.config.dashed_idents => {
                let dest = &mut self.dest;
                css_module.config.pattern.write(
                    &css_module.hashes[self.loc.source_index as usize],
                    &css_module.sources[self.loc.source_index as usize],
                    &ident[2..],
                    if let Some(content_hashes) = &css_module.content_hashes {
                        &content_hashes[self.loc.source_index as usize]
                    } else {
                        ""
                    },
                    |s| {
                        self.col += s.len() as u32;
                        serialize_name(s, dest)
                    },
                )?;

                if is_declaration {
                    css_module.add_dashed(ident, self.loc.source_index);
                }
            }
            _ => {
                serialize_name(&ident[2..], self)?;
            }
        }

        Ok(())
    }

    /// Returns an error of the given kind at the provided location in the current source file.
    pub fn error(
        &self,
        kind: PrinterErrorKind,
        loc: crate::dependencies::Location,
    ) -> Error<PrinterErrorKind> {
        Error {
            kind,
            loc: Some(ErrorLocation {
                filename: self.filename().into(),
                line: loc.line - 1,
                column: loc.column,
            }),
        }
    }

    pub(crate) fn is_react_compat(&self) -> bool {
        self.normalization_profile.is_react_compat()
    }

    pub(crate) fn uses_authored_values(&self) -> bool {
        self.is_react_compat() && self.use_authored_values
    }

    pub(crate) fn preserve_trailing_semicolon(&self) -> bool {
        self.normalization_profile.preserve_trailing_semicolon()
    }

    pub(crate) fn namespace_middleware_enabled(&self) -> bool {
        self.namespace_middleware
    }
}

impl<'a, 'b, 'c, W: std::fmt::Write + Sized> std::fmt::Write for Printer<'a, 'b, 'c, W> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.col += s.len() as u32;
        self.dest.write_str(s)
    }
}
