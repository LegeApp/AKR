//! The source-library index: chunk storage, incremental sync and BM25 retrieval.
//!
//! Its DDL is [`spec/schema/sources.sql`](../../../../spec/schema/sources.sql), included
//! verbatim for the same reason the record cache includes its own: a schema that is
//! retyped drifts.
//!
//! # Retrieval never authorises
//!
//! `docs/09-context-assembly.md` §1 holds here unchanged, and matters more: these rows
//! come from *outside* the project. A hit says where a passage is, never that the project
//! agreed with it. Everything this module returns is labelled non-authoritative on the
//! way out, and the context assembler cannot call it.

use super::{IndexError, codes};
use crate::source::{LoadedSource, PARSER_VERSION, SourceChunk, chunk_markdown, corpus_hash};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

/// The DDL, verbatim from the spec.
pub const SOURCES_SQL: &str = include_str!("../../../../spec/schema/sources.sql");

/// `meta.schema_version` for the source index. Bumped by hand when the DDL changes.
pub const SOURCES_SCHEMA_VERSION: i64 = 1;

/// The conventional path beneath an `.akr` directory.
#[must_use]
pub fn sources_cache_path(akr_dir: &Path) -> PathBuf {
    akr_dir.join("cache").join("sources.sqlite")
}

/// What a sync did, for `akr build`'s summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceIndexStats {
    /// Documents in the index after the sync.
    pub documents: usize,
    /// Chunks in the index after the sync.
    pub chunks: usize,
    /// Documents chunked by this sync.
    pub added: usize,
    /// Documents dropped by this sync.
    pub removed: usize,
    /// Whether the whole index was rebuilt rather than synced.
    pub rebuilt: bool,
}

/// Brings the index at `path` into agreement with `corpus`.
///
/// Incremental by construction, because a registered document is immutable: a document
/// already present under the current parser version is left exactly as it is, a new one is
/// chunked and inserted, and one that has left the catalog takes its chunks with it. A
/// `parser_version` or `schema_version` change is the only thing that rechunks everything.
///
/// # Errors
/// [`IndexError`] carrying the `AKR-I` code of whichever step failed.
pub fn sync(
    path: &Path,
    corpus: &[LoadedSource],
    tool_version: &str,
    built_at: &str,
) -> Result<SourceIndexStats, IndexError> {
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return Err(IndexError::new(
            codes::I003,
            format!(
                "source index directory {} is not writable: {error}",
                parent.display()
            ),
        ));
    }

    let mut connection = super::open_configured(path).map_err(|error| {
        IndexError::new(
            codes::I001,
            format!("cannot read source index at {}: {error}", path.display()),
        )
    })?;

    let documents: Vec<_> = corpus.iter().map(|item| item.document.clone()).collect();
    let expected_corpus = corpus_hash(&documents);

    let generation_ok = meta(&connection, "schema_version")
        .is_some_and(|value| value == SOURCES_SCHEMA_VERSION.to_string())
        && meta(&connection, "parser_version").is_some_and(|v| v == PARSER_VERSION.to_string());

    let mut rebuilt = false;
    if !generation_ok {
        drop_everything(&connection)?;
        connection
            .execute_batch(SOURCES_SQL)
            .map_err(|error| failed("cannot create the source index schema", &error))?;
        rebuilt = true;
    } else if meta(&connection, "corpus_hash").is_some_and(|value| value == expected_corpus) {
        // Same documents, same scanner: there is nothing this sync could change.
        let stats = counts(&connection);
        return Ok(SourceIndexStats {
            rebuilt: false,
            ..stats
        });
    }

    let transaction = connection
        .transaction()
        .map_err(|error| failed("cannot open the source index transaction", &error))?;

    let present: Vec<(String, String)> = if rebuilt {
        Vec::new()
    } else {
        let mut statement = transaction
            .prepare("SELECT id, content_hash FROM source_documents")
            .map_err(|error| failed("cannot read source_documents", &error))?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|error| failed("cannot read source_documents", &error))?;
        rows.filter_map(Result::ok).collect()
    };

    let mut removed = 0;
    for (id, hash) in &present {
        let still_here = corpus
            .iter()
            .any(|item| &item.document.id == id && &item.document.content_hash == hash);
        if !still_here {
            transaction
                .execute("DELETE FROM source_chunks WHERE document_id = ?1", [id])
                .map_err(|error| failed("cannot drop stale chunks", &error))?;
            transaction
                .execute("DELETE FROM source_documents WHERE id = ?1", [id])
                .map_err(|error| failed("cannot drop a stale document", &error))?;
            removed += 1;
        }
    }

    let mut added = 0;
    for item in corpus {
        let known = present
            .iter()
            .any(|(id, hash)| id == &item.document.id && hash == &item.document.content_hash);
        if known {
            continue;
        }
        insert_document(&transaction, item)?;
        added += 1;
    }

    // `superseded` is derived from the catalog rather than stored on the entry, so a
    // superseding registration updates its predecessor without rewriting the catalog.
    transaction
        .execute("UPDATE source_documents SET superseded = 0", [])
        .map_err(|error| failed("cannot reset supersession", &error))?;
    for item in corpus {
        if let Some(older) = &item.document.supersedes {
            transaction
                .execute(
                    "UPDATE source_documents SET superseded = 1 WHERE id = ?1",
                    [older],
                )
                .map_err(|error| failed("cannot mark a superseded document", &error))?;
        }
    }

    for (key, value) in [
        ("schema_version", SOURCES_SCHEMA_VERSION.to_string()),
        ("parser_version", PARSER_VERSION.to_string()),
        ("corpus_hash", expected_corpus),
        ("tool_version", tool_version.to_owned()),
        ("built_at", built_at.to_owned()),
    ] {
        transaction
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(|error| failed("cannot write source index meta", &error))?;
    }

    transaction
        .commit()
        .map_err(|error| failed("cannot commit the source index transaction", &error))?;

    let stats = counts(&connection);
    Ok(SourceIndexStats {
        added,
        removed,
        rebuilt,
        ..stats
    })
}

