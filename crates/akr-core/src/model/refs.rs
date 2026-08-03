//! References in the four forms of D-009, and revision identifiers.

use super::ident::{IdentError, LogicalKey, Segment};
use std::fmt;

/// A revision identifier: the key plus the revision number.
///
/// This is what every reference ultimately resolves to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RevisionId {
    /// The logical key.
    pub key: LogicalKey,
    /// The revision number, from 1.
    pub revision: u32,
}

impl RevisionId {
    /// Builds a revision identifier.
    #[must_use]
    pub fn new(key: LogicalKey, revision: u32) -> Self {
        Self { key, revision }
    }
}

impl fmt::Display for RevisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.key, self.revision)
    }
}

/// Whether a reference follows the head or names one revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RefMode {
    /// `@key` or `@key#anchor` — resolves to whichever revision is the head.
    CurrentHead,
    /// `@key/2` or `@key/2#anchor` — always that revision.
    Pinned,
}

/// A reference. Exactly four forms exist and no others (D-009).
///
/// | Form | `revision` | `anchor` |
/// | --- | --- | --- |
/// | `@key` | `None` | `None` |
/// | `@key/2` | `Some` | `None` |
/// | `@key#anchor` | `None` | `Some` |
/// | `@key/2#anchor` | `Some` | `Some` |
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reference {
    /// The key being referred to.
    pub key: LogicalKey,
    /// The pinned revision, if any.
    pub revision: Option<u32>,
    /// The claim or check anchor, if any.
    pub anchor: Option<Segment>,
}

impl Reference {
    /// `@key`.
    #[must_use]
    pub fn head(key: LogicalKey) -> Self {
        Self {
            key,
            revision: None,
            anchor: None,
        }
    }

    /// `@key/N`.
    #[must_use]
    pub fn pinned(key: LogicalKey, revision: u32) -> Self {
        Self {
            key,
            revision: Some(revision),
            anchor: None,
        }
    }

    /// `@key#anchor`.
    #[must_use]
    pub fn head_anchor(key: LogicalKey, anchor: Segment) -> Self {
        Self {
            key,
            revision: None,
            anchor: Some(anchor),
        }
    }

    /// `@key/N#anchor`.
    #[must_use]
    pub fn pinned_anchor(key: LogicalKey, revision: u32, anchor: Segment) -> Self {
        Self {
            key,
            revision: Some(revision),
            anchor: Some(anchor),
        }
    }

    /// Parses `@key`, `@key/N`, `@key#anchor` or `@key/N#anchor`. The `@` is optional.
    ///
    /// # Errors
    /// Returns an error if the key, revision or anchor is malformed.
    pub fn parse(text: &str) -> Result<Self, IdentError> {
        let body = text.strip_prefix('@').unwrap_or(text);
        let (body, anchor) = match body.split_once('#') {
            Some((b, a)) => (b, Some(Segment::new(a)?)),
            None => (body, None),
        };
        let (key_text, revision) = match body.split_once('/') {
            Some((k, r)) => (
                k,
                Some(
                    r.parse::<u32>()
                        .map_err(|_| IdentError::BadSegment(r.to_owned()))?,
                ),
            ),
            None => (body, None),
        };
        Ok(Self {
            key: LogicalKey::parse(key_text)?,
            revision,
            anchor,
        })
    }

    /// Whether this reference follows the head or names one revision.
    #[must_use]
    pub fn mode(&self) -> RefMode {
        if self.revision.is_some() {
            RefMode::Pinned
        } else {
            RefMode::CurrentHead
        }
    }

    /// True for `@key/N` and `@key/N#anchor`.
    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.mode() == RefMode::Pinned
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.key)?;
        if let Some(r) = self.revision {
            write!(f, "/{r}")?;
        }
        if let Some(a) = &self.anchor {
            write!(f, "#{a}")?;
        }
        Ok(())
    }
}
