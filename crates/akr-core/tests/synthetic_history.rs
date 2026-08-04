//! The `save-your-skin` synthetic history, materialised as a real git repository.
//!
//! `examples/save-your-skin/MANIFEST.md` §4 freezes a five-commit history and §7 and §9
//! freeze what the tools must say about it. Those expectations have been prose until now:
//! the transcripts in `examples/save-your-skin/transcripts/` were written by hand against
//! them, and nothing checked that a real implementation agrees.
//!
//! This file builds a repository whose file set mirrors the manifest's watched paths,
//! replays the five commits, points the example's records at the resulting hashes, and
//! runs the real freshness computation over it. The frozen expectations become
//! executable.
//!
//! # Why the hashes cannot be the manifest's
//!
//! The manifest's commit hashes are invented — the example's history is fictional
//! (§4) — so a real repository necessarily produces different ones. What is asserted is
//! everything else: which records go stale, why, which records go at risk, at what depth,
//! along which relation, and what `akr impact` says about the C4..C5 range. Those are the
//! claims the manifest actually makes.

mod support;

use akr_core::freshness::{StaleCause, derive, impact_of_range};
use akr_core::model::{Date, Ledger, LogicalKey, RevisionId, key};
use akr_core::resolve::load_workspace;
use std::path::Path;
use support::{Step, SyntheticHistory};

/// "Today", for `review_after` evaluation everywhere in this design set (MANIFEST §4).
fn today() -> Date {
    Date::new(2026, 8, 3).expect("a valid date")
}

fn id(text: &str, revision: u32) -> RevisionId {
    RevisionId::new(key(text), revision)
}

/// The five commits of MANIFEST §4, with the paths each one touched.
///
/// | Id | Touched |
/// | --- | --- |
/// | C1 | `sim/src/**`, `lege/src/**` (initial skeleton) |
/// | C2 | `sim/src/project/**`, `sim/src/step.rs` |
/// | C3 | `lege/src/**`, `sim/src/step.rs` |
/// | C4 | `sim/src/project/**`, `sim/tests/determinism.rs` |
/// | C5 | `lege/src/render/**`, `docs/generated/**` |
const STEPS: &[(&str, &[(&str, &str)])] = &[
    (
        "C1 initial skeleton",
        &[
            ("sim/src/lib.rs", "// simulator\n"),
            ("sim/src/step.rs", "// fixed timestep\n"),
            ("sim/src/project/mod.rs", "// projection pass\n"),
            ("sim/src/tick/mod.rs", "// tick loop\n"),
            ("lege/src/lib.rs", "// viewer\n"),
            ("lege/src/render/mod.rs", "// render graph\n"),
            ("lege/src/light/mod.rs", "// lighting\n"),
            ("content/day/light/dawn.toml", "# dawn keys\n"),
            ("tools/audit.rs", "// asset audit\n"),
            ("docs/generated/.keep", ""),
        ],
    ),
    (
        "C2 projection and step",
        &[
            ("sim/src/project/mod.rs", "// projection pass, revised\n"),
            ("sim/src/step.rs", "// fixed timestep, 8 ms\n"),
        ],
    ),
    (
        "C3 renderer boundary",
        &[
            ("lege/src/lib.rs", "// viewer, frame snapshot boundary\n"),
            ("sim/src/step.rs", "// fixed timestep, accumulator\n"),
        ],
    ),
    (
        "C4 projection rework and determinism suite",
        &[
            ("sim/src/project/mod.rs", "// projection pass, reworked\n"),
            ("sim/tests/determinism.rs", "// 512-seed sweep\n"),
        ],
    ),
    (
        "C5 render graph and views",
        &[
            ("lege/src/render/mod.rs", "// render graph, extracted\n"),
            ("docs/generated/ROADMAP.md", "<!-- generated -->\n"),
        ],
    ),
];

/// The manifest's invented hashes, paired with their position in the table.
const MAPPING: &[(&str, usize)] = &[
    ("3f0a1c9d5b7e2648a0d4f1b8c36e9752ad014b6f", 1),
    ("7c41d0ba92e6f37518a3cd406b5e2f91d8074a63", 2),
    ("b2e58f1406c7a9d3e41b60258fa3d7c6195e0b48", 3),
    ("5d9c2a70e31f8b46c07d5924ab6e3f1074c9d285", 4),
    ("e806b3f54a2d7091c5e13b8a26f490dc7b135e64", 5),
];