fn insert_document(
    transaction: &rusqlite::Transaction,
    item: &LoadedSource,
) -> Result<(), IndexError> {
    let doc = &item.document;
    transaction
        .execute(
            "INSERT INTO source_documents \
             (id, title, path, content_hash, origin, media_type, byte_len, added_at, \
              observed_at, scope, supersedes, superseded) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0)",
            params![
                doc.id,
                doc.title,
                doc.path,
                doc.content_hash,
                doc.origin.as_str(),
                doc.media_type,
                i64::try_from(doc.byte_len).unwrap_or(i64::MAX),
                doc.added_at,
                doc.observed_at,
                doc.scope,
                doc.supersedes,
            ],
        )
        .map_err(|error| failed("cannot insert a source document", &error))?;

    for chunk in chunk_markdown(&item.text) {
        insert_chunk(transaction, &doc.id, &doc.content_hash, &chunk)?;
    }
    Ok(())
}

fn insert_chunk(
    transaction: &rusqlite::Transaction,
    document_id: &str,
    document_hash: &str,
    chunk: &SourceChunk,
) -> Result<(), IndexError> {
    let symbols = chunk.symbols.join("\n");
    let heading = chunk.heading();
    transaction
        .execute(
            "INSERT INTO source_chunks \
             (chunk_id, document_id, parser_version, ordinal, heading_path, kind, \
              start_byte, end_byte, start_line, end_line, raw_text, search_text, \
              symbols, content_hash) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                chunk.id(document_id, document_hash),
                document_id,
                PARSER_VERSION,
                i64::from(chunk.ordinal),
                heading,
                chunk.kind.as_str(),
                i64::try_from(chunk.start_byte).unwrap_or(i64::MAX),
                i64::try_from(chunk.end_byte).unwrap_or(i64::MAX),
                i64::from(chunk.start_line),
                i64::from(chunk.end_line),
                chunk.raw_text,
                chunk.search_text,
                symbols,
                chunk.content_hash,
            ],
        )
        .map_err(|error| failed("cannot insert a source chunk", &error))?;

    // External-content FTS: the shadow row has to be written explicitly, keyed by the
    // rowid the insert above allocated.
    let rowid = transaction.last_insert_rowid();
    transaction
        .execute(
            "INSERT INTO source_chunks_fts (rowid, heading_path, search_text, symbols) \
             VALUES (?1, ?2, ?3, ?4)",
            params![rowid, heading, chunk.search_text, symbols],
        )
        .map_err(|error| {
            IndexError::new(
                codes::I021,
                format!("cannot build source_chunks_fts: {error}"),
            )
        })?;
    Ok(())
}

