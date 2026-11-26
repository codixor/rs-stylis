//! CSS parsing, transformation, and serialization.
//!
//! Parse a whole CSS file with [`StyleSheet::parse`] or an inline declaration
//! list with [`StyleAttribute::parse`]. [`StyleSheet::minify`] can simplify values,
//! merge rules, and lower CSS for the browsers specified in [`Targets`].
//! [`StyleSheet::to_css`] returns the serialized CSS and any requested
//! [CSS Modules exports](css_modules) or [dependency information](dependencies).
//!
//! The [rules], [properties], and [values] modules expose the CSS syntax tree.

#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(
    clippy::assign_op_pattern,
    clippy::borrow_deref_ref,
    clippy::borrowed_box,
    clippy::clone_on_copy,
    clippy::collapsible_else_if,
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::deref_addrof,
    clippy::derivable_impls,
    clippy::excessive_precision,
    clippy::explicit_auto_deref,
    clippy::extend_with_drain,
    clippy::extra_unused_lifetimes,
    clippy::extra_unused_type_parameters,
    clippy::from_over_into,
    clippy::get_first,
    clippy::len_zero,
    clippy::manual_clamp,
    clippy::manual_contains,
    clippy::manual_is_ascii_check,
    clippy::manual_map,
    clippy::manual_range_contains,
    clippy::manual_repeat_n,
    clippy::map_flatten,
    clippy::match_like_matches_macro,
    clippy::missing_transmute_annotations,
    clippy::needless_bool,
    clippy::needless_borrow,
    clippy::needless_borrowed_reference,
    clippy::needless_borrows_for_generic_args,
    clippy::needless_lifetimes,
    clippy::needless_range_loop,
    clippy::needless_return,
    clippy::new_without_default,
    clippy::nonminimal_bool,
    clippy::only_used_in_recursion,
    clippy::op_ref,
    clippy::partialeq_to_none,
    clippy::precedence,
    clippy::ptr_arg,
    clippy::question_mark,
    clippy::redundant_closure,
    clippy::redundant_field_names,
    clippy::redundant_static_lifetimes,
    clippy::result_unit_err,
    clippy::should_implement_trait,
    clippy::single_match,
    clippy::unnecessary_cast,
    clippy::unnecessary_lazy_evaluations,
    clippy::unnecessary_map_or,
    clippy::unnecessary_mut_passed,
    clippy::unnecessary_to_owned,
    clippy::unnecessary_unwrap,
    clippy::upper_case_acronyms,
    clippy::useless_conversion,
    clippy::while_let_loop,
    clippy::while_let_on_iterator,
    clippy::wildcard_in_or_patterns,
    clippy::wrong_self_convention,
    clippy::pedantic,
    unexpected_cfgs
)]
// Donor-derived crate — keep `clippy::all` clean while not forcing a crate-wide
// pedantic rewrite on imported code.
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate cssparser;
#[macro_use]
extern crate log;

mod compat;
mod context;
pub mod css_modules;
pub mod declaration;
pub mod dependencies;
pub mod error;
mod logical;
mod macros;
pub mod media_query;
#[allow(missing_docs)]
#[path = "selectors/lib.rs"]
pub mod parcel_selectors;
mod parser;
mod prefixes;
pub mod printer;
pub mod properties;
mod react_compat;
pub mod rules;
pub mod selector;
mod smallvec_compat;
pub mod stylesheet;
pub mod targets;
pub mod traits;
pub mod values;
pub mod vendor_prefix;

pub use crate::css_modules::Config as CssModulesConfig;
pub use crate::printer::PrinterOptions;
pub use crate::stylesheet::{
    MinifyOptions, NormalizationProfile, ParserFlags, ParserOptions, StyleAttribute, StyleSheet,
    ToCssResult,
};
pub use crate::targets::{Browsers, Features, Targets};
