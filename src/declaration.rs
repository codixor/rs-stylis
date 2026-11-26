//! CSS declarations.

use std::borrow::Cow;
use std::ops::Range;

use crate::context::{DeclarationContext, PropertyHandlerContext};
use crate::error::{ParserError, PrinterError, PrinterErrorKind};
use crate::parser::ParserOptions;
use crate::printer::Printer;
use crate::properties::box_shadow::BoxShadowHandler;
use crate::properties::custom::{CustomProperty, CustomPropertyName};
use crate::properties::masking::MaskHandler;
use crate::properties::text::{Direction, UnicodeBidi};
use crate::properties::{Property, PropertyId};
use crate::properties::{
    align::AlignHandler,
    animation::AnimationHandler,
    background::BackgroundHandler,
    border::BorderHandler,
    contain::ContainerHandler,
    display::DisplayHandler,
    flex::FlexHandler,
    font::FontHandler,
    grid::GridHandler,
    list::ListStyleHandler,
    margin_padding::*,
    outline::OutlineHandler,
    overflow::OverflowHandler,
    position::PositionHandler,
    prefix_handler::{FallbackHandler, PrefixHandler},
    size::SizeHandler,
    text::TextDecorationHandler,
    transform::TransformHandler,
    transition::TransitionHandler,
    ui::ColorSchemeHandler,
};
use crate::selector::SelectorList;
use crate::smallvec_compat::SmallVec;
use crate::targets::Features;
use crate::traits::{PropertyHandler, ToCss};
use crate::values::ident::DashedIdent;
use crate::values::string::CowArcStr;
use cssparser::*;
use indexmap::IndexMap;

struct ReactDeclaration {
    name: String,
    value: String,
    important: bool,
}

/// A CSS declaration block.
///
/// Properties are separated into a list of `!important` declararations,
/// and a list of normal declarations. This reduces memory usage compared
/// with storing a boolean along with each property.
#[derive(Debug, PartialEq, Clone, Default)]
pub struct DeclarationBlock<'i> {
    /// A list of `!important` declarations in the block.
    pub important_declarations: Vec<Property<'i>>,
    /// A list of normal declarations in the block.
    pub declarations: Vec<Property<'i>>,
    /// Parse-time declaration order. `false` means normal, `true` means important.
    pub source_order: Vec<bool>,
    /// Authored declaration text in parse order, without `!important`.
    pub authored_declarations: Vec<String>,
    /// Whether React-compatible minification requested vendor prefix expansion.
    pub(crate) react_compat_prefixes: Option<bool>,
}

impl<'i> DeclarationBlock<'i> {
    /// Parses a declaration block from CSS syntax.
    pub fn parse<'a, 'o, 't>(
        input: &mut Parser<'i, 't>,
        options: &'a ParserOptions<'o, 'i>,
    ) -> Result<Self, ParseError<'i, ParserError<'i>>> {
        let mut important_declarations = DeclarationList::new();
        let mut declarations = DeclarationList::new();
        let mut source_order = Vec::new();
        let mut authored_declarations = Vec::new();
        let mut decl_parser = PropertyDeclarationParser {
            important_declarations: &mut important_declarations,
            declarations: &mut declarations,
            source_order: &mut source_order,
            authored_declarations: &mut authored_declarations,
            options,
        };
        let mut parser = RuleBodyParser::new(input, &mut decl_parser);
        while let Some(res) = parser.next() {
            if let Err((err, _)) = res {
                if options.error_recovery {
                    options.warn(err);
                    continue;
                }
                return Err(err);
            }
        }

        Ok(DeclarationBlock {
            important_declarations,
            declarations,
            source_order,
            authored_declarations,
            react_compat_prefixes: None,
        })
    }

    /// Parses a declaration block from a string.
    pub fn parse_string<'o>(
        input: &'i str,
        options: ParserOptions<'o, 'i>,
    ) -> Result<Self, ParseError<'i, ParserError<'i>>> {
        let mut input = ParserInput::new(input);
        let mut parser = Parser::new(&mut input);
        let result = Self::parse(&mut parser, &options)?;
        parser.expect_exhausted()?;
        Ok(result)
    }

    /// Returns an empty declaration block.
    pub fn new() -> Self {
        Self {
            declarations: vec![],
            important_declarations: vec![],
            source_order: vec![],
            authored_declarations: vec![],
            react_compat_prefixes: None,
        }
    }

    /// Returns the total number of declarations in the block.
    pub fn len(&self) -> usize {
        self.declarations.len() + self.important_declarations.len()
    }
}

impl<'i> ToCss for DeclarationBlock<'i> {
    fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        if dest.is_react_compat()
            && let Some(prefixes) = self.react_compat_prefixes
        {
            return self.write_react_compat_declarations(dest, prefixes, None, false);
        }

        let len = self.declarations.len() + self.important_declarations.len();
        let mut i = 0usize;

        if dest.is_react_compat() && self.has_valid_source_order() {
            let mut normal = self.declarations.iter();
            let mut important = self.important_declarations.iter();

            for is_important in &self.source_order {
                let decl = if *is_important {
                    important
                        .next()
                        .expect("important declaration order mismatch")
                } else {
                    normal.next().expect("normal declaration order mismatch")
                };
                decl.to_css(dest, *is_important)?;
                if i != len - 1 || dest.preserve_trailing_semicolon() {
                    dest.write_char(';')?;
                    if i != len - 1 {
                        dest.whitespace()?;
                    }
                }
                i += 1;
            }
            return Ok(());
        }

        macro_rules! write {
            ($decls: expr, $important: literal) => {
                for decl in &$decls {
                    decl.to_css(dest, $important)?;
                    if i != len - 1 || dest.preserve_trailing_semicolon() {
                        dest.write_char(';')?;
                        if i != len - 1 {
                            dest.whitespace()?;
                        }
                    }
                    i += 1;
                }
            };
        }

        write!(self.declarations, false);
        write!(self.important_declarations, true);
        Ok(())
    }
}