// ---------------------------------------------------------------------------------------
// retrieval
// ---------------------------------------------------------------------------------------

/// How a query is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueryMode {
    /// Ordinary words. Punctuation is escaped, not handed to the engine (the default).
    #[default]
    Text,
    /// An exact substring, verified against the stored bytes.
    Literal,
    /// A raw FTS5 expression, for somebody who knows they want one.
    Fts,
}

/// A source-library query.
#[derive(Debug, Clone, Default)]
pub struct SourceRequest {
    /// What to look for.
    pub query: String,
    /// How to read [`Self::query`].
    pub mode: QueryMode,
    /// Restrict to these document ids. Empty means every live document.
    pub documents: Vec<String>,
    /// Include documents a later registration supersedes.
    pub all_versions: bool,
    /// Maximum results. `None` is the documented default of 10.
    pub limit: Option<usize>,
}

/// One hit, with everything a citation needs and nothing more.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceHit {
    /// The registered document.
    pub document_id: String,
    /// Its title.
    pub document_title: String,
    /// Its repository-relative path.
    pub document_path: String,
    /// The derived chunk id, usable with `akr source get --chunk`.
    pub chunk_id: String,
    /// The heading path, `a > b > c`.
    pub heading: String,
    /// What the chunk mostly is.
    pub kind: String,
    /// Byte range into the registered bytes.
    pub start_byte: u64,
    /// Byte range into the registered bytes, exclusive.
    pub end_byte: u64,
    /// One-based line range, for people.
    pub start_line: u32,
    /// One-based line range, for people.
    pub end_line: u32,
    /// BM25, sign-flipped so larger is better. Zero for a literal scan, which does not rank.
    pub score: f64,
    /// The first line or so of the chunk.
    pub snippet: String,
}

/// One chunk, whole, plus its neighbours when asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkText {
    /// The hit metadata.
    pub hit: SourceHit,
    /// The exact bytes of the chunk.
    pub text: String,
}

/// Runs a query against the source index at `path`.
///
/// # Errors
/// `AKR-I031` when the index has not been built, `AKR-X031` when a raw FTS expression is
/// rejected, and `AKR-I001` when the index cannot be read at all.
pub fn search(path: &Path, request: &SourceRequest) -> Result<Vec<SourceHit>, IndexError> {
    let connection = open_for_read(path)?;
    let limit = request.limit.unwrap_or(10).min(100);

    if request.mode == QueryMode::Literal {
        return literal_scan(&connection, request, limit);
    }

    let expression = match request.mode {
        QueryMode::Fts => request.query.clone(),
        _ => escape_query(&request.query),
    };
    if expression.trim().is_empty() {
        return Ok(Vec::new());
    }

    // Heading and symbol matches outrank ordinary prose: a query that names a section or
    // an identifier is asking for that section, and BM25 over undifferentiated columns
    // would bury it under a paragraph that repeats the words more often.
    let mut sql = String::from(
        "SELECT d.id, d.title, d.path, c.chunk_id, c.heading_path, c.kind, \
                c.start_byte, c.end_byte, c.start_line, c.end_line, c.raw_text, \
                bm25(source_chunks_fts, 4.0, 1.0, 3.0) AS score \
           FROM source_chunks_fts \
           JOIN source_chunks c ON c.rowid = source_chunks_fts.rowid \
           JOIN source_documents d ON d.id = c.document_id \
          WHERE source_chunks_fts MATCH ?1",
    );
    if !request.all_versions {
        sql.push_str(" AND d.superseded = 0");
    }
    if !request.documents.is_empty() {
        sql.push_str(&format!(
            " AND d.id IN ({})",
            placeholders(request.documents.len(), 2)
        ));
    }
    sql.push_str(" ORDER BY score ASC, d.id ASC, c.ordinal ASC LIMIT ");
    sql.push_str(&limit.to_string());

    let mut bound: Vec<&dyn rusqlite::ToSql> = vec![&expression];
    for document in &request.documents {
        bound.push(document);
    }

    let mut statement = connection.prepare(&sql).map_err(|e| query_error(&e))?;
    let rows = statement
        .query_map(bound.as_slice(), |row| hit_from_row(row, true))
        .map_err(|e| query_error(&e))?;
    let mut hits = Vec::new();
    for row in rows {
        hits.push(row.map_err(|e| query_error(&e))?);
    }
    Ok(hits)
}

