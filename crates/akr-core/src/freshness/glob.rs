//! Matching a `watches` glob against a changed path.
//!
//! The D-008 subset and nothing else: `/` separators, `*` and `?` matching within one
//! path segment, `**` matching any run of segments, and `[...]` character classes. No
//! brace expansion, no `!` negation, no backslash escapes.
//!
//! # Why this is not the scope-overlap test
//!
//! [`glob_prefixes_comparable`](crate::model::glob_prefixes_comparable) answers "could
//! these two globs ever describe the same file?" — a conservative comparison of two
//! *patterns*, used for D-010 scope overlap. This module answers "does this pattern
//! describe this file?" — an exact match of a pattern against a *path*. They are
//! different questions and the freshness computation needs the exact one: a watch that
//! fired on a merely-comparable prefix would flag records every time a sibling directory
//! changed.

use crate::model::Glob;

/// Why a glob is not in the D-008 subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobError {
    /// Brace expansion, which the subset excludes.
    BraceExpansion,
    /// A leading `!`, which the subset excludes.
    Negation,
    /// A backslash, which is not an escape here and not a separator either.
    Backslash,
    /// An unterminated `[` class.
    UnterminatedClass,
    /// A `**` that is not a whole path segment.
    PartialGlobstar,
    /// An empty glob.
    Empty,
    /// An absolute path; globs are repo-root-relative.
    Absolute,
}

impl std::fmt::Display for GlobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::BraceExpansion => "brace expansion is not in the glob subset (D-008)",
            Self::Negation => "negation is not in the glob subset (D-008)",
            Self::Backslash => "backslashes are not path separators; use `/`",
            Self::UnterminatedClass => "unterminated `[` character class",
            Self::PartialGlobstar => "`**` must be a whole path segment",
            Self::Empty => "a glob may not be empty",
            Self::Absolute => "globs are repo-root-relative and may not start with `/`",
        })
    }
}

/// Checks a glob against the D-008 subset.
///
/// # Errors
/// Returns the first way in which the glob leaves the subset.
pub fn validate(glob: &Glob) -> Result<(), GlobError> {
    let text = glob.as_str();
    if text.is_empty() {
        return Err(GlobError::Empty);
    }
    if text.starts_with('/') {
        return Err(GlobError::Absolute);
    }
    if text.starts_with('!') {
        return Err(GlobError::Negation);
    }
    if text.contains('{') || text.contains('}') {
        return Err(GlobError::BraceExpansion);
    }
    if text.contains('\\') {
        return Err(GlobError::Backslash);
    }
    let mut in_class = false;
    for ch in text.chars() {
        match ch {
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            _ => {}
        }
    }
    if in_class {
        return Err(GlobError::UnterminatedClass);
    }
    for segment in text.split('/') {
        if segment.contains("**") && segment != "**" {
            return Err(GlobError::PartialGlobstar);
        }
    }
    Ok(())
}

/// Whether a glob matches a repo-root-relative path.
///
/// `**` matches any run of segments including none, so `a/**` matches `a/b` and `a/b/c`
/// — and, deliberately, `a` itself: a watch on `a/**` is a watch on the subtree, and a
/// change that deletes the subtree and leaves a file named `a` has certainly invalidated
/// anything observing it.
#[must_use]
pub fn matches(glob: &Glob, path: &str) -> bool {
    let pattern: Vec<&str> = glob.as_str().split('/').collect();
    let target: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match_segments(&pattern, &target)
}

fn match_segments(pattern: &[&str], target: &[&str]) -> bool {
    match pattern.first() {
        None => target.is_empty(),
        Some(&"**") => {
            // Zero or more segments.
            (0..=target.len()).any(|skip| match_segments(&pattern[1..], &target[skip..]))
        }
        Some(segment) => match target.first() {
            Some(name) if match_segment(segment, name) => {
                match_segments(&pattern[1..], &target[1..])
            }
            _ => false,
        },
    }
}

/// Matches one path segment against one pattern segment: `*`, `?` and `[...]`, none of
/// which cross a `/`.
fn match_segment(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    match_here(&p, &n)
}

fn match_here(pattern: &[char], name: &[char]) -> bool {
    match pattern.first() {
        None => name.is_empty(),
        Some('*') => (0..=name.len()).any(|skip| match_here(&pattern[1..], &name[skip..])),
        Some('?') => !name.is_empty() && match_here(&pattern[1..], &name[1..]),
        Some('[') => {
            let Some(end) = pattern.iter().position(|c| *c == ']') else {
                return false;
            };
            let Some(&candidate) = name.first() else {
                return false;
            };
            let class = &pattern[1..end];
            let (negated, class) = match class.first() {
                Some('!' | '^') => (true, &class[1..]),
                _ => (false, class),
            };
            let hit = class_matches(class, candidate);
            (hit != negated) && match_here(&pattern[end + 1..], &name[1..])
        }
        Some(literal) => {
            !name.is_empty() && name[0] == *literal && match_here(&pattern[1..], &name[1..])
        }
    }
}

fn class_matches(class: &[char], candidate: char) -> bool {
    let mut at = 0;
    while at < class.len() {
        if at + 2 < class.len() && class[at + 1] == '-' {
            if class[at] <= candidate && candidate <= class[at + 2] {
                return true;
            }
            at += 3;
        } else {
            if class[at] == candidate {
                return true;
            }
            at += 1;
        }
    }
    false
}
