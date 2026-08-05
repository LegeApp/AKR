//! `OPEN-QUESTIONS.md` — what is not yet known.
//!
//! Normative specification: `docs/11-projections.md` §9.
//!
//! **Source query.** `question` records: live ones (`open`, `deferred`) in the main
//! sections, terminal ones (`resolved`, `closed_without_resolution`) in a trailing
//! section. Archived questions are excluded, like everywhere except
//! `DECISION-HISTORY.md` (D-018).
//!
//! **Section order.** Open, Deferred, Recently closed. Within each, by key.

use super::common::{is_archived, link, one_line, prose};
use super::{RenderContext, banner};
use crate::model::{ContentSlot, Kind, Record, Relation, State};

/// Renders `OPEN-QUESTIONS.md`.
#[must_use]
pub fn render_open_questions(cx: RenderContext<'_>) -> String {
    let ledger = cx.ledger();
    let mut blocks: Vec<String> = Vec::new();
    blocks.push(banner(cx.model).trim_end().to_owned());
    blocks.push(format!("# Open questions — {}", ledger.project.name));
    blocks.push("What is not yet known, and what has since been answered.".to_owned());

    let questions = heads_of_kind(cx.model);

    for (heading, state) in [("Open", State::Open), ("Deferred", State::Deferred)] {
        blocks.push(format!("## {heading}"));
        let section: Vec<&Record> = questions
            .iter()
            .filter(|q| q.state == state)
            .copied()
            .collect();
        if section.is_empty() {
            blocks.push("_(none)_".to_owned());
        }
        for record in section {
            blocks.extend(question_blocks(cx, record));
        }
    }

    blocks.push("## Recently closed".to_owned());
    let closed: Vec<&Record> = questions
        .iter()
        .filter(|q| matches!(q.state, State::Resolved | State::ClosedWithoutResolution))
        .copied()
        .collect();
    if closed.is_empty() {
        blocks.push("_(none)_".to_owned());
    }
    for record in closed {
        blocks.extend(question_blocks(cx, record));
    }

    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}

fn question_blocks<'a>(cx: RenderContext<'a>, record: &'a Record) -> Vec<String> {
    let mut blocks = vec![format!("### {}", record.title), metadata_line(cx, record)];
    if let Some(question) = prose(record, ContentSlot::Question) {
        blocks.push(question);
    }
    if let Some(block) = blocks_list(cx, record) {
        blocks.push(block);
    }
    if record.state == State::Resolved {
        if let Some(resolution) = prose(record, ContentSlot::Resolution) {
            blocks.push(format!("**Resolution.** {}", one_line(&resolution)));
        }
        if let Some(line) = resolved_by_line(cx, record) {
            blocks.push(line);
        }
    }
    if let Some(quote) = super::review_required::freshness_quote(cx, &record.id) {
        blocks.push(quote);
    }
    blocks
}

fn metadata_line(cx: RenderContext<'_>, record: &Record) -> String {
    let mut line = format!("`{}` · `@{}`", record.state.name(), record.id);
    if let Some(marker) = cx.freshness.marker(&record.id) {
        line.push_str(&format!(" · {marker}"));
    }
    line
}

/// `**Blocks**` — the live planning or normative records this question holds up, as
/// links: `blocks`'s domain is `question`/`work`/`observation`/`constraint`, so a
/// question is itself a possible source of the edge.
fn blocks_list(cx: RenderContext<'_>, record: &Record) -> Option<String> {
    let ledger = cx.ledger();
    let targets: Vec<String> = record
        .targets(Relation::Blocks)
        .iter()
        .filter_map(|r| ledger.resolve(r).ok().flatten())
        .map(|target| format!("- `{}` {} `@{}`", target.state.name(), link(target), target.id))
        .collect();
    (!targets.is_empty()).then(|| format!("**Blocks**\n{}", targets.join("\n")))
}

/// The live `resolves` edge that closed a resolved question (V-011).
fn resolved_by_line(cx: RenderContext<'_>, record: &Record) -> Option<String> {
    let ledger = cx.ledger();
    let mut resolvers: Vec<&Record> = ledger
        .records()
        .iter()
        .filter(|r| r.is_live())
        .filter(|r| {
            r.targets(Relation::Resolves)
                .iter()
                .filter_map(|reference| ledger.resolve(reference).ok().flatten())
                .any(|t| t.id == record.id)
        })
        .collect();
    resolvers.sort_by(|a, b| a.id.cmp(&b.id));
    let resolver = resolvers.first()?;
    Some(format!(
        "**Resolved by** {} `@{}`",
        link(resolver),
        resolver.id
    ))
}

/// The head revision of every non-archived `question`, sorted by key.
fn heads_of_kind<'a>(model: &'a crate::resolve::ResolvedModel<'a>) -> Vec<&'a Record> {
    let ledger = model.ledger();
    let mut out: Vec<&Record> = model
        .heads
        .values()
        .filter_map(|id| ledger.get(id))
        .filter(|record| record.kind == Kind::Question && !is_archived(record))
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}
