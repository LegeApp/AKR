//! Immutable external source library (`sources/`).
//!
//! AKR's authoritative knowledge lives in `.akr/`; exact outside advice,
//! audits and reports live in `sources/external/` as immutable, content-hashed
//! files. This module manages the catalog and the file-system invariants.
//!
//! The catalog is `sources/catalog.json` — a deterministic, append-only list
//! of [`SourceDocument`] entries. Every entry carries a SHA-256 content hash;
//! verification recomputes it and reports `AKR-S021` on mismatch. The older
//! file remains when a source is superseded.
//!
//! [`chunk`] turns those exact bytes into retrieval units. Chunking is derived and
//! rebuildable: it can only affect how well search finds a passage, never what the
//! project believes about it.

pub mod chunk;

pub use chunk::{ChunkKind, PARSER_VERSION, SourceChunk, chunk_markdown};

use crate::hash::Sha256;
use std::path::{Path, PathBuf};

/// Origin of a registered source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOrigin {
    /// Advice, audits and reports from outside the project.
    External,
    /// Reference material the project keeps but does not maintain.
    InternalReference,
}

impl SourceOrigin {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::External => "external",
            Self::InternalReference => "internal-reference",
        }
    }

    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "external" => Some(Self::External),
            "internal-reference" | "internal" => Some(Self::InternalReference),
            _ => None,
        }
    }
}

/// A registered source document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDocument {
    /// Stable identifier, e.g. `2026-08-05-jp2lam-audit`.
    pub id: String,
    /// Human title.
    pub title: String,
    /// Origin.
    pub origin: SourceOrigin,
    /// Media type, `text/markdown` for now.
    pub media_type: String,
    /// Repo-relative path, e.g. `sources/external/2026-08-05-foo--a1b2c3d4.md`.
    pub path: String,
    /// `sha256:` + 64 hex.
    pub content_hash: String,
    /// Byte length of the stored file.
    pub byte_len: u64,
    /// Date string `YYYY-MM-DD` from wall clock at add time, or caller-supplied.
    pub added_at: String,
    /// Optional observed git commit or URL.
    pub observed_at: Option<String>,
    /// Optional scope globs.
    pub scope: Option<String>,
    /// Previous version this supersedes, if any.
    pub supersedes: Option<String>,
}

/// Verification diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceDiagnostic {
    /// Stored bytes do not match their hash.
    HashMismatch {
        id: String,
        path: String,
        expected: String,
        found: String,
    },
    /// Catalog references a missing file.
    MissingFile { id: String, path: String },
    /// Catalog itself unreadable.
    CatalogError(String),
}

impl std::fmt::Display for SourceDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HashMismatch {
                id,
                path,
                expected,
                found,
            } => write!(
                f,
                "AKR-S021 registered source bytes do not match their content hash\nsource: {id}\npath: {path}\nexpected: {expected}\nfound: {found}\nhelp: restore the original bytes or register a superseding source version"
            ),
            Self::MissingFile { id, path } => write!(
                f,
                "AKR-S021 registered source file is missing\nsource: {id}\npath: {path}"
            ),
            Self::CatalogError(msg) => write!(f, "AKR-S021 catalog error: {msg}"),
        }
    }
}

/// Catalog file location: `<workspace-root>/sources/catalog.json`.
///
/// `workspace_root` is the directory containing `.akr/`.
#[must_use]
pub fn catalog_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("sources").join("catalog.json")
}

/// External sources directory.
#[must_use]
pub fn external_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join("sources").join("external")
}

/// Computes `sha256:` + hex for bytes.
#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("sha256:{}", h.finish().to_hex())
}

/// Loads the catalog, or returns empty if absent.
pub fn load_catalog(workspace_root: &Path) -> Result<Vec<SourceDocument>, SourceDiagnostic> {
    let path = catalog_path(workspace_root);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| {
        SourceDiagnostic::CatalogError(format!("cannot read {}: {e}", path.display()))
    })?;
    parse_catalog(&raw)
}

/// Parses catalog JSON.
pub fn parse_catalog(json: &str) -> Result<Vec<SourceDocument>, SourceDiagnostic> {
    let v: serde_json_value::Value = parse_json(json)?;
    let arr = v
        .as_array()
        .ok_or_else(|| SourceDiagnostic::CatalogError("catalog root must be array".into()))?;
    let mut out = Vec::new();
    for item in arr {
        out.push(parse_document(item)?);
    }
    Ok(out)
}

