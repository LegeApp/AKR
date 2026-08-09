//! The write commands through the binary: `docs/07-cli.md` §4 and §6.
//!
//! Exit criterion 3 of P6 — a refused write leaves the working tree byte-identical — is
//! proved at library level in `akr-core/tests/ops_atomicity.rs`. This is the same claim
//! made where a user can observe it: one refusing path per command, run as a process,
//! with every file under `.akr/` hashed either side of the call.
//!
//! The refusal *shapes* are checked here too, against the structured fields of
//! `ops::Refused` rather than against message text — the point of the structure being
//! that the rendering and the library can be checked independently.

mod support;

use support::Example;

/// Asserts that a command refuses, exits 1, and writes nothing.
fn refuses(example: &Example, args: &[&str]) -> String {
    let before = example.sources();
    let run = example.run(args);
    let after = example.sources();
    assert_eq!(
        run.code,
        1,
        "akr {} should refuse with exit 1:\n{}",
        args.join(" "),
        run.output()
    );
    assert_eq!(
        before,
        after,
        "akr {} refused but changed the working tree",
        args.join(" ")
    );
    assert!(
        run.output().contains("nothing written"),
        "akr {} does not say the tree is untouched:\n{}",
        args.join(" "),
        run.output()
    );
    run.output()
}

#[test]
fn propose_refuses_an_existing_key_and_writes_nothing() {
    let example = Example::materialise("write-propose");
    let text = refuses(
        &example,
        &["propose", "sys.term.playable-day", "--kind", "term"],
    );
    assert!(text.contains("AKR-L041"), "{text}");
    assert!(text.contains("akr revise"), "names the way forward: {text}");
}

#[test]
fn revise_refuses_an_in_place_edit_of_a_sealed_head() {
    let example = Example::materialise("write-revise");
    let text = refuses(
        &example,
        &[
            "revise",
            "sys.term.playable-day",
            "--in-place",
            "--title",
            "Something else",
        ],
    );
    assert!(text.contains("AKR-C032"), "{text}");
}

#[test]
fn supersede_lists_the_children_it_needs_a_disposition_for() {
    let example = Example::materialise("write-supersede");
    // D-017's demand attaches to a *pinned* `part_of @key/n`, which is what makes a child
    // the property of one plan revision rather than of the key. The worked example pins
    // the plan's children; this pins one of the milestone's so the refusal has something
    // to list without depending on the plan's own disposition blocks.
    let path = ".akr/records/sim/work.akr";
    let source = example.read_file(path);
    example.write_file(
        path,
        &source.replace(
            "part_of [ @sys.milestone.m3-playable-day ]",
            "part_of [ @sys.milestone.m3-playable-day/1 ]",
        ),
    );

    let text = refuses(&example, &["supersede", "sys.milestone.m3-playable-day"]);
    assert!(text.contains("unfinished child"), "{text}");
    assert!(text.contains("@sim.work.rewrite-projection"), "{text}");
    assert!(text.contains("blocked"), "names the state: {text}");
    assert!(
        text.contains("--disposition sim.work.rewrite-projection=intentionally_dropped"),
        "offers a runnable fix: {text}"
    );

    // And the fix works, which is the half of the interaction that matters.
    let applied = example.run(&[
        "supersede",
        "sys.milestone.m3-playable-day",
        "--disposition",
        "sim.work.rewrite-projection=intentionally_dropped",
    ]);
    assert_eq!(applied.code, 0, "{}", applied.output());
    assert!(applied.stdout.contains("sys.milestone.m3-playable-day/2"));
    assert!(
        applied.stdout.contains("akr.lock is now stale"),
        "a write always stales the lock (D-014): {}",
        applied.stdout
    );
}

#[test]
fn complete_names_the_unsatisfied_check_and_writes_nothing() {
    let example = Example::materialise("write-complete");
    let text = refuses(&example, &["complete", "sys.milestone.m3-playable-day"]);
    assert!(text.contains("AKR-R022"), "{text}");
    assert!(text.contains("no-placeholder-assets"), "{text}");
    assert!(
        text.contains("--check <id>=<evidence-ref>"),
        "offers the fix: {text}"
    );
}