impl<'i> DeclarationBlock<'i> {
    fn write_react_compat_declarations<W>(
        &self,
        dest: &mut Printer<W>,
        prefixes: bool,
        rule: Option<(&SelectorList, u32)>,
        has_nested_rules: bool,
    ) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        let mut source = Vec::with_capacity(self.len());
        for (index, (property, important)) in self.ordered_properties().into_iter().enumerate() {
            if let Some((selectors, source_index)) = rule
                && handle_composes(property, dest, selectors, source_index)?
            {
                continue;
            }

            let value = dest.declaration_text(|printer| {
                if let Property::Unparsed(unparsed) = property
                    && matches!(
                        unparsed.property_id,
                        PropertyId::Animation(_) | PropertyId::AnimationName(_)
                    )
                    && printer
                        .css_module
                        .as_ref()
                        .is_some_and(|m| m.config.animation)
                {
                    let mut value = unparsed.value.clone();
                    crate::properties::animation::mark_animation_names(
                        &mut value,
                        matches!(unparsed.property_id, PropertyId::Animation(_)),
                    );
                    return value.to_css(printer, false);
                }
                property.value_to_css(printer)
            })?;
            let authored = self.authored_declarations.get(index).map(String::as_str);
            if let Property::Custom(custom) = property {
                let name = dest.declaration_text(|printer| custom.name.to_css(printer))?;
                source.push(ReactDeclaration::new(
                    name, value, important, authored, dest,
                ));
            } else {
                let id = property.property_id();
                for prefix in id.prefix().or_none() {
                    let mut name = cssparser::ToCss::to_css_string(&prefix);
                    name.push_str(id.name());
                    source.push(ReactDeclaration::new(
                        name,
                        value.clone(),
                        important,
                        authored,
                        dest,
                    ));
                }
            }
        }

        let mut first = true;
        for (index, decl) in source.iter().enumerate() {
            let declarations = if prefixes {
                expand_react_compat_declaration(&source, index)
            } else {
                vec![emit_raw_declaration(&decl.name, &decl.value)]
            };
            for text in declarations {
                if !first {
                    if has_nested_rules {
                        dest.newline()?;
                    } else {
                        dest.whitespace()?;
                    }
                }
                first = false;
                dest.write_str_with_newlines(&text)?;
                if decl.important {
                    dest.whitespace()?;
                    dest.write_str("!important")?;
                }
                dest.write_char(';')?;
            }
        }

        Ok(())
    }

    /// Writes the declarations to a CSS block, including starting and ending braces.
    pub fn to_css_block<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        dest.whitespace()?;
        dest.write_char('{')?;
        dest.indent();
        dest.newline()?;

        self.to_css_declarations(
            dest,
            false,
            &crate::parcel_selectors::SelectorList(SmallVec::new()),
            0,
        )?;

        dest.dedent();
        dest.newline()?;
        dest.write_char('}')
    }

    pub(crate) fn has_printable_declarations(&self) -> bool {
        if self.len() > 1 {
            return true;
        }

        if self.declarations.len() == 1 {
            !matches!(
                self.declarations[0],
                crate::properties::Property::Composes(_)
            )
        } else if self.important_declarations.len() == 1 {
            !matches!(
                self.important_declarations[0],
                crate::properties::Property::Composes(_)
            )
        } else {
            false
        }
    }

    /// Writes the declarations to a CSS declaration block.
    pub fn to_css_declarations<W>(
        &self,
        dest: &mut Printer<W>,
        has_nested_rules: bool,
        selectors: &SelectorList,
        source_index: u32,
    ) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        if dest.is_react_compat()
            && let Some(prefixes) = self.react_compat_prefixes
        {
            return self.write_react_compat_declarations(
                dest,
                prefixes,
                Some((selectors, source_index)),
                has_nested_rules,
            );
        }

        let mut i = 0;
        let len = self.len();

        if dest.is_react_compat() && self.has_valid_source_order() {
            let mut normal = self.declarations.iter();
            let mut important = self.important_declarations.iter();

            for is_important in &self.source_order {
                let decl = if *is_important {
                    important
                        .next()
                        .expect("important declaration order mismatch")
                } else {
                    normal.next().expect("normal declaration order mismatch")
                };

                if handle_composes(decl, dest, selectors, source_index)? {
                    continue;
                }

                if i > 0 {
                    dest.newline()?;
                }

                decl.to_css(dest, *is_important)?;
                if i != len - 1
                    || !dest.minify
                    || has_nested_rules
                    || dest.preserve_trailing_semicolon()
                {
                    dest.write_char(';')?;
                }

                i += 1;
            }

            return Ok(());
        }

        macro_rules! write {
      ($decls: expr, $important: literal) => {
        for decl in &$decls {
          // The CSS modules `composes` property is handled specially, and omitted during printing.
          // We need to add the classes it references to the list for the selectors in this rule.
          if handle_composes(decl, dest, selectors, source_index)? {
            continue;
          }

          if i > 0 {
            dest.newline()?;
          }

          decl.to_css(dest, $important)?;
          if i != len - 1
            || !dest.minify
            || has_nested_rules
            || dest.preserve_trailing_semicolon()
          {
            dest.write_char(';')?;
          }

          i += 1;
        }
      };
    }

        write!(self.declarations, false);
        write!(self.important_declarations, true);
        Ok(())
    }
}

impl<'i> DeclarationBlock<'i> {
    pub(crate) fn minify(
        &mut self,
        handler: &mut DeclarationHandler<'i>,
        important_handler: &mut DeclarationHandler<'i>,
        context: &mut PropertyHandlerContext<'i, '_>,
        react_compat: bool,
    ) {
        self.react_compat_prefixes =
            react_compat.then(|| context.targets.include.contains(Features::VendorPrefixes));
        if react_compat {
            return;
        }

        handler.decls.reserve(self.declarations.len());
        important_handler
            .decls
            .reserve(self.important_declarations.len());

        macro_rules! handle {
            ($decls: expr, $handler: expr, $important: literal) => {
                for decl in $decls.iter() {
                    context.is_important = $important;
                    let handled = $handler.handle_property(decl, context);

                    if !handled {
                        $handler.decls.push(decl.clone());
                    }
                }
            };
        }

        handle!(self.important_declarations, important_handler, true);
        handle!(self.declarations, handler, false);

        handler.finalize(context);
        important_handler.finalize(context);
        self.important_declarations = std::mem::take(&mut important_handler.decls);
        self.declarations = std::mem::take(&mut handler.decls);
    }

    fn has_valid_source_order(&self) -> bool {
        self.source_order.len() == self.len()
            && self
                .source_order
                .iter()
                .filter(|&&important| important)
                .count()
                == self.important_declarations.len()
    }