/// Exact-substring search, verified against the stored bytes.
///
/// FTS narrows the candidates when the literal has anything word-like in it; otherwise
/// this scans. For a corpus of ordinary project size a scan is milliseconds, and a
/// substring index would be a second index format to keep in step for no measured gain.
fn literal_scan(
    connection: &Connection,
    request: &SourceRequest,
    limit: usize,
) -> Result<Vec<SourceHit>, IndexError> {
    let mut sql = String::from(
        "SELECT d.id, d.title, d.path, c.chunk_id, c.heading_path, c.kind, \
                c.start_byte, c.end_byte, c.start_line, c.end_line, c.raw_text, 0.0 \
           FROM source_chunks c \
           JOIN source_documents d ON d.id = c.document_id \
          WHERE instr(c.raw_text, ?1) > 0",
    );
    if !request.all_versions {
        sql.push_str(" AND d.superseded = 0");
    }
    if !request.documents.is_empty() {
        sql.push_str(&format!(
            " AND d.id IN ({})",
            placeholders(request.documents.len(), 2)
        ));
    }
    sql.push_str(" ORDER BY d.id ASC, c.ordinal ASC LIMIT ");
    sql.push_str(&limit.to_string());

    let mut bound: Vec<&dyn rusqlite::ToSql> = vec![&request.query];
    for document in &request.documents {
        bound.push(document);
    }
    let mut statement = connection.prepare(&sql).map_err(|e| query_error(&e))?;
    let rows = statement
        .query_map(bound.as_slice(), |row| hit_from_row(row, false))
        .map_err(|e| query_error(&e))?;
    let mut hits = Vec::new();
    for row in rows {
        hits.push(row.map_err(|e| query_error(&e))?);
    }
    Ok(hits)
}

/// Fetches one chunk by its derived id, with `neighbors` chunks either side.
///
/// # Errors
/// `AKR-I031` when the index has not been built, `AKR-X031` when the id is unknown.
pub fn get_chunk(
    path: &Path,
    chunk_id: &str,
    neighbors: usize,
) -> Result<Vec<ChunkText>, IndexError> {
    let connection = open_for_read(path)?;
    let anchor: Option<(String, i64)> = connection
        .query_row(
            "SELECT document_id, ordinal FROM source_chunks WHERE chunk_id = ?1",
            [chunk_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();
    let Some((document, ordinal)) = anchor else {
        return Err(IndexError::new(
            codes::X031,
            format!("no chunk {chunk_id:?} in the source index"),
        ));
    };
    let span = i64::try_from(neighbors).unwrap_or(0);
    let mut statement = connection
        .prepare(
            "SELECT d.id, d.title, d.path, c.chunk_id, c.heading_path, c.kind, \
                    c.start_byte, c.end_byte, c.start_line, c.end_line, c.raw_text, 0.0 \
               FROM source_chunks c \
               JOIN source_documents d ON d.id = c.document_id \
              WHERE c.document_id = ?1 AND c.ordinal BETWEEN ?2 AND ?3 \
              ORDER BY c.ordinal ASC",
        )
        .map_err(|e| query_error(&e))?;
    let rows = statement
        .query_map(params![document, ordinal - span, ordinal + span], |row| {
            let text: String = row.get(10)?;
            hit_from_row(row, false).map(|hit| ChunkText { hit, text })
        })
        .map_err(|e| query_error(&e))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| query_error(&e))?);
    }
    Ok(out)
}

