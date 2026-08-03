//! Exit criteria 2 and 3: `fixtures/parse/ok` parses clean and formats idempotently;
//! `fixtures/parse/err` produces exactly the codes in its `.expected` file, at the
//! spans named.

use akr_core::diagnostics::{Diagnostic, FileId, SourceMap};
use akr_core::syntax::{format, format_source, parse};
use std::path::{Path, PathBuf};

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn fixtures(dir: &str, extension: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(repo(dir))
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == extension))
        .collect();
    out.sort();
    out
}

/// `CODE line[:col]`, one per line, `#` comments ignored.
fn expected(path: &Path) -> Vec<(String, u32, Option<u32>)> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|line| {
            let mut parts = line.split_whitespace();
            let code = parts.next().expect("a code").to_owned();
            let position = parts.next().expect("a position");
            let (line_text, column) = match position.split_once(':') {
                Some((l, c)) => (l, Some(c.parse().expect("a column"))),
                None => (position, None),
            };
            (code, line_text.parse().expect("a line"), column)
        })
        .collect()
}

fn actual(text: &str, diagnostics: &[Diagnostic], path: &str) -> Vec<(String, u32, Option<u32>)> {
    let mut sources = SourceMap::new();
    let id = sources.add(path, text);
    let file = sources.get(id).expect("the file was just added");
    diagnostics
        .iter()
        .map(|d| {
            let (line, column) = d.primary.span.map_or((0, 0), |s| file.location(s.start));
            (d.code.as_str().to_owned(), line, Some(column))
        })
        .collect()
}

#[test]
fn every_ok_fixture_parses_with_no_diagnostics() {
    for path in fixtures("fixtures/parse/ok", "akr") {
        let text = std::fs::read_to_string(&path).expect("readable");
        let parsed = parse(&text, FileId(0));
        assert!(
            parsed.diagnostics.is_empty(),
            "{}: expected a clean parse, got {:?}",
            path.display(),
            parsed
                .diagnostics
                .iter()
                .map(|d| (d.code.as_str(), d.message.as_str()))
                .collect::<Vec<_>>()
        );
        assert!(
            parsed.file.is_some(),
            "{}: produced no tree",
            path.display()
        );
    }
}

/// Exit criterion 2, as a property over the whole corpus rather than a spot check.
#[test]
fn formatting_is_idempotent_for_every_ok_fixture() {
    for path in fixtures("fixtures/parse/ok", "akr") {
        let text = std::fs::read_to_string(&path).expect("readable");
        let (once, _) = format_source(&text, FileId(0));
        let once = once.unwrap_or_else(|| panic!("{}: does not format", path.display()));
        let (twice, _) = format_source(&once, FileId(0));
        assert_eq!(
            Some(&once),
            twice.as_ref(),
            "{}: fmt(fmt(x)) != fmt(x)",
            path.display()
        );
    }
}

/// The semantic-preservation half of `docs/03` §7: formatting does not change the tree,
/// including comment attachment.
#[test]
fn formatting_preserves_the_tree_for_every_ok_fixture() {
    for path in fixtures("fixtures/parse/ok", "akr") {
        let text = std::fs::read_to_string(&path).expect("readable");
        let before = parse(&text, FileId(0));
        let formatted = format(before.file.as_ref().expect("parses"));
        let after = parse(&formatted, FileId(0));
        let strip = |f: &akr_core::syntax::cst::File| {
            (
                f.keyword.clone(),
                f.project.clone(),
                f.items.len(),
                f.leading.iter().map(|c| c.text.clone()).collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            strip(before.file.as_ref().expect("parses")),
            strip(after.file.as_ref().expect("reparses")),
            "{}: reparse differs",
            path.display()
        );
    }
}

#[test]
fn every_err_fixture_produces_exactly_its_expected_diagnostics() {
    let mut checked = 0;
    for path in fixtures("fixtures/parse/err", "akr") {
        let text = std::fs::read_to_string(&path).expect("readable");
        let parsed = parse(&text, FileId(0));
        let name = path
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned();
        let found = actual(&text, &parsed.diagnostics, &name);
        let want = expected(&path.with_extension("expected"));

        let found_codes: Vec<&str> = found.iter().map(|(c, _, _)| c.as_str()).collect();
        let want_codes: Vec<&str> = want.iter().map(|(c, _, _)| c.as_str()).collect();
        assert_eq!(want_codes, found_codes, "{name}: codes differ");

        for ((wc, wl, wcol), (_, fl, fcol)) in want.iter().zip(found.iter()) {
            assert_eq!(*wl, *fl, "{name}: {wc} on the wrong line");
            if let Some(wcol) = wcol {
                assert_eq!(Some(*wcol), *fcol, "{name}: {wc} at the wrong column");
            }
        }
        checked += 1;
    }
    assert!(
        checked >= 12,
        "expected the err corpus, found {checked} fixtures"
    );
}
