//! `REVIEW-REQUIRED.md` — what should not be trusted without re-checking.
//!
//! Normative specification: `docs/11-projections.md` §8. This is the committed half of
//! `akr review-queue` (`crates/akr-cli`): both read the same [`super::Freshness`] the
//! build derived, so a reader comparing the two never finds them disagreeing.
//!
//! **Source query.** Every record flagged `stale` or `at_risk` by stage D.
//!
//! **Section order.** Stale, then At risk, using the review-queue ordering of
//! `docs/10-freshness-and-git.md` §7.

use super::common::link;
use super::{RenderContext, banner, slug};
use crate::freshness::StaleCause;
use crate::model::{Record, RevisionId};

/// Renders `REVIEW-REQUIRED.md`.
#[must_use]
pub fn render_review_required(cx: RenderContext<'_>) -> String {
    let ledger = cx.ledger();
    let mut blocks: Vec<String> = Vec::new();
    blocks.push(banner(cx.model).trim_end().to_owned());
    blocks.push(format!("# Review required — {}", ledger.project.name));
    blocks.push(
        "What should not be trusted without re-checking: records the build flagged \
         `stale` or `at_risk`. Neither flag means a record is wrong (D-003); both mean \
         look at it. This view is generated on every successful build, including one \
         that exits 0 with a long queue (D-024). An empty file on an active project is \
         more often a sign the `watches` globs are wrong than a sign the knowledge is \
         perfect."
            .to_owned(),
    );

    let stale = cx.freshness.stale_in_order();
    let at_risk = cx.freshness.at_risk_in_order();

    blocks.push(format!("## Stale ({})", stale.len()));
    if stale.is_empty() {
        blocks.push("_(none)_".to_owned());
    }
    for id in &stale {
        if let Some(record) = ledger.get(id) {
            blocks.extend(stale_blocks(cx, record));
        }
    }

    blocks.push(format!("## At risk ({})", at_risk.len()));
    if at_risk.is_empty() {
        blocks.push("_(none)_".to_owned());
    }
    for entry in &at_risk {
        if let Some(record) = ledger.get(&entry.id) {
            blocks.extend(at_risk_blocks(cx, record, entry));
        }
    }

    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}

fn stale_blocks(cx: RenderContext<'_>, record: &Record) -> Vec<String> {
    let mut blocks = vec![
        format!("### {}", record.title),
        format!(
            "`{}` · `@{}` · {} · **stale** · {}",
            record.state.name(),
            record.id,
            record.kind.name(),
            link(record)
        ),
    ];
    if let Some(cause) = cx.freshness.cause(&record.id) {
        blocks.push(format!("**Cause** — {}", cause_text(cause)));
    }
    blocks
}

fn at_risk_blocks(
    cx: RenderContext<'_>,
    record: &Record,
    entry: &crate::graph::AtRisk,
) -> Vec<String> {
    let mut blocks = vec![
        format!("### {}", record.title),
        format!(
            "`{}` · `@{}` · {} · **depth {}** · {}",
            record.state.name(),
            record.id,
            record.kind.name(),
            entry.depth,
            link(record)
        ),
    ];
    blocks.push(format!("**Via** {}", via_path(cx, entry)));
    blocks
}

/// `` `supported_by` → `@id` → `@id` (stale: <cause>) ``, naming every hop.
fn via_path(cx: RenderContext<'_>, entry: &crate::graph::AtRisk) -> String {
    let mut hops: Vec<String> = vec![format!("`{}`", entry.via.name())];
    hops.extend(entry.path.iter().map(|id| format!("`@{id}`")));
    let mut line = hops.join(" → ");
    if let Some(last) = entry.path.last() {
        let note = if cx.freshness.is_stale(last) {
            cx.freshness
                .cause(last)
                .map_or("stale".to_owned(), |c| format!("stale: {}", cause_text(c)))
        } else if let Some(inner) = cx.freshness.at_risk(last) {
            format!("at risk, depth {}", inner.depth)
        } else {
            String::new()
        };
        if !note.is_empty() {
            line.push_str(&format!(" ({note})"));
        }
    }
    line
}

/// The freshness block quote a record carries in the view that hosts it (§3): "stale" or
/// "at risk", the reason, and a link into `REVIEW-REQUIRED.md`.
///
/// Shared with other renderers (`current_state`, `active_work`) so the wording is the
/// same wherever the marker appears.
#[must_use]
pub(super) fn freshness_quote(cx: RenderContext<'_>, id: &RevisionId) -> Option<String> {
    let anchor = cx.ledger().get(id).map(|r| slug(&r.title))?;
    if cx.freshness.is_stale(id) {
        let cause = cx
            .freshness
            .cause(id)
            .map_or("No cause was recorded.".to_owned(), cause_text);
        Some(format!(
            "> **Stale** — {cause} See [REVIEW-REQUIRED.md](REVIEW-REQUIRED.md#{anchor})."
        ))
    } else {
        let entry = cx.freshness.at_risk(id)?;
        Some(format!(
            "> **At risk** at depth {} via {}. See [REVIEW-REQUIRED.md](REVIEW-REQUIRED.md#{anchor}).",
            entry.depth,
            via_path(cx, entry)
        ))
    }
}

/// Why a record is stale, in prose (`docs/10-freshness-and-git.md` §3).
fn cause_text(cause: &StaleCause) -> String {
    match cause {
        StaleCause::Watch { glob, commit, path } => format!(
            "`watches {:?}` was matched by `{}`, which touched `{path}`.",
            glob.as_str(),
            &commit.as_str()[..8]
        ),
        StaleCause::ReviewAfter { date } => format!("`review_after {date}` has passed."),
    }
}
