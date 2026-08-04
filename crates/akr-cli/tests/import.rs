//! P8's five exit criteria (`docs/13-implementation-roadmap.md` §3), through the binary.
//!
//! 1. A legacy document with no model available imports — one record per heading.
//! 2. Everything lands `proposed` with a `source { kind legacy }` block.
//! 3. `--lenient` changes exit status only; the warning list is identical without it.
//! 4. `akr complete` on the tracking record fails with `AKR-R022` while checks stand.
//! 5. Excerpts are byte-identical substrings of the source document.
//!
//! The extraction-level halves of 1 and 5 are in
//! `crates/akr-core/tests/import_extract.rs`; here the same properties are asserted on
//! what the write pipeline actually put on disk, because the formatter re-emits every
//! record and could paraphrase an excerpt without any extraction test noticing.

mod support;

use support::Example;

/// A corpus with one of everything: a document title, four durable-looking sections, a
/// status paragraph, a dead relative link and a live absolute one.
const CORPUS: &str = "\
# Old engine notes

## Determinism

The simulator must produce the same run from the same seed.

## Snapshot boundary

We decided to put a snapshot between the sim and the viewer.

M3 is about 60% done.

## Ambient occlusion

Ambient occlusion is ongoing; no milestone owns it. See [the plan](PLAN-v1.md).

## Nightly soak

A soak test runs every night on the build machine.
";

const DOCUMENT: &str = "docs/legacy/OLD-NOTES.md";

fn with_corpus(name: &str) -> Example {
    let example = Example::materialise(name);
    example.write_file(DOCUMENT, CORPUS);
    example
}

fn import(example: &Example, extra: &[&str]) -> support::Run {
    let mut args = vec!["--lenient", "import", DOCUMENT, "--namespace", "sys"];
    args.extend_from_slice(extra);
    example.run(&args)
}

// -------------------------------------------------------------------------------------
// exit criterion 1 — the deterministic floor, end to end
// -------------------------------------------------------------------------------------

