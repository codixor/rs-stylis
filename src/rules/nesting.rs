//! The `@nest` rule.

use crate::smallvec_compat::SmallVec;

use super::Location;
use super::MinifyContext;
use super::StyleContext;
use super::style::StyleRule;
use crate::context::DeclarationContext;
use crate::declaration::DeclarationBlock;
use crate::error::{MinifyError, PrinterError};
use crate::parser::DefaultAtRule;
use crate::printer::Printer;
use crate::selector::to_css_with_source;
use crate::targets::should_compile;
use crate::traits::ToCss;

/// A [@nest](https://www.w3.org/TR/css-nesting-1/#at-nest) rule.
#[derive(Debug, PartialEq, Clone)]
pub struct NestingRule<'i, R = DefaultAtRule> {
    /// The style rule that defines the selector and declarations for the `@nest` rule.
    pub style: StyleRule<'i, R>,
    /// The location of the rule in the source file.
    pub loc: Location,
}

impl<'i, T: Clone> NestingRule<'i, T> {
    pub(crate) fn minify(
        &mut self,
        context: &mut MinifyContext<'_, 'i>,
        parent_is_unused: bool,
    ) -> Result<bool, MinifyError> {
        self.style.minify(context, parent_is_unused)
    }
}

impl<'a, 'i, T: ToCss> ToCss for NestingRule<'i, T> {
    fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        self.to_css_with_context(dest, None)
    }
}

impl<'i, T: ToCss> NestingRule<'i, T> {
    pub(crate) fn to_css_with_context<'a, W>(
        &self,
        dest: &mut Printer<W>,
        context: Option<&'a StyleContext<'a, 'i>>,
    ) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        if context.is_none() {
            dest.write_str("@nest ")?;
        }
        self.style.to_css_with_context(dest, context)
    }
}

/// A [nested declarations](https://drafts.csswg.org/css-nesting/#nested-declarations-rule) rule.
#[derive(Debug, PartialEq, Clone)]
pub struct NestedDeclarationsRule<'i> {
    /// The style rule that defines the selector and declarations for the `@nest` rule.
    pub declarations: DeclarationBlock<'i>,
    /// The location of the rule in the source file.
    pub loc: Location,
}

impl<'i> NestedDeclarationsRule<'i> {
    pub(crate) fn minify(
        &mut self,
        context: &mut MinifyContext<'_, 'i>,
        parent_is_unused: bool,
    ) -> bool {
        if parent_is_unused {
            return true;
        }

        context.handler_context.context = DeclarationContext::StyleRule;
        self.declarations.minify(
            context.handler,
            context.important_handler,
            &mut context.handler_context,
            context.react_compat,
        );
        context.handler_context.context = DeclarationContext::None;
        return false;
    }
}

impl<'i> ToCss for NestedDeclarationsRule<'i> {
    fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        self.to_css_with_context(dest, None)
    }
}

impl<'i> NestedDeclarationsRule<'i> {
    pub(crate) fn to_css_with_context<'a, W>(
        &self,
        dest: &mut Printer<W>,
        context: Option<&'a StyleContext<'a, 'i>>,
    ) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        if should_compile!(dest.targets.current, Nesting) {
            if let Some(context) = context {
                let has_printable_declarations = self.declarations.has_printable_declarations();
                if has_printable_declarations {
                    to_css_with_source(
                        context.selectors,
                        dest,
                        context.parent,
                        context.selector_source,
                    )?;
                    dest.whitespace()?;
                    dest.write_char('{')?;
                    dest.indent();
                    dest.newline()?;
                }

                self.declarations.to_css_declarations(
                    dest,
                    false,
                    context.selectors,
                    self.loc.source_index,
                )?;

                if has_printable_declarations {
                    dest.dedent();
                    dest.newline()?;
                    dest.write_char('}')?;
                }
                return Ok(());
            }
        }

        self.declarations.to_css_declarations(
            dest,
            false,
            &crate::parcel_selectors::SelectorList(SmallVec::new()),
            0,
        )
    }
}
