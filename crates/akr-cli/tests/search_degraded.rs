//! P7 exit criterion 4, against the real absence of a ranker.
//!
//! `tests/search.rs` covers the criterion by dropping `records_fts` from a cache that has
//! one, which exercises the code path but simulates the condition. This file runs only in
//! a binary built with `--no-default-features`, where stage E genuinely never creates the
//! table — the configuration the criterion is actually about.
//!
//! The claim is two-sided and the second side is the one worth having: search fails
//! honestly, *and* nothing else notices. A degraded ranker must not degrade the ledger.
#![cfg(not(feature = "fts5"))]

mod support;

use support::Example;

#[test]
fn the_cache_is_built_without_a_full_text_table() {
    let example = Example::materialise("degraded-build");
    std::fs::remove_dir_all(example.root().join(".akr/cache")).expect("the cache goes");
    let run = example.run(&["build"]);
    assert_eq!(run.code, 0, "{}", run.output());
    // The build succeeds and says how little it indexed, rather than failing because an
    // extension is missing. Stage E is not optional; the full-text half of it is.
    assert!(run.stdout.contains("(0 full-text)"), "{}", run.stdout);

    let connection =
        rusqlite::Connection::open(example.root().join(".akr/cache/index.sqlite")).expect("opens");
    let tables: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'records_fts'",
            [],
            |row| row.get(0),
        )
        .expect("query");
    assert_eq!(tables, 0, "a binary without FTS5 wrote a full-text table");
    let revisions: i64 = connection
        .query_row("SELECT count(*) FROM revisions", [], |row| row.get(0))
        .expect("query");
    assert!(revisions > 30, "the rest of the cache is missing too");
}

#[test]
fn search_fails_with_i022_and_nothing_else_changes() {
    let example = Example::materialise("degraded-search");
    assert_eq!(example.run(&["build"]).code, 0);

    let run = example.run(&["search", "projection"]);
    assert_ne!(run.code, 0, "{}", run.output());
    assert!(run.output().contains("AKR-I022"), "{}", run.output());

    for args in [
        vec!["check"],
        vec!["build"],
        vec!["get", "@sys.milestone.m3-playable-day"],
        vec!["context", "--goal", "sys.milestone.m3-playable-day"],
        vec!["impact", "@sim.obs.projection-gaps"],
        vec!["review-queue"],
        vec!["view", "roadmap"],
        vec!["lock", "--check"],
    ] {
        let other = example.run(&args);
        assert_eq!(other.code, 0, "akr {}: {}", args.join(" "), other.output());
    }
}

#[test]
fn a_bundle_is_the_same_bundle_without_a_ranker() {
    // The strongest form of "search ranks, never authorises": this binary has no ranker at
    // all, and the bundle is still the bundle. `tests/search.rs` asserts the same equality
    // from the other side, where a ranker exists and is taken away.
    let example = Example::materialise("degraded-bundle");
    assert_eq!(example.run(&["build"]).code, 0);
    let goal = "sys.milestone.m3-playable-day";

    let with_cache = example.run(&["context", "--goal", goal]);
    assert_eq!(with_cache.code, 0, "{}", with_cache.output());
    std::fs::remove_dir_all(example.root().join(".akr/cache")).expect("the cache goes");
    let without_cache = example.run(&["context", "--goal", goal]);
    assert_eq!(without_cache.code, 0, "{}", without_cache.output());
    assert_eq!(with_cache.stdout, without_cache.stdout);
}
