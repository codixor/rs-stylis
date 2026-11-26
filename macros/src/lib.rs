//! Derive macros used by the rs-stylis CSS syntax tree.

use proc_macro::TokenStream;

mod parse;
mod to_css;
mod visit;

/// Generates visitor traversal over the fields of a syntax tree node.
///
/// This derive uses the invoking crate's visitor runtime. Provide
/// `crate::visitor::{Visit, Visitor, VisitTypes}` and bring that `Visit` trait
/// into scope alongside the derive. The crate README describes the contract.
#[proc_macro_derive(Visit, attributes(visit, skip_visit, skip_type, visit_types))]
pub fn derive_visit_children(input: TokenStream) -> TokenStream {
    visit::derive_visit_children(input)
}

/// Implements CSS parsing for an enum of CSS values.
#[proc_macro_derive(Parse, attributes(css))]
pub fn derive_parse(input: TokenStream) -> TokenStream {
    parse::derive_parse(input)
}

/// Implements CSS serialization for an enum of CSS values.
#[proc_macro_derive(ToCss, attributes(css))]
pub fn derive_to_css(input: TokenStream) -> TokenStream {
    to_css::derive_to_css(input)
}
