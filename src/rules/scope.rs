//! The `@scope` rule.

use super::Location;
use super::{CssRuleList, MinifyContext, StyleContext};
use crate::error::{MinifyError, PrinterError};
use crate::parser::DefaultAtRule;
use crate::printer::Printer;
use crate::selector::{SelectorList, is_pure_css_modules_selector, to_css_with_source};
use crate::traits::ToCss;

/// A [@scope](https://drafts.csswg.org/css-cascade-6/#scope-atrule) rule.
///
/// @scope (<scope-start>) [to (<scope-end>)]? {
///  <stylesheet>
/// }
#[derive(Debug, PartialEq, Clone)]
pub struct ScopeRule<'i, R = DefaultAtRule> {
    /// A selector list used to identify the scoping root(s).
    pub scope_start: Option<SelectorList<'i>>,
    /// A selector list used to identify any scoping limits.
    pub scope_end: Option<SelectorList<'i>>,
    /// Nested rules within the `@scope` rule.
    pub rules: CssRuleList<'i, R>,
    /// The location of the rule in the source file.
    pub loc: Location,
}

impl<'i, T: Clone> ScopeRule<'i, T> {
    pub(crate) fn minify(
        &mut self,
        context: &mut MinifyContext<'_, 'i>,
    ) -> Result<(), MinifyError> {
        if context.pure_css_modules {
            if let Some(scope_start) = &self.scope_start {
                if !scope_start.0.iter().all(is_pure_css_modules_selector) {
                    return Err(MinifyError {
                        kind: crate::error::MinifyErrorKind::ImpureCSSModuleSelector,
                        loc: self.loc,
                    });
                }
            }

            if let Some(scope_end) = &self.scope_end {
                if !scope_end.0.iter().all(is_pure_css_modules_selector) {
                    return Err(MinifyError {
                        kind: crate::error::MinifyErrorKind::ImpureCSSModuleSelector,
                        loc: self.loc,
                    });
                }
            }
        }

        self.rules.minify(context, false)
    }
}

impl<'i, T: ToCss> ToCss for ScopeRule<'i, T> {
    fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        self.to_css_with_context(dest, None)
    }
}

impl<'i, T: ToCss> ScopeRule<'i, T> {
    pub(crate) fn to_css_with_context<'a, W>(
        &self,
        dest: &mut Printer<W>,
        context: Option<&'a StyleContext<'a, 'i>>,
    ) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        dest.write_str("@scope")?;
        dest.whitespace()?;
        if let Some(scope_start) = &self.scope_start {
            dest.write_char('(')?;
            scope_start.to_css(dest)?;
            dest.write_char(')')?;
            dest.whitespace()?;
        }
        if let Some(scope_end) = &self.scope_end {
            if dest.minify {
                dest.write_char(' ')?;
            }
            dest.write_str("to (")?;
            // <scope-start> is treated as an ancestor of scope end.
            // https://drafts.csswg.org/css-nesting/#nesting-at-scope
            if let Some(scope_start) = &self.scope_start {
                let scope_context = StyleContext {
                    selectors: scope_start,
                    selector_source: None,
                    parent: context,
                };
                to_css_with_source(scope_end, dest, Some(&scope_context), None)?;
            } else {
                scope_end.to_css(dest)?;
            }
            dest.write_char(')')?;
            dest.whitespace()?;
        }
        dest.write_char('{')?;
        dest.indent();
        dest.newline()?;
        // Nested style rules within @scope are implicitly relative to the <scope-start>
        // so clear our style context while printing them to avoid replacing & ourselves.
        // https://drafts.csswg.org/css-cascade-6/#scoped-rules
        self.rules.to_css_with_context(dest, None)?;
        dest.dedent();
        dest.newline()?;
        dest.write_char('}')
    }
}