    fn ordered_properties(&self) -> Vec<(&Property<'i>, bool)> {
        let len = self.len();
        let mut ordered = Vec::with_capacity(len);
        if self.has_valid_source_order() {
            let mut normal = self.declarations.iter();
            let mut important = self.important_declarations.iter();
            for is_important in &self.source_order {
                if *is_important {
                    if let Some(property) = important.next() {
                        ordered.push((property, true));
                    }
                } else if let Some(property) = normal.next() {
                    ordered.push((property, false));
                }
            }
            return ordered;
        }

        ordered.extend(self.declarations.iter().map(|property| (property, false)));
        ordered.extend(
            self.important_declarations
                .iter()
                .map(|property| (property, true)),
        );
        ordered
    }

    /// Returns whether the declaration block is empty.
    pub fn is_empty(&self) -> bool {
        return self.declarations.is_empty() && self.important_declarations.is_empty();
    }

    pub(crate) fn property_location<'t>(
        &self,
        input: &mut Parser<'i, 't>,
        index: usize,
    ) -> Result<(Range<SourceLocation>, Range<SourceLocation>), ParseError<'i, ParserError<'i>>>
    {
        // Skip to the requested property index.
        for _ in 0..index {
            input.expect_ident()?;
            input.expect_colon()?;
            input.parse_until_after(Delimiter::Semicolon, |parser| {
                while parser.next().is_ok() {}
                Ok(())
            })?;
        }

        // Get property name range.
        input.skip_whitespace();
        let key_start = input.current_source_location();
        input.expect_ident()?;
        let key_end = input.current_source_location();
        let key_range = key_start..key_end;

        input.expect_colon()?;
        input.skip_whitespace();

        // Get value range.
        let val_start = input.current_source_location();
        input.parse_until_before(Delimiter::Semicolon, |parser| {
            while parser.next().is_ok() {}
            Ok(())
        })?;
        let val_end = input.current_source_location();
        let val_range = val_start..val_end;

        Ok((key_range, val_range))
    }
}

impl ReactDeclaration {
    fn new<W: std::fmt::Write>(
        name: String,
        value: String,
        important: bool,
        authored: Option<&str>,
        dest: &Printer<W>,
    ) -> Self {
        // Keep the authored spelling when it serializes to the same value as the current AST.
        // This also excludes spellings that precede CSS Modules or URL placeholder rewrites.
        if let Some((raw_name, raw_value)) = authored.and_then(|text| text.split_once(':'))
            && (raw_name == name
                || (!name.starts_with("--") && raw_name.eq_ignore_ascii_case(&name)))
        {
            let normalized = minify_react_compat_authored_value(raw_value);
            if normalized == value
                || Self::typed_value(raw_name, &normalized, dest).as_ref() == Some(&value)
            {
                return Self {
                    name: raw_name.to_owned(),
                    value: normalized,
                    important,
                };
            }
        }
        Self {
            name,
            value,
            important,
        }
    }

    fn typed_value<W: std::fmt::Write>(
        name: &str,
        value: &str,
        dest: &Printer<W>,
    ) -> Option<String> {
        let mut input = ParserInput::new(value);
        let mut parser = Parser::new(&mut input);
        let id = PropertyId::from(CowArcStr::from(name));
        let property = parser
            .parse_entirely(|p| Property::parse(id, p, &ParserOptions::default()))
            .ok()?;
        let mut css = String::new();
        property
            .value_to_css(&mut dest.declaration_printer(&mut css))
            .ok()?;
        Some(css)
    }
}

fn handle_composes<W: std::fmt::Write>(
    property: &Property,
    dest: &mut Printer<W>,
    selectors: &SelectorList,
    source_index: u32,
) -> Result<bool, PrinterError> {
    if let Property::Composes(composes) = property {
        let nested = dest.is_nested();
        if let Some(module) = &mut dest.css_module {
            if nested {
                return Err(dest.error(PrinterErrorKind::InvalidComposesNesting, composes.loc));
            }
            let source_index = if module.sources.len() == 1 {
                0
            } else {
                source_index
            };
            module
                .handle_composes(selectors, composes, source_index)
                .map_err(|e| dest.error(e, composes.loc))?;
            return Ok(true);
        }
    }
    Ok(false)
}

fn emit_raw_declaration(name: &str, value: &str) -> String {
    format!("{name}:{value}")
}

fn emit_prefixed_declaration(prefix: &str, name: &str, value: &str) -> String {
    emit_raw_declaration(&format!("-{prefix}-{name}"), value)
}

fn minify_react_compat_authored_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    let mut pending_space = false;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if in_single_quote || in_double_quote {
            out.push(ch);
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if in_single_quote && ch == '\'' {
                in_single_quote = false;
            } else if in_double_quote && ch == '"' {
                in_double_quote = false;
            }
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            pending_space = true;
            let mut prev = '\0';
            for next in chars.by_ref() {
                if prev == '*' && next == '/' {
                    break;
                }
                prev = next;
            }
            continue;
        }

        if ch.is_ascii_whitespace() {
            pending_space = true;
            continue;
        }

        if pending_space && !out.is_empty() {
            out.push(' ');
        }
        pending_space = false;

        out.push(ch);
        if ch == '\'' {
            in_single_quote = true;
        } else if ch == '"' {
            in_double_quote = true;
        }
    }

    out
}

fn is_webkit_duplicate_property(name: &str) -> bool {
    matches!(
        name,
        "animation"
            | "animation-delay"
            | "animation-direction"
            | "animation-duration"
            | "animation-fill-mode"
            | "animation-iteration-count"
            | "animation-name"
            | "animation-play-state"
            | "animation-timing-function"
            | "text-decoration"
            | "filter"
            | "clip-path"
            | "backface-visibility"
            | "column"
            | "box-decoration-break"
            | "background-clip"
            | "columns"
            | "column-count"
            | "column-fill"
            | "column-gap"
            | "column-rule"
            | "column-rule-color"
            | "column-rule-style"
            | "column-rule-width"
            | "column-span"
            | "column-width"
            | "mask"
            | "mask-image"
            | "mask-mode"
            | "mask-clip"
            | "mask-size"
            | "mask-repeat"
            | "mask-origin"
            | "mask-position"
    )
}

