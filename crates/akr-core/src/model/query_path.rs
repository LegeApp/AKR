//! Normalizing a query-time path argument (`--paths` / MCP `paths`) into canonical glob
//! text, before it ever becomes a [`Glob`].
//!
//! # Why this is not a grammar change
//!
//! [`crate::freshness::validate_glob`] enforces the D-008 subset verbatim: no backslashes,
//! no leading `/`. That is correct for text stored in a record's `watches` field — it is
//! canonically formatted and must round-trip byte-for-byte, so a record author who typed a
//! backslash made a mistake and should be told so. It is the wrong rule for a path an agent
//! or a human pastes on the command line or over MCP, where the ambient shell already uses
//! `\` as a separator and an absolute path is the natural thing to have on hand.
//!
//! This module sits upstream of that grammar: it turns the ambient spelling into the one
//! canonical form *before* a [`Glob`] is constructed from it, so [`validate_glob`], the
//! D-010 overlap test ([`crate::model::glob_prefixes_comparable`]) and the freshness matcher
//! never see anything but the canonical form. Nothing here loosens what those accept, and
//! nothing here touches how a `watches` field parses out of a record.
//!
//! [`validate_glob`]: crate::freshness::validate_glob

use super::Glob;
use std::path::Path;

/// Why a query-time path argument could not be normalized into a glob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryPathError {
    /// An absolute path that does not lie inside the repository root.
    OutsideRepo {
        /// The path as given (before normalization), for the diagnostic.
        path: String,
        /// The repository root it was checked against.
        root: String,
    },
}

impl std::fmt::Display for QueryPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutsideRepo { path, root } => {
                write!(f, "{path}: outside the repository (root {root})")
            }
        }
    }
}

impl std::error::Error for QueryPathError {}

/// Builds a glob from a query-time path argument (`--paths` / MCP `paths`).
///
/// Backslashes are always treated as separators, never literal characters: the D-008 subset
/// has no escape syntax, so a backslash can only ever mean "wrong separator" — there is no
/// case where turning it into `/` changes what a well-formed glob means. An absolute path —
/// POSIX-rooted (`/…`), a Windows drive (`C:\…`), or a Windows verbatim/UNC form
/// (`\\?\C:\…`, `\\host\share\…`) — is made repo-root-relative when it lies inside `root`.
/// One that does not is rejected outright: silently keeping it as an absolute glob would
/// violate D-008 anyway (globs may not start with `/`), and silently truncating it some other
/// way would risk matching something the caller never intended.
///
/// Absoluteness is detected textually rather than with [`std::path::Path::is_absolute`]:
/// that method disagrees with itself across platforms (a Windows drive path is not absolute
/// by POSIX's rules, and a POSIX-rooted path is not absolute by Windows's), and the D-008
/// subset only ever separates on `/`, so comparing normalized strings is both simpler and
/// gives the same answer on every platform for the same input — which is what makes this
/// normalization safe to unit-test on any host regardless of which platform eventually runs
/// it.
///
/// A relative, already-forward-slash input passes through unchanged.
///
/// # Errors
/// [`QueryPathError::OutsideRepo`] when an absolute path does not lie inside `root`.
pub fn normalize_query_path(text: &str, root: &Path) -> Result<Glob, QueryPathError> {
    let slashed = text.replace('\\', "/");
    if !is_absolute(&slashed) {
        return Ok(Glob::new(&slashed));
    }

    let root_slashed = strip_verbatim_prefix(&root.to_string_lossy().replace('\\', "/"))
        .trim_end_matches('/')
        .to_owned();
    let candidate = strip_verbatim_prefix(&slashed);
    let candidate = uppercase_drive(candidate);
    let root_slashed = uppercase_drive(&root_slashed);

    match candidate.strip_prefix(root_slashed.as_str()) {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => {
            Ok(Glob::new(rest.trim_start_matches('/')))
        }
        _ => Err(QueryPathError::OutsideRepo {
            path: text.to_owned(),
            root: root.display().to_string(),
        }),
    }
}

/// Whether normalized (forward-slash) path text is absolute: POSIX-rooted, a Windows drive
/// (`C:/…`), or a UNC/verbatim share (`//…`, which a backslash-to-slash pass turns
/// `\\host\share` into).
fn is_absolute(slashed: &str) -> bool {
    slashed.starts_with('/') || is_drive_absolute(slashed)
}

/// Whether normalized path text starts with a Windows drive letter (`C:/…` or bare `C:`).
fn is_drive_absolute(slashed: &str) -> bool {
    let bytes = slashed.as_bytes();
    bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes.len() == 2 || bytes[2] == b'/')
}

/// Strips the `//?/` verbatim prefix that [`Path::canonicalize`] adds on Windows (after a
/// backslash-to-slash pass, `\\?\` reads as `//?/`), so a canonicalized repository root and
/// an ordinary user-typed path compare equal.
fn strip_verbatim_prefix(slashed: &str) -> &str {
    slashed.strip_prefix("//?/").unwrap_or(slashed)
}

