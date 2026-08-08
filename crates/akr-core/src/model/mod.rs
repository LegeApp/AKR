//! The AKR semantic model.
//!
//! Twelve kinds in four classes, four lifecycles, twelve relations, references in the
//! four forms of D-009, scope terms with the conservative overlap test of D-010, and the
//! records that carry them.
//!
//! `spec/tables/vocabulary.json` is authoritative for names. Every table here is checked
//! against it by `tests/vocabulary.rs`; if the two disagree, the JSON wins and the code
//! is wrong.

mod builder;
mod ident;
mod kind;
mod ledger;
mod record;
mod refs;
mod relation;
mod scope;
mod state;

pub use builder::{RecordBuilder, key, reference};
pub use ident::{Commit, Date, Glob, IdentError, LogicalKey, Segment};
pub use kind::{Class, ContentSlot, ContentSlotSpec, Kind};
pub use ledger::{
    Ancestry, ContentHash, HeadError, Ledger, LedgerFacts, PartOfIndex, Project, SealFact,
};
pub use record::{
    Acceptance, Check, CheckMethod, Claim, ContentValue, Disposition, EvidenceResult, Outcome,
    Record, Source, SourceKind, SourceRange,
};
pub use refs::{RefMode, Reference, RevisionId};
pub use relation::{Cardinality, Domain, Range, Relation};
pub use scope::{ScopeTerm, glob_prefixes_comparable, scopes_overlap, terms_overlap};
pub use state::{State, Transition};
