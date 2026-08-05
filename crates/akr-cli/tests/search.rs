//! P7's four exit criteria (`docs/13-implementation-roadmap.md` §3).
//!
//! 1. Search results are stable: the same query returns the same order twice.
//! 2. Deleting `.akr/cache/` and rebuilding produces identical query results.
//! 3. **No context bundle changes when the ranker is disabled** — the direct executable
//!    form of "search ranks, never authorises".
//! 4. FTS5 absent degrades to `AKR-I022` on `akr search` and affects nothing else.
//!
//! The third is the important one. The other three are about a cache behaving like a
//! cache; that one is about the boundary the whole design rests on, and it is the only
//! test here that would still matter if search were deleted tomorrow.
//!
//! The whole file needs a ranker to exist. `tests/search_degraded.rs` is its counterpart
//! for a binary built without one, where criterion 4 is the only one that still has
//! meaning — and where it is tested against the real absence rather than a simulated one.
#![cfg(feature = "fts5")]

mod support;

use support::Example;

fn cache(example: &Example) -> std::path::PathBuf {
    example.root().join(".akr/cache/index.sqlite")
}

// -------------------------------------------------------------------------------------
// Exit criterion 1 — stable order
// -------------------------------------------------------------------------------------

#[test]
fn the_same_query_returns_the_same_order_twice() {
    let example = Example::materialise("search-stable");
    assert_eq!(example.run(&["build"]).code, 0);

    let first = example.run(&["search", "projection"]);
    let second = example.run(&["search", "projection"]);
    assert_eq!(first.code, 0, "{}", first.output());
    assert_eq!(first.stdout, second.stdout);
    assert!(first.stdout.contains("results"), "{}", first.stdout);

    // BM25 alone is not a total order — two revisions can score identically, and SQLite
    // may return ties however it likes. The key tiebreak is what makes repetition a
    // guarantee rather than a habit, so a query with many equal scores is the one worth
    // asking twice.
    let broad = example.run(&["search", "the OR a OR day"]);
    let broad_again = example.run(&["search", "the OR a OR day"]);
    assert_eq!(broad.stdout, broad_again.stdout);
}

