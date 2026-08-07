//! Durable ingest manifest primitives.

use crate::hash::Sha256;
use crate::ingest::review::{
    CandidateFingerprint, CandidateId, IngestCandidate, ReviewDiagnostic, SourceSpan,
};
use crate::model::{Reference, SourceKind};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestId(String);

impl IngestId {
    #[must_use]
    pub fn from_seed(seed: &str) -> Self {
        let digest = hash_text(seed);
        Self(format!("ing_{digest}"))
    }

    #[must_use]
    pub fn raw(seed: &str) -> Self {
        Self(seed.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for IngestId {
    fn from(value: &str) -> Self {
        Self::raw(value)
    }
}

impl From<String> for IngestId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for IngestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type ManifestVersion = u32;

#[derive(Debug, Clone)]
pub struct SourceSnapshot {
    pub kind: SourceKind,
    pub path: Option<String>,
    pub url: Option<String>,
    pub text_digest: Option<String>,
    pub reference: Option<String>,
}

impl SourceSnapshot {
    #[must_use]
    pub fn external_url(url: &str) -> Self {
        Self {
            kind: SourceKind::External,
            path: None,
            url: Some(url.to_owned()),
            text_digest: None,
            reference: Some(url.to_owned()),
        }
    }

    #[must_use]
    pub fn internal_path(path: &str) -> Self {
        Self {
            kind: SourceKind::Internal,
            path: Some(path.to_owned()),
            url: None,
            text_digest: None,
            reference: Some(path.to_owned()),
        }
    }

    #[must_use]
    pub fn external_bytes(bytes: &[u8]) -> Self {
        let digest = hash_bytes(bytes);
        Self {
            kind: SourceKind::External,
            path: None,
            url: None,
            text_digest: Some(digest),
            reference: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceRegistration {
    pub source: SourceSnapshot,
    pub extractor_version: String,
    pub source_digest: String,
}

#[derive(Debug, Clone)]
pub struct IngestManifest {
    pub ingest_id: IngestId,
    pub source: SourceRegistration,
    pub manifest_version: ManifestVersion,
    pub extractor_version: String,
    pub source_snapshot: String,
    pub candidates: Vec<IngestCandidate>,
    pub diagnostics: Vec<ReviewDiagnostic>,
    pub unresolved_relations: Vec<UnresolvedRelation>,
    pub applied: Vec<AppliedCandidate>,
}

impl IngestManifest {
    #[must_use]
    pub fn new(
        ingest_id: IngestId,
        source: SourceRegistration,
        extractor_version: String,
        source_snapshot: String,
        candidates: Vec<IngestCandidate>,
    ) -> Self {
        Self {
            ingest_id,
            source,
            manifest_version: 1,
            extractor_version,
            source_snapshot,
            candidates,
            diagnostics: Vec::new(),
            unresolved_relations: Vec::new(),
            applied: Vec::new(),
        }
    }

    #[must_use]
    pub fn candidate(&self, id: CandidateId) -> Option<&IngestCandidate> {
        self.candidates.iter().find(|candidate| candidate.id == id)
    }

    #[must_use]
    pub fn candidate_mut(&mut self, id: CandidateId) -> Option<&mut IngestCandidate> {
        self.candidates
            .iter_mut()
            .find(|candidate| candidate.id == id)
    }

    #[must_use]
    pub fn candidate_by_fingerprint(&self, fingerprint: &str) -> Option<&IngestCandidate> {
        self.candidates
            .iter()
            .find(|candidate| candidate.fingerprint.as_str() == fingerprint)
    }

    pub fn next_version(&mut self) {
        self.manifest_version += 1;
    }

    #[must_use]
    pub fn summary(&self) -> IngestManifestSummary {
        IngestManifestSummary {
            candidate_count: self.candidates.len(),
            pending_count: self
                .candidates
                .iter()
                .filter(|candidate| {
                    candidate.review.disposition == crate::ingest::review::Disposition::Pending
                })
                .count(),
            applied_count: self.applied.len(),
            unresolved_relation_count: self.unresolved_relations.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IngestManifestSummary {
    pub candidate_count: usize,
    pub pending_count: usize,
    pub applied_count: usize,
    pub unresolved_relation_count: usize,
}

#[derive(Debug, Clone)]
pub struct UnresolvedRelation {
    pub from: CandidateId,
    pub reference: String,
    pub target: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct AppliedCandidate {
    pub candidate: CandidateId,
    pub target: Reference,
    pub fingerprint: CandidateFingerprint,
}

#[derive(Debug, Clone)]
pub enum ManifestDiagnostic {
    OrphanSupport {
        candidate_id: Option<CandidateId>,
        span: SourceSpan,
        help: String,
    },
    DuplicateCandidate {
        span: SourceSpan,
        fingerprint: String,
    },
    SourceDigestChanged {
        old_digest: String,
        new_digest: String,
    },
}

#[derive(Debug, Clone)]
pub struct ManifestPath {
    /// `.akr/reviews/<ingest-id>/manifest.json`
    pub manifest: PathBuf,
    /// `.akr/reviews/<ingest-id>/source.md`
    pub source: PathBuf,
}

impl ManifestPath {
    pub const MANIFEST_NAME: &'static str = "manifest.json";
    pub const SOURCE_NAME: &'static str = "source.md";

    #[must_use]
    pub fn new(akr_dir: &Path, ingest_id: &IngestId) -> Self {
        let base = akr_dir.join("reviews").join(ingest_id.as_str());
        Self {
            manifest: base.join(Self::MANIFEST_NAME),
            source: base.join(Self::SOURCE_NAME),
        }
    }
}

#[must_use]
pub fn hash_text(text: &str) -> String {
    hash_bytes(text.as_bytes())
}

#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finish().to_hex()
}