/// Uppercases a leading drive letter (`d:/…` -> `D:/…`).
///
/// Windows drive letters are case-insensitive, and which case a given path spells them in is
/// an accident of who typed it or which API produced it — [`Path::canonicalize`] and a
/// human's shell do not reliably agree. Aligning the case here, on both the candidate and the
/// root, keeps that accident from reading as "outside the repository".
fn uppercase_drive(slashed: &str) -> String {
    if is_drive_absolute(slashed) {
        let mut chars = slashed.chars();
        let letter = chars.next().expect("is_drive_absolute checked len >= 2");
        format!("{}{}", letter.to_ascii_uppercase(), chars.as_str())
    } else {
        slashed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        std::path::PathBuf::from("D:/Rust-projects/AKR")
    }

    #[test]
    fn relative_forward_slash_passes_through_unchanged() {
        let glob = normalize_query_path("crates/akr-core/src", &root()).unwrap();
        assert_eq!(glob.as_str(), "crates/akr-core/src");
    }

    #[test]
    fn relative_backslash_path_becomes_forward_slash() {
        let glob = normalize_query_path(r"crates\akr-core\src", &root()).unwrap();
        assert_eq!(glob.as_str(), "crates/akr-core/src");
    }

    #[test]
    fn mixed_separators_normalize() {
        let glob = normalize_query_path(r"crates\akr-core/src\lib.rs", &root()).unwrap();
        assert_eq!(glob.as_str(), "crates/akr-core/src/lib.rs");
    }

    #[test]
    fn absolute_in_repo_path_becomes_repo_relative() {
        let glob =
            normalize_query_path(r"D:\Rust-projects\AKR\crates\akr-core\src", &root()).unwrap();
        assert_eq!(glob.as_str(), "crates/akr-core/src");
    }

    #[test]
    fn absolute_in_repo_path_forward_slash_form_also_works() {
        let glob = normalize_query_path("D:/Rust-projects/AKR/crates", &root()).unwrap();
        assert_eq!(glob.as_str(), "crates");
    }

    #[test]
    fn absolute_path_exactly_the_repo_root_normalizes_to_empty() {
        // Downstream `validate_glob` rejects the empty glob (`GlobError::Empty`); this
        // module's job stops at producing the correct (empty) repo-relative text.
        let glob = normalize_query_path(r"D:\Rust-projects\AKR", &root()).unwrap();
        assert_eq!(glob.as_str(), "");
    }

    #[test]
    fn absolute_out_of_repo_windows_path_is_rejected() {
        let error = normalize_query_path(r"C:\Windows\System32", &root()).unwrap_err();
        assert!(matches!(error, QueryPathError::OutsideRepo { .. }));
    }

    #[test]
    fn absolute_out_of_repo_posix_path_is_rejected_against_a_windows_root() {
        let error = normalize_query_path("/etc/passwd", &root()).unwrap_err();
        assert!(matches!(error, QueryPathError::OutsideRepo { .. }));
    }

    #[test]
    fn absolute_out_of_repo_posix_path_is_rejected_against_a_posix_root() {
        let root = std::path::PathBuf::from("/home/dk/repo");
        let error = normalize_query_path("/etc/passwd", &root).unwrap_err();
        assert!(matches!(error, QueryPathError::OutsideRepo { .. }));
    }

    #[test]
    fn absolute_in_repo_path_matches_a_posix_root() {
        let root = std::path::PathBuf::from("/home/dk/repo");
        let glob = normalize_query_path("/home/dk/repo/crates/akr-core", &root).unwrap();
        assert_eq!(glob.as_str(), "crates/akr-core");
    }

    #[test]
    fn sibling_directory_sharing_a_prefix_is_not_treated_as_inside() {
        // `AKR2` must not be accepted just because `AKR` is a byte-prefix of it.
        let error = normalize_query_path(r"D:\Rust-projects\AKR2\src", &root()).unwrap_err();
        assert!(matches!(error, QueryPathError::OutsideRepo { .. }));
    }

    #[test]
    fn drive_letter_case_is_not_significant() {
        let glob = normalize_query_path(r"d:\Rust-projects\AKR\crates", &root()).unwrap();
        assert_eq!(glob.as_str(), "crates");
    }

    #[test]
    fn verbatim_prefixed_root_still_matches_an_ordinary_absolute_path() {
        let root = std::path::PathBuf::from(r"\\?\D:\Rust-projects\AKR");
        let glob = normalize_query_path(r"D:\Rust-projects\AKR\crates", &root).unwrap();
        assert_eq!(glob.as_str(), "crates");
    }

    #[test]
    fn determinism_relative_input_is_platform_independent() {
        // The whole point of doing this textually rather than via `std::path`: the same
        // relative input normalizes identically no matter which OS runs the test.
        let a = normalize_query_path(r"a\b\c", &root()).unwrap();
        let b = normalize_query_path("a/b/c", &root()).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.as_str(), "a/b/c");
    }
}