fn parse_document(v: &serde_json_value::Value) -> Result<SourceDocument, SourceDiagnostic> {
    let obj = v
        .as_object()
        .ok_or_else(|| SourceDiagnostic::CatalogError("catalog entry must be object".into()))?;
    let get_str = |k: &str, required: bool| -> Result<Option<String>, SourceDiagnostic> {
        match obj.get(k) {
            Some(serde_json_value::Value::String(s)) => Ok(Some(s.clone())),
            Some(serde_json_value::Value::Null) | None if !required => Ok(None),
            None if !required => Ok(None),
            Some(other) => Err(SourceDiagnostic::CatalogError(format!(
                "field {k:?} must be string, got {other:?}"
            ))),
            None => Err(SourceDiagnostic::CatalogError(format!(
                "missing field {k:?}"
            ))),
        }
    };
    let id = get_str("id", true)?.unwrap();
    let title = get_str("title", true)?.unwrap_or_else(|| id.clone());
    let origin = get_str("origin", true)?.unwrap();
    let origin = SourceOrigin::from_str(&origin).ok_or_else(|| {
        SourceDiagnostic::CatalogError(format!(
            "origin {origin:?} is not external|internal-reference"
        ))
    })?;
    let media_type = get_str("media_type", true)?.unwrap();
    let path = get_str("path", true)?.unwrap();
    let content_hash = get_str("content_hash", true)?.unwrap();
    let byte_len = match obj.get("byte_len") {
        Some(serde_json_value::Value::Number(n)) => n.as_u64().unwrap_or(0),
        _ => 0,
    };
    let added_at = get_str("added_at", false)?.unwrap_or_default();
    let observed_at = get_str("observed_at", false)?;
    let scope = get_str("scope", false)?;
    let supersedes = get_str("supersedes", false)?;
    Ok(SourceDocument {
        id,
        title,
        origin,
        media_type,
        path,
        content_hash,
        byte_len,
        added_at,
        observed_at,
        scope,
        supersedes,
    })
}

/// Serializes catalog deterministically (sorted by id).
#[must_use]
pub fn serialize_catalog(docs: &[SourceDocument]) -> String {
    let mut sorted = docs.to_vec();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let mut parts = Vec::new();
    for d in &sorted {
        let mut obj = serde_json_value::Map::new();
        obj.insert("id".into(), serde_json_value::Value::String(d.id.clone()));
        obj.insert(
            "title".into(),
            serde_json_value::Value::String(d.title.clone()),
        );
        obj.insert(
            "origin".into(),
            serde_json_value::Value::String(d.origin.as_str().to_owned()),
        );
        obj.insert(
            "media_type".into(),
            serde_json_value::Value::String(d.media_type.clone()),
        );
        obj.insert(
            "path".into(),
            serde_json_value::Value::String(d.path.clone()),
        );
        obj.insert(
            "content_hash".into(),
            serde_json_value::Value::String(d.content_hash.clone()),
        );
        obj.insert(
            "byte_len".into(),
            serde_json_value::Value::Number(serde_json_value::Number::from(d.byte_len)),
        );
        obj.insert(
            "added_at".into(),
            serde_json_value::Value::String(d.added_at.clone()),
        );
        if let Some(v) = &d.observed_at {
            obj.insert(
                "observed_at".into(),
                serde_json_value::Value::String(v.clone()),
            );
        }
        if let Some(v) = &d.scope {
            obj.insert("scope".into(), serde_json_value::Value::String(v.clone()));
        }
        if let Some(v) = &d.supersedes {
            obj.insert(
                "supersedes".into(),
                serde_json_value::Value::String(v.clone()),
            );
        }
        parts.push(serde_json_value::Value::Object(obj));
    }
    let v = serde_json_value::Value::Array(parts);
    // pretty with 2 spaces, deterministic
    serde_json_value::to_string_pretty(&v).unwrap_or_else(|_| "[]".into()) + "\n"
}

/// Saves catalog atomically (write temp then rename).
pub fn save_catalog(
    workspace_root: &Path,
    docs: &[SourceDocument],
) -> Result<(), SourceDiagnostic> {
    let path = catalog_path(workspace_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            SourceDiagnostic::CatalogError(format!("cannot create {}: {e}", parent.display()))
        })?;
    }
    let raw = serialize_catalog(docs);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw.as_bytes()).map_err(|e| {
        SourceDiagnostic::CatalogError(format!("cannot write {}: {e}", tmp.display()))
    })?;
    std::fs::rename(&tmp, &path).map_err(|e| {
        SourceDiagnostic::CatalogError(format!(
            "cannot rename {} -> {}: {e}",
            tmp.display(),
            path.display()
        ))
    })?;
    Ok(())
}