#[test]
fn abandon_requires_a_reason_and_writes_nothing() {
    let example = Example::materialise("write-abandon");
    let text = refuses(&example, &["abandon", "sys.work.m3-plan"]);
    assert!(text.contains("AKR-C031"), "{text}");
    assert!(text.contains("--reason"), "{text}");
}

#[test]
fn evidence_add_refuses_an_incomplete_request_without_writing() {
    let example = Example::materialise("write-evidence-refuse");
    let before = example.sources();
    // A missing `--result` cannot even be turned into a request, so it never reaches the
    // pipeline; the guarantee is the same and is checked the same way.
    let run = example.run(&["evidence", "add", "sys.evidence.asset-audit"]);
    assert_eq!(run.code, 3, "{}", run.output());
    assert!(run.output().contains("AKR-C003"), "{}", run.output());
    assert_eq!(before, example.sources());

    let bad = example.run(&[
        "evidence",
        "add",
        "sys.evidence.asset-audit",
        "--result",
        "maybe",
        "--method",
        "command",
    ]);
    assert_eq!(bad.code, 3, "{}", bad.output());
    assert!(bad.output().contains("AKR-C004"), "{}", bad.output());
    assert_eq!(before, example.sources());
}

// -------------------------------------------------------------------------------------
// The paths that succeed
// -------------------------------------------------------------------------------------

#[test]
fn a_proposal_with_no_body_is_refused_outright() {
    let example = Example::materialise("write-propose-bodyless");
    // §4 validates the *resulting* ledger, and a record with no required prose slot does
    // not validate. So `--from`/`--edit` is not a convenience on `akr propose`: without
    // one there is nothing to write that the pipeline will accept. §6 says so explicitly;
    // this is that sentence being true.
    let text = refuses(
        &example,
        &[
            "propose",
            "sys.term.day-loop",
            "--kind",
            "term",
            "--title",
            "The day loop",
        ],
    );
    assert!(text.contains("AKR-C031"), "{text}");
    assert!(
        text.contains("definition"),
        "names the missing slot: {text}"
    );
}

#[test]
fn a_proposal_from_a_file_lands_and_checks_clean() {
    let example = Example::materialise("write-propose-from");
    let body = example.root().join("day-loop.akr");
    std::fs::write(
        &body,
        "akr 0.1\nproject save-your-skin\n\nrecord sys.term.day-loop/1 : term {\n    \
         title \"The day loop\"\n    state active\n    scope [ all ]\n    definition \"\"\"\n        \
         The repeating structure of one in-game day: wake, work, evening, sleep.\n        \
         \"\"\"\n    author \"test\"\n    created_at 2026-08-03\n}\n",
    )
    .expect("write body");

    let run = example.run(&[
        "propose",
        "sys.term.day-loop",
        "--kind",
        "term",
        "--from",
        body.to_str().expect("utf-8"),
    ]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        example
            .read_file(".akr/records/sys/terms.akr")
            .contains("sys.term.day-loop/1")
    );
    // The lock is stale until the next build (D-014), so the ledger only checks clean
    // after one. That is the sequence every write leaves behind.
    assert_eq!(example.run(&["build"]).code, 0);
    assert_eq!(example.run(&["check"]).code, 0);
}

