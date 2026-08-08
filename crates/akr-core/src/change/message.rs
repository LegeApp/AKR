//! Commit-message generation from a transaction and a staged semantic delta.
//!
//! The message has five sources and no sixth: the transaction's summary becomes the
//! subject, the primary work record's intent explains why, the semantic delta says what
//! happened, the evidence records say what was verified, and the trailers carry the
//! machine-readable links.
//!
//! # Why trailers rather than stored commit hashes
//!
//! A commit hash cannot be written into a record contained in that same commit without an
//! amendment loop, and a rebase would invalidate every stored hash anyway. Trailers go the
//! other way round — the commit names the records — so they survive rebases and
//! cherry-picks, `git log --grep` finds them, and the whole AKR-to-git index can be
//! rebuilt by walking history.

use super::{ChangeIntent, SemanticDelta};
use crate::model::{Ledger, Record};

/// The trailer names AKR writes, all `git interpret-trailers`-compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trailer {
    /// The transaction id.
    Change,
    /// A work record this commit advances.
    Work,
    /// An evidence record this commit introduces.
    Evidence,
    /// A decision this commit records.
    Decision,
    /// The source-graph hash of the staged ledger.
    Graph,
    /// The staged tree object id.
    Tree,
}

impl Trailer {
    /// The token, without its colon.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Change => "AKR-Change",
            Self::Work => "AKR-Work",
            Self::Evidence => "AKR-Evidence",
            Self::Decision => "AKR-Decision",
            Self::Graph => "AKR-Graph",
            Self::Tree => "AKR-Tree",
        }
    }
}

/// The maximum subject length, counting `kind(scope): `.
const SUBJECT_LIMIT: usize = 72;