fn history() -> SyntheticHistory {
    let steps: Vec<Step<'_>> = STEPS
        .iter()
        .map(|(message, writes)| Step::new(message, writes))
        .collect();
    SyntheticHistory::build("save-your-skin-history", &steps)
}

/// The example's real records, repointed at the materialised history.
fn example_ledger(history: &SyntheticHistory) -> Ledger {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/save-your-skin");
    let workspace = load_workspace(&root, &root.join(".akr")).expect("the example loads");
    history.remap(&workspace.ledger, MAPPING)
}

// -------------------------------------------------------------------------------------

#[test]
fn the_history_touches_the_paths_the_manifest_says_it_does() {
    let history = history();
    let git = history.git();
    let expected: [(usize, &[&str]); 4] = [
        (2, &["sim/src/project/mod.rs", "sim/src/step.rs"]),
        (3, &["lege/src/lib.rs", "sim/src/step.rs"]),
        (4, &["sim/src/project/mod.rs", "sim/tests/determinism.rs"]),
        (5, &["docs/generated/ROADMAP.md", "lege/src/render/mod.rs"]),
    ];
    for (n, paths) in expected {
        let touched: Vec<String> = git
            .touches_in(Some(history.at(n - 1)), history.at(n))
            .expect("touches")
            .into_iter()
            .map(|t| t.path)
            .collect();
        assert_eq!(touched, paths, "C{n}");
    }
}

/// MANIFEST §7 and §9: two stale records and four at risk, propagation depth 2.
#[test]
fn the_frozen_freshness_expectations_hold_against_a_real_repository() {
    let history = history();
    let ledger = example_ledger(&history);
    let git = history.git();

    let queue = derive(&ledger, &git, history.at(5), today()).expect("derives");

    let stale: Vec<String> = queue
        .stale
        .iter()
        .map(|s| format!("{} ({})", s.id, s.cause.name()))
        .collect();
    assert_eq!(
        stale,
        [
            "sim.obs.projection-gaps/1 (watch)",
            "sim.obs.timestep-drift/1 (review_after)",
        ],
        "MANIFEST §7"
    );

    // The watch cause names the glob, the commit and the path.
    match &queue.stale[0].cause {
        StaleCause::Watch { glob, commit, path } => {
            assert_eq!(glob.as_str(), "sim/src/project/**");
            assert_eq!(commit, history.at(4), "C4 touched the projection pass");
            assert_eq!(path, "sim/src/project/mod.rs");
        }
        other => panic!("expected a watch cause, got {other:?}"),
    }

    let at_risk: Vec<String> = queue
        .at_risk
        .iter()
        .map(|r| format!("{} depth {} via {}", r.id, r.depth, r.via))
        .collect();
    assert_eq!(
        at_risk,
        [
            "sim.work.rewrite-projection/1 depth 1 via depends_on",
            "sys.assessment.m3-readiness/1 depth 1 via supported_by",
            "sys.assessment.projection-gaps/1 depth 1 via supported_by",
            "sys.policy.tandem-work/1 depth 2 via supported_by",
        ],
        "MANIFEST §7, as amended: four at risk"
    );
    assert_eq!(queue.at_risk.iter().map(|r| r.depth).max(), Some(2));
}

#[test]
fn the_fresh_observation_stays_fresh() {
    // MANIFEST §7: `lege.obs.frame-budget-headroom/1` watches `lege/src/render/**`, which
    // C5 touched — and its `observed_at` *is* C5, so the change is already accounted for.
    let history = history();
    let ledger = example_ledger(&history);
    let queue = derive(&ledger, &history.git(), history.at(5), today()).expect("derives");
    assert!(
        !queue
            .stale
            .iter()
            .any(|s| s.id == id("lege.obs.frame-budget-headroom", 1))
    );
}

