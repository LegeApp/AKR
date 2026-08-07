//! Logging a papercut: a small friction hit while working, recorded in the moment
//! (D-027).
//!
//! The library operation behind `akr papercut` and the `knowledge.papercut` MCP tool.
//! Like [`crate::evidence`], this module builds the record; serialising and writing is
//! the caller's, through the one write pipeline of `docs/07-cli.md` §4.
//!
//! # Zero ceremony, by construction
//!
//! A papercut costs one message. Everything else — the key, the slug, the commit, the
//! author, the date — is filled in here, because a log that asks for ceremony does not
//! get written in the moment, and a papercut written later is a papercut forgotten.

use crate::import::slug_of;
use crate::model::{
    Commit, ContentSlot, ContentValue, Date, Kind, Ledger, LogicalKey, Record, RevisionId, State,
};
use std::collections::BTreeMap;

pub mod collate;

/// What to log. The message is the only thing the caller has to say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogPapercut {
    /// One or two sentences: what you were doing, what got in the way, and — as a
    /// bonus — a guess at the cause or fix.
    pub message: String,
    /// Who hit it: a model or harness name. Lands in the `author` slot.
    pub agent: String,
    /// The commit it happened at. The tooling defaults this to HEAD.
    pub observed_at: Commit,
    /// The authoring date. The tooling fills this from `--today` or the system date.
    pub created_at: Option<Date>,
}

/// Why a papercut key could not be allocated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PapercutKeyError {
    /// No namespace was given and the project declares more than one.
    AmbiguousNamespace(Vec<String>),
    /// The requested namespace is not declared in the project.
    UnknownNamespace(String),
    /// The project declares no namespaces at all.
    NoNamespaces,
}

impl std::fmt::Display for PapercutKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AmbiguousNamespace(names) => write!(
                f,
                "the project declares several namespaces ({}); say which with --namespace",
                names.join(", ")
            ),
            Self::UnknownNamespace(name) => {
                write!(f, "namespace {name:?} is not declared in .akr/project.akr")
            }
            Self::NoNamespaces => write!(f, "the project declares no namespaces"),
        }
    }
}

impl std::error::Error for PapercutKeyError {}

/// Allocates the key for a new papercut: `<namespace>.papercut.<slug-of-message>`,
/// suffixed `-2`, `-3`, … until it collides with nothing in the ledger.
///
/// With no namespace given, the project's sole declared namespace is used; several
/// declared namespaces make the choice the caller's.
///
/// # Errors
/// [`PapercutKeyError`] when the namespace cannot be determined.
pub fn allocate_key(
    ledger: &Ledger,
    namespace: Option<&str>,
    message: &str,
) -> Result<LogicalKey, PapercutKeyError> {
    let declared: Vec<String> = ledger
        .project
        .namespaces
        .iter()
        .map(ToString::to_string)
        .collect();
    let namespace = match namespace {
        Some(name) => {
            if !declared.iter().any(|d| d == name) {
                return Err(PapercutKeyError::UnknownNamespace(name.to_owned()));
            }
            name.to_owned()
        }
        None => match declared.as_slice() {
            [] => return Err(PapercutKeyError::NoNamespaces),
            [sole] => sole.clone(),
            _ => return Err(PapercutKeyError::AmbiguousNamespace(declared)),
        },
    };

    let base = {
        let slug = slug_of(message);
        if slug.is_empty() {
            "papercut".to_owned()
        } else {
            slug
        }
    };
    let taken: std::collections::BTreeSet<String> = ledger
        .records()
        .iter()
        .map(|record| record.id.key.to_string())
        .collect();
    let mut candidate = format!("{namespace}.papercut.{base}");
    let mut n = 1usize;
    while taken.contains(&candidate) {
        n += 1;
        candidate = format!("{namespace}.papercut.{base}-{n}");
    }
    Ok(LogicalKey::parse(&candidate).expect("namespace and slug are valid segments"))
}

impl LogPapercut {
    /// Builds the record this request describes, without validating anything.
    #[must_use]
    pub fn to_record(&self, key: LogicalKey) -> Record {
        let mut content: BTreeMap<ContentSlot, ContentValue> = BTreeMap::new();
        content.insert(
            ContentSlot::Statement,
            ContentValue::Prose(self.message.clone()),
        );
        content.insert(
            ContentSlot::ObservedAt,
            ContentValue::Commit(self.observed_at.clone()),
        );

        Record {
            id: RevisionId::new(key, 1),
            kind: Kind::Papercut,
            title: title_of(&self.message),
            // Empirical kinds have no proposal state: the friction either was hit or
            // was not.
            state: State::Verified,
            scope: Vec::new(),
            topic: None,
            content,
            claims: Vec::new(),
            retired_claims: Vec::new(),
            acceptance: None,
            dispositions: Vec::new(),
            relations: BTreeMap::new(),
            acknowledged: false,
            author: Some(self.agent.clone()),
            created_at: self.created_at,
            sources: Vec::new(),
            file: None,
        }
    }
}

/// The message as a title: its first line, ellipsized near 72 bytes on a word break.
fn title_of(message: &str) -> String {
    let line = message.lines().next().unwrap_or("").trim();
    if line.len() <= 72 {
        return line.to_owned();
    }
    let cut = line[..72].rfind(' ').unwrap_or(72);
    format!("{}…", line[..cut].trim_end())
}
