//! Exit criteria 4 and 5: every `fixtures/format/` pair formats input to expected output
//! exactly, and comments survive with their attachment.

use akr_core::diagnostics::FileId;
use akr_core::syntax::{format_source, parse};
use std::path::{Path, PathBuf};

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn pairs() -> Vec<(PathBuf, PathBuf)> {
    let mut out: Vec<(PathBuf, PathBuf)> = std::fs::read_dir(repo("fixtures/format"))
        .expect("fixtures/format")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".in.akr"))
        .map(|input| {
            let expected = PathBuf::from(input.to_string_lossy().replace(".in.akr", ".out.akr"));
            (input, expected)
        })
        .collect();
    out.sort();
    out
}

fn first_difference(want: &str, got: &str) -> String {
    for (n, (a, b)) in want.lines().zip(got.lines()).enumerate() {
        if a != b {
            return format!("line {}:\n  want: {a:?}\n  got:  {b:?}", n + 1);
        }
    }
    format!(
        "line counts differ: want {}, got {}",
        want.lines().count(),
        got.lines().count()
    )
}

#[test]
fn every_format_pair_formats_exactly() {
    let pairs = pairs();
    assert!(
        pairs.len() >= 8,
        "expected the format corpus, found {}",
        pairs.len()
    );
    for (input, expected) in pairs {
        let source = std::fs::read_to_string(&input).expect("readable");
        let want = std::fs::read_to_string(&expected).expect("readable");
        let (got, diagnostics) = format_source(&source, FileId(0));
        let got = got.unwrap_or_else(|| panic!("{}: does not format", input.display()));
        assert!(
            diagnostics.is_empty(),
            "{}: {:?}",
            input.display(),
            diagnostics
                .iter()
                .map(|d| d.code.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            want,
            got,
            "{} did not format to {}\n{}",
            input.display(),
            expected.display(),
            first_difference(&want, &got)
        );
    }
}

#[test]
fn every_expected_output_is_a_fixed_point() {
    for (_, expected) in pairs() {
        let want = std::fs::read_to_string(&expected).expect("readable");
        let (got, _) = format_source(&want, FileId(0));
        assert_eq!(
            Some(&want),
            got.as_ref(),
            "{} is not canonical",
            expected.display()
        );
    }
}

/// Exit criterion 5, on the fixture that exercises every D-006 attachment position.
#[test]
fn comments_survive_with_their_attachment() {
    let source = std::fs::read_to_string(repo("fixtures/format/004-comment-preservation.in.akr"))
        .expect("readable");
    let before = parse(&source, FileId(0));
    let (formatted, _) = format_source(&source, FileId(0));
    let after = parse(&formatted.expect("formats"), FileId(0));

    let comments = |parsed: &akr_core::syntax::Parsed| {
        let file = parsed.file.as_ref().expect("parses");
        let mut out: Vec<(String, String)> = Vec::new();
        for comment in &file.leading {
            out.push(("file/leading".to_owned(), comment.text.clone()));
        }
        for item in &file.items {
            if let akr_core::syntax::cst::Item::Record(record) = item {
                for comment in &record.trivia.leading {
                    out.push(("record/leading".to_owned(), comment.text.clone()));
                }
                for body_item in &record.body {
                    let trivia = body_item.trivia();
                    for comment in &trivia.leading {
                        out.push((
                            format!("{}/leading", body_item.name()),
                            comment.text.clone(),
                        ));
                    }
                    if let Some(comment) = &trivia.trailing {
                        out.push((
                            format!("{}/trailing", body_item.name()),
                            comment.text.clone(),
                        ));
                    }
                }
                for comment in &record.inner_trailing {
                    out.push(("record/inner-trailing".to_owned(), comment.text.clone()));
                }
            }
        }
        out
    };

    let before_comments = comments(&before);
    assert_eq!(
        before_comments.len(),
        4,
        "the fixture must exercise every attachment position, found {before_comments:?}"
    );
    assert_eq!(
        before_comments,
        comments(&after),
        "comment attachment changed under formatting"
    );
}
