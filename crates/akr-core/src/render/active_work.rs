//! `ACTIVE-WORK.md` — what is being worked on, and what is stuck.
//!
//! Normative specification: `docs/11-projections.md` §7.
//!
//! **Source query.** Live `work` records — `proposed`, `ready`, `active`, `blocked`.
//!
//! **Section order.** Grouped by `part_of` parent, parents in `ROADMAP.md` order,
//! unparented work last under "Unparented". Within a group: by state in the order
//! `active`, `blocked`, `ready`, `proposed`, then by key.

use super::common::{is_archived, link, note_block, prose};
use super::roadmap::{parent_order, sorted_work};
use super::{RenderContext, banner};
use crate::model::{ContentSlot, Kind, Ledger, Record, Relation, RevisionId};
use crate::resolve::Verdict;
use std::collections::BTreeMap;

/// Renders `ACTIVE-WORK.md`.
#[must_use]
pub fn render_active_work(cx: RenderContext<'_>) -> String {
    let ledger = cx.ledger();
    let mut blocks: Vec<String> = Vec::new();
    blocks.push(banner(cx.model).trim_end().to_owned());
    blocks.push(format!("# Active work — {}", ledger.project.name));
    blocks.push(
        "Live work, grouped by parent in `ROADMAP.md` order. Blocked work names its \
         blocker inline."
            .to_owned(),
    );

    let live = live_work(ledger);
    let mut by_parent: BTreeMap<RevisionId, Vec<&Record>> = BTreeMap::new();
    let mut unparented: Vec<&Record> = Vec::new();
    for item in &live {
        match direct_parent(cx, item) {
            Some(parent) => by_parent.entry(parent).or_default().push(item),
            None => unparented.push(*item),
        }
    }

    // Group headings in ROADMAP.md order first (milestones, then tracks); any parent
    // that is itself a `work` record — nested work, which ROADMAP.md does not list —
    // follows, by key; unparented work is always last.
    let mut ordered_parents: Vec<RevisionId> = parent_order(cx.model)
        .into_iter()
        .filter(|id| by_parent.contains_key(id))
        .collect();
    let mut other_parents: Vec<RevisionId> = by_parent
        .keys()
        .filter(|id| !ordered_parents.contains(id))
        .cloned()
        .collect();
    other_parents.sort();
    ordered_parents.extend(other_parents);

    for parent_id in &ordered_parents {
        let Some(parent) = ledger.get(parent_id) else {
            continue;
        };
        let items = sorted_work(by_parent.remove(parent_id).unwrap_or_default());
        blocks.push(format!("## {} `@{}`", link(parent), parent.id));
        for item in &items {
            blocks.extend(work_blocks(cx, item));
        }
    }

    blocks.push("## Unparented".to_owned());
    if unparented.is_empty() {
        blocks.push("_(none)_".to_owned());
    } else {
        for item in &sorted_work(unparented) {
            blocks.extend(work_blocks(cx, item));
        }
    }

    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}

fn work_blocks<'a>(cx: RenderContext<'a>, record: &'a Record) -> Vec<String> {
    let mut blocks = vec![format!("### {}", record.title), metadata_line(cx, record)];
    if let Some(intent) = prose(record, ContentSlot::Intent) {
        blocks.push(intent);
    }
    if let Some(note) = note_block(record) {
        blocks.push(note);
    }
    if let Some(line) = plan_of_record_line(cx, record) {
        blocks.push(line);
    }
    blocks.extend(disposition_blocks(record, cx.ledger()));
    if let Some(block) = blocked_by_block(cx, record) {
        blocks.push(block);
    }
    blocks.extend(acceptance_blocks(cx, record));
    if let Some(quote) = super::review_required::freshness_quote(cx, &record.id) {
        blocks.push(quote);
    }
    blocks
}

/// `` `state` · `@key/rev` [· part of `@parent`] [· marker] ``
fn metadata_line(cx: RenderContext<'_>, record: &Record) -> String {
    let mut parts = vec![
        format!("`{}`", record.state.name()),
        format!("`@{}`", record.id),
    ];
    if let Some(parent) = record
        .targets(Relation::PartOf)
        .first()
        .and_then(|r| cx.ledger().resolve(r).ok().flatten())
    {
        parts.push(format!("part of `@{}`", parent.id));
    }
    if let Some(marker) = cx.freshness.marker(&record.id) {
        parts.push(marker.to_owned());
    }
    parts.join(" · ")
}