/// Every chunk of one document, in order. Used by `akr source get --sections`.
///
/// # Errors
/// `AKR-I031` when the index has not been built.
pub fn document_chunks(path: &Path, document_id: &str) -> Result<Vec<SourceHit>, IndexError> {
    let connection = open_for_read(path)?;
    let mut statement = connection
        .prepare(
            "SELECT d.id, d.title, d.path, c.chunk_id, c.heading_path, c.kind, \
                    c.start_byte, c.end_byte, c.start_line, c.end_line, c.raw_text, 0.0 \
               FROM source_chunks c \
               JOIN source_documents d ON d.id = c.document_id \
              WHERE c.document_id = ?1 ORDER BY c.ordinal ASC",
        )
        .map_err(|e| query_error(&e))?;
    let rows = statement
        .query_map([document_id], |row| hit_from_row(row, false))
        .map_err(|e| query_error(&e))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| query_error(&e))?);
    }
    Ok(out)
}

/// Turns a user's words into an FTS5 expression that cannot be a syntax error.
///
/// The record search takes raw FTS5, which is a trap for an agent: `DecodeRequest::default()`
/// is a parse error, not a query. Quoting every term makes punctuation ordinary text, and
/// `--fts` is still there for anyone who wants the operators.
#[must_use]
pub fn escape_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| {
            // Each whitespace-separated term becomes one quoted phrase, with its
            // punctuation turned into token boundaries. `DecodeRequest::default()`
            // becomes the phrase "DecodeRequest default", which is exactly how the
            // tokeniser stored the symbol — matching it without the caller having to
            // know that FTS5 would have read `::` as an operator.
            let parts: Vec<&str> = term
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .filter(|part| !part.is_empty())
                .collect();
            parts.join(" ")
        })
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>()
        .join(" ")
}

fn hit_from_row(row: &rusqlite::Row<'_>, ranked: bool) -> rusqlite::Result<SourceHit> {
    let raw: String = row.get(10)?;
    let score: f64 = row.get(11)?;
    Ok(SourceHit {
        document_id: row.get(0)?,
        document_title: row.get(1)?,
        document_path: row.get(2)?,
        chunk_id: row.get(3)?,
        heading: row.get(4)?,
        kind: row.get(5)?,
        start_byte: row.get::<_, i64>(6)?.try_into().unwrap_or(0),
        end_byte: row.get::<_, i64>(7)?.try_into().unwrap_or(0),
        start_line: row.get::<_, i64>(8)?.try_into().unwrap_or(0),
        end_line: row.get::<_, i64>(9)?.try_into().unwrap_or(0),
        score: if ranked { -score } else { 0.0 },
        snippet: snippet_of(&raw),
    })
}

fn snippet_of(raw: &str) -> String {
    let flat = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= 160 {
        return flat;
    }
    let mut out: String = flat.chars().take(157).collect();
    out.push_str("...");
    out
}

fn open_for_read(path: &Path) -> Result<Connection, IndexError> {
    if !path.exists() {
        return Err(IndexError::new(
            codes::I031,
            "the source index has not been built; run `akr build`",
        ));
    }
    super::open_configured(path).map_err(|error| {
        IndexError::new(
            codes::I001,
            format!("cannot read source index at {}: {error}", path.display()),
        )
    })
}

fn meta(connection: &Connection, key: &str) -> Option<String> {
    connection
        .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .ok()
}

fn counts(connection: &Connection) -> SourceIndexStats {
    let count = |table: &str| -> usize {
        connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get::<_, i64>(0)
            })
            .ok()
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(0)
    };
    SourceIndexStats {
        documents: count("source_documents"),
        chunks: count("source_chunks"),
        ..SourceIndexStats::default()
    }
}

