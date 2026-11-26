//! Types used to represent strings.

use crate::traits::{Parse, ToCss};
use cssparser::{CowRcStr, serialize_string};
use std::borrow::{Borrow, Cow};
use std::cmp;
use std::fmt;
use std::hash;
use std::ops::Deref;
use std::str;

#[inline]
fn can_serialize_simple_quoted_string(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| matches!(byte, b' '..=b'~') && byte != b'"' && byte != b'\\')
}

pub(crate) fn serialize_css_string<W>(value: &str, dest: &mut W) -> fmt::Result
where
    W: fmt::Write,
{
    if can_serialize_simple_quoted_string(value) {
        dest.write_char('"')?;
        dest.write_str(value)?;
        return dest.write_char('"');
    }

    serialize_string(value, dest)
}

pub(crate) fn serialize_react_compat_string<W>(value: &str, dest: &mut W) -> fmt::Result
where
    W: fmt::Write,
{
    use fmt::Write;

    dest.write_char('"')?;
    let mut parts = value.split('\u{a0}');
    cssparser::CssStringWriter::new(dest).write_str(parts.next().unwrap_or_default())?;
    for part in parts {
        dest.write_str("\\a0")?;
        // A hex digit would extend the escape, and CSS consumes a following space.
        if part.starts_with(|ch: char| ch.is_ascii_hexdigit() || ch == ' ') {
            dest.write_char(' ')?;
        }
        cssparser::CssStringWriter::new(dest).write_str(part)?;
    }
    dest.write_char('"')
}

/// A string that is either owned or borrowed from the input CSS source code.
///
/// The historical `CowArcStr` name is kept for API compatibility, but the
/// representation is now a fully safe `Cow<'a, str>`. Conversions from
/// `cssparser::CowRcStr` materialize owned strings so the wider AST remains
/// `Send`/`Sync` friendly for existing bench/example surfaces.
#[derive(Clone)]
pub struct CowArcStr<'a>(Cow<'a, str>);

impl<'a> From<CowRcStr<'a>> for CowArcStr<'a> {
    #[inline]
    fn from(s: CowRcStr<'a>) -> Self {
        CowArcStr(Cow::Owned(s.to_string()))
    }
}

impl<'a> From<&CowRcStr<'a>> for CowArcStr<'a> {
    #[inline]
    fn from(s: &CowRcStr<'a>) -> Self {
        CowArcStr(Cow::Owned(s.to_string()))
    }
}

impl<'a> From<&'a str> for CowArcStr<'a> {
    #[inline]
    fn from(s: &'a str) -> Self {
        CowArcStr(Cow::Borrowed(s))
    }
}

impl<'a> From<String> for CowArcStr<'a> {
    #[inline]
    fn from(s: String) -> Self {
        CowArcStr(Cow::Owned(s))
    }
}

impl<'a> From<Cow<'a, str>> for CowArcStr<'a> {
    #[inline]
    fn from(s: Cow<'a, str>) -> Self {
        CowArcStr(s)
    }
}

impl<'a> Deref for CowArcStr<'a> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        &self.0
    }
}

impl<'a> AsRef<str> for CowArcStr<'a> {
    #[inline]
    fn as_ref(&self) -> &str {
        self
    }
}

impl<'a> Borrow<str> for CowArcStr<'a> {
    #[inline]
    fn borrow(&self) -> &str {
        self
    }
}

impl<'a> Default for CowArcStr<'a> {
    #[inline]
    fn default() -> Self {
        Self::from("")
    }
}

impl<'a> hash::Hash for CowArcStr<'a> {
    #[inline]
    fn hash<H: hash::Hasher>(&self, hasher: &mut H) {
        str::hash(self, hasher)
    }
}

impl<'a, T: AsRef<str>> PartialEq<T> for CowArcStr<'a> {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        str::eq(self, other.as_ref())
    }
}

impl<'a, T: AsRef<str>> PartialOrd<T> for CowArcStr<'a> {
    #[inline]
    fn partial_cmp(&self, other: &T) -> Option<cmp::Ordering> {
        str::partial_cmp(self, other.as_ref())
    }
}

impl<'a> Eq for CowArcStr<'a> {}

impl<'a> Ord for CowArcStr<'a> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        str::cmp(self, other)
    }
}

impl<'a> fmt::Display for CowArcStr<'a> {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        str::fmt(self, formatter)
    }
}

impl<'a> fmt::Debug for CowArcStr<'a> {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        str::fmt(self, formatter)
    }
}

/// A quoted CSS string.
#[derive(Clone, Eq, Ord, Hash, Debug)]
pub struct CSSString<'i>(pub CowArcStr<'i>);

impl<'i> Parse<'i> for CSSString<'i> {
    fn parse<'t>(
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, cssparser::ParseError<'i, crate::error::ParserError<'i>>> {
        let s = input.expect_string()?;
        Ok(CSSString(s.into()))
    }
}

impl<'i> ToCss for CSSString<'i> {
    fn to_css<W>(
        &self,
        dest: &mut crate::printer::Printer<W>,
    ) -> Result<(), crate::error::PrinterError>
    where
        W: std::fmt::Write,
    {
        if dest.is_react_compat() {
            serialize_react_compat_string(&self.0, dest)?;
            return Ok(());
        }

        serialize_css_string(&self.0, dest)?;
        Ok(())
    }
}

impl<'i> cssparser::ToCss for CSSString<'i> {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        serialize_react_compat_string(&self.0, dest)
    }
}

macro_rules! impl_string_type {
    ($t: ident) => {
        impl<'i> From<CowRcStr<'i>> for $t<'i> {
            fn from(s: CowRcStr<'i>) -> Self {
                $t(s.into())
            }
        }

        impl<'a> From<&CowRcStr<'a>> for $t<'a> {
            fn from(s: &CowRcStr<'a>) -> Self {
                $t(s.into())
            }
        }

        impl<'i> From<String> for $t<'i> {
            fn from(s: String) -> Self {
                $t(s.into())
            }
        }

        impl<'i> From<&'i str> for $t<'i> {
            fn from(s: &'i str) -> Self {
                $t(s.into())
            }
        }

        impl<'a> From<std::borrow::Cow<'a, str>> for $t<'a> {
            #[inline]
            fn from(s: std::borrow::Cow<'a, str>) -> Self {
                match s {
                    std::borrow::Cow::Borrowed(s) => s.into(),
                    std::borrow::Cow::Owned(s) => s.into(),
                }
            }
        }

        impl<'a> Deref for $t<'a> {
            type Target = str;

            #[inline]
            fn deref(&self) -> &str {
                self.0.deref()
            }
        }

        impl<'a> AsRef<str> for $t<'a> {
            #[inline]
            fn as_ref(&self) -> &str {
                self
            }
        }

        impl<'a> Borrow<str> for $t<'a> {
            #[inline]
            fn borrow(&self) -> &str {
                self
            }
        }

        impl<'a> std::fmt::Display for $t<'a> {
            #[inline]
            fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                str::fmt(self, formatter)
            }
        }

        impl<'a, T: AsRef<str>> PartialEq<T> for $t<'a> {
            #[inline]
            fn eq(&self, other: &T) -> bool {
                str::eq(self, other.as_ref())
            }
        }

        impl<'a, T: AsRef<str>> PartialOrd<T> for $t<'a> {
            #[inline]
            fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
                str::partial_cmp(self, other.as_ref())
            }
        }
    };
}

impl_string_type!(CSSString);

pub(crate) use impl_string_type;
