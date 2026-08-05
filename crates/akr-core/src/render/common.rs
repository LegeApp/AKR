//! Shared rendering helpers used by more than one view.
//!
//! `docs/11-projections.md` §3's universal rendering rules — heading links, the
//! terminal-record `note` block, archived-record exclusion, and prose extraction — are
//! the same rule in every view. One implementation here is what keeps them from drifting
//! the way five copy-pasted ones would.

use super::View;
use crate::model::{ContentSlot, ContentValue, Kind, Record};

/// A link to wherever a record is rendered, labelled with its `title`.
///
/// A record no view hosts renders as plain text: a dead link is worse than none (§3). A
/// `decision`'s heading in `DECISION-HISTORY.md` is `Revision N — title` — one heading
/// per revision, since a key can carry several live-and-terminal revisions in one view —
/// so the anchor a link computes has to match that, not the bare title.
#[must_use]
pub(super) fn link(record: &Record) -> String {
    match View::hosting(record.kind) {
        Some(view) => format!(
            "[{}]({}#{})",
            record.title,
            view.file_name(),
            super::slug(&heading_text(record))
        ),
        None => record.title.clone(),
    }
}

/// The exact text a record's heading renders, wherever it is rendered — the input
/// [`super::slug`] turns into an anchor. Every renderer that writes a `###` heading for a
/// record, and [`link`], compute the anchor from this so they can never disagree.
#[must_use]
pub(super) fn heading_text(record: &Record) -> String {
    if record.kind == Kind::Decision {
        format!("Revision {} — {}", record.id.revision, record.title)
    } else {
        record.title.clone()
    }
}

/// The `note` block quote a terminal planning record carries (D-026, §3).
///
/// Only in a terminal state. On a live record a note is working commentary that `intent`
/// should be carrying instead; on a terminal one it is the last thing anybody wrote about
/// the record, and the only place a reader finds out why the plan stopped.
#[must_use]
pub(super) fn note_block(record: &Record) -> Option<String> {
    if !record.is_terminal() {
        return None;
    }
    let note = prose(record, ContentSlot::Note)?;
    let text = one_line(&note);
    (!text.is_empty()).then(|| format!("> **Note:** {text}"))
}

/// A prose slot's text, verbatim (dedented as the parser produced it), or `None`.
#[must_use]
pub(super) fn prose(record: &Record, slot: ContentSlot) -> Option<String> {
    match record.get(slot) {
        Some(ContentValue::Prose(text) | ContentValue::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

/// A prose slot collapsed onto one line: newlines become spaces, extra whitespace
/// dropped. For contexts — list entries, table cells — where a multi-line value would
/// break the surrounding layout.
#[must_use]
pub(super) fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether a record lives under `.akr/archive/`. Archived records still resolve; they are
/// excluded from every view but `DECISION-HISTORY.md` (D-018).
#[must_use]
pub(super) fn is_archived(record: &Record) -> bool {
    record
        .file
        .as_deref()
        .is_some_and(|path| path.contains("/archive/") || path.starts_with("archive/"))
}

/// The kind's required prose slot, verbatim — the one sentence every kind has that says
/// what the record claims. `evidence` requires none, so it falls back to its optional
/// `summary`.
#[must_use]
pub(super) fn required_prose(record: &Record) -> Option<String> {
    let slot = match record.kind {
        Kind::Term => ContentSlot::Definition,
        Kind::Constraint | Kind::Requirement | Kind::Observation | Kind::Assessment => {
            ContentSlot::Statement
        }
        Kind::Policy => ContentSlot::Rule,
        Kind::Decision => ContentSlot::Decision,
        Kind::Evidence => ContentSlot::Summary,
        _ => return None,
    };
    prose(record, slot)
}