/// A registered document together with its exact bytes.
#[derive(Debug, Clone)]
pub struct LoadedSource {
    /// The catalog entry.
    pub document: SourceDocument,
    /// The file's contents, verified against `document.content_hash`.
    pub text: String,
}

/// Whether a newer catalog entry supersedes this one.
#[must_use]
pub fn is_superseded(doc: &SourceDocument, catalog: &[SourceDocument]) -> bool {
    catalog
        .iter()
        .any(|other| other.supersedes.as_deref() == Some(doc.id.as_str()))
}

/// A digest over the corpus's identity: sorted `(id, content_hash)` pairs.
///
/// This is the source library's half of the cache generation pair (D-031). It moves when
/// a document is registered or superseded and at no other time, which is what lets a
/// record write leave the chunk tables alone and a source registration leave the record
/// tables alone.
#[must_use]
pub fn corpus_hash(docs: &[SourceDocument]) -> String {
    let mut sorted: Vec<&SourceDocument> = docs.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let mut hasher = Sha256::new();
    for doc in sorted {
        hasher.update(doc.id.as_bytes());
        hasher.update(b"\0");
        hasher.update(doc.content_hash.as_bytes());
        hasher.update(b"\0");
    }
    hasher.update(chunk::PARSER_VERSION.to_string().as_bytes());
    format!("sha256:{}", hasher.finish().to_hex())
}

/// Reads every catalog entry's bytes, verifying each hash on the way through.
///
/// Serving a passage from a file that no longer matches its registration would defeat the
/// whole point of registering it, so a mismatch is an error here rather than a warning.
///
/// # Errors
/// [`SourceDiagnostic::HashMismatch`] or [`SourceDiagnostic::MissingFile`] for the first
/// entry that fails, and [`SourceDiagnostic::CatalogError`] for an unreadable catalog.
pub fn load_corpus(workspace_root: &Path) -> Result<Vec<LoadedSource>, SourceDiagnostic> {
    let catalog = load_catalog(workspace_root)?;
    let mut out = Vec::with_capacity(catalog.len());
    for document in catalog {
        let file = workspace_root.join(&document.path);
        let bytes = std::fs::read(&file).map_err(|_| SourceDiagnostic::MissingFile {
            id: document.id.clone(),
            path: document.path.clone(),
        })?;
        let found = hash_bytes(&bytes);
        if found != document.content_hash {
            return Err(SourceDiagnostic::HashMismatch {
                id: document.id.clone(),
                path: document.path.clone(),
                expected: document.content_hash.clone(),
                found,
            });
        }
        out.push(LoadedSource {
            document,
            text: String::from_utf8_lossy(&bytes).into_owned(),
        });
    }
    Ok(out)
}

/// A record's citation into the source library, checked against the library.
///
/// Four things can be wrong with a citation and they fail differently, so they are
/// reported separately rather than as one "bad source" verdict: the reader's next action
/// is different for each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitationProblem {
    /// The named document is not in the catalog.
    UnknownDocument { document: String },
    /// The byte range runs past the end of the document.
    RangeOutOfBounds {
        document: String,
        end_byte: u64,
        byte_len: u64,
    },
    /// The range does not begin and end on character boundaries.
    RangeNotOnBoundary {
        document: String,
        start: u64,
        end: u64,
    },
    /// The recorded excerpt hash disagrees with the bytes in that range.
    ExcerptHashMismatch {
        document: String,
        expected: String,
        found: String,
    },
    /// The line range does not describe the same passage as the byte range.
    LinesDisagree {
        document: String,
        recorded: (u32, u32),
        actual: (u32, u32),
    },
}

