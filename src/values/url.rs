//! CSS url() values.

use crate::dependencies::{Dependency, Location, SourceSpan, UrlDependency};
use crate::error::{ParserError, PrinterError};
use crate::printer::Printer;
use crate::traits::{Parse, ToCss};
use crate::values::string::CowArcStr;
use cssparser::*;

/// A CSS [url()](https://www.w3.org/TR/css-values-4/#urls) value and its source location.
#[derive(Debug, Clone)]
pub struct Url<'i> {
    /// The url string.
    pub url: CowArcStr<'i>,
    /// The location where the `url()` was seen in the CSS source file.
    pub loc: Location,
    /// The original source span of the URL contents, excluding quotes and delimiters.
    pub span: SourceSpan,
}

impl<'i> PartialEq for Url<'i> {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

impl<'i> Parse<'i> for Url<'i> {
    fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
        Self::parse_token(input, false)
    }
}

impl<'i> Url<'i> {
    pub(crate) fn parse_url_or_string<'t>(
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i, ParserError<'i>>> {
        Self::parse_token(input, true)
    }

    fn parse_token<'t>(
        input: &mut Parser<'i, 't>,
        allow_string: bool,
    ) -> Result<Self, ParseError<'i, ParserError<'i>>> {
        input.skip_whitespace();
        let start = input.state();
        let value = if allow_string {
            input.expect_url_or_string()?
        } else {
            input.expect_url()?
        };
        let loc = start.source_location().into();
        let raw = input.slice_from(start.position());
        Ok(Self {
            url: value.into(),
            loc,
            span: content_span(loc, raw),
        })
    }
}

fn css_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\x0c')
}

/// The tokenizer has already validated the token and decoded its value. Walk only its
/// original spelling to locate delimiters, which can themselves be escaped URL contents.
fn content_span(loc: Location, raw: &str) -> SourceSpan {
    let value = if raw.starts_with(['\'', '"']) {
        raw
    } else {
        let (_, arguments) = raw.split_once('(').expect("parsed url() function");
        arguments.trim_start_matches(css_whitespace)
    };
    let quote = value.chars().next().filter(|ch| matches!(ch, '\'' | '"'));
    let value = &value[usize::from(quote.is_some())..];
    let (start, _) = advance(loc, &raw[..raw.len() - value.len()]);

    let mut chars = value.char_indices().peekable();
    let mut len = value.len();
    while let Some((index, ch)) = chars.next() {
        if ch == '\\' {
            if chars.next_if(|(_, ch)| ch.is_ascii_hexdigit()).is_some() {
                for _ in 1..6 {
                    if chars.next_if(|(_, ch)| ch.is_ascii_hexdigit()).is_none() {
                        break;
                    }
                }
                if matches!(
                    chars.next_if(|(_, ch)| css_whitespace(*ch)),
                    Some((_, '\r'))
                ) {
                    chars.next_if(|(_, ch)| *ch == '\n');
                }
            } else {
                chars.next();
            }
        } else if Some(ch) == quote || (quote.is_none() && (ch == ')' || css_whitespace(ch))) {
            len = index;
            break;
        }
    }
    let (_, end) = advance(start, &value[..len]);
    SourceSpan { start, end }
}

/// Returns the cursor after the text and the inclusive position of its last UTF-16 code unit.
fn advance(mut cursor: Location, text: &str) -> (Location, Location) {
    let mut end = Location {
        column: cursor.column - 1,
        ..cursor
    };
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        end = cursor;
        match ch {
            '\r' | '\n' | '\x0c' => {
                if ch == '\r' && chars.next_if_eq(&'\n').is_some() {
                    end.column += 1;
                }
                cursor.line += 1;
                cursor.column = 1;
            }
            _ => {
                cursor.column += ch.len_utf16() as u32;
                end.column = cursor.column - 1;
            }
        }
    }
    (cursor, end)
}

impl<'i> ToCss for Url<'i> {
    fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        let dep = if dest.dependencies.is_some() {
            Some(UrlDependency::new(self, dest.filename()))
        } else {
            None
        };

        // If adding dependencies, always write url() with quotes so that the placeholder can
        // be replaced without escaping more easily. Quotes may be removed later during minification.
        if let Some(dep) = dep {
            dest.write_str("url(")?;
            serialize_string(&dep.placeholder, dest)?;
            dest.write_char(')')?;

            if let Some(dependencies) = &mut dest.dependencies {
                dependencies.push(Dependency::Url(dep))
            }

            return Ok(());
        }

        use cssparser::ToCss;
        if dest.minify {
            let mut buf = String::new();
            Token::UnquotedUrl(CowRcStr::from(self.url.as_ref())).to_css(&mut buf)?;

            // If the unquoted url is longer than it would be quoted (e.g. `url("...")`)
            // then serialize as a string and choose the shorter version.
            if buf.len() > self.url.len() + 7 {
                let mut buf2 = String::new();
                serialize_string(&self.url, &mut buf2)?;
                if buf2.len() + 5 < buf.len() {
                    dest.write_str("url(")?;
                    dest.write_str(&buf2)?;
                    return dest.write_char(')');
                }
            }

            dest.write_str(&buf)?;
        } else {
            dest.write_str("url(")?;
            serialize_string(&self.url, dest)?;
            dest.write_char(')')?;
        }

        Ok(())
    }
}

impl<'i> Url<'i> {
    /// Returns whether the URL is absolute, and not relative.
    pub fn is_absolute(&self) -> bool {
        let url = self.url.as_ref();

        // Quick checks. If the url starts with '.', it is relative.
        if url.starts_with('.') {
            return false;
        }

        // If the url starts with '/' it is absolute.
        if url.starts_with('/') {
            return true;
        }

        // If the url starts with '#' we have a fragment URL.
        // These are resolved relative to the document rather than the CSS file.
        // https://drafts.csswg.org/css-values-4/#local-urls
        if url.starts_with('#') {
            return true;
        }

        // Otherwise, we might have a scheme. These must start with an ascii alpha character.
        // https://url.spec.whatwg.org/#scheme-start-state
        if !url.starts_with(|c| matches!(c, 'a'..='z' | 'A'..='Z')) {
            return false;
        }

        // https://url.spec.whatwg.org/#scheme-state
        for b in url.as_bytes() {
            let c = *b as char;
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '-' | '.' => {}
                ':' => return true,
                _ => break,
            }
        }

        false
    }
}