/// `**Plan of record for** <link> \`@id\`` when this work item designates one.
fn plan_of_record_line(cx: RenderContext<'_>, record: &Record) -> Option<String> {
    let target = record
        .targets(Relation::PlanOfRecord)
        .first()
        .and_then(|r| cx.ledger().resolve(r).ok().flatten())?;
    Some(format!(
        "**Plan of record for** {} `@{}`",
        link(target),
        target.id
    ))
}

/// A plan's own `disposition` blocks, in full — the record of what happened to the
/// previous plan's unfinished children (D-017), rendered nowhere else a casual reader
/// will see it.
fn disposition_blocks(record: &Record, ledger: &Ledger) -> Vec<String> {
    if record.dispositions.is_empty() {
        return Vec::new();
    }
    let mut lines = vec!["**Dispositions**".to_owned()];
    let mut items = Vec::new();
    for disposition in &record.dispositions {
        let Some(target) = ledger.resolve(&disposition.target).ok().flatten() else {
            continue;
        };
        let mut line = format!("- `@{}` — `{}`", target.id, disposition.outcome.name());
        if let Some(into) = &disposition.into
            && let Some(into_target) = ledger.resolve(into).ok().flatten()
        {
            line.push_str(&format!(" into `@{}`", into_target.id));
        }
        if let Some(note) = &disposition.note {
            line.push_str(&format!(" — {}", super::common::one_line(note)));
        }
        items.push(line);
    }
    if items.is_empty() {
        return Vec::new();
    }
    lines.push(items.join("\n"));
    lines
}

/// The live `blocks` edges holding this record, each naming the blocker: "blocked"
/// without "by what" is the least useful status in software.
fn blocked_by_block(cx: RenderContext<'_>, record: &Record) -> Option<String> {
    if record.state != crate::model::State::Blocked {
        return None;
    }
    let ledger = cx.ledger();
    let mut lines: Vec<String> = Vec::new();
    for other in crate::graph::sorted_records(ledger) {
        if !other.is_live() {
            continue;
        }
        let blocks_this = other.targets(Relation::Blocks).iter().any(|r| {
            ledger
                .resolve(r)
                .ok()
                .flatten()
                .is_some_and(|t| t.id == record.id)
        });
        if blocks_this {
            lines.push(format!(
                "- `{}` {} `@{}`",
                other.state.name(),
                link(other),
                other.id
            ));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!("**Blocked by**\n{}", lines.join("\n")))
}

/// The acceptance heading and table, mirroring `ROADMAP.md`'s rendering, for a work
/// record that carries checks.
fn acceptance_blocks(cx: RenderContext<'_>, record: &Record) -> Vec<String> {
    let verdicts = cx.model.checks_of(&record.id);
    if verdicts.is_empty() {
        return Vec::new();
    }
    let satisfied = verdicts.iter().filter(|v| v.verdict.is_satisfied()).count();
    let mut table = String::from("| Check | Method | Verdict |\n| --- | --- | --- |");
    for entry in &verdicts {
        let method = record
            .acceptance
            .as_ref()
            .and_then(|a| a.checks.iter().find(|c| c.id == entry.check))
            .map_or("manual", |c| c.method.name());
        table.push_str(&format!(
            "\n| `{}` | {method} | {} |",
            entry.check,
            verdict_text(&entry.verdict)
        ));
    }
    vec![
        format!(
            "**Acceptance** — {satisfied} of {} satisfied",
            verdicts.len()
        ),
        table,
    ]
}

fn verdict_text(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Satisfied { by, .. } => format!("**satisfied** by `@{by}`"),
        Verdict::NoEvidence => "not satisfied — no evidence".to_owned(),
        Verdict::Unresolved => "not satisfied — the cited evidence does not resolve".to_owned(),
        Verdict::Failing { by, result } => format!(
            "not satisfied — `@{by}` reports `{}`",
            result.map_or("no result", crate::model::EvidenceResult::name)
        ),
        Verdict::TooOld { by, .. } => format!("not satisfied — `@{by}` predates the last change"),
    }
}

/// The direct `part_of` parent, resolved, ignoring the target's kind: `ROADMAP.md`
/// order covers milestones and tracks; a `work` parent (nested work) is grouped too, by
/// key, since the source query does not exclude it (`docs/11-projections.md` §7).
fn direct_parent(cx: RenderContext<'_>, record: &Record) -> Option<RevisionId> {
    record
        .targets(Relation::PartOf)
        .first()
        .and_then(|r| cx.ledger().resolve(r).ok().flatten())
        .map(|t| t.id.clone())
}

fn live_work(ledger: &Ledger) -> Vec<&Record> {
    ledger
        .records()
        .iter()
        .filter(|r| r.kind == Kind::Work && r.is_live() && !is_archived(r))
        .collect()
}
