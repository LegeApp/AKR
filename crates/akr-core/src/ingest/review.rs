//! Review model for a candidate extracted from source material.

use crate::model::{Reference, Relation};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CandidateId(u32);

impl CandidateId {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl TryFrom<usize> for CandidateId {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        u32::try_from(value).map(Self).map_err(|_| ())
    }
}

impl From<CandidateId> for u32 {
    fn from(value: CandidateId) -> Self {
        value.0
    }
}

impl From<CandidateId> for usize {
    fn from(value: CandidateId) -> Self {
        value.0 as usize
    }
}

impl fmt::Display for CandidateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "c_{:04}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CandidateFingerprint(String);

impl CandidateFingerprint {
    /// A fingerprint assigned by the extractor.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CandidateFingerprint {
    fn default() -> Self {
        Self(String::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    Paragraph,
    ListItem,
    TableRow,
    BlockQuote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Pending,
    Promote,
    VerifiedSatisfied,
    AlreadyRepresented,
    Declined,
    Split,
    Contradicted,
}

impl Disposition {
    #[must_use]
    pub const fn symbol(self) -> char {
        match self {
            Self::Pending => '?',
            Self::Promote => '+',
            Self::VerifiedSatisfied => 'x',
            Self::AlreadyRepresented => '=',
            Self::Declined => '-',
            Self::Split => '~',
            Self::Contradicted => '!',
        }
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Promote
                | Self::VerifiedSatisfied
                | Self::AlreadyRepresented
                | Self::Declined
                | Self::Split
                | Self::Contradicted
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportBlock {
    pub span: SourceSpan,
    pub language: Option<String>,
    pub raw_text: String,
}

#[derive(Debug, Clone)]
pub struct IngestCandidate {
    pub id: CandidateId,
    pub fingerprint: CandidateFingerprint,
    pub ordinal: u32,
    pub source_span: SourceSpan,
    pub section_path: Vec<String>,
    pub parent: Option<CandidateId>,
    pub kind: CandidateKind,
    pub raw_text: String,
    pub semantic_text: String,
    pub support: Vec<SupportBlock>,
    pub review: CandidateReview,
}

impl IngestCandidate {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: CandidateId,
        ordinal: u32,
        source_span: SourceSpan,
        section_path: Vec<String>,
        parent: Option<CandidateId>,
        kind: CandidateKind,
        raw_text: String,
        semantic_text: String,
        support: Vec<SupportBlock>,
    ) -> Self {
        Self {
            id,
            fingerprint: CandidateFingerprint::default(),
            ordinal,
            source_span,
            section_path,
            parent,
            kind,
            raw_text,
            semantic_text,
            support,
            review: CandidateReview::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateReview {
    pub disposition: Disposition,
    pub promotion: Option<PromotionPlan>,
    pub target: Option<Reference>,
    pub basis: Vec<Reference>,
    pub relations: Vec<StagedRelation>,
    pub note: Option<String>,
    /// Child candidates when disposition is `Split`.
    pub split_children: Vec<CandidateId>,
    /// Set by a successful apply pipeline after a candidate gets a corresponding record.
    pub applied_as: Option<Reference>,
}

impl Default for CandidateReview {
    fn default() -> Self {
        Self {
            disposition: Disposition::Pending,
            promotion: None,
            target: None,
            basis: Vec::new(),
            relations: Vec::new(),
            note: None,
            split_children: Vec::new(),
            applied_as: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionPlan {
    Create {
        kind: crate::model::Kind,
        requested_key: Option<String>,
    },
    Revise {
        target: Reference,
    },
    AttachSource {
        target: Reference,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedRelation {
    pub relation: Relation,
    pub target: Reference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewDiagnostic {
    PromotionPlanRequired {
        candidate_id: CandidateId,
    },
    VerificationBasisRequired {
        candidate_id: CandidateId,
    },
    ExistingTargetRequired {
        candidate_id: CandidateId,
    },
    SplitChildrenRequired {
        candidate_id: CandidateId,
    },
    SingleRelationHasMultipleTargets {
        candidate_id: CandidateId,
        relation: Relation,
    },
    UnknownDisposition {
        source: char,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewError {
    UnknownDisposition(char),
}

impl fmt::Display for ReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDisposition(value) => {
                write!(f, "unknown disposition character {value}")
            }
        }
    }
}

impl TryFrom<char> for Disposition {
    type Error = ReviewError;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '?' => Ok(Self::Pending),
            '+' => Ok(Self::Promote),
            'x' | 'X' => Ok(Self::VerifiedSatisfied),
            '=' => Ok(Self::AlreadyRepresented),
            '-' => Ok(Self::Declined),
            '~' => Ok(Self::Split),
            '!' => Ok(Self::Contradicted),
            other => Err(ReviewError::UnknownDisposition(other)),
        }
    }
}

pub fn validate_candidate_review(candidate: &IngestCandidate) -> Result<(), ReviewDiagnostic> {
    let review = &candidate.review;
    match review.disposition {
        Disposition::Pending => {}
        Disposition::Promote if review.promotion.is_none() => {
            return Err(ReviewDiagnostic::PromotionPlanRequired {
                candidate_id: candidate.id,
            });
        }
        Disposition::VerifiedSatisfied if review.basis.is_empty() => {
            return Err(ReviewDiagnostic::VerificationBasisRequired {
                candidate_id: candidate.id,
            });
        }
        Disposition::AlreadyRepresented if review.target.is_none() => {
            return Err(ReviewDiagnostic::ExistingTargetRequired {
                candidate_id: candidate.id,
            });
        }
        Disposition::Split if candidate_has_no_children(candidate) => {
            return Err(ReviewDiagnostic::SplitChildrenRequired {
                candidate_id: candidate.id,
            });
        }
        _ => {}
    }

    validate_staged_relations(candidate)?;
    Ok(())
}

fn validate_staged_relations(candidate: &IngestCandidate) -> Result<(), ReviewDiagnostic> {
    let mut counts = BTreeMap::<Relation, usize>::new();
    for staged in &candidate.review.relations {
        let count = counts.entry(staged.relation).or_insert(0);
        *count += 1;
        if *count > 1
            && matches!(
                staged.relation.cardinality(),
                crate::model::Cardinality::One
            )
        {
            return Err(ReviewDiagnostic::SingleRelationHasMultipleTargets {
                candidate_id: candidate.id,
                relation: staged.relation,
            });
        }
    }
    Ok(())
}

#[must_use]
pub fn candidate_has_no_children(candidate: &IngestCandidate) -> bool {
    candidate.review.split_children.is_empty()
}