#[test]
fn a_document_imports_with_one_record_per_heading() {
    let example = with_corpus("import-floor");
    let dry = import(&example, &["--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.output());
    for line in [
        "4 durable claims",
        "would propose  sys.requirement.determinism",
        "would propose  sys.decision.snapshot-boundary",
        "would propose  sys.track.ambient-occlusion",
        "would propose  sys.policy.nightly-soak",
        "4 checks to @sys.work.old-notes-import",
        "nothing written (--dry-run)",
    ] {
        assert!(
            dry.stdout.contains(line),
            "missing {line:?}:\n{}",
            dry.stdout
        );
    }

    let before = example.sources();
    assert_eq!(before, example.sources(), "sources digest is stable");
    let real = import(&example, &[]);
    assert_eq!(real.code, 0, "{}", real.output());
    for line in [
        "created sys.requirement.determinism/1",
        "created sys.track.ambient-occlusion/1",
        "created sys.work.old-notes-import/1",
        "akr.lock is now stale",
    ] {
        assert!(
            real.stdout.contains(line),
            "missing {line:?}:\n{}",
            real.stdout
        );
    }
    assert_ne!(before, example.sources(), "the import wrote records");

    // The resulting ledger is valid: the strict check finds nothing to say.
    assert_eq!(example.run(&["build"]).code, 0);
    let check = example.run(&["check"]);
    assert_eq!(check.code, 0, "{}", check.output());
}

// -------------------------------------------------------------------------------------
// exit criteria 2 and 5 — proposed, provenanced, verbatim; asserted from disk
// -------------------------------------------------------------------------------------

#[test]
fn every_imported_record_is_proposed_with_legacy_provenance_and_verbatim_excerpt() {
    let example = with_corpus("import-provenance");
    assert_eq!(import(&example, &[]).code, 0);

    let staged =
        akr_core::ops::Staged::load(&example.root().join(".akr")).expect("the ledger loads");
    let imported: Vec<_> = staged
        .ledger
        .records()
        .iter()
        .filter(|r| {
            r.sources
                .iter()
                .any(|s| s.path.as_deref() == Some(DOCUMENT))
        })
        .collect();
    assert_eq!(imported.len(), 5, "four claims and the tracking record");

    for record in &imported {
        // AKR-M042: everything lands proposed. (No question in this corpus, so the
        // inquiry exception of docs/12 §3 does not soften the assertion.)
        assert_eq!(
            record.state,
            akr_core::model::State::Proposed,
            "{}",
            record.id
        );
        // AKR-M021: provenance is a legacy source block.
        let source = record
            .sources
            .iter()
            .find(|s| s.kind == akr_core::model::SourceKind::Legacy)
            .unwrap_or_else(|| panic!("{} has no legacy source", record.id));
        // Exit criterion 5, on what was actually written: the excerpt survived the
        // model round trip and the canonical formatter byte-identical.
        if let Some(excerpt) = &source.excerpt {
            assert!(
                CORPUS.contains(excerpt.as_str()),
                "{} paraphrased its excerpt: {excerpt:?}",
                record.id
            );
        }
    }
    // And the claims all carry one; only the tracking record's source block goes
    // without.
    let with_excerpts = imported
        .iter()
        .filter(|r| r.sources.iter().any(|s| s.excerpt.is_some()))
        .count();
    assert_eq!(with_excerpts, 4);
}

// -------------------------------------------------------------------------------------
// exit criterion 3 — --lenient changes exit status only
// -------------------------------------------------------------------------------------

#[test]
fn lenient_changes_the_exit_status_and_not_the_warning_list() {
    let example = with_corpus("import-lenient");
    let strict = example.run(&["import", DOCUMENT, "--namespace", "sys", "--dry-run"]);
    let lenient = example.run(&[
        "--lenient",
        "import",
        DOCUMENT,
        "--namespace",
        "sys",
        "--dry-run",
    ]);

    // The dead link is a warning either way; strict adds AKR-M041 and fails.
    assert_eq!(strict.code, 1, "{}", strict.output());
    assert_eq!(lenient.code, 0, "{}", lenient.output());
    let warnings = |run: &support::Run| -> Vec<String> {
        run.stdout
            .lines()
            .filter(|l| l.starts_with("warning["))
            .map(str::to_owned)
            .collect()
    };
    let strict_warnings = warnings(&strict);
    assert_eq!(strict_warnings, warnings(&lenient));
    assert!(
        strict_warnings.iter().any(|w| w.contains("AKR-M022")),
        "{strict_warnings:?}"
    );
    assert!(strict.stdout.contains("AKR-M041"), "{}", strict.stdout);
    assert!(!lenient.stdout.contains("AKR-M041"), "{}", lenient.stdout);

    // And without --dry-run, the strict invocation writes nothing at all.
    let before = example.sources();
    let refused = example.run(&["import", DOCUMENT, "--namespace", "sys"]);
    assert_eq!(refused.code, 1, "{}", refused.output());
    assert!(refused.stdout.contains("nothing written (AKR-M041)"));
    assert_eq!(
        before,
        example.sources(),
        "a refused import wrote something"
    );
}

// -------------------------------------------------------------------------------------
// exit criterion 4 — the tracking record cannot be closed early
// -------------------------------------------------------------------------------------

#[test]
fn completing_the_tracking_record_fails_while_any_check_is_unsatisfied() {
    let example = with_corpus("import-tracking");
    assert_eq!(import(&example, &[]).code, 0);

    let refused = example.run(&["complete", "sys.work.old-notes-import"]);
    assert_ne!(refused.code, 0, "{}", refused.output());
    assert!(
        refused.output().contains("AKR-R022"),
        "{}",
        refused.output()
    );
    assert!(
        refused.output().contains("determinism-claim"),
        "the refusal names the checks:\n{}",
        refused.output()
    );
}

// -------------------------------------------------------------------------------------
// the AKR-M faults
// -------------------------------------------------------------------------------------

#[test]
fn the_document_faults_are_diagnostics_not_environment_failures() {
    let example = Example::materialise("import-faults");

    let missing = example.run(&["import", "docs/legacy/ABSENT.md"]);
    assert_eq!(missing.code, 1, "{}", missing.output());
    assert!(
        missing.output().contains("AKR-M001"),
        "{}",
        missing.output()
    );

    example.write_file("docs/legacy/SLIDES.pdf", "%PDF-1.4\n");
    let format = example.run(&["import", "docs/legacy/SLIDES.pdf"]);
    assert_eq!(format.code, 1, "{}", format.output());
    assert!(format.output().contains("AKR-M002"), "{}", format.output());
}

#[test]
fn an_undeclared_namespace_is_m013_and_the_default_comes_from_the_path() {
    let example = with_corpus("import-namespace");
    // The document's first path segment is `docs`, which save-your-skin does not
    // declare — so the default fails, and says how to fix it.
    let defaulted = example.run(&["--lenient", "import", DOCUMENT]);
    assert_eq!(defaulted.code, 1, "{}", defaulted.output());
    assert!(
        defaulted.output().contains("AKR-M013"),
        "{}",
        defaulted.output()
    );
    assert!(
        defaulted.output().contains("--namespace"),
        "{}",
        defaulted.output()
    );

    let named = example.run(&["--lenient", "import", DOCUMENT, "--namespace", "nope"]);
    assert!(named.output().contains("AKR-M013"), "{}", named.output());
}

#[test]
fn a_colliding_key_is_m012_and_writes_nothing() {
    let example = with_corpus("import-collision");
    assert_eq!(import(&example, &[]).code, 0);
    let before = example.sources();
    let again = import(&example, &[]);
    assert_eq!(again.code, 1, "{}", again.output());
    assert!(again.output().contains("AKR-M012"), "{}", again.output());
    assert!(again.output().contains("akr revise"), "{}", again.output());
    assert_eq!(
        before,
        example.sources(),
        "a refused import wrote something"
    );
}

#[test]
fn a_document_with_nothing_durable_is_m011_and_writes_nothing() {
    let example = Example::materialise("import-empty");
    example.write_file("docs/legacy/EMPTY.md", "\n\n");
    let before = example.sources();
    let run = example.run(&[
        "--lenient",
        "import",
        "docs/legacy/EMPTY.md",
        "--namespace",
        "sys",
    ]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(run.output().contains("AKR-M011"), "{}", run.output());
    assert!(run.stdout.contains("nothing written"), "{}", run.stdout);
    assert_eq!(before, example.sources());
}

// -------------------------------------------------------------------------------------
// the audit, through `akr check`
// -------------------------------------------------------------------------------------

#[test]
fn deleting_the_document_after_import_turns_up_m022_on_check() {
    let example = with_corpus("import-audit");
    assert_eq!(import(&example, &[]).code, 0);
    assert_eq!(example.run(&["build"]).code, 0);
    assert_eq!(example.run(&["check"]).code, 0);

    std::fs::remove_file(example.root().join(DOCUMENT)).expect("the document goes");
    let strict = example.run(&["check"]);
    assert!(strict.output().contains("AKR-M022"), "{}", strict.output());
    assert_eq!(strict.code, 1, "strict makes the warning fatal (D-013)");
    let lenient = example.run(&["--lenient", "check"]);
    assert!(
        lenient.output().contains("AKR-M022"),
        "{}",
        lenient.output()
    );
    assert_eq!(lenient.code, 0, "{}", lenient.output());
}

// -------------------------------------------------------------------------------------
// the other example
// -------------------------------------------------------------------------------------

#[test]
fn the_sys_tandem_legacy_roadmap_imports() {
    // The real pre-AKR document the sys-tandem example was distilled from, imported
    // under its own namespace. This is the command meeting the artefact P8 exists for.
    let example = Example::of(&support::SYS_TANDEM, "import-tandem");
    let source = std::fs::read_dir(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sys-tandem/legacy"),
    )
    .expect("the legacy directory exists")
    .next()
    .expect("a legacy document")
    .expect("readable")
    .path();
    let text = std::fs::read_to_string(&source).expect("the document reads");
    example.write_file("legacy/roadmap.md", &text);

    let dry = example.run(&[
        "--lenient",
        "import",
        "legacy/roadmap.md",
        "--namespace",
        "tandem",
        "--dry-run",
    ]);
    assert_eq!(dry.code, 0, "{}", dry.output());
    assert!(dry.stdout.contains("durable claims"), "{}", dry.stdout);
    assert!(!dry.stdout.contains("0 durable claims"), "{}", dry.stdout);

    let real = example.run(&[
        "--lenient",
        "import",
        "legacy/roadmap.md",
        "--namespace",
        "tandem",
    ]);
    assert_eq!(real.code, 0, "{}", real.output());
    assert_eq!(example.run(&["build"]).code, 0);
    assert_eq!(example.run(&["--lenient", "check"]).code, 0);
}
