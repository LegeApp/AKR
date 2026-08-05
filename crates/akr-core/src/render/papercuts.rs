//! `PAPERCUTS.md` — small frictions, newest first (D-027).
//!
//! **Source query.** The head revision of every `papercut` record, excluding archived
//! ones. Terminal papercuts (withdrawn, superseded, disproven) are excluded: a papercut
//! someone has explicitly retired has been dealt with.
//!
//! **Order.** `created_at` descending, then key descending — newest first, because the
//! reader of this file is asking "what has been hurting lately". Both are total: the
//! tooling always stamps `created_at`, and the key breaks ties deterministically.
//!
//! **Emission.** [`render_papercuts`] returns `None` when the ledger holds no papercut,
//! so a project that never logs one never grows the file, and the frozen worked example
//! stays byte-identical.

use super::common::is_archived;
use super::{RenderContext, banner};
use crate::model::{ContentSlot, ContentValue, Kind, Record};

/// Renders `PAPERCUTS.md`, or `None` when there is nothing to render.
#[must_use]
pub fn render_papercuts(cx: RenderContext<'_>) -> Option<String> {
    let ledger = cx.ledger();
    let mut papercuts: Vec<&Record> = cx
        .model
        .heads
        .values()
        .filter_map(|id| ledger.get(id))
        .filter(|record| record.kind == Kind::Papercut && record.is_live() && !is_archived(record))
        .collect();
    if papercuts.is_empty() {
        return None;
    }
    papercuts.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });

    let mut blocks: Vec<String> = Vec::new();
    blocks.push(banner(cx.model).trim_end().to_owned());
    blocks.push(format!("# Papercuts — {}", ledger.project.name));
    blocks.push(
        "Small frictions hit while working, logged in the moment (D-027). None of these \
         blocked anything; together they show where the project needs sanding down. \
         Newest first."
            .to_owned(),
    );

    let mut items: Vec<String> = Vec::new();
    for record in &papercuts {
        let date = record
            .created_at
            .map_or_else(String::new, |d| format!("{d} "));
        let author = record
            .author
            .as_deref()
            .map_or_else(String::new, |a| format!("[{a}] "));
        items.push(format!(
            "- {date}{author}{}  `@{}`",
            statement_line(record),
            record.id
        ));
    }
    blocks.push(items.join("\n"));

    let mut out = blocks.join("\n\n");
    out.push('\n');
    Some(out)
}

/// The statement as one line: prose newlines become spaces, because the bullet list is
/// the view's whole layout and a multi-line entry would break it.
fn statement_line(record: &Record) -> String {
    match record.get(ContentSlot::Statement) {
        Some(ContentValue::Prose(text) | ContentValue::Text(text)) => text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        _ => record.title.clone(),
    }
}
