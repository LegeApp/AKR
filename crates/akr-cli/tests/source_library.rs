//! The immutable source library, end to end through the binary.
//!
//! `docs/15-external-sources.md`: registering a source creates no records, editing one
//! is an error, and searching one ranks passages without granting any of them authority.
//! These assertions run the commands rather than the functions, because the properties
//! under test are about the *workflow* — what a build does to a catalog, what a search
//! result says about its own standing — and a unit test cannot see either.

mod support;

use support::Example;

const AUDIT: &str = "\
# jp2lam decoder performance audit

Non-authoritative outside advice. Nothing here is the plan of record.

## 6. P1: nonzero tile origins unnecessarily disable optimized DWT

Aligned nonzero origins fall through to the scalar path even when the phase is even.
Route them through the optimized backend instead, and keep genuinely odd phases on the
existing path.

## 5.1 Scratch allocation per parallel row

Rayon's `for_each_init()` does not necessarily allocate once per worker. Use a coarse
chunk with one scratch buffer per chunk:

```rust
let scratch = vec![0f32; width];
rows.par_chunks_mut(64).for_each(|chunk| reconstruct(chunk, &scratch));
```

## 17. What should not be done first

Do not begin with MQ instruction-level tuning or additional fine-grained Rayon nesting.
";

/// Registers `AUDIT` and rebuilds, returning the registered id.
fn register(example: &Example) -> String {
    example.write_file("advice.md", AUDIT);
    let add = example.run(&["source", "add", "advice.md", "--id", "jp2lam-audit"]);
    assert_eq!(add.code, 0, "{}", add.output());
    let build = example.run(&["build"]);
    assert_eq!(build.code, 0, "{}", build.output());
    "jp2lam-audit".to_owned()
}

#[test]
fn registering_a_source_creates_no_records() {
    let example = Example::materialise("source-registers-nothing");
    let before = example.run(&["check", "--format", "json"]);
    register(&example);
    let after = example.run(&["check", "--format", "json"]);
    // The whole correction the design set records: an outside document becomes source
    // material, never a pile of proposed work.
    assert_eq!(
        before.stdout.contains("\"records\""),
        after.stdout.contains("\"records\""),
    );
    let listed = example.run(&["source", "list"]);
    assert!(
        listed.stdout.contains("jp2lam-audit"),
        "{}",
        listed.output()
    );
    assert!(
        !example
            .read_file("docs/generated/ACTIVE-WORK.md")
            .contains("nonzero tile origins"),
        "an unreviewed source must not reach the active-work projection"
    );
}

#[test]
fn editing_a_registered_source_is_an_error() {
    let example = Example::materialise("source-immutable");
    register(&example);
    let path = example
        .run(&["source", "list"])
        .stdout
        .split_whitespace()
        .last()
        .expect("a path")
        .to_owned();
    example.write_file(&path, "# tampered\n");
    let verify = example.run(&["source", "verify"]);
    assert_ne!(verify.code, 0, "{}", verify.output());
    assert!(verify.output().contains("AKR-S021"), "{}", verify.output());
}

#[test]
fn a_source_can_be_finalized_without_changing_records() {
    let example = Example::materialise("source-finalize-metadata");
    let id = register(&example);
    let path = example
        .run(&["source", "list"])
        .stdout
        .split_whitespace()
        .last()
        .expect("a source path")
        .to_owned();
    let status = example.run(&["source", "status", &id]);
    assert_eq!(status.code, 0, "{}", status.output());
    assert!(
        status.stdout.contains("availability     full"),
        "{}",
        status.output()
    );

    let finalized = example.run(&[
        "source",
        "finalize",
        &id,
        "--retain",
        "metadata",
        "--remove-file",
    ]);
    assert_eq!(finalized.code, 0, "{}", finalized.output());
    assert!(
        !example.root().join(&path).exists(),
        "the full source should be removed"
    );
    assert!(
        example
            .run(&["source", "status", &id])
            .stdout
            .contains("availability     metadata-only")
    );
    assert_eq!(example.run(&["source", "verify"]).code, 0);
    assert_eq!(example.run(&["check"]).code, 0);
}