#[test]
fn revise_on_a_sealed_head_retires_it_in_the_same_write() {
    let example = Example::materialise("write-revise-ok");
    let run = example.run(&[
        "revise",
        "sys.term.playable-day",
        "--title",
        "A playable day",
    ]);
    assert_eq!(run.code, 0, "{}", run.output());
    // Both halves in one write: revision 2 created and revision 1 retired. Leaving the old
    // head live would be two live heads, which V-012 refuses, and §4 refuses to write a
    // ledger that does not validate — so they cannot be separated.
    assert!(
        run.stdout.contains("created sys.term.playable-day/2"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("sys.term.playable-day/1 active -> superseded"),
        "{}",
        run.stdout
    );
    let source = example.read_file(".akr/records/sys/terms.akr");
    assert!(source.contains("state superseded"));
    assert!(source.contains("supersedes [ @sys.term.playable-day/1 ]"));

    let before_build = example.run(&["check"]);
    assert_eq!(before_build.code, 1, "{}", before_build.output());
    assert!(before_build.output().contains("AKR-R052"));
    assert!(!before_build.output().contains("AKR-R051"));
}

#[test]
fn abandon_writes_the_reason_into_the_note_slot() {
    let example = Example::materialise("write-abandon-ok");
    let run = example.run(&[
        "abandon",
        "sys.work.m3-plan",
        "--reason",
        "The milestone was rescoped and this plan no longer describes it.",
    ]);
    assert_eq!(run.code, 0, "{}", run.output());
    let source = example.read_file(".akr/records/sys/work.akr");
    // D-026: a comment would be excluded from the seal hash and invisible to every view.
    assert!(source.contains("    note"), "{source}");
    assert!(source.contains("no longer describes it"), "{source}");
    assert!(!source.contains("# The milestone was rescoped"), "{source}");
}

#[test]
fn evidence_add_creates_a_record_that_completes_a_check() {
    let example = Example::materialise("write-evidence-ok");
    let head = example.commit(5).to_owned();
    let run = example.run(&[
        "evidence",
        "add",
        "sys.evidence.asset-audit",
        "--result",
        "pass",
        "--method",
        "command",
        "--command",
        "cargo run -p tools -- audit-assets --path content/day-loop",
        "--summary",
        "Zero placeholder assets on the day-loop path.",
        "--observed-at",
        &head,
    ]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        run.stdout.contains("created sys.evidence.asset-audit/1"),
        "{}",
        run.stdout
    );

    // Evidence never declares what it verifies (D-016); the link is made on the check.
    // Completing the milestone still fails, because V-019 will not leave an active
    // `plan_of_record` pointing at a completed milestone — §6 says the plan is retired
    // first, and this is the refusal that says so.
    let blocked = example.run(&[
        "complete",
        "sys.milestone.m3-playable-day",
        "--check",
        "no-placeholder-assets=@sys.evidence.asset-audit/1",
    ]);
    assert_eq!(blocked.code, 1, "{}", blocked.output());
    assert!(
        blocked.output().contains("AKR-R021"),
        "{}",
        blocked.output()
    );

    // Retire the plan, and the same call goes through.
    assert_eq!(
        example
            .run(&[
                "abandon",
                "sys.work.m3-plan",
                "--reason",
                "The milestone is complete; the plan has nothing left to schedule.",
            ])
            .code,
        0
    );
    let complete = example.run(&[
        "complete",
        "sys.milestone.m3-playable-day",
        "--check",
        "no-placeholder-assets=@sys.evidence.asset-audit/1",
    ]);
    assert_eq!(complete.code, 0, "{}", complete.output());
    assert!(
        example
            .read_file(".akr/records/sys/milestones.akr")
            .contains("state completed")
    );
}

#[test]
fn a_write_is_visible_to_the_next_read_and_the_lock_says_so() {
    let example = Example::materialise("write-then-read");
    assert_eq!(example.run(&["lock", "--check"]).code, 0);
    let run = example.run(&[
        "revise",
        "sys.term.playable-day",
        "--title",
        "A playable day",
    ]);
    assert_eq!(run.code, 0, "{}", run.output());

    // The lock records a build, and no write operation may invent one (D-014). Until the
    // next `akr build` the lock is honestly stale, and `akr check` says so rather than
    // pretending otherwise.
    let stale = example.run(&["lock", "--check"]);
    assert_eq!(stale.code, 1, "{}", stale.output());
    assert_eq!(example.run(&["build"]).code, 0);
    assert_eq!(example.run(&["lock", "--check"]).code, 0);
}

#[test]
fn the_json_form_carries_the_structured_refusal() {
    let example = Example::materialise("write-json");
    let run = example.run(&[
        "--format",
        "json",
        "complete",
        "sys.milestone.m3-playable-day",
    ]);
    assert_eq!(run.code, 1, "{}", run.output());
    let text = run.stdout;
    assert!(text.contains("\"command\": \"complete\""), "{text}");
    assert!(text.contains("\"refused\": true"), "{text}");
    assert!(text.contains("\"code\": \"AKR-R022\""), "{text}");
    assert!(text.contains("\"unsatisfied_checks\""), "{text}");
    assert!(text.contains("\"id\": \"no-placeholder-assets\""), "{text}");
    assert!(text.contains("\"exit_code\": 1"), "{text}");
}

// -------------------------------------------------------------------------------------
// `akr papercut collate` (D-030): one master record, sisters never written
// -------------------------------------------------------------------------------------

/// Lays down a minimal `.akr` workspace holding one live papercut, next to the example's
/// own temp root, and returns the scan directory the command should point at.
fn sibling_workspaces(example: &Example) -> std::path::PathBuf {
    // Each Example root already carries a process id plus an atomic counter. Keeping the
    // scan below it prevents the two collation tests in this binary from deleting each
    // other's process-id-only directory when the test harness runs them in parallel.
    let scan = example.root().join("collate-scan");
    let _ = std::fs::remove_dir_all(&scan);
    for (project, slug) in [("alpha", "alpha-annoyance"), ("beta", "beta-friction")] {
        let akr = scan.join(project).join(".akr");
        let records = akr.join("records").join(project);
        std::fs::create_dir_all(&records).expect("sibling records dir");
        std::fs::write(
            akr.join("project.akr"),
            format!(
                "akr 0.1\nproject {project}\n\nnamespace {project} \"{project} knowledge.\"\n\n\
                 defaults {{\n    review_after_days 90\n    view_output \"docs/generated\"\n}}\n"
            ),
        )
        .expect("sibling project.akr");
        std::fs::write(
            records.join("papercuts.akr"),
            format!(
                "akr 0.1\nproject {project}\n\nrecord {project}.papercut.{slug}/1 : papercut {{\n    \
                 title \"{project} friction\"\n    state verified\n    statement \"\"\"\n        A \
                 {project}-side friction worth logging.\n        \"\"\"\n    observed_at \
                 git:0123456789abcdef0123456789abcdef01234567\n    author \"test\"\n    created_at \
                 2026-08-07\n}}\n"
            ),
        )
        .expect("sibling papercuts.akr");

        // …and one whose subject is AKR itself, not this sibling. D-033: this is the
        // record that used to be invisible to whoever maintains the tool.
        std::fs::write(
            records.join("tool-papercuts.akr"),
            format!(
                "akr 0.1\nproject {project}\n\nrecord {project}.papercut.{slug}-tool/1 : papercut {{\n    \
                 title \"{project} hit an akr bug\"\n    state verified\n    statement \"\"\"\n        \
                 An akr diagnostic was misleading.\n        \"\"\"\n    observed_at \
                 git:0123456789abcdef0123456789abcdef01234567\n    about \"akr\"\n    author \"test\"\n    created_at \
                 2026-08-07\n}}\n"
            ),
        )
        .expect("sibling tool-papercuts.akr");
    }
    scan
}

#[test]
fn papercut_collate_gathers_sister_papercuts_once() {
    let example = Example::materialise("write-collate");
    let scan = sibling_workspaces(&example);
    let projects = scan.to_str().expect("utf-8 scan dir");

    let run = example.run(&[
        "papercut",
        "collate",
        "--projects",
        projects,
        "--namespace",
        "sys",
    ]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        run.stdout
            .contains("created sys.papercut.collated-4-papercuts-from-alpha-beta/1"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("wrote"), "{}", run.stdout);

    let source = example.read_file(".akr/records/sys/papercuts.akr");
    assert!(source.contains("collated ["), "{source}");
    assert!(
        source.contains("alpha.papercut.alpha-annoyance"),
        "{source}"
    );
    assert!(source.contains("beta.papercut.beta-friction"), "{source}");
    // The statement carries each absorbed papercut with its owning project — and its
    // full text, not a truncated title. The first real collation stored titles and told
    // the reader to go and open eight other ledgers, so none of it was acted on (D-033).
    assert!(
        source.contains("## alpha @alpha.papercut.alpha-annoyance"),
        "{source}"
    );
    assert!(
        source.contains("## beta @beta.papercut.beta-friction"),
        "{source}"
    );
    assert!(
        source.contains("A alpha-side friction worth logging."),
        "the absorbed statement must be readable here:\n{source}"
    );

    // The sisters were only read: neither workspace grew anything.
    assert!(
        !std::fs::read_to_string(scan.join("alpha/.akr/records/alpha/papercuts.akr"))
            .expect("alpha source")
            .contains("collated"),
        "the sister ledger must be untouched"
    );

    // A rerun is a no-op: the keys are already in a live collation's `collated` slot.
    let again = example.run(&[
        "papercut",
        "collate",
        "--projects",
        projects,
        "--namespace",
        "sys",
    ]);
    assert_eq!(again.code, 0, "{}", again.output());
    assert!(
        again.stdout.contains("nothing new to collate"),
        "{}",
        again.output()
    );
    let source = example.read_file(".akr/records/sys/papercuts.akr");
    assert_eq!(
        source.matches("record sys.papercut.collated").count(),
        1,
        "a second run must not add another master record:\n{source}"
    );

    let _ = std::fs::remove_dir_all(&scan);
}

#[test]
fn collate_can_narrow_to_one_subject_and_says_what_it_left() {
    let example = Example::materialise("write-collate-about");
    let scan = sibling_workspaces(&example);
    let projects = scan.to_str().expect("utf-8 scan dir");

    // The case the `about` slot exists for: an agent working in a sibling hit a friction
    // with *akr*, and the only ledger it could reach was the sibling's (D-033).
    let run = example.run(&[
        "papercut",
        "collate",
        "--projects",
        projects,
        "--about",
        "akr",
        "--namespace",
        "sys",
    ]);
    assert_eq!(run.code, 0, "{}", run.output());

    let source = example.read_file(".akr/records/sys/papercuts.akr");
    assert!(
        source.contains("alpha.papercut.alpha-annoyance-tool"),
        "the akr-subject papercut should have been absorbed:\n{source}"
    );
    assert!(
        !source.contains("\"alpha.papercut.alpha-annoyance\""),
        "the sibling's own friction is not ours to collate:\n{source}"
    );
    // What the filter left behind is counted, never dropped in silence.
    assert!(
        run.stdout.contains("left behind by the subject filter"),
        "{}",
        run.output()
    );
    assert!(
        run.stdout.contains("subjects seen: akr"),
        "{}",
        run.output()
    );

    let _ = std::fs::remove_dir_all(&scan);
}

#[test]
fn a_papercut_can_name_what_it_was_about() {
    let example = Example::materialise("write-papercut-about");
    let run = example.run(&[
        "papercut",
        "-m",
        "tester",
        "the akr error message did not say which key",
        "--about",
        "akr",
        "--namespace",
        "sys",
    ]);
    assert_eq!(run.code, 0, "{}", run.output());
    let source = example.read_file(".akr/records/sys/papercuts.akr");
    assert!(source.contains("about \"akr\""), "{source}");

    // And it renders under its own heading, so nobody reads it as this project's backlog.
    let build = example.run(&["build"]);
    assert_eq!(build.code, 0, "{}", build.output());
    let view = example.read_file("docs/generated/PAPERCUTS.md");
    assert!(view.contains("## Not about this project"), "{view}");
    assert!(view.contains("(akr)"), "{view}");
}
