# rs-stylis-derive

`Parse` and `ToCss` generate CSS parsing and serialization for the value enums
in [rs-stylis](https://crates.io/crates/rs-stylis).

For stylesheet parsing and transformation, add `rs-stylis` to your dependencies.
Cargo installs the matching macro crate automatically.

### Using `Visit` with your own syntax tree

`Visit` generates traversal for projects that already define a visitor runtime.
The expansion resolves `crate::visitor::{Visit, Visitor, VisitTypes}` in your
crate. Import its `Visit` trait in the module containing the derive. Your
`Visitor` supplies the error type and selects the node types to visit.
If you use `#[visit(method, KIND)]`, provide the corresponding visitor method
and flag. The `#[visit_types(...)]` attribute uses your `crate::visit_types!` macro.

The published CSS engine provides a separate selector visitor API. For the
runtime contract and a working example on an application-defined tree, see
[the visitor integration test](https://github.com/codixor/rs-stylis/blob/main/tests/derive_visit.rs).