fn is_size_keyword_property(name: &str) -> bool {
    matches!(
        name,
        "width"
            | "height"
            | "min-width"
            | "min-height"
            | "max-width"
            | "max-height"
            | "inline-size"
            | "block-size"
            | "min-inline-size"
            | "min-block-size"
            | "max-inline-size"
            | "max-block-size"
    )
}

fn is_scroll_margin_property(name: &str) -> bool {
    matches!(
        name,
        "scroll-margin"
            | "scroll-margin-top"
            | "scroll-margin-right"
            | "scroll-margin-bottom"
            | "scroll-margin-left"
    )
}

fn webkit_image_set_value(value: &str) -> Option<String> {
    value
        .contains("image-set(")
        .then(|| value.replacen("image-set(", "-webkit-image-set(", 1))
}

fn webkit_transform_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 16);
    let mut index = 0usize;
    while let Some(offset) = value[index..].find("transform") {
        let found = index + offset;
        if found > 0 && value.as_bytes()[found - 1] == b'-' {
            out.push_str(&value[index..found + "transform".len()]);
            index = found + "transform".len();
            continue;
        }

        out.push_str(&value[index..found]);
        out.push_str("-webkit-transform");
        index = found + "transform".len();
    }
    out.push_str(&value[index..]);
    out
}

fn webkit_cursor_value(value: &str) -> String {
    let mut prefixed = value.replacen("zoom-", "-webkit-zoom-", 1);
    if prefixed == value {
        prefixed = value.replacen("grab", "-webkit-grab", 1);
    }
    prefixed.replacen("image-set", "-webkit-image-set", 1)
}

fn ms_writing_mode_value(value: &str) -> String {
    match value {
        "vertical-lr" => "tb".to_string(),
        "vertical-rl" => "tb-rl".to_string(),
        "horizontal-tb" => "lr".to_string(),
        _ => value.to_string(),
    }
}

fn grid_line_number(value: &str) -> Option<i32> {
    value.trim().parse::<i32>().ok()
}

fn grid_span_number(value: &str) -> Option<i32> {
    value
        .trim()
        .strip_prefix("span")
        .map(str::trim)
        .and_then(|number| number.parse::<i32>().ok())
        .or_else(|| value.trim().parse::<i32>().ok())
}

fn find_first_grid_pair<'a>(source: &'a [ReactDeclaration], suffix: &str) -> Option<&'a str> {
    source
        .iter()
        .find(|decl| decl.name.starts_with("grid-") && decl.name.ends_with(suffix))
        .map(|decl| decl.value.as_str())
}

fn expand_grid_start(
    name: &str,
    value: &str,
    source: &[ReactDeclaration],
    original: &str,
) -> Vec<String> {
    let Some(pair_value) = find_first_grid_pair(source, "-end") else {
        return vec![
            emit_raw_declaration(&format!("-ms-{}", name.replace("-start", "")), value),
            original.to_string(),
        ];
    };
    if pair_value.contains("span") {
        return vec![original.to_string()];
    }

    let Some(start) = grid_line_number(value) else {
        return vec![original.to_string()];
    };
    let Some(end) = grid_line_number(pair_value) else {
        return vec![original.to_string()];
    };
    let span = end - start;
    vec![
        emit_raw_declaration(&format!("-ms-{}", name.replace("-start", "")), value),
        original.to_string(),
        emit_raw_declaration("-ms-grid-row-span", &span.to_string()),
    ]
}

fn expand_grid_end(
    name: &str,
    value: &str,
    source: &[ReactDeclaration],
    original: &str,
) -> Vec<String> {
    if find_first_grid_pair(source, "-start").is_some() {
        return vec![original.to_string()];
    }

    let Some(span) = grid_span_number(value) else {
        return vec![original.to_string()];
    };
    vec![
        emit_raw_declaration(
            &format!("-ms-{}", name.replace("-end", "-span")),
            &span.to_string(),
        ),
        original.to_string(),
    ]
}

fn expand_grid_shorthand(name: &str, value: &str, original: &str) -> Vec<String> {
    let Some((start, end)) = value.split_once('/') else {
        return vec![original.to_string()];
    };
    let start = start.trim();
    let end = end.trim();
    let normalized_original = emit_raw_declaration(name, &format!("{start}/{end}"));
    let Some(start_num) = grid_line_number(start) else {
        return vec![normalized_original];
    };
    let span = if end.starts_with("span") {
        grid_span_number(end)
    } else {
        grid_line_number(end).map(|end_num| end_num - start_num)
    };
    let Some(span) = span else {
        return vec![normalized_original];
    };

    vec![
        emit_raw_declaration(&format!("-ms-{name}"), start),
        emit_raw_declaration(&format!("-ms-{name}-span"), &span.to_string()),
        normalized_original,
    ]
}

fn expand_size_keyword_declaration(name: &str, value: &str, original: &str) -> Vec<String> {
    let fallback_value = match value {
        "stretch" => "fill-available",
        "fill-available" => "fill-available",
        "max-content" => "max-content",
        "min-content" => "min-content",
        "fit-content" => "fit-content",
        _ => return vec![original.to_string()],
    };

    let moz_value = if fallback_value == "fill-available" {
        "available"
    } else {
        fallback_value
    };

    let mut decls = vec![
        emit_raw_declaration(name, &format!("-webkit-{fallback_value}")),
        emit_raw_declaration(name, &format!("-moz-{moz_value}")),
    ];
    if fallback_value != value {
        decls.push(emit_raw_declaration(name, fallback_value));
    }
    decls.push(original.to_string());
    decls
}