#[test]
fn a_search_finds_the_section_and_says_it_is_not_authoritative() {
    let example = Example::materialise("source-search");
    register(&example);
    let run = example.run(&["source", "search", "nonzero tile origins"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        run.stdout.contains("nonzero tile origins"),
        "{}",
        run.output()
    );
    assert!(
        run.stdout.contains("non-authoritative"),
        "every source result carries its standing: {}",
        run.output()
    );
}

#[test]
fn the_golden_queries_of_the_plan_all_land() {
    let example = Example::materialise("source-golden-queries");
    register(&example);
    for (query, expected) in [
        ("nonzero tile origins", "nonzero tile origins"),
        ("for_each_init worker scratch", "Scratch allocation"),
        ("coarse chunk scratch buffer", "Scratch allocation"),
        ("what should not be done first", "should not be done first"),
    ] {
        let run = example.run(&["source", "search", query, "--limit", "3"]);
        assert_eq!(run.code, 0, "{query}: {}", run.output());
        assert!(
            run.stdout.contains(expected),
            "{query:?} did not return {expected:?}:\n{}",
            run.output()
        );
    }
}

#[test]
fn a_punctuated_query_is_not_a_syntax_error() {
    let example = Example::materialise("source-search-punctuation");
    register(&example);
    // `akr search` takes raw FTS5 and would reject this. The source surface must not.
    let run = example.run(&["source", "search", "for_each_init()"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        run.stdout.contains("Scratch allocation"),
        "{}",
        run.output()
    );
}

#[test]
fn a_literal_query_matches_exact_bytes_only() {
    let example = Example::materialise("source-search-literal");
    register(&example);
    let hit = example.run(&["source", "search", "--literal", "par_chunks_mut(64)"]);
    assert_eq!(hit.code, 0, "{}", hit.output());
    assert!(
        hit.stdout.contains("Scratch allocation"),
        "{}",
        hit.output()
    );

    let miss = example.run(&["source", "search", "--literal", "par_chunks_mut(65)"]);
    assert_eq!(miss.code, 0, "{}", miss.output());
    assert!(miss.stdout.contains("no matches"), "{}", miss.output());
}

#[test]
fn a_chunk_can_be_retrieved_with_its_neighbours() {
    let example = Example::materialise("source-get-chunk");
    register(&example);
    let search = example.run(&["source", "search", "nonzero tile origins", "--limit", "1"]);
    let chunk = search
        .stdout
        .split_whitespace()
        .find(|word| word.starts_with("c_"))
        .expect("a chunk id in the result")
        .to_owned();

    let one = example.run(&["source", "get", "--chunk", &chunk]);
    assert_eq!(one.code, 0, "{}", one.output());
    assert!(one.stdout.contains("non-authoritative"), "{}", one.output());

    let widened = example.run(&["source", "get", "--chunk", &chunk, "--neighbors", "1"]);
    assert_eq!(widened.code, 0, "{}", widened.output());
    assert!(
        widened.stdout.len() > one.stdout.len(),
        "--neighbors 1 should widen the window"
    );
}

#[test]
fn a_second_build_leaves_the_source_index_alone() {
    let example = Example::materialise("source-index-stable");
    register(&example);
    let again = example.run(&["build"]);
    assert_eq!(again.code, 0, "{}", again.output());
    assert!(
        again.stdout.contains("source index current"),
        "the corpus did not move, so nothing should have been rechunked:\n{}",
        again.output()
    );
}

#[test]
fn a_record_write_does_not_rechunk_the_corpus() {
    let example = Example::materialise("source-index-generations");
    register(&example);
    let path = example.root().join(".akr/cache/sources.sqlite");
    let before = std::fs::metadata(&path).expect("the index").len();
    let chunks_before = chunk_ids(&example);

    // `--today` moves `meta.today`, which is one of the five inputs the record cache is
    // stamped with, so this build rebuilds every record table. The corpus has not moved,
    // so it must not cost a single rechunk.
    let build = example.run(&["--today", "2026-08-05", "build"]);
    assert_eq!(build.code, 0, "{}", build.output());
    assert!(
        build.stdout.contains("indexed"),
        "the record cache should have been rebuilt:\n{}",
        build.output()
    );

    // D-031: two generations, so a ledger write cannot touch the chunk tables.
    assert_eq!(chunk_ids(&example), chunks_before);
    assert_eq!(std::fs::metadata(&path).expect("the index").len(), before);
    assert!(
        build.stdout.contains("source index current"),
        "{}",
        build.output()
    );
}

fn chunk_ids(example: &Example) -> Vec<String> {
    let path = example.root().join(".akr/cache/sources.sqlite");
    let connection = rusqlite::Connection::open(&path).expect("the source index opens");
    let mut statement = connection
        .prepare("SELECT chunk_id FROM source_chunks ORDER BY document_id, ordinal")
        .expect("the query prepares");
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("the query runs");
    rows.filter_map(Result::ok).collect()
}
