//! `DECISION-HISTORY.md` — what was decided, and what was retired.
//!
//! Normative specification: `docs/11-projections.md` §10.
//!
//! **Source query.** Two sets, unioned: every revision of every `decision`, live or
//! terminal, including archived; and every **terminal** revision of any other normative
//! kind, including archived. This is the one view that includes archived records
//! (D-018) — it is the project's memory of what it used to think.
//!
//! **Section order.** By key, then by revision descending, so the current revision of a
//! key precedes the revisions it replaced.

use super::common::{heading_text, one_line, required_prose};
use super::{RenderContext, banner};
use crate::model::{Class, ContentSlot, ContentValue, Kind, LogicalKey, Record, Relation, Segment};

/// Renders `DECISION-HISTORY.md`.
#[must_use]
pub fn render_decision_history(cx: RenderContext<'_>) -> String {
    let ledger = cx.ledger();
    let mut blocks: Vec<String> = Vec::new();
    blocks.push(banner(cx.model).trim_end().to_owned());
    blocks.push(format!("# Decision history — {}", ledger.project.name));
    blocks.push(
        "Every revision of every decision, plus the terminal revisions of other \
         normative records. Includes archived records: this is the project's memory of \
         what it used to think."
            .to_owned(),
    );

    let mut by_key: std::collections::BTreeMap<LogicalKey, Vec<&Record>> =
        std::collections::BTreeMap::new();
    for record in ledger.records() {
        let include = record.kind == Kind::Decision
            || (record.kind.class() == Class::Normative && record.is_terminal());
        if include {
            by_key
                .entry(record.id.key.clone())
                .or_default()
                .push(record);
        }
    }

    if by_key.is_empty() {
        blocks.push("_(none)_".to_owned());
    }

    for (key, mut revisions) in by_key {
        revisions.sort_by_key(|r| std::cmp::Reverse(r.id.revision));
        blocks.push(format!("## {key}"));
        for record in revisions {
            blocks.extend(revision_blocks(cx, record));
        }
    }

    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}

fn revision_blocks<'a>(cx: RenderContext<'a>, record: &'a Record) -> Vec<String> {
    let mut blocks = vec![
        format!("### {}", heading_text(record)),
        metadata_line(record),
    ];
    if let Some(body) = required_prose(record) {
        blocks.push(body);
    }
    if let Some(text) = optional_prose(record, ContentSlot::Context, "Context") {
        blocks.push(text);
    }
    if let Some(text) = optional_prose(record, ContentSlot::Consequences, "Consequences") {
        blocks.push(text);
    }
    if let Some(line) = retired_claims_line(record) {
        blocks.push(line);
    }
    if let Some(line) = supersession_line(cx, record) {
        blocks.push(line);
    }
    blocks
}

fn metadata_line(record: &Record) -> String {
    let mut parts = vec![
        format!("`{}`", record.state.name()),
        format!("`@{}`", record.id),
    ];
    if !record.scope.is_empty() {
        parts.push(format!(
            "scope {}",
            record
                .scope
                .iter()
                .map(|term| format!("`{term}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(topic) = &record.topic {
        parts.push(format!("topic `{topic}`"));
    }
    parts.join(" · ")
}

fn optional_prose(record: &Record, slot: ContentSlot, label: &str) -> Option<String> {
    match record.get(slot) {
        Some(ContentValue::Prose(text) | ContentValue::Text(text)) => {
            Some(format!("**{label}.** {}", one_line(text)))
        }
        _ => None,
    }
}

/// The anchors this revision dropped that the previous one carried (D-011).
fn retired_claims_line(record: &Record) -> Option<String> {
    if record.retired_claims.is_empty() {
        return None;
    }
    let anchors: Vec<String> = record
        .retired_claims
        .iter()
        .map(Segment::to_string)
        .collect();
    Some(format!(
        "**Retired claims** — {}, dropped at this revision.",
        anchors
            .iter()
            .map(|a| format!("`#{a}`"))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// The `supersedes` / superseded-by edges, one compact line.
fn supersession_line(cx: RenderContext<'_>, record: &Record) -> Option<String> {
    let ledger = cx.ledger();
    let mut parts: Vec<String> = Vec::new();

    let supersedes: Vec<String> = record
        .targets(Relation::Supersedes)
        .iter()
        .filter_map(|r| ledger.resolve(r).ok().flatten())
        .map(|t| format!("`@{}`", t.id))
        .collect();
    if !supersedes.is_empty() {
        parts.push(format!("**supersedes** {}", supersedes.join(", ")));
    }

    let superseded_by: Vec<String> = ledger
        .records()
        .iter()
        .filter(|other| {
            other
                .targets(Relation::Supersedes)
                .iter()
                .filter_map(|r| ledger.resolve(r).ok().flatten())
                .any(|t| t.id == record.id)
        })
        .map(|other| format!("`@{}`", other.id))
        .collect();
    if !superseded_by.is_empty() {
        parts.push(format!("**superseded by** {}", superseded_by.join(", ")));
    }

    (!parts.is_empty()).then(|| parts.join(" · "))
}
