//! Every code the crate can raise is registered in `spec/diagnostics/codes-lang.md`, and
//! every code stage letter maps to a stage. Codes appear in CI logs and agent
//! transcripts; an unregistered one is a code nobody can look up.

use akr_core::diagnostics::codes::ALL;
use std::collections::BTreeSet;

fn registry() -> BTreeSet<String> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/diagnostics/codes-lang.md"
    );
    let text = std::fs::read_to_string(path).expect("codes-lang.md is readable");
    let mut found = BTreeSet::new();
    let bytes = text.as_bytes();
    for (i, _) in text.match_indices("AKR-") {
        let code = &text[i..(i + 8).min(text.len())];
        if code.len() == 8
            && code.as_bytes()[4].is_ascii_uppercase()
            && code[5..].bytes().all(|b| b.is_ascii_digit())
            && i > 0
            && bytes[i - 1] == b'`'
        {
            found.insert(code.to_owned());
        }
    }
    found
}

#[test]
fn every_code_the_crate_raises_is_registered() {
    let registered = registry();
    assert!(
        registered.len() > 80,
        "registry parse found only {}",
        registered.len()
    );
    for code in ALL {
        assert!(
            registered.contains(code.as_str()),
            "{code} is raised in code but absent from spec/diagnostics/codes-lang.md"
        );
    }
}

#[test]
fn every_code_has_a_stage_and_lives_in_a_language_stage() {
    use akr_core::diagnostics::Stage;
    for code in ALL {
        let stage = code
            .stage()
            .unwrap_or_else(|| panic!("{code} has no stage letter"));
        assert!(
            matches!(
                stage,
                Stage::Parse | Stage::Format | Stage::Type | Stage::Link | Stage::Resolve
            ),
            "{code} is outside the language stages this crate owns"
        );
    }
}

#[test]
fn codes_are_unique() {
    let unique: BTreeSet<&str> = ALL.iter().map(|c| c.as_str()).collect();
    assert_eq!(unique.len(), ALL.len(), "a code constant is duplicated");
}