fn expand_react_compat_declaration(source: &[ReactDeclaration], index: usize) -> Vec<String> {
    let ReactDeclaration { name, value, .. } = &source[index];
    let original = &emit_raw_declaration(name, value);
    if name.starts_with('-') {
        return vec![original.clone()];
    }

    match name.as_str() {
        name if is_webkit_duplicate_property(name) => {
            vec![
                emit_prefixed_declaration("webkit", name, value),
                original.clone(),
            ]
        }
        "mask-composite" => {
            let webkit_value = value
                .replace("add", "source-over")
                .replace("substract", "source-out")
                .replace("intersect", "source-in")
                .replace("exclude", "xor");
            vec![
                emit_prefixed_declaration("webkit", name, &webkit_value),
                original.clone(),
            ]
        }
        "tab-size" => vec![
            emit_prefixed_declaration("moz", name, value),
            original.clone(),
        ],
        "appearance" | "user-select" | "transform" | "hyphens" | "text-size-adjust" => vec![
            emit_prefixed_declaration("webkit", name, value),
            emit_prefixed_declaration("moz", name, value),
            emit_prefixed_declaration("ms", name, value),
            original.clone(),
        ],
        "writing-mode" => vec![
            emit_prefixed_declaration("webkit", name, value),
            emit_prefixed_declaration("ms", name, &ms_writing_mode_value(value)),
            original.clone(),
        ],
        "flex" | "flex-direction" | "scroll-snap-type" => vec![
            emit_prefixed_declaration("webkit", name, value),
            emit_prefixed_declaration("ms", name, value),
            original.clone(),
        ],
        "order" => vec![
            emit_prefixed_declaration("webkit", name, value),
            emit_raw_declaration("-ms-flex-order", value),
            original.clone(),
        ],
        "align-items" => vec![
            emit_prefixed_declaration("webkit", name, value),
            emit_raw_declaration("-webkit-box-align", value),
            emit_raw_declaration("-ms-flex-align", value),
            original.clone(),
        ],
        "align-self" => {
            let ms_value = value.replacen("flex-", "", 1);
            let mut decls = vec![
                emit_prefixed_declaration("webkit", name, value),
                emit_raw_declaration("-ms-flex-item-align", &ms_value),
            ];
            if !value.contains("flex-") && !value.contains("baseline") {
                decls.push(emit_raw_declaration("-ms-grid-row-align", &ms_value));
            }
            decls.push(original.clone());
            decls
        }
        "align-content" => {
            let ms_value = value.replacen("flex-", "", 1);
            vec![
                emit_prefixed_declaration("webkit", name, value),
                emit_raw_declaration("-ms-flex-line-pack", &ms_value),
                original.clone(),
            ]
        }
        "flex-shrink" => vec![
            emit_prefixed_declaration("webkit", name, value),
            emit_raw_declaration("-ms-flex-negative", value),
            original.clone(),
        ],
        "flex-basis" => vec![
            emit_prefixed_declaration("webkit", name, value),
            emit_raw_declaration("-ms-flex-preferred-size", value),
            original.clone(),
        ],
        "flex-grow" => vec![
            emit_raw_declaration("-webkit-box-flex", value),
            emit_prefixed_declaration("webkit", name, value),
            emit_raw_declaration("-ms-flex-positive", value),
            original.clone(),
        ],
        "transition" => {
            let webkit_value = webkit_transform_value(value);
            if webkit_value == *value {
                vec![original.clone()]
            } else {
                vec![
                    emit_prefixed_declaration("webkit", name, &webkit_value),
                    original.clone(),
                ]
            }
        }
        "cursor" => {
            let webkit_value = webkit_cursor_value(value);
            if webkit_value == *value {
                vec![original.clone()]
            } else {
                vec![emit_raw_declaration(name, &webkit_value), original.clone()]
            }
        }
        "background" | "background-image" => {
            if let Some(webkit_value) = webkit_image_set_value(value) {
                vec![emit_raw_declaration(name, &webkit_value), original.clone()]
            } else {
                vec![original.clone()]
            }
        }
        "justify-content" => {
            let ms_value = value.replacen("flex-", "", 1);
            let webkit_box_value = if ms_value == "space-between" {
                "justify"
            } else {
                ms_value.as_str()
            };
            vec![
                emit_raw_declaration("-webkit-box-pack", webkit_box_value),
                emit_raw_declaration("-ms-flex-pack", &ms_value),
                emit_prefixed_declaration("webkit", name, value),
                original.clone(),
            ]
        }
        "justify-self" => {
            if value.contains("flex-") || value.contains("baseline") {
                vec![original.clone()]
            } else {
                vec![
                    emit_raw_declaration("-ms-grid-column-align", value),
                    original.clone(),
                ]
            }
        }
        "grid-template-columns" => {
            vec![
                emit_raw_declaration("-ms-grid-columns", value),
                original.clone(),
            ]
        }
        "grid-template-rows" => {
            vec![
                emit_raw_declaration("-ms-grid-rows", value),
                original.clone(),
            ]
        }
        "grid-row-start" | "grid-column-start" => expand_grid_start(name, value, source, original),
        "grid-row-end" | "grid-column-end" => expand_grid_end(name, value, source, original),
        "grid-row" | "grid-column" => expand_grid_shorthand(name, value, original),
        "margin-inline-start" => {
            vec![
                emit_raw_declaration("-webkit-margin-start", value),
                original.clone(),
            ]
        }
        "margin-inline-end" => {
            vec![
                emit_raw_declaration("-webkit-margin-end", value),
                original.clone(),
            ]
        }
        "padding-inline-start" => {
            vec![
                emit_raw_declaration("-webkit-padding-start", value),
                original.clone(),
            ]
        }
        "padding-inline-end" => {
            vec![
                emit_raw_declaration("-webkit-padding-end", value),
                original.clone(),
            ]
        }
        name if is_size_keyword_property(name) => {
            expand_size_keyword_declaration(name, value, original)
        }
        "position" if value == "sticky" => {
            vec![
                emit_raw_declaration("position", "-webkit-sticky"),
                original.clone(),
            ]
        }
        "display" => match value.as_str() {
            "flex" => vec![
                emit_raw_declaration("display", "-webkit-box"),
                emit_raw_declaration("display", "-webkit-flex"),
                emit_raw_declaration("display", "-ms-flexbox"),
                original.clone(),
            ],
            "inline-flex" => vec![
                emit_raw_declaration("display", "-webkit-inline-box"),
                emit_raw_declaration("display", "-webkit-inline-flex"),
                emit_raw_declaration("display", "-ms-inline-flexbox"),
                original.clone(),
            ],
            "grid" => vec![
                emit_raw_declaration("display", "-ms-grid"),
                original.clone(),
            ],
            "inline-grid" => vec![
                emit_raw_declaration("display", "-ms-inline-grid"),
                original.clone(),
            ],
            _ => vec![original.clone()],
        },
        name if is_scroll_margin_property(name) => {
            vec![
                emit_raw_declaration(&name.replacen("scroll-", "scroll-snap-", 1), value),
                original.clone(),
            ]
        }
        "color-adjust" => vec![
            emit_raw_declaration("-webkit-print-color-adjust", value),
            original.clone(),
        ],
        _ => vec![original.clone()],
    }
}