fn drop_everything(connection: &Connection) -> Result<(), IndexError> {
    connection
        .execute_batch("PRAGMA foreign_keys = OFF")
        .map_err(|error| failed("cannot suspend foreign keys", &error))?;
    let mut objects: Vec<(String, String)> = Vec::new();
    {
        let Ok(mut statement) = connection.prepare(
            "SELECT type, name FROM sqlite_master \
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'",
        ) else {
            return Ok(());
        };
        if let Ok(rows) = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            objects.extend(rows.filter_map(Result::ok));
        }
    }
    for (kind, name) in objects.iter().filter(|(_, n)| n == "source_chunks_fts") {
        let _ = connection.execute(&format!("DROP {kind} IF EXISTS \"{name}\""), []);
    }
    for (kind, name) in &objects {
        if name.starts_with("source_chunks_fts") {
            continue;
        }
        let _ = connection.execute(&format!("DROP {kind} IF EXISTS \"{name}\""), []);
    }
    connection
        .execute_batch("PRAGMA foreign_keys = ON")
        .map_err(|error| failed("cannot restore foreign keys", &error))?;
    Ok(())
}

fn failed(what: &str, error: &rusqlite::Error) -> IndexError {
    IndexError::new(codes::I002, format!("{what}: {error}"))
}

fn query_error(error: &rusqlite::Error) -> IndexError {
    IndexError::new(codes::X031, format!("source search query: {error}"))
}