impl std::fmt::Display for CitationProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownDocument { document } => write!(
                f,
                "cites source {document:?}, which is not registered; \
                 run `akr source list` or register it with `akr source add`"
            ),
            Self::RangeOutOfBounds {
                document,
                end_byte,
                byte_len,
            } => write!(
                f,
                "cites bytes up to {end_byte} of source {document:?}, which is {byte_len} bytes long"
            ),
            Self::RangeNotOnBoundary {
                document,
                start,
                end,
            } => write!(
                f,
                "cites bytes {start}..{end} of source {document:?}, which do not fall on \
                 character boundaries"
            ),
            Self::ExcerptHashMismatch {
                document,
                expected,
                found,
            } => write!(
                f,
                "the excerpt hash of the citation into {document:?} does not match the \
                 bytes in its range\nexpected: {expected}\nfound: {found}"
            ),
            Self::LinesDisagree {
                document,
                recorded,
                actual,
            } => write!(
                f,
                "the citation into {document:?} records lines {}-{} for a byte range that \
                 covers lines {}-{}",
                recorded.0, recorded.1, actual.0, actual.1
            ),
        }
    }
}

/// Checks one citation against a loaded corpus.
///
/// Returns nothing for a `source` block that names no document: a loose `path` is still
/// legitimate provenance (a legacy migration writes one), and only a citation into the
/// registered library makes a promise this can check.
#[must_use]
pub fn check_citation(
    source: &crate::model::Source,
    corpus: &[LoadedSource],
) -> Vec<CitationProblem> {
    let Some(document) = &source.document else {
        return Vec::new();
    };
    let Some(loaded) = corpus.iter().find(|item| &item.document.id == document) else {
        return vec![CitationProblem::UnknownDocument {
            document: document.clone(),
        }];
    };
    let Some(range) = &source.range else {
        return Vec::new();
    };

    let text = &loaded.text;
    let byte_len = text.len() as u64;
    if range.end_byte > byte_len || range.start_byte > range.end_byte {
        return vec![CitationProblem::RangeOutOfBounds {
            document: document.clone(),
            end_byte: range.end_byte,
            byte_len,
        }];
    }
    let start = usize::try_from(range.start_byte).unwrap_or(usize::MAX);
    let end = usize::try_from(range.end_byte).unwrap_or(usize::MAX);
    let Some(slice) = text.get(start..end) else {
        return vec![CitationProblem::RangeNotOnBoundary {
            document: document.clone(),
            start: range.start_byte,
            end: range.end_byte,
        }];
    };

    let mut problems = Vec::new();
    if let Some(expected) = &range.excerpt_hash {
        let found = hash_bytes(slice.as_bytes());
        if &found != expected {
            problems.push(CitationProblem::ExcerptHashMismatch {
                document: document.clone(),
                expected: expected.clone(),
                found,
            });
        }
    }

    // Lines are the human half of the locator, and a citation whose lines and bytes
    // disagree is worse than one with no lines at all: a reader would open the file at
    // the wrong place and believe they had found the passage.
    let start_line = u32::try_from(text[..start].matches('\n').count() + 1).unwrap_or(u32::MAX);
    let counted = slice.trim_end_matches('\n').matches('\n').count();
    let end_line = start_line.saturating_add(u32::try_from(counted).unwrap_or(0));
    if (range.start_line, range.end_line) != (start_line, end_line) {
        problems.push(CitationProblem::LinesDisagree {
            document: document.clone(),
            recorded: (range.start_line, range.end_line),
            actual: (start_line, end_line),
        });
    }
    problems
}

/// Verifies every catalog entry's file hash.
#[must_use]
pub fn verify_catalog(workspace_root: &Path) -> Vec<SourceDiagnostic> {
    let docs = match load_catalog(workspace_root) {
        Ok(d) => d,
        Err(e) => return vec![e],
    };
    let mut diags = Vec::new();
    for doc in &docs {
        let file = workspace_root.join(&doc.path);
        if !file.is_file() {
            diags.push(SourceDiagnostic::MissingFile {
                id: doc.id.clone(),
                path: doc.path.clone(),
            });
            continue;
        }
        match std::fs::read(&file) {
            Ok(bytes) => {
                let found = hash_bytes(&bytes);
                if found != doc.content_hash {
                    diags.push(SourceDiagnostic::HashMismatch {
                        id: doc.id.clone(),
                        path: doc.path.clone(),
                        expected: doc.content_hash.clone(),
                        found,
                    });
                } else if bytes.len() as u64 != doc.byte_len {
                    // byte_len mismatch is also a hash mismatch in practice, but report plainly
                    diags.push(SourceDiagnostic::HashMismatch {
                        id: doc.id.clone(),
                        path: doc.path.clone(),
                        expected: doc.content_hash.clone(),
                        found: found.clone(),
                    });
                }
            }
            Err(e) => diags.push(SourceDiagnostic::CatalogError(format!(
                "cannot read {}: {e}",
                file.display()
            ))),
        }
    }
    diags
}

