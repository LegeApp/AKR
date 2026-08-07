//! Apply-plan generation for reviewed ingest candidates.

use crate::ingest::manifest::IngestManifest;
use crate::ingest::review::{CandidateId, Disposition, IngestCandidate, PromotionPlan};
use crate::model::Reference;

#[derive(Debug, Clone)]
pub enum ApplyAction {
    CreateRecord {
        candidate_id: CandidateId,
        kind: crate::model::Kind,
        requested_key: Option<String>,
        basis: Vec<Reference>,
        relations: Vec<Reference>,
    },
    ReviseRecord {
        candidate_id: CandidateId,
        target: Reference,
    },
    AttachSource {
        candidate_id: CandidateId,
        target: Reference,
    },
    VerifyWithBasis {
        candidate_id: CandidateId,
        basis: Vec<Reference>,
    },
    AlreadyRepresented {
        candidate_id: CandidateId,
        target: Reference,
    },
    Decline(CandidateId),
    Split(CandidateId),
    Contradict(CandidateId),
    Pending(CandidateId),
}

#[derive(Debug, Clone)]
pub struct ApplyPlan {
    pub actions: Vec<ApplyAction>,
}

#[derive(Debug, Clone)]
pub struct ActionContext {
    pub candidate_count: usize,
    pub ready_count: usize,
}

#[derive(Debug, Clone)]
pub struct ApplyResult {
    pub actions: Vec<ApplyAction>,
    pub ready_count: usize,
}

/// Derives a deterministic action list from reviewed candidates.
#[must_use]
pub fn plan_apply(manifest: &IngestManifest) -> ApplyPlan {
    ApplyPlan {
        actions: manifest
            .candidates
            .iter()
            .filter_map(|candidate| action_for_candidate(candidate).map(|a| (candidate.id, a)))
            .map(|(_, action)| action)
            .collect(),
    }
}

/// Return a plain, deterministic context for callers.
#[must_use]
pub fn action_context(manifest: &IngestManifest) -> ActionContext {
    let ready_count = manifest
        .candidates
        .iter()
        .filter(|candidate| is_apply_ready(candidate))
        .count();
    ActionContext {
        candidate_count: manifest.candidates.len(),
        ready_count,
    }
}

/// Placeholder apply summary for phase 1 scaffolding.
#[must_use]
pub fn plan_apply_preview(manifest: &IngestManifest) -> ApplyResult {
    let plan = plan_apply(manifest);
    let ready_count = plan
        .actions
        .iter()
        .filter(|action| is_ready_action(action))
        .count();
    ApplyResult {
        actions: plan.actions,
        ready_count,
    }
}

/// Candidate action synthesis.
fn action_for_candidate(candidate: &IngestCandidate) -> Option<ApplyAction> {
    match candidate.review.disposition {
        Disposition::Pending => Some(ApplyAction::Pending(candidate.id)),
        Disposition::Promote => {
            if let Some(promotion) = candidate.review.promotion.as_ref() {
                match promotion {
                    PromotionPlan::Create {
                        kind,
                        requested_key,
                    } => Some(ApplyAction::CreateRecord {
                        candidate_id: candidate.id,
                        kind: *kind,
                        requested_key: requested_key.clone(),
                        basis: candidate.review.basis.clone(),
                        relations: candidate
                            .review
                            .relations
                            .iter()
                            .filter_map(|r| Some(r.target.clone()))
                            .collect(),
                    }),
                    PromotionPlan::Revise { target } => Some(ApplyAction::ReviseRecord {
                        candidate_id: candidate.id,
                        target: target.to_owned(),
                    }),
                    PromotionPlan::AttachSource { target } => Some(ApplyAction::AttachSource {
                        candidate_id: candidate.id,
                        target: target.to_owned(),
                    }),
                }
            } else {
                None
            }
        }
        Disposition::VerifiedSatisfied => Some(ApplyAction::VerifyWithBasis {
            candidate_id: candidate.id,
            basis: candidate.review.basis.clone(),
        }),
        Disposition::AlreadyRepresented => {
            candidate.review.target.as_ref().cloned().map(|target| {
                ApplyAction::AlreadyRepresented {
                    candidate_id: candidate.id,
                    target,
                }
            })
        }
        Disposition::Declined => Some(ApplyAction::Decline(candidate.id)),
        Disposition::Split => Some(ApplyAction::Split(candidate.id)),
        Disposition::Contradicted => Some(ApplyAction::Contradict(candidate.id)),
    }
}

/// True when the action corresponds to a non-blocking, ready-to-apply candidate.
#[must_use]
pub fn is_ready_action(action: &ApplyAction) -> bool {
    !matches!(action, ApplyAction::Pending(_))
}

/// True when the candidate has enough review data to enter apply planning.
#[must_use]
pub fn is_apply_ready(candidate: &IngestCandidate) -> bool {
    match candidate.review.disposition {
        Disposition::Pending => false,
        Disposition::Promote => candidate.review.promotion.is_some(),
        Disposition::Split => !candidate.review.split_children.is_empty(),
        Disposition::VerifiedSatisfied | Disposition::AlreadyRepresented => true,
        Disposition::Declined => true,
        Disposition::Contradicted => true,
    }
}