#[test]
fn zero_results_is_an_answer_and_exits_zero() {
    let example = Example::materialise("search-empty");
    assert_eq!(example.run(&["build"]).code, 0);
    let run = example.run(&["search", "gorgonzola"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(run.stdout.contains("0 results"), "{}", run.stdout);
}

#[test]
fn filters_are_applied_before_ranking() {
    let example = Example::materialise("search-filters");
    assert_eq!(example.run(&["build"]).code, 0);

    let all = example.run(&["search", "day"]);
    let milestones = example.run(&["search", "day", "--kind", "milestone"]);
    assert_eq!(milestones.code, 0, "{}", milestones.output());
    for line in milestones.stdout.lines().filter(|l| l.starts_with("  ")) {
        assert!(line.contains(" milestone "), "{line}");
    }
    // A filter narrows the set; it does not reorder what survives it. Compared by key
    // rather than by line, because the columns are padded to the widest cell and a
    // narrower result set legitimately pads them differently.
    let keys = |run: &support::Run, only_milestones: bool| -> Vec<String> {
        run.stdout
            .lines()
            .filter(|line| line.starts_with("  "))
            .filter(|line| !only_milestones || line.contains(" milestone "))
            .filter_map(|line| line.split_whitespace().nth(1).map(ToOwned::to_owned))
            .collect()
    };
    assert_eq!(
        keys(&all, true),
        keys(&milestones, false),
        "filtering changed the surviving order"
    );
}

// -------------------------------------------------------------------------------------
// Exit criterion 2 — a rebuilt cache answers identically
// -------------------------------------------------------------------------------------

#[test]
fn deleting_the_cache_and_rebuilding_answers_identically() {
    let example = Example::materialise("search-rebuild");
    assert_eq!(example.run(&["build"]).code, 0);
    let before = example.run(&["search", "projection"]);
    assert_eq!(before.code, 0, "{}", before.output());

    std::fs::remove_dir_all(example.root().join(".akr/cache")).expect("the cache goes");
    assert_eq!(example.run(&["build"]).code, 0);
    let after = example.run(&["search", "projection"]);

    // Byte-identical, scores included. A cache that ranked differently after a rebuild
    // would make every result an artefact of when the cache happened to be made.
    assert_eq!(before.stdout, after.stdout);
}

// -------------------------------------------------------------------------------------
// Exit criterion 3 — the ranker cannot reach a bundle
// -------------------------------------------------------------------------------------

#[test]
fn no_context_bundle_changes_when_the_ranker_is_disabled() {
    // The executable form of `docs/09-context-assembly.md` §1. "Disabled" is taken at its
    // strongest: not a flag that asks the assembler to skip ranking, but the ranker
    // physically absent — no full-text table, and then no cache at all. If any record's
    // membership or order in a bundle depended on search, one of these three bundles would
    // differ from the others.
    let example = Example::materialise("search-authority");
    assert_eq!(example.run(&["build"]).code, 0);

    let goal = "sys.milestone.m3-playable-day";
    let with_ranker = example.run(&["context", "--goal", goal]);
    assert_eq!(with_ranker.code, 0, "{}", with_ranker.output());
    let json_with_ranker = example.run(&["--format", "json", "context", "--goal", goal]);

    // Drop the full-text table, which is what a cache built without FTS5 looks like.
    {
        let connection = rusqlite::Connection::open(cache(&example)).expect("the cache opens");
        connection
            .execute("DROP TABLE records_fts", [])
            .expect("the ranker goes");
    }
    let without_table = example.run(&["context", "--goal", goal]);
    assert_eq!(without_table.code, 0, "{}", without_table.output());
    assert_eq!(
        with_ranker.stdout, without_table.stdout,
        "a bundle changed when the full-text index was removed"
    );

    // And with no cache at all.
    std::fs::remove_dir_all(example.root().join(".akr/cache")).expect("the cache goes");
    let without_cache = example.run(&["context", "--goal", goal]);
    assert_eq!(without_cache.code, 0, "{}", without_cache.output());
    assert_eq!(
        with_ranker.stdout, without_cache.stdout,
        "a bundle changed when the cache was removed"
    );
    let json_without_cache = example.run(&["--format", "json", "context", "--goal", goal]);
    assert_eq!(json_with_ranker.stdout, json_without_cache.stdout);
}

#[test]
fn a_high_scoring_record_outside_the_goals_reach_stays_out_of_the_bundle() {
    // The same claim from the other side, and the one an agent would actually be harmed
    // by: a record can be the top hit for a term the goal's own title contains and still
    // have no place in the bundle. Standing comes from state, scope and relations, which
    // are declared and reviewable in a diff — never from a score.
    let example = Example::materialise("search-no-authority");
    assert_eq!(example.run(&["build"]).code, 0);

    let hits = example.run(&["search", "day"]);
    assert_eq!(hits.code, 0, "{}", hits.output());
    let ranked: Vec<String> = hits
        .stdout
        .lines()
        .filter(|line| line.starts_with("  "))
        .filter_map(|line| line.split_whitespace().nth(1).map(ToOwned::to_owned))
        .collect();
    assert!(ranked.len() > 1, "{}", hits.stdout);

    let bundle = example.run(&["context", "--goal", "sys.milestone.m5-ship-demo"]);
    assert_eq!(bundle.code, 0, "{}", bundle.output());
    let outside: Vec<&String> = ranked
        .iter()
        .filter(|hit| {
            let key = hit.split('/').next().unwrap_or(hit);
            !bundle.stdout.contains(key)
        })
        .collect();
    assert!(
        !outside.is_empty(),
        "every ranked record happened to be in the bundle, so this proves nothing: {}\n{}",
        hits.stdout,
        bundle.stdout
    );
}

// -------------------------------------------------------------------------------------
// Exit criterion 4 — FTS5 absent
// -------------------------------------------------------------------------------------

#[test]
fn a_cache_without_fts5_fails_search_and_affects_nothing_else() {
    let example = Example::materialise("search-no-fts5");
    assert_eq!(example.run(&["build"]).code, 0);
    {
        let connection = rusqlite::Connection::open(cache(&example)).expect("the cache opens");
        connection
            .execute("DROP TABLE records_fts", [])
            .expect("the table goes");
    }

    let run = example.run(&["search", "projection"]);
    assert_ne!(run.code, 0, "{}", run.output());
    assert!(run.output().contains("AKR-I022"), "{}", run.output());

    // "Affects nothing else" is the half of the criterion that is easy to skip and is the
    // reason the criterion exists: a degraded ranker must not degrade the ledger.
    for args in [
        vec!["check"],
        vec!["get", "@sys.milestone.m3-playable-day"],
        vec!["context", "--goal", "sys.milestone.m3-playable-day"],
        vec!["impact", "@sim.obs.projection-gaps"],
        vec!["review-queue"],
        vec!["view", "roadmap"],
    ] {
        let other = example.run(&args);
        assert_eq!(other.code, 0, "akr {}: {}", args.join(" "), other.output());
    }
}

#[test]
fn a_malformed_query_says_so_rather_than_returning_nothing() {
    let example = Example::materialise("search-malformed");
    assert_eq!(example.run(&["build"]).code, 0);
    // An unbalanced quote. Returning zero results here would be the wrong answer twice
    // over: it is not true, and it teaches the caller that the ledger is empty.
    let run = example.run(&["search", "\"unterminated"]);
    assert_ne!(run.code, 0, "{}", run.output());
    assert!(run.output().contains("AKR-X031"), "{}", run.output());
}

// -------------------------------------------------------------------------------------
// A write between build and search leaves the cache stale — and search now says so.
// -------------------------------------------------------------------------------------

#[test]
fn a_write_between_build_and_search_is_flagged_not_hidden() {
    // The friction `tandem.papercut.search-after-write-stale` recorded: an agent logs a
    // record and searches for it in the same breath, the cache still predates the write
    // (D-019: only `akr build` may touch it), and the old answer comes back with nothing
    // to say it is old. Search cannot rebuild, but it can — and now does — notice.
    let example = Example::materialise("search-after-write");
    assert_eq!(example.run(&["build"]).code, 0);

    // A fresh build: the cache matches the sources, so nothing is stale and nothing warns.
    let fresh = example.run(&["search", "projection"]);
    assert_eq!(fresh.code, 0, "{}", fresh.output());
    assert!(!fresh.stdout.contains("stale index"), "{}", fresh.stdout);
    let fresh_json = example.run(&["--format", "json", "search", "projection"]);
    assert!(
        fresh_json
            .stdout
            .replace(' ', "")
            .contains("\"index_stale\":false"),
        "{}",
        fresh_json.stdout
    );

    // A write goes through the pipeline; by D-019 it does not touch the cache.
    let wrote = example.run(&[
        "papercut",
        "-m",
        "codex",
        "a small friction while testing search staleness",
        "--namespace",
        "sys",
    ]);
    assert_eq!(wrote.code, 0, "{}", wrote.output());

    // The same query still answers — from a cache that now predates the write — and says
    // so rather than pretending the ledger is unchanged. Staleness never changes the exit
    // code (D-024).
    let stale = example.run(&["search", "projection"]);
    assert_eq!(stale.code, 0, "{}", stale.output());
    assert!(stale.stdout.contains("stale index"), "{}", stale.stdout);
    assert!(stale.stdout.contains("akr build"), "{}", stale.stdout);
    let stale_json = example.run(&["--format", "json", "search", "projection"]);
    assert!(
        stale_json
            .stdout
            .replace(' ', "")
            .contains("\"index_stale\":true"),
        "{}",
        stale_json.stdout
    );

    // A rebuild reconciles the cache with the sources, and the warning goes away.
    assert_eq!(example.run(&["build"]).code, 0);
    let rebuilt = example.run(&["search", "projection"]);
    assert_eq!(rebuilt.code, 0, "{}", rebuilt.output());
    assert!(
        !rebuilt.stdout.contains("stale index"),
        "{}",
        rebuilt.stdout
    );
}