#[test]
fn the_disproven_observation_is_never_evaluated() {
    // `lege.obs.viewer-imports-engine/1` watches `lege/**`, which C3 and C5 both touched.
    // It is `disproven`, so it is not evaluated at all (MANIFEST §7).
    let history = history();
    let ledger = example_ledger(&history);
    let queue = derive(&ledger, &history.git(), history.at(5), today()).expect("derives");
    assert!(
        !queue
            .stale
            .iter()
            .any(|s| s.id == id("lege.obs.viewer-imports-engine", 1))
    );
}

/// MANIFEST §9: `akr impact --git-diff C4..C5` reports no newly stale records.
#[test]
fn the_frozen_impact_expectation_holds_against_a_real_repository() {
    let history = history();
    let ledger = example_ledger(&history);
    let git = history.git();

    let already = derive(&ledger, &git, history.at(5), today())
        .expect("derives")
        .stale_set();
    let impact =
        impact_of_range(&ledger, &git, history.at(4), history.at(5), &already).expect("impact");

    assert_eq!(impact.commits, 1);
    assert_eq!(
        impact.touched.iter().cloned().collect::<Vec<_>>(),
        ["docs/generated/ROADMAP.md", "lege/src/render/mod.rs"]
    );
    assert!(
        impact.newly_stale.is_empty(),
        "C5 touches only lege/src/render/**, which the frame-budget observation already \
         observes at C5: {:?}",
        impact.newly_stale
    );
    assert!(impact.newly_at_risk.is_empty());
}

/// The range that *does* invalidate something, as `akr-impact.txt` records it.
#[test]
fn the_c2_to_c4_range_invalidates_the_projection_observation() {
    let history = history();
    let ledger = example_ledger(&history);
    let git = history.git();

    let impact = impact_of_range(
        &ledger,
        &git,
        history.at(2),
        history.at(4),
        &std::collections::BTreeSet::new(),
    )
    .expect("impact");

    assert_eq!(impact.commits, 2);
    let stale: Vec<String> = impact
        .newly_stale
        .iter()
        .map(|s| s.id.to_string())
        .collect();
    assert_eq!(stale, ["sim.obs.projection-gaps/1"]);
    let at_risk: Vec<String> = impact
        .newly_at_risk
        .iter()
        .map(|r| format!("{} depth {}", r.id, r.depth))
        .collect();
    assert_eq!(
        at_risk,
        [
            "sim.work.rewrite-projection/1 depth 1",
            "sys.assessment.projection-gaps/1 depth 1",
            "sys.policy.tandem-work/1 depth 2",
        ]
    );
}

#[test]
fn no_watch_glob_in_the_example_is_malformed_or_dead() {
    // V-102, over the real corpus: every glob is in the D-008 subset, and every one can
    // still match something at HEAD. A watch that can never fire is silent rot.
    let history = history();
    let ledger = example_ledger(&history);
    let git = history.git();

    let queue = derive(&ledger, &git, history.at(5), today()).expect("derives");
    assert!(
        queue.diagnostics.is_empty(),
        "unexpected: {:?}",
        queue
            .diagnostics
            .iter()
            .map(|d| (d.code.as_str(), d.message.clone()))
            .collect::<Vec<_>>()
    );

    let dead = akr_core::freshness::unmatched_watches(&ledger, &git, history.at(5)).expect("lists");
    assert!(
        dead.is_empty(),
        "every watched path exists in the materialised tree: {:?}",
        dead.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn a_key_used_by_this_file_exists_in_the_example() {
    // Guards against the manifest and this test drifting apart silently.
    let history = history();
    let ledger = example_ledger(&history);
    for text in [
        "sim.obs.projection-gaps",
        "sim.obs.timestep-drift",
        "lege.obs.frame-budget-headroom",
        "lege.obs.viewer-imports-engine",
        "sys.assessment.projection-gaps",
        "sys.assessment.m3-readiness",
        "sys.policy.tandem-work",
        "sim.work.rewrite-projection",
    ] {
        let parsed = LogicalKey::parse(text).expect("a valid key");
        assert!(
            !ledger.revisions_of(&parsed).is_empty(),
            "{text} is missing from the example"
        );
    }
}
