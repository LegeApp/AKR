//! Text in, model out, and back again.
//!
//! `docs/03-syntax.md` is normative for meaning and `spec/grammar/akr.ebnf` for shape.
//! The three round-trip invariants of `docs/03` §7 are tested in
//! `tests/round_trip.rs`.

pub mod cst;
pub mod emit;
pub mod format;
pub mod lexer;
pub mod lower;
pub mod parser;

use crate::diagnostics::{Diagnostic, FileId};

pub use emit::{record_node, record_text};
pub use format::format;
pub use lower::{lower_all, lower_file};
pub use parser::{Parsed, parse};

/// Parses and reformats source into its canonical form.
///
/// Returns `None` for the text when the file could not be parsed far enough to print.
#[must_use]
pub fn format_source(text: &str, file: FileId) -> (Option<String>, Vec<Diagnostic>) {
    let parsed = parse(text, file);
    (parsed.file.as_ref().map(format), parsed.diagnostics)
}

/// Checks whether source is already canonical, raising `AKR-F001` if not.
///
/// This is `akr fmt --check`, and it runs before anything else in `akr check`: an
/// uncanonical file has an unstable content hash, and V-024 depends on it.
#[must_use]
pub fn check_formatted(text: &str, file: FileId, path: &str) -> Vec<Diagnostic> {
    use crate::diagnostics::{Label, Severity, Span, Subject, codes};
    let (formatted, mut diagnostics) = format_source(text, file);
    if let Some(formatted) = formatted
        && formatted != text
    {
        diagnostics.push(Diagnostic {
            code: codes::F001,
            severity: Severity::Error,
            rule: None,
            message: format!("{path} is not canonically formatted; run `akr fmt`"),
            primary: Label {
                subject: Subject::File(path.to_owned()),
                span: Some(Span {
                    file,
                    start: 0,
                    end: u32::try_from(text.len()).unwrap_or(u32::MAX),
                }),
                message: None,
            },
            notes: Vec::new(),
            help: None,
        });
    }
    diagnostics
}
