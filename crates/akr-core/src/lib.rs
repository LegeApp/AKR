//! AKR core: the semantic model, its invariants, and the validation rules.
//!
//! This crate is built semantics-first (`docs/13-implementation-roadmap.md` §1). Phase P1
//! contains no text format at all: there is no lexer, no parser, and no serialisation
//! dependency. Records are constructed programmatically, and every rule in
//! `docs/05-validation-rules.md` is expressed and tested against the in-memory model
//! before any syntax exists.
//!
//! # Layout
//!
//! - [`model`] — kinds, classes, lifecycle states, relations, records, references, scope.
//! - [`syntax`] — lexer, parser, CST, canonical formatter, and lowering to the model.
//! - [`validate`] — `V-001`..`V-024` as named functions over a [`model::Ledger`].
//! - [`diagnostics`] — codes, severity, subjects, and the span-ready diagnostic type.
//!
//! # Sources of truth
//!
//! `spec/tables/vocabulary.json` is authoritative for names; the tables in [`model`] are
//! checked against it by `tests/vocabulary.rs`. `docs/02-data-model.md` is authoritative
//! for meaning, and `spec/diagnostics/codes-lang.md` for diagnostic codes.

pub mod diagnostics;
pub mod model;
pub mod syntax;
pub mod validate;
