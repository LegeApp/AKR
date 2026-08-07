//! Deterministic external-source ingest for review-based workflows.
//!
//! This module intentionally runs alongside legacy [`crate::import`] and does not
//! alter its behavior.

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