/// Generates the commit message for a prepared transaction.
///
/// Deterministic: the same transaction and the same staged delta produce the same bytes.
/// That is what makes `akr git message` reviewable — a message an agent cannot predict is
/// a message nobody checks.
#[must_use]
pub fn commit_message(
    intent: &ChangeIntent,
    delta: &SemanticDelta,
    staged: Option<&Ledger>,
    tree: Option<&str>,
    graph: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str(&subject(intent));
    out.push('\n');

    let mut body: Vec<String> = Vec::new();
    if let Some(why) = intent
        .primary_work
        .as_deref()
        .and_then(|reference| staged.and_then(|ledger| intent_of(ledger, reference)))
    {
        body.push(wrap(&why));
    }
    if let Some(note) = &intent.implementation_note {
        body.push(wrap(note));
    }
    if let Some(paragraph) = transitions_paragraph(delta) {
        body.push(paragraph);
    }
    if let Some(paragraph) = verification_paragraph(delta, staged) {
        body.push(paragraph);
    }
    if let Some(reason) = &intent.untracked_reason {
        body.push(wrap(&format!("No AKR work record: {reason}")));
    }
    for paragraph in body {
        out.push('\n');
        out.push_str(&paragraph);
        out.push('\n');
    }

    let trailers = trailers(intent, delta, tree, graph);
    if !trailers.is_empty() {
        out.push('\n');
        for line in trailers {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// `kind(scope): summary`, truncated on a word boundary if it has to be.
fn subject(intent: &ChangeIntent) -> String {
    let prefix = match &intent.scope {
        Some(scope) if !scope.is_empty() => format!("{}({scope}): ", intent.kind.as_str()),
        _ => format!("{}: ", intent.kind.as_str()),
    };
    let room = SUBJECT_LIMIT.saturating_sub(prefix.len());
    let summary = intent.summary.trim();
    if summary.chars().count() <= room {
        return format!("{prefix}{summary}");
    }
    let mut cut = String::new();
    for word in summary.split_whitespace() {
        if cut.chars().count() + word.chars().count() + 1 > room.saturating_sub(3) {
            break;
        }
        if !cut.is_empty() {
            cut.push(' ');
        }
        cut.push_str(word);
    }
    format!("{prefix}{cut}...")
}

/// The one-line intent of the primary work record, when the staged ledger has it.
fn intent_of(ledger: &Ledger, reference: &str) -> Option<String> {
    let record = find(ledger, reference)?;
    crate::context::body_of(record)
        .map(|body| body.lines().take(3).collect::<Vec<_>>().join(" "))
        .filter(|text| !text.trim().is_empty())
}

fn find<'a>(ledger: &'a Ledger, reference: &str) -> Option<&'a Record> {
    let text = reference.trim_start_matches('@');
    let (key, revision) = match text.split_once('/') {
        Some((key, rev)) => (key, rev.parse::<u32>().ok()),
        None => (text, None),
    };
    ledger.records().iter().find(|record| {
        record.id.key.to_string() == key && revision.is_none_or(|rev| record.id.revision == rev)
    })
}

fn transitions_paragraph(delta: &SemanticDelta) -> Option<String> {
    if delta.transitions.is_empty() {
        return None;
    }
    let mut lines = Vec::new();
    for transition in &delta.transitions {
        let from = transition
            .from
            .map_or_else(|| "new".to_owned(), |state| state.name().to_owned());
        lines.push(format!(
            "- {} {from} -> {}",
            transition.id.key,
            transition.to.name()
        ));
    }
    Some(lines.join("\n"))
}

/// Compact evidence results — never the whole evidence record.
///
/// Git wants a concise historical explanation; the ledger keeps the method, artefacts,
/// commands, metrics and acceptance mapping. Copying all of that into the message would
/// make the message the second copy that goes stale.
fn verification_paragraph(delta: &SemanticDelta, staged: Option<&Ledger>) -> Option<String> {
    if delta.evidence.is_empty() {
        return None;
    }
    let mut lines = vec!["Verified by:".to_owned()];
    for id in &delta.evidence {
        let summary = staged
            .and_then(|ledger| ledger.get(id))
            .map(|record| record.title.clone())
            .unwrap_or_else(|| id.key.to_string());
        lines.push(format!("- {summary}"));
    }
    Some(lines.join("\n"))
}

fn trailers(
    intent: &ChangeIntent,
    delta: &SemanticDelta,
    tree: Option<&str>,
    graph: Option<&str>,
) -> Vec<String> {
    let mut out = vec![format!("{}: {}", Trailer::Change.as_str(), intent.id)];
    for reference in intent.work_refs() {
        out.push(format!("{}: {reference}", Trailer::Work.as_str()));
    }
    for id in &delta.evidence {
        out.push(format!("{}: @{id}", Trailer::Evidence.as_str()));
    }
    if let Some(graph) = graph {
        out.push(format!("{}: {graph}", Trailer::Graph.as_str()));
    }
    if let Some(tree) = tree {
        out.push(format!("{}: {tree}", Trailer::Tree.as_str()));
    }
    out
}

fn wrap(text: &str) -> String {
    let mut out = String::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + word.chars().count() + 1 > 72 {
            out.push_str(&line);
            out.push('\n');
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push_str(&line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change::{ChangeKind, SemanticDelta, Transition};
    use crate::model::{RevisionId, State};

    fn intent() -> ChangeIntent {
        let mut intent = ChangeIntent::new(
            "ff74d3b2",
            ChangeKind::Fix,
            "gate reconstructed highlight chroma by uncertainty",
        );
        intent.scope = Some("tone".into());
        intent.primary_work = Some("@raw.work.slice-6/2".into());
        intent.related_work = vec!["@raw.work.slice-1/2".into()];
        intent
    }

    fn id(key: &str, rev: u32) -> RevisionId {
        RevisionId::new(crate::model::LogicalKey::parse(key).expect("a key"), rev)
    }

    #[test]
    fn the_subject_carries_the_kind_and_scope() {
        let message = commit_message(&intent(), &SemanticDelta::default(), None, None, None);
        assert!(
            message.starts_with("fix(tone): gate reconstructed highlight chroma by"),
            "{message}"
        );
        assert!(
            message.lines().next().expect("a subject").len() <= 72,
            "{message}"
        );
    }

    #[test]
    fn a_long_summary_is_cut_on_a_word_boundary() {
        let mut intent = intent();
        intent.summary = "word ".repeat(40);
        let subject = super::subject(&intent);
        assert!(subject.len() <= 72, "{subject}");
        assert!(subject.ends_with("..."), "{subject}");
    }

    #[test]
    fn transitions_and_trailers_appear() {
        let delta = SemanticDelta {
            transitions: vec![Transition {
                id: id("raw.work.slice-6", 2),
                title: "Slice 6".into(),
                from: Some(State::Active),
                to: State::Completed,
            }],
            evidence: vec![id("raw.evidence.slice-6-verify", 1)],
            ..SemanticDelta::default()
        };
        let message = commit_message(&intent(), &delta, None, Some("41cd7e"), Some("sha256:5a0a"));
        assert!(
            message.contains("- raw.work.slice-6 active -> completed"),
            "{message}"
        );
        assert!(message.contains("AKR-Change: "), "{message}");
        assert!(
            message.contains("AKR-Work: @raw.work.slice-6/2"),
            "{message}"
        );
        assert!(
            message.contains("AKR-Work: @raw.work.slice-1/2"),
            "{message}"
        );
        assert!(
            message.contains("AKR-Evidence: @raw.evidence.slice-6-verify/1"),
            "{message}"
        );
        assert!(message.contains("AKR-Graph: sha256:5a0a"), "{message}");
        assert!(message.contains("AKR-Tree: 41cd7e"), "{message}");
    }

    #[test]
    fn the_same_inputs_produce_the_same_message() {
        let delta = SemanticDelta::default();
        assert_eq!(
            commit_message(&intent(), &delta, None, Some("t"), Some("g")),
            commit_message(&intent(), &delta, None, Some("t"), Some("g"))
        );
    }

    #[test]
    fn an_untracked_change_says_why_in_the_body() {
        let mut intent = ChangeIntent::new("ff74d3b2", ChangeKind::Chore, "pin the build image");
        intent.scope = Some("ci".into());
        intent.untracked_reason =
            Some("repository maintenance; no project behaviour changed".into());
        let message = commit_message(&intent, &SemanticDelta::default(), None, None, None);
        assert!(
            message.contains("No AKR work record: repository maintenance"),
            "{message}"
        );
        assert!(!message.contains("AKR-Work:"), "{message}");
    }

    #[test]
    fn no_evidence_body_is_emitted_without_evidence() {
        let message = commit_message(&intent(), &SemanticDelta::default(), None, None, None);
        assert!(!message.contains("Verified by"), "{message}");
    }
}