impl<'i> DeclarationBlock<'i> {
    /// Returns an iterator over all properties in the declaration.
    pub fn iter(&self) -> impl std::iter::DoubleEndedIterator<Item = (&Property<'i>, bool)> {
        self.declarations
            .iter()
            .map(|property| (property, false))
            .chain(
                self.important_declarations
                    .iter()
                    .map(|property| (property, true)),
            )
    }

    /// Returns a mutable iterator over all properties in the declaration.
    pub fn iter_mut(&mut self) -> impl std::iter::DoubleEndedIterator<Item = &mut Property<'i>> {
        self.declarations
            .iter_mut()
            .chain(self.important_declarations.iter_mut())
    }

    /// Returns the value for a given property id based on the properties in this declaration block.
    ///
    /// If the property is a shorthand, the result will be a combined value of all of the included
    /// longhands, or `None` if some of the longhands are not declared. Otherwise, the value will be
    /// either an explicitly declared longhand, or a value extracted from a shorthand property.
    pub fn get<'a>(&'a self, property_id: &PropertyId) -> Option<(Cow<'a, Property<'i>>, bool)> {
        if property_id.is_shorthand() {
            if let Some((shorthand, important)) = property_id.shorthand_value(&self) {
                return Some((Cow::Owned(shorthand), important));
            }
        } else {
            for (property, important) in self.iter().rev() {
                if property.property_id() == *property_id {
                    return Some((Cow::Borrowed(property), important));
                }

                if let Some(val) = property.longhand(&property_id) {
                    return Some((Cow::Owned(val), important));
                }
            }
        }

        None
    }

    /// Sets the value and importance for a given property, replacing any existing declarations.
    ///
    /// If the property already exists within the declaration block, it is updated in place. Otherwise,
    /// a new declaration is appended. When updating a longhand property and a shorthand is defined which
    /// includes the longhand, the shorthand will be updated rather than appending a new declaration.
    pub fn set(&mut self, property: Property<'i>, important: bool) {
        let property_id = property.property_id();
        let declarations = if important {
            // Remove any non-important properties with this id.
            self.declarations
                .retain(|decl| decl.property_id() != property_id);
            &mut self.important_declarations
        } else {
            // Remove any important properties with this id.
            self.important_declarations
                .retain(|decl| decl.property_id() != property_id);
            &mut self.declarations
        };

        let longhands = property_id
            .longhands()
            .unwrap_or_else(|| vec![property.property_id()]);

        for decl in declarations.iter_mut().rev() {
            {
                // If any of the longhands being set are in the same logical property group as any of the
                // longhands in this property, but in a different category (i.e. logical or physical),
                // then we cannot modify in place, and need to append a new property.
                let id = decl.property_id();
                let id_longhands = id.longhands().unwrap_or_else(|| vec![id]);
                if longhands.iter().any(|longhand| {
                    let logical_group = longhand.logical_group();
                    let category = longhand.category();

                    logical_group.is_some()
                        && id_longhands.iter().any(|id_longhand| {
                            logical_group == id_longhand.logical_group()
                                && category != id_longhand.category()
                        })
                }) {
                    break;
                }
            }

            if decl.property_id() == property_id {
                *decl = property;
                return;
            }

            // Update shorthand.
            if decl.set_longhand(&property).is_ok() {
                return;
            }
        }

        declarations.push(property)
    }

    /// Removes all declarations of the given property id from the declaration block.
    ///
    /// When removing a longhand property and a shorthand is defined which includes the longhand,
    /// the shorthand will be split apart into its component longhand properties, minus the property
    /// to remove. When removing a shorthand, all included longhand properties are also removed.
    pub fn remove(&mut self, property_id: &PropertyId) {
        fn remove<'i, 'a>(declarations: &mut Vec<Property<'i>>, property_id: &PropertyId<'a>) {
            let longhands = property_id.longhands().unwrap_or(vec![]);
            let mut i = 0;
            while i < declarations.len() {
                let replacement = {
                    let property = &declarations[i];
                    let id = property.property_id();
                    if id == *property_id || longhands.contains(&id) {
                        // If the property matches the requested property id, or is a longhand
                        // property that is included in the requested shorthand, remove it.
                        None
                    } else if longhands.is_empty()
                        && id.longhands().unwrap_or(vec![]).contains(&property_id)
                    {
                        // If this is a shorthand property that includes the requested longhand,
                        // split it apart into its component longhands, excluding the requested one.
                        Some(
                            id.longhands()
                                .unwrap()
                                .iter()
                                .filter_map(|longhand| {
                                    if *longhand == *property_id {
                                        None
                                    } else {
                                        property.longhand(longhand)
                                    }
                                })
                                .collect::<Vec<Property>>(),
                        )
                    } else {
                        i += 1;
                        continue;
                    }
                };

                match replacement {
                    Some(properties) => {
                        let count = properties.len();
                        declarations.splice(i..i + 1, properties);
                        i += count;
                    }
                    None => {
                        declarations.remove(i);
                    }
                }
            }
        }

        remove(&mut self.declarations, property_id);
        remove(&mut self.important_declarations, property_id);
    }
}

struct PropertyDeclarationParser<'a, 'o, 'i> {
    important_declarations: &'a mut Vec<Property<'i>>,
    declarations: &'a mut Vec<Property<'i>>,
    source_order: &'a mut Vec<bool>,
    authored_declarations: &'a mut Vec<String>,
    options: &'a ParserOptions<'o, 'i>,
}

/// Parse a declaration within {} block: `color: blue`
impl<'a, 'o, 'i> cssparser::DeclarationParser<'i> for PropertyDeclarationParser<'a, 'o, 'i> {
    type Declaration = ();
    type Error = ParserError<'i>;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut cssparser::Parser<'i, 't>,
        _start: &cssparser::ParserState,
    ) -> Result<Self::Declaration, cssparser::ParseError<'i, Self::Error>> {
        parse_declaration(
            name,
            input,
            &mut self.declarations,
            &mut self.important_declarations,
            &mut self.source_order,
            &mut self.authored_declarations,
            &self.options,
        )
    }
}

/// Default methods reject all at rules.
impl<'a, 'o, 'i> AtRuleParser<'i> for PropertyDeclarationParser<'a, 'o, 'i> {
    type Prelude = ();
    type AtRule = ();
    type Error = ParserError<'i>;
}

