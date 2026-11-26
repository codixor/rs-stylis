/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![allow(
    clippy::doc_overindented_list_items,
    clippy::enum_variant_names,
    clippy::identity_op,
    clippy::len_without_is_empty,
    clippy::many_single_char_names,
    clippy::map_clone,
    clippy::needless_late_init,
    clippy::redundant_else,
    clippy::similar_names,
    clippy::type_complexity,
    clippy::unreadable_literal,
    clippy::unnested_or_patterns,
    clippy::useless_transmute,
    unsafe_code
)]

pub mod attr;
pub mod bloom;
mod builder;
pub mod context;
pub mod matching;
mod nth_index_cache;
pub mod parser;
pub mod sink;
mod tree;
pub mod visitor;

#[cfg(feature = "serde")]
mod serialization;

pub use self::nth_index_cache::NthIndexCache;
pub use self::parser::{Parser, SelectorImpl, SelectorList};
pub use self::tree::{Element, OpaqueElement};
