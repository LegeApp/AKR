//! Deterministic external-source ingest for review-based workflows.
//!
//! This module intentionally runs alongside legacy [`crate::import`] and does not
//! alter its behavior.
//!
//! # Why `missing_docs` is allowed here, and only here
//!
//! The workspace warns on `missing_docs` and every other module earns it. This one does
//! not yet: it has no entry in `docs/` or `spec/`, its types are still moving, and its 196
//! undocumented fields and variants were drowning the lint for the whole workspace — over
//! 200 warnings, which is how a real one in a *different* crate went unread until somebody
//! logged a papercut about it.
//!
//! Silencing it here is the honest trade. A doc comment on a field whose meaning is still
//! being decided is a comment that will be wrong before it is read, and the alternative —
//! leaving the noise in place — costs the lint everywhere else. Delete this attribute when
//! the module gets a normative document; the warnings it uncovers are then real work, not
//! a wall.
#![allow(missing_docs)]

pub mod apply;
pub mod manifest;
pub mod markdown;
pub mod review;

pub use apply::{ActionContext, ApplyAction, ApplyPlan, ApplyResult, is_apply_ready};
pub use manifest::{
    AppliedCandidate, IngestId, IngestManifest, IngestManifestSummary, ManifestDiagnostic,
    ManifestPath, SourceRegistration, SourceSnapshot,
};
pub use markdown::{ExtractOptions, Extraction, TableMode, extract_markdown_items};
pub use review::{
    CandidateFingerprint, CandidateId, CandidateKind, CandidateReview, Disposition,
    IngestCandidate, PromotionPlan, ReviewDiagnostic, ReviewError, SourceSpan, StagedRelation,
    SupportBlock, candidate_has_no_children, validate_candidate_review,
};
