//! Exit criterion 1 and the round-trip invariants of `docs/03` §7.

use akr_core::diagnostics::FileId;
use akr_core::syntax::{format, format_source, parse};

fn repo(path: &str) -> String {
    let full = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_exemplar_round_trips_byte_identically() {
    let source = repo("spec/exemplar.akr");
    let parsed = parse(&source, FileId(0));
    assert!(
        parsed.diagnostics.is_empty(),
        "exemplar must parse clean, got {:?}",
        parsed
            .diagnostics
            .iter()
            .map(|d| (d.code.as_str(), &d.message))
            .collect::<Vec<_>>()
    );
    let formatted = format(parsed.file.as_ref().expect("exemplar parses"));
    if formatted != source {
        for (n, (a, b)) in source.lines().zip(formatted.lines()).enumerate() {
            assert_eq!(a, b, "line {} differs", n + 1);
        }
        assert_eq!(
            source.len(),
            formatted.len(),
            "length differs after equal lines"
        );
    }
    assert_eq!(formatted, source, "spec/exemplar.akr must be canonical");
}

#[test]
fn formatting_is_idempotent_on_the_exemplar() {
    let source = repo("spec/exemplar.akr");
    let (once, _) = format_source(&source, FileId(0));
    let once = once.expect("formats");
    let (twice, _) = format_source(&once, FileId(0));
    assert_eq!(Some(once), twice);
}
