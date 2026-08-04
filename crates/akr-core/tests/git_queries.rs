//! The git layer: commits, ancestry, changed paths, and the working tree.
//!
//! Every test builds a real repository in a temp directory. Ancestry and rename detection
//! are exactly the questions where a hand-rolled model would drift from git, so the tests
//! ask git.

mod support;

use akr_core::git::{GitError, Repository, ancestry_over, last_change_of, last_changes};
use akr_core::model::{Commit, Ledger, LogicalKey, Project, RevisionId, key};
use akr_core::syntax::{lower::lower_all, parse};
use std::path::Path;
use support::TempRepo;

fn commit(hash: &str) -> Commit {
    Commit::new(hash).expect("a full hash")
}

// -------------------------------------------------------------------------------------
// Opening
// -------------------------------------------------------------------------------------

#[test]
fn opening_a_non_repository_says_so() {
    let dir = std::env::temp_dir().join(format!("akr-p5-not-a-repo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let error = Repository::open(&dir).expect_err("not a repository");
    assert!(matches!(error, GitError::NotARepository(_)));
    assert!(error.to_diagnostic().code == akr_core::git::codes::G001);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_revision_is_reported_as_such() {
    let mut repo = TempRepo::new("unknown-rev");
    repo.commit_file("a.txt", "one\n", "first");
    let git = Repository::open(repo.root()).expect("opens");
    let error = git.rev_parse("deadbeef").expect_err("no such revision");
    assert!(matches!(error, GitError::UnknownRevision(_)));
    assert_eq!(error.to_diagnostic().code, akr_core::git::codes::G013);
}

#[test]
fn head_resolves_to_the_latest_commit() {
    let mut repo = TempRepo::new("head");
    repo.commit_file("a.txt", "one\n", "first");
    let second = repo.commit_file("a.txt", "two\n", "second");
    let git = Repository::open(repo.root()).expect("opens");
    assert_eq!(git.head().expect("head").as_str(), second);
}

// -------------------------------------------------------------------------------------
// Ancestry
// -------------------------------------------------------------------------------------

#[test]
fn ancestry_is_reflexive_and_directional() {
    let mut repo = TempRepo::new("ancestry");
    let first = commit(&repo.commit_file("a.txt", "1\n", "first"));
    let second = commit(&repo.commit_file("a.txt", "2\n", "second"));
    let git = Repository::open(repo.root()).expect("opens");

    assert!(git.is_descendant(&first, &first).expect("reflexive"));
    assert!(git.is_descendant(&second, &first).expect("later descends"));
    assert!(
        !git.is_descendant(&first, &second)
            .expect("earlier does not")
    );
}

#[test]
fn ancestry_across_a_merge_is_answered_by_git() {
    // A hand-rolled first-parent walk gets this wrong; `merge-base --is-ancestor` does not.
    let mut repo = TempRepo::new("merge");
    let base = commit(&repo.commit_file("a.txt", "base\n", "base"));
    repo.branch("side");
    let side = commit(&repo.commit_file("side.txt", "side\n", "on the side branch"));
    repo.checkout("main");
    let main = commit(&repo.commit_file("main.txt", "main\n", "on main"));
    let merged = commit(&repo.merge("side"));

    let git = Repository::open(repo.root()).expect("opens");
    assert!(git.is_descendant(&merged, &base).expect("q"));
    assert!(
        git.is_descendant(&merged, &side).expect("q"),
        "a merge descends from its second parent"
    );
    assert!(git.is_descendant(&merged, &main).expect("q"));
    assert!(!git.is_descendant(&side, &main).expect("q"), "siblings");
    assert!(!git.is_descendant(&main, &side).expect("q"), "siblings");
}

#[test]
fn ancestry_of_a_missing_commit_is_an_error_not_a_false() {
    let mut repo = TempRepo::new("missing-ancestry");
    let only = commit(&repo.commit_file("a.txt", "1\n", "first"));
    let git = Repository::open(repo.root()).expect("opens");
    let absent = commit("0123456789abcdef0123456789abcdef01234567");
    assert!(matches!(
        git.is_descendant(&only, &absent),
        Err(GitError::UnknownRevision(_))
    ));
}

#[test]
fn ancestry_over_reproduces_git_answers_on_linear_history() {
    let mut repo = TempRepo::new("ancestry-over");
    let c1 = commit(&repo.commit_file("a.txt", "1\n", "c1"));
    let c2 = commit(&repo.commit_file("a.txt", "2\n", "c2"));
    let c3 = commit(&repo.commit_file("a.txt", "3\n", "c3"));
    let git = Repository::open(repo.root()).expect("opens");

    let ancestry = ancestry_over(&git, [c1.clone(), c2.clone(), c3.clone()]).expect("builds");
    for (descendant, ancestor) in [(&c3, &c1), (&c3, &c2), (&c2, &c1), (&c1, &c1)] {
        assert_eq!(
            ancestry.is_descendant(descendant, ancestor),
            Some(true),
            "{descendant} should descend from {ancestor}"
        );
    }
    assert_eq!(ancestry.is_descendant(&c1, &c3), Some(false));
    assert_eq!(ancestry.is_descendant(&c2, &c3), Some(false));
}

#[test]
fn ancestry_over_ignores_commits_the_repository_does_not_have() {
    let mut repo = TempRepo::new("ancestry-filter");
    let c1 = commit(&repo.commit_file("a.txt", "1\n", "c1"));
    let git = Repository::open(repo.root()).expect("opens");
    let absent = commit("0123456789abcdef0123456789abcdef01234567");
    let ancestry = ancestry_over(&git, [c1.clone(), absent.clone()]).expect("builds");
    assert_eq!(
        ancestry.is_descendant(&c1, &absent),
        None,
        "an unknown commit stays unknown rather than becoming a false answer"
    );
}

// -------------------------------------------------------------------------------------
// Changed paths
// -------------------------------------------------------------------------------------

#[test]
fn touches_report_every_path_a_range_changed() {
    let mut repo = TempRepo::new("touches");
    let base = commit(&repo.commit_file("src/a.rs", "1\n", "base"));
    repo.write("src/b.rs", "2\n");
    repo.write("docs/x.md", "x\n");
    let second = commit(&repo.commit("two files"));
    let git = Repository::open(repo.root()).expect("opens");

    let touches = git.touches_in(Some(&base), &second).expect("touches");
    let paths: Vec<&str> = touches.iter().map(|t| t.path.as_str()).collect();
    assert_eq!(paths, ["docs/x.md", "src/b.rs"]);
    assert!(touches.iter().all(|t| t.commit == second));
}

#[test]
fn touches_exclude_commits_before_the_range() {
    let mut repo = TempRepo::new("touch-range");
    repo.commit_file("early.rs", "1\n", "early");
    let base = commit(&repo.commit_file("mid.rs", "1\n", "mid"));
    let head = commit(&repo.commit_file("late.rs", "1\n", "late"));
    let git = Repository::open(repo.root()).expect("opens");

    let paths: Vec<String> = git
        .touches_in(Some(&base), &head)
        .expect("touches")
        .into_iter()
        .map(|t| t.path)
        .collect();
    assert_eq!(paths, ["late.rs"]);
}

#[test]
fn a_rename_reports_both_paths() {
    // An observation watching the old location and one watching the new location have
    // each had their subject moved out from under them.
    let mut repo = TempRepo::new("rename");
    let base = commit(&repo.commit_file(
        "engine/bridge.rs",
        "// a reasonably long file so rename detection is unambiguous\nfn main() {}\n",
        "base",
    ));
    repo.rename("engine/bridge.rs", "engine/seam.rs");
    let renamed = commit(&repo.commit("rename the bridge"));
    let git = Repository::open(repo.root()).expect("opens");

    let paths: Vec<String> = git
        .touches_in(Some(&base), &renamed)
        .expect("touches")
        .into_iter()
        .map(|t| t.path)
        .collect();
    assert!(paths.contains(&"engine/bridge.rs".to_owned()), "{paths:?}");
    assert!(paths.contains(&"engine/seam.rs".to_owned()), "{paths:?}");
}

#[test]
fn a_deletion_is_a_touch() {
    let mut repo = TempRepo::new("delete");
    let base = commit(&repo.commit_file("gone.rs", "1\n", "base"));
    repo.remove("gone.rs");
    let deleted = commit(&repo.commit("delete it"));
    let git = Repository::open(repo.root()).expect("opens");
    let paths: Vec<String> = git
        .touches_in(Some(&base), &deleted)
        .expect("touches")
        .into_iter()
        .map(|t| t.path)
        .collect();
    assert_eq!(paths, ["gone.rs"]);
}

#[test]
fn touches_are_deterministic() {
    let mut repo = TempRepo::new("touch-determinism");
    let base = commit(&repo.commit_file("a.rs", "1\n", "base"));
    repo.write("z.rs", "z\n");
    repo.write("a.rs", "2\n");
    repo.write("m.rs", "m\n");
    let head = commit(&repo.commit("several"));
    let git = Repository::open(repo.root()).expect("opens");
    let once = git.touches_in(Some(&base), &head).expect("touches");
    let twice = git.touches_in(Some(&base), &head).expect("touches");
    assert_eq!(once, twice);
    let paths: Vec<&str> = once.iter().map(|t| t.path.as_str()).collect();
    assert_eq!(paths, ["a.rs", "m.rs", "z.rs"], "sorted");
}

#[test]
fn commits_in_a_range_are_counted() {
    let mut repo = TempRepo::new("range-count");
    let base = commit(&repo.commit_file("a.rs", "1\n", "base"));
    repo.commit_file("a.rs", "2\n", "two");
    repo.commit_file("a.rs", "3\n", "three");
    let head = commit(&repo.commit_file("a.rs", "4\n", "four"));
    let git = Repository::open(repo.root()).expect("opens");
    assert_eq!(git.commits_in(Some(&base), &head).expect("range").len(), 3);
}

#[test]
fn the_working_tree_reports_uncommitted_changes() {
    let mut repo = TempRepo::new("dirty");
    repo.commit_file("a.rs", "1\n", "base");
    repo.write("a.rs", "edited but not committed\n");
    repo.write("new.rs", "untracked\n");
    let git = Repository::open(repo.root()).expect("opens");
    let dirty = git.working_tree_changes().expect("status");
    assert!(dirty.contains("a.rs"), "{dirty:?}");
    assert!(dirty.contains("new.rs"), "{dirty:?}");
}

#[test]
fn a_clean_tree_reports_nothing() {
    let mut repo = TempRepo::new("clean");
    repo.commit_file("a.rs", "1\n", "base");
    let git = Repository::open(repo.root()).expect("opens");
    assert!(git.working_tree_changes().expect("status").is_empty());
}

// -------------------------------------------------------------------------------------
// last_change: the D-016 anchor
// -------------------------------------------------------------------------------------

const RECORD_V1: &str = "\
akr 0.1
project fixtures

record fx.milestone.m1/1 : milestone {
    title \"M1\"
    state active
    intent \"\"\"
        The first milestone.
        \"\"\"
    acceptance {
        check done {
            statement \"\"\"
                It is done.
                \"\"\"
            method manual
        }
    }
}
";

#[test]
fn last_change_is_the_commit_that_gave_a_record_its_current_text() {
    let mut repo = TempRepo::new("last-change");
    let introduced = repo.commit_file(".akr/records/m.akr", RECORD_V1, "introduce");
    // Two commits that touch the file without changing the record's content.
    repo.commit_file("unrelated.txt", "a\n", "unrelated");
    let with_comment = RECORD_V1.replace(
        "    state active\n",
        "    # still the plan of record\n    state active\n",
    );
    repo.commit_file(".akr/records/m.akr", &with_comment, "add a comment");

    let git = Repository::open(repo.root()).expect("opens");
    let answer = last_change_of(&git, ".akr/records/m.akr", &key("fx.milestone.m1"), 1)
        .expect("query")
        .expect("the record exists");
    assert_eq!(
        answer.as_str(),
        introduced,
        "a comment is not a content change (spec/schema/akr-lock.md §3.3)"
    );
}

#[test]
fn last_change_moves_when_the_record_body_changes() {
    let mut repo = TempRepo::new("last-change-moves");
    repo.commit_file(".akr/records/m.akr", RECORD_V1, "introduce");
    let edited = RECORD_V1.replace("The first milestone.", "The first milestone, restated.");
    let changed = repo.commit_file(".akr/records/m.akr", &edited, "restate");

    let git = Repository::open(repo.root()).expect("opens");
    let answer = last_change_of(&git, ".akr/records/m.akr", &key("fx.milestone.m1"), 1)
        .expect("query")
        .expect("the record exists");
    assert_eq!(answer.as_str(), changed);
}

#[test]
fn last_change_ignores_edits_to_other_records_in_the_same_file() {
    // Adding a record to a file must not invalidate every acceptance check in it.
    let mut repo = TempRepo::new("last-change-sibling");
    let introduced = repo.commit_file(".akr/records/m.akr", RECORD_V1, "introduce");
    let with_sibling = format!(
        "{RECORD_V1}\nrecord fx.milestone.m2/1 : milestone {{\n    title \"M2\"\n    state proposed\n    intent \"\"\"\n        The second milestone.\n        \"\"\"\n    acceptance {{\n        check done {{\n            statement \"\"\"\n                It is done.\n                \"\"\"\n            method manual\n        }}\n    }}\n}}\n"
    );
    repo.commit_file(".akr/records/m.akr", &with_sibling, "add a sibling");

    let git = Repository::open(repo.root()).expect("opens");
    let answer = last_change_of(&git, ".akr/records/m.akr", &key("fx.milestone.m1"), 1)
        .expect("query")
        .expect("the record exists");
    assert_eq!(answer.as_str(), introduced);
}

#[test]
fn last_change_is_none_for_an_untracked_record() {
    let mut repo = TempRepo::new("last-change-untracked");
    repo.commit_file("a.txt", "1\n", "base");
    let git = Repository::open(repo.root()).expect("opens");
    assert_eq!(
        last_change_of(&git, ".akr/records/absent.akr", &key("fx.milestone.m1"), 1).expect("query"),
        None
    );
}

#[test]
fn last_changes_fills_the_ledger_facts() {
    let mut repo = TempRepo::new("last-changes");
    let introduced = repo.commit_file(".akr/records/m.akr", RECORD_V1, "introduce");
    let git = Repository::open(repo.root()).expect("opens");

    let ledger = load(repo.root(), ".akr/records/m.akr");
    let facts = last_changes(&git, &ledger).expect("query");
    assert_eq!(
        facts.get(&RevisionId::new(key("fx.milestone.m1"), 1)),
        Some(&commit(&introduced))
    );
}

/// Parses one file into a ledger, tagging records with the path V-003 needs.
fn load(root: &Path, path: &str) -> Ledger {
    let text = std::fs::read_to_string(root.join(path)).expect("readable");
    let parsed = parse(&text, akr_core::diagnostics::FileId(0));
    let file = parsed.file.expect("parses");
    let (mut ledger, _) = lower_all(&[(path.to_owned(), file)]);
    ledger.project = Project::new("fixtures", &["fx"]);
    ledger
}

#[test]
fn a_logical_key_round_trips_through_git_queries() {
    // Guards the string comparison `last_change_of` makes between the CST's key text and
    // the model's key.
    let parsed = LogicalKey::parse("fx.milestone.m1").expect("valid");
    assert_eq!(parsed.to_string(), "fx.milestone.m1");
}