impl<'a, 'o, 'i> QualifiedRuleParser<'i> for PropertyDeclarationParser<'a, 'o, 'i> {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = ParserError<'i>;
}

impl<'a, 'o, 'i> RuleBodyItemParser<'i, (), ParserError<'i>>
    for PropertyDeclarationParser<'a, 'o, 'i>
{
    fn parse_qualified(&self) -> bool {
        false
    }

    fn parse_declarations(&self) -> bool {
        true
    }
}

pub(crate) fn parse_declaration<'i, 't>(
    name: CowRcStr<'i>,
    input: &mut cssparser::Parser<'i, 't>,
    declarations: &mut DeclarationList<'i>,
    important_declarations: &mut DeclarationList<'i>,
    source_order: &mut Vec<bool>,
    authored_declarations: &mut Vec<String>,
    options: &ParserOptions<'_, 'i>,
) -> Result<(), cssparser::ParseError<'i, ParserError<'i>>> {
    // Stop if we hit a `{` token in a non-custom property to
    // avoid ambiguity between nested rules and declarations.
    // https://github.com/w3c/csswg-drafts/issues/9317
    let property_id = PropertyId::from(CowArcStr::from(name.clone()));
    let mut delimiters = Delimiter::Bang;
    if !matches!(
        property_id,
        PropertyId::Custom(CustomPropertyName::Custom(..))
    ) {
        delimiters = delimiters | Delimiter::CurlyBracketBlock;
    }
    let value_start = input.state();
    let property = input.parse_until_before(delimiters, |input| {
        Property::parse(property_id, input, options)
    })?;
    let authored_text = emit_raw_declaration(
        name.as_ref(),
        input.slice_from(value_start.position()).trim(),
    );
    let important = input
        .try_parse(|input| {
            input.expect_delim('!')?;
            input.expect_ident_matching("important")
        })
        .is_ok();
    input.expect_exhausted()?;
    if important {
        important_declarations.push(property);
    } else {
        declarations.push(property);
    }
    source_order.push(important);
    authored_declarations.push(authored_text);
    Ok(())
}

pub(crate) type DeclarationList<'i> = Vec<Property<'i>>;

#[derive(Default)]
pub(crate) struct DeclarationHandler<'i> {
    background: BackgroundHandler<'i>,
    border: BorderHandler<'i>,
    outline: OutlineHandler,
    flex: FlexHandler,
    grid: GridHandler<'i>,
    align: AlignHandler,
    size: SizeHandler,
    margin: MarginHandler<'i>,
    padding: PaddingHandler<'i>,
    scroll_margin: ScrollMarginHandler<'i>,
    scroll_padding: ScrollPaddingHandler<'i>,
    font: FontHandler<'i>,
    text: TextDecorationHandler<'i>,
    list: ListStyleHandler<'i>,
    transition: TransitionHandler<'i>,
    animation: AnimationHandler<'i>,
    display: DisplayHandler<'i>,
    position: PositionHandler,
    inset: InsetHandler<'i>,
    overflow: OverflowHandler,
    transform: TransformHandler,
    box_shadow: BoxShadowHandler,
    mask: MaskHandler<'i>,
    container: ContainerHandler<'i>,
    color_scheme: ColorSchemeHandler,
    fallback: FallbackHandler,
    prefix: PrefixHandler,
    direction: Option<Direction>,
    unicode_bidi: Option<UnicodeBidi>,
    custom_properties: IndexMap<DashedIdent<'i>, usize>,
    decls: DeclarationList<'i>,
    active_handlers: u32,
}

impl<'i> DeclarationHandler<'i> {
    const ACTIVE_BACKGROUND: u32 = 1 << 0;
    const ACTIVE_BORDER: u32 = 1 << 1;
    const ACTIVE_OUTLINE: u32 = 1 << 2;
    const ACTIVE_FLEX: u32 = 1 << 3;
    const ACTIVE_GRID: u32 = 1 << 4;
    const ACTIVE_ALIGN: u32 = 1 << 5;
    const ACTIVE_SIZE: u32 = 1 << 6;
    const ACTIVE_MARGIN: u32 = 1 << 7;
    const ACTIVE_PADDING: u32 = 1 << 8;
    const ACTIVE_SCROLL_MARGIN: u32 = 1 << 9;
    const ACTIVE_SCROLL_PADDING: u32 = 1 << 10;
    const ACTIVE_FONT: u32 = 1 << 11;
    const ACTIVE_TEXT: u32 = 1 << 12;
    const ACTIVE_LIST: u32 = 1 << 13;
    const ACTIVE_TRANSITION: u32 = 1 << 14;
    const ACTIVE_ANIMATION: u32 = 1 << 15;
    const ACTIVE_DISPLAY: u32 = 1 << 16;
    const ACTIVE_POSITION: u32 = 1 << 17;
    const ACTIVE_INSET: u32 = 1 << 18;
    const ACTIVE_OVERFLOW: u32 = 1 << 19;
    const ACTIVE_TRANSFORM: u32 = 1 << 20;
    const ACTIVE_BOX_SHADOW: u32 = 1 << 21;
    const ACTIVE_MASK: u32 = 1 << 22;
    const ACTIVE_CONTAINER: u32 = 1 << 23;
    const ACTIVE_COLOR_SCHEME: u32 = 1 << 24;
    const ACTIVE_FALLBACK: u32 = 1 << 25;
    const ACTIVE_PREFIX: u32 = 1 << 26;

    #[inline]
    fn is_active(&self, bit: u32) -> bool {
        self.active_handlers & bit != 0
    }