// Minimal JSON value without adding serde dependency. We vendor a tiny parser
// using akr_core::json? Not suitable. Instead we depend on `serde_json` via
// `akr-core` Cargo — keep dependency minimal by using `serde_json` if available
// or fall back to manual. To avoid adding dep, we shell out to akr_core::json::Value
// round-trip? For now we implement with akr_core::json::Value parser and emit
// with manual pretty printing — but catalog read/write needs real JSON. The simplest
// dep-free path: use `serde_json` if present, else hand-parse with akr_core::json.
// We choose to use `akr_core::json` for reading and manual emit for writing above
// already handles writing without serde. For reading we need a small shim.
mod serde_json_value {
    // Lightweight JSON value that we build from akr_core::json::Value
    use std::collections::BTreeMap;
    #[derive(Debug, Clone, PartialEq)]
    pub enum Value {
        Null,
        Bool(bool),
        Number(Number),
        String(String),
        Array(Vec<Value>),
        Object(Map),
    }
    pub type Map = BTreeMap<String, Value>;

    #[derive(Debug, Clone, PartialEq)]
    pub struct Number(u64, String);
    impl Number {
        pub fn from(n: u64) -> Self {
            Self(n, n.to_string())
        }
        pub fn as_u64(&self) -> Option<u64> {
            Some(self.0)
        }
    }

    pub fn to_string_pretty(v: &Value) -> Result<String, String> {
        Ok(pretty(v, 0))
    }

    fn pretty(v: &Value, indent: usize) -> String {
        match v {
            Value::Null => "null".into(),
            Value::Bool(b) => {
                if *b {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            Value::Number(n) => n.1.clone(),
            Value::String(s) => format!("\"{}\"", escape(s)),
            Value::Array(a) => {
                if a.is_empty() {
                    return "[]".into();
                }
                let mut s = String::from("[\n");
                for (i, item) in a.iter().enumerate() {
                    s.push_str(&"  ".repeat(indent + 1));
                    s.push_str(&pretty(item, indent + 1));
                    if i + 1 < a.len() {
                        s.push(',');
                    }
                    s.push('\n');
                }
                s.push_str(&"  ".repeat(indent));
                s.push(']');
                s
            }
            Value::Object(m) => {
                if m.is_empty() {
                    return "{}".into();
                }
                let mut s = String::from("{\n");
                let mut first = true;
                for (k, v) in m {
                    if !first {
                        s.push_str(",\n");
                    }
                    first = false;
                    s.push_str(&"  ".repeat(indent + 1));
                    s.push_str(&format!("\"{}\": {}", escape(k), pretty(v, indent + 1)));
                }
                s.push('\n');
                s.push_str(&"  ".repeat(indent));
                s.push('}');
                s
            }
        }
    }

    fn escape(s: &str) -> String {
        let mut out = String::new();
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
                _ => out.push(c),
            }
        }
        out
    }

    impl Value {
        pub fn as_array(&self) -> Option<&Vec<Value>> {
            if let Self::Array(a) = self {
                Some(a)
            } else {
                None
            }
        }
        pub fn as_object(&self) -> Option<&Map> {
            if let Self::Object(m) = self {
                Some(m)
            } else {
                None
            }
        }
    }
}

fn parse_json(raw: &str) -> Result<serde_json_value::Value, SourceDiagnostic> {
    // Delegate to akr_core::json::parse then convert
    let v = crate::json::parse(raw)
        .map_err(|e| SourceDiagnostic::CatalogError(format!("invalid catalog JSON: {e}")))?;
    Ok(convert(v))
}

fn convert(v: crate::json::Value) -> serde_json_value::Value {
    match v {
        crate::json::Value::Null => serde_json_value::Value::Null,
        crate::json::Value::Bool(b) => serde_json_value::Value::Bool(b),
        crate::json::Value::Integer(n) => {
            let u = u64::try_from(n).unwrap_or(0);
            serde_json_value::Value::Number(serde_json_value::Number::from(u))
        }
        crate::json::Value::String(s) => serde_json_value::Value::String(s),
        crate::json::Value::Array(a) => {
            serde_json_value::Value::Array(a.into_iter().map(convert).collect())
        }
        crate::json::Value::Object(m) => {
            let mut map = std::collections::BTreeMap::new();
            for (k, v) in m {
                map.insert(k, convert(v));
            }
            serde_json_value::Value::Object(map)
        }
    }
}