fn placeholders(count: usize, first: usize) -> String {
    (first..first + count)
        .map(|n| format!("?{n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{SourceAvailability, SourceDocument, SourceOrigin};

    fn document(id: &str, text: &str) -> LoadedSource {
        LoadedSource {
            document: SourceDocument {
                id: id.to_owned(),
                title: format!("{id} title"),
                origin: SourceOrigin::External,
                media_type: "text/markdown".into(),
                path: format!("sources/external/{id}.md"),
                content_hash: crate::source::hash_bytes(text.as_bytes()),
                byte_len: text.len() as u64,
                added_at: "2026-08-07".into(),
                observed_at: None,
                scope: None,
                supersedes: None,
                availability: SourceAvailability::Full,
                fragments: Vec::new(),
            },
            text: text.to_owned(),
        }
    }

    const AUDIT: &str = "\
# Decoder audit

## 6. P1: nonzero tile origins unnecessarily disable optimized DWT

Aligned nonzero origins are routed to the scalar path even when the phase is even.
Call `DecodeRequest::default()` first.

## 5.1 Scratch allocation per parallel row

Rayon's for_each_init does not necessarily allocate once per worker.
";

    fn indexed(dir: &Path, corpus: &[LoadedSource]) -> PathBuf {
        let path = dir.join("sources.sqlite");
        sync(&path, corpus, "0.1.0", "2026-08-07T00:00:00Z").expect("sync");
        path
    }

    #[test]
    fn a_query_finds_the_expected_section() {
        let dir = tempdir();
        let path = indexed(dir.path(), &[document("audit", AUDIT)]);
        let hits = search(
            &path,
            &SourceRequest {
                query: "nonzero tile origins".into(),
                ..SourceRequest::default()
            },
        )
        .expect("search");
        assert!(!hits.is_empty());
        assert!(hits[0].heading.contains("nonzero tile origins"), "{hits:?}");
    }

    #[test]
    fn a_punctuated_query_is_not_a_syntax_error() {
        let dir = tempdir();
        let path = indexed(dir.path(), &[document("audit", AUDIT)]);
        // Raw FTS5 would reject this; the default mode must not.
        let hits = search(
            &path,
            &SourceRequest {
                query: "DecodeRequest::default()".into(),
                ..SourceRequest::default()
            },
        )
        .expect("search");
        assert!(!hits.is_empty(), "expected the symbol expansion to match");
    }

    #[test]
    fn literal_search_verifies_the_exact_bytes() {
        let dir = tempdir();
        let path = indexed(dir.path(), &[document("audit", AUDIT)]);
        let hits = search(
            &path,
            &SourceRequest {
                query: "DecodeRequest::default()".into(),
                mode: QueryMode::Literal,
                ..SourceRequest::default()
            },
        )
        .expect("search");
        assert_eq!(hits.len(), 1);
        let miss = search(
            &path,
            &SourceRequest {
                query: "DecodeRequest::never()".into(),
                mode: QueryMode::Literal,
                ..SourceRequest::default()
            },
        )
        .expect("search");
        assert!(miss.is_empty());
    }

    #[test]
    fn a_byte_range_reproduces_the_registered_source() {
        let dir = tempdir();
        let path = indexed(dir.path(), &[document("audit", AUDIT)]);
        for hit in document_chunks(&path, "audit").expect("chunks") {
            let start = usize::try_from(hit.start_byte).unwrap();
            let end = usize::try_from(hit.end_byte).unwrap();
            assert!(AUDIT.get(start..end).is_some(), "{hit:?}");
        }
    }

    #[test]
    fn syncing_the_same_corpus_twice_does_no_work() {
        let dir = tempdir();
        let corpus = [document("audit", AUDIT)];
        let path = indexed(dir.path(), &corpus);
        let again = sync(&path, &corpus, "0.1.0", "2026-08-08T00:00:00Z").expect("sync");
        assert_eq!(again.added, 0);
        assert_eq!(again.removed, 0);
        assert!(!again.rebuilt);
    }

    #[test]
    fn registering_a_second_document_leaves_the_first_alone() {
        let dir = tempdir();
        let mut corpus = vec![document("audit", AUDIT)];
        let path = indexed(dir.path(), &corpus);
        let first = document_chunks(&path, "audit").expect("chunks");
        corpus.push(document("notes", "# Notes\n\nSomething else entirely.\n"));
        let stats = sync(&path, &corpus, "0.1.0", "2026-08-08T00:00:00Z").expect("sync");
        assert_eq!(stats.added, 1);
        assert!(!stats.rebuilt);
        assert_eq!(document_chunks(&path, "audit").expect("chunks"), first);
    }

    #[test]
    fn a_superseded_document_is_hidden_unless_asked_for() {
        let dir = tempdir();
        let mut newer = document("audit-2", AUDIT);
        newer.document.supersedes = Some("audit".into());
        let corpus = vec![document("audit", AUDIT), newer];
        let path = indexed(dir.path(), &corpus);
        let request = SourceRequest {
            query: "nonzero tile origins".into(),
            ..SourceRequest::default()
        };
        let live = search(&path, &request).expect("search");
        assert!(
            live.iter().all(|hit| hit.document_id == "audit-2"),
            "{live:?}"
        );
        let all = search(
            &path,
            &SourceRequest {
                all_versions: true,
                ..request
            },
        )
        .expect("search");
        assert!(all.iter().any(|hit| hit.document_id == "audit"));
    }

    #[test]
    fn neighbors_widen_a_chunk_without_overlap() {
        let dir = tempdir();
        let path = indexed(dir.path(), &[document("audit", AUDIT)]);
        let all = document_chunks(&path, "audit").expect("chunks");
        assert!(all.len() >= 2, "{} chunks", all.len());
        let widened = get_chunk(&path, &all[0].chunk_id, 1).expect("chunk");
        assert!(widened.len() >= 2);
        assert_eq!(widened[0].hit.chunk_id, all[0].chunk_id);
    }

    #[test]
    fn escaping_turns_punctuation_into_terms() {
        assert_eq!(
            escape_query("DecodeRequest::default()"),
            "\"DecodeRequest default\""
        );
        assert_eq!(
            escape_query("nonzero tile origins"),
            "\"nonzero\" \"tile\" \"origins\""
        );
        assert_eq!(escape_query("  "), "");
    }

    // A tiny temp directory, so the crate keeps its one-dependency rule.
    struct TempDir(PathBuf);
    impl TempDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn tempdir() -> TempDir {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("akr-source-index-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("temp dir");
        TempDir(path)
    }
}