    pub fn handle_property(
        &mut self,
        property: &Property<'i>,
        context: &mut PropertyHandlerContext<'i, '_>,
    ) -> bool {
        macro_rules! handle_handler {
            ($field:ident, $bit:ident) => {
                if self
                    .$field
                    .handle_property(property, &mut self.decls, context)
                {
                    self.active_handlers |= Self::$bit;
                    return true;
                }
            };
        }

        handle_handler!(background, ACTIVE_BACKGROUND);
        handle_handler!(border, ACTIVE_BORDER);
        handle_handler!(outline, ACTIVE_OUTLINE);
        handle_handler!(flex, ACTIVE_FLEX);
        handle_handler!(grid, ACTIVE_GRID);
        handle_handler!(align, ACTIVE_ALIGN);
        handle_handler!(size, ACTIVE_SIZE);
        handle_handler!(margin, ACTIVE_MARGIN);
        handle_handler!(padding, ACTIVE_PADDING);
        handle_handler!(scroll_margin, ACTIVE_SCROLL_MARGIN);
        handle_handler!(scroll_padding, ACTIVE_SCROLL_PADDING);
        handle_handler!(font, ACTIVE_FONT);
        handle_handler!(text, ACTIVE_TEXT);
        handle_handler!(list, ACTIVE_LIST);
        handle_handler!(transition, ACTIVE_TRANSITION);
        handle_handler!(animation, ACTIVE_ANIMATION);
        handle_handler!(display, ACTIVE_DISPLAY);
        handle_handler!(position, ACTIVE_POSITION);
        handle_handler!(inset, ACTIVE_INSET);
        handle_handler!(overflow, ACTIVE_OVERFLOW);
        handle_handler!(transform, ACTIVE_TRANSFORM);
        handle_handler!(box_shadow, ACTIVE_BOX_SHADOW);
        handle_handler!(mask, ACTIVE_MASK);
        handle_handler!(container, ACTIVE_CONTAINER);
        handle_handler!(color_scheme, ACTIVE_COLOR_SCHEME);
        handle_handler!(fallback, ACTIVE_FALLBACK);
        handle_handler!(prefix, ACTIVE_PREFIX);

        self.handle_all(property) || self.handle_custom_property(property, context)
    }

    fn handle_custom_property(
        &mut self,
        property: &Property<'i>,
        context: &mut PropertyHandlerContext<'i, '_>,
    ) -> bool {
        if let Property::Custom(custom) = property {
            if context.unused_symbols.contains(custom.name.as_ref()) {
                return true;
            }

            if let CustomPropertyName::Custom(name) = &custom.name {
                if let Some(index) = self.custom_properties.get(name) {
                    if self.decls[*index] == *property {
                        return true;
                    }
                    let mut custom = custom.clone();
                    self.add_conditional_fallbacks(&mut custom, context);
                    self.decls[*index] = Property::Custom(custom);
                } else {
                    self.custom_properties
                        .insert(name.clone(), self.decls.len());
                    let mut custom = custom.clone();
                    self.add_conditional_fallbacks(&mut custom, context);
                    self.decls.push(Property::Custom(custom));
                }

                return true;
            }
        }

        false
    }

    fn handle_all(&mut self, property: &Property<'i>) -> bool {
        // The `all` property resets all properies except `unicode-bidi`, `direction`, and custom properties.
        // https://drafts.csswg.org/css-cascade-5/#all-shorthand
        match property {
            Property::UnicodeBidi(bidi) => {
                self.unicode_bidi = Some(*bidi);
                true
            }
            Property::Direction(direction) => {
                self.direction = Some(*direction);
                true
            }
            Property::All(keyword) => {
                let mut handler = DeclarationHandler {
                    unicode_bidi: self.unicode_bidi.clone(),
                    direction: self.direction.clone(),
                    ..Default::default()
                };
                for (key, index) in self.custom_properties.drain(..) {
                    handler.custom_properties.insert(key, handler.decls.len());
                    handler.decls.push(self.decls[index].clone());
                }
                handler.decls.push(Property::All(keyword.clone()));
                *self = handler;
                true
            }
            _ => false,
        }
    }

    fn add_conditional_fallbacks(
        &self,
        custom: &mut CustomProperty<'i>,
        context: &mut PropertyHandlerContext<'i, '_>,
    ) {
        if context.context != DeclarationContext::Keyframes {
            let fallbacks = custom.value.get_fallbacks(context.targets);
            for (condition, fallback) in fallbacks {
                context.add_conditional_property(
                    condition,
                    Property::Custom(CustomProperty {
                        name: custom.name.clone(),
                        value: fallback,
                    }),
                );
            }
        }
    }

    pub fn finalize(&mut self, context: &mut PropertyHandlerContext<'i, '_>) {
        if let Some(direction) = std::mem::take(&mut self.direction) {
            self.decls.push(Property::Direction(direction));
        }
        if let Some(unicode_bidi) = std::mem::take(&mut self.unicode_bidi) {
            self.decls.push(Property::UnicodeBidi(unicode_bidi));
        }

        if self.active_handlers == 0 {
            self.custom_properties.clear();
            return;
        }

        macro_rules! finalize_handler {
            ($field:ident, $bit:ident) => {
                if self.is_active(Self::$bit) {
                    self.$field.finalize(&mut self.decls, context);
                }
            };
        }

        finalize_handler!(background, ACTIVE_BACKGROUND);
        finalize_handler!(border, ACTIVE_BORDER);
        finalize_handler!(outline, ACTIVE_OUTLINE);
        finalize_handler!(flex, ACTIVE_FLEX);
        finalize_handler!(grid, ACTIVE_GRID);
        finalize_handler!(align, ACTIVE_ALIGN);
        finalize_handler!(size, ACTIVE_SIZE);
        finalize_handler!(margin, ACTIVE_MARGIN);
        finalize_handler!(padding, ACTIVE_PADDING);
        finalize_handler!(scroll_margin, ACTIVE_SCROLL_MARGIN);
        finalize_handler!(scroll_padding, ACTIVE_SCROLL_PADDING);
        finalize_handler!(font, ACTIVE_FONT);
        finalize_handler!(text, ACTIVE_TEXT);
        finalize_handler!(list, ACTIVE_LIST);
        finalize_handler!(transition, ACTIVE_TRANSITION);
        finalize_handler!(animation, ACTIVE_ANIMATION);
        finalize_handler!(display, ACTIVE_DISPLAY);
        finalize_handler!(position, ACTIVE_POSITION);
        finalize_handler!(inset, ACTIVE_INSET);
        finalize_handler!(overflow, ACTIVE_OVERFLOW);
        finalize_handler!(transform, ACTIVE_TRANSFORM);
        finalize_handler!(box_shadow, ACTIVE_BOX_SHADOW);
        finalize_handler!(mask, ACTIVE_MASK);
        finalize_handler!(container, ACTIVE_CONTAINER);
        finalize_handler!(color_scheme, ACTIVE_COLOR_SCHEME);
        finalize_handler!(fallback, ACTIVE_FALLBACK);
        finalize_handler!(prefix, ACTIVE_PREFIX);

        self.active_handlers = 0;
        self.custom_properties.clear();
    }
}
