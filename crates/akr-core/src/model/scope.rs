//! Scope terms and the conservative overlap test of D-010.

use super::ident::Glob;
use super::ledger::PartOfIndex;
use super::refs::Reference;
use std::fmt;

/// One scope term. Three forms and no others (D-010).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScopeTerm {
    /// Project-wide. Overlaps everything.
    All,
    /// Organisational scope. The target must be a milestone, track or constraint.
    Ref(Reference),
    /// Code scope, repo-root-relative.
    Path(Glob),
}

impl fmt::Display for ScopeTerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => f.write_str("all"),
            Self::Ref(r) => write!(f, "ref {r}"),
            Self::Path(g) => write!(f, "path {:?}", g.as_str()),
        }
    }
}

/// Whether two scopes overlap: any term of one against any term of the other.
///
/// The test is deliberately **conservative**. It may report an overlap where none exists
/// in practice, and it must never miss one. A false positive is resolved by narrowing a
/// scope or dropping a `topic`; a false negative would silently permit two contradictory
/// policies to govern the same code.
///
/// An empty scope overlaps nothing, including itself — a record with no scope governs
/// nothing, so V-013 has nothing to compare.
#[must_use]
pub fn scopes_overlap(a: &[ScopeTerm], b: &[ScopeTerm], parents: &PartOfIndex) -> bool {
    a.iter()
        .any(|x| b.iter().any(|y| terms_overlap(x, y, parents)))
}

/// Whether two individual scope terms overlap.
///
/// A `ref` term and a `path` term never overlap directly: inferring paths from a track's
/// contents would make overlap depend on the whole graph.
#[must_use]
pub fn terms_overlap(a: &ScopeTerm, b: &ScopeTerm, parents: &PartOfIndex) -> bool {
    match (a, b) {
        (ScopeTerm::All, _) | (_, ScopeTerm::All) => true,
        (ScopeTerm::Ref(x), ScopeTerm::Ref(y)) => {
            x.key == y.key
                || parents.is_ancestor(&x.key, &y.key)
                || parents.is_ancestor(&y.key, &x.key)
        }
        (ScopeTerm::Path(x), ScopeTerm::Path(y)) => glob_prefixes_comparable(x, y),
        _ => false,
    }
}

/// Whether two globs' literal segment prefixes are prefix-comparable.
///
/// `**` is treated as matching any run of segments and `*`/`?` as matching within one
/// segment, so comparison stops at the first wildcard segment on each side.
#[must_use]
pub fn glob_prefixes_comparable(a: &Glob, b: &Glob) -> bool {
    let (pa, pb) = (a.literal_prefix(), b.literal_prefix());
    pa.iter().zip(pb.iter()).all(|(x, y)| x == y)
}
