-- spec/schema/sources.sql — DDL for the AKR source-library index.
--
-- WHAT THIS IS
--   The schema of `.akr/cache/sources.sqlite`, a derived index over the
--   immutable documents registered in `sources/catalog.json`
--   (docs/15-external-sources.md §5). It answers `akr source search` and
--   `akr source get --chunk`.
--
-- WHY IT IS NOT `index.sqlite` (D-031)
--   The record cache is rebuilt wholesale whenever `meta.source_graph_hash`
--   moves, which is every ledger write. Chunking a source corpus on every
--   record write, and re-resolving the ledger on every source registration,
--   are both work nobody asked for. Two files give the two generations the
--   design wants — `source_corpus_hash` here, `source_graph_hash` there —
--   without a partial-rebuild path through the record cache's drop-everything
--   invalidation. It is still one storage engine, one query language and one
--   ranker: the thing the design refused was a second *kind* of index, not a
--   second file.
--
-- WHAT THIS IS NOT (D-019 applies unchanged)
--   * NOT authoritative. `sources/external/*` are the only source of truth,
--     and the ledger is the only source of project authority. A chunk boundary
--     is a retrieval convenience; it can make a passage harder to find and can
--     never make it mean something else.
--   * NOT committed, NOT stable, NOT a public interface. Agents reach source
--     text through `akr source get` or `knowledge.source_get`.
--
-- INVALIDATION
--   A full rebuild is triggered by a change in `meta.schema_version` or
--   `meta.parser_version`. Otherwise the sync is incremental: registered
--   documents are immutable and append-only, so a document whose content hash
--   is already present needs no work, a new hash is chunked and inserted, and
--   a catalog entry that has gone away takes its chunks with it.
--
-- CONVENTIONS
--   * `hash` columns hold the `sha256:`-prefixed form, matching the catalog.
--   * Byte offsets are into the exact registered bytes; lines are one-based.
--   * Rows are inserted in (document_id, ordinal) order so a dump is stable.

PRAGMA foreign_keys = ON;

CREATE TABLE meta (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
) WITHOUT ROWID;

-- Required keys:
--   schema_version   integer, bumped on any change to this file
--   parser_version   akr_core::source::chunk::PARSER_VERSION
--   corpus_hash      sha256 over sorted (id, content_hash) pairs plus the
--                    parser version; the generation this index is stamped with
--   tool_version     semver of the akr binary that wrote it
--   built_at         UTC timestamp, informational only, never an input

CREATE TABLE source_documents (
    id            TEXT PRIMARY KEY,
    title         TEXT NOT NULL,
    path          TEXT NOT NULL UNIQUE,
    content_hash  TEXT NOT NULL,
    origin        TEXT NOT NULL,
    media_type    TEXT NOT NULL,
    byte_len      INTEGER NOT NULL,
    added_at      TEXT NOT NULL,
    observed_at   TEXT,
    scope         TEXT,
    supersedes    TEXT,
    superseded    INTEGER NOT NULL DEFAULT 0,   -- some other entry supersedes it
    CHECK (origin IN ('external', 'internal-reference')),
    CHECK (superseded IN (0, 1))
) WITHOUT ROWID;

CREATE INDEX source_documents_live ON source_documents(superseded, id);

-- ---------------------------------------------------------------------------
-- source_chunks — one row per retrieval unit.
--
-- `chunk_id` is DERIVED (document content hash + parser version + byte range).
-- It is a cursor into this cache and never a citation: a record cites a
-- document and a byte range, so improving the scanner cannot break provenance.
-- ---------------------------------------------------------------------------

CREATE TABLE source_chunks (
    rowid           INTEGER PRIMARY KEY,
    chunk_id        TEXT NOT NULL UNIQUE,
    document_id     TEXT NOT NULL,
    parser_version  INTEGER NOT NULL,
    ordinal         INTEGER NOT NULL,
    heading_path    TEXT NOT NULL,          -- 'a > b > c', outermost first
    kind            TEXT NOT NULL,
    start_byte      INTEGER NOT NULL,
    end_byte        INTEGER NOT NULL,
    start_line      INTEGER NOT NULL,
    end_line        INTEGER NOT NULL,
    raw_text        TEXT NOT NULL,          -- the exact slice, for literal search
    search_text     TEXT NOT NULL,          -- soft wraps joined, markers dropped
    symbols         TEXT NOT NULL,          -- newline-separated identifier variants
    content_hash    TEXT NOT NULL,
    CHECK (kind IN ('prose', 'list', 'code', 'table', 'quote')),
    CHECK (end_byte >= start_byte),
    UNIQUE (document_id, parser_version, ordinal),
    FOREIGN KEY (document_id) REFERENCES source_documents(id) ON DELETE CASCADE
);

CREATE INDEX source_chunks_by_document ON source_chunks(document_id, ordinal);
CREATE INDEX source_chunks_by_range    ON source_chunks(document_id, start_byte, end_byte);

-- ---------------------------------------------------------------------------
-- source_chunks_fts — BM25 over heading, prose and symbols.
--
-- External-content table: the row text lives in `source_chunks` and is not
-- duplicated here. Columns are weighted at query time (heading and symbols
-- above prose), because a query naming a section or an identifier almost
-- always wants that section, and a query naming ordinary words does not.
-- ---------------------------------------------------------------------------

CREATE VIRTUAL TABLE source_chunks_fts USING fts5 (
    heading_path,
    search_text,
    symbols,
    content = 'source_chunks',
    content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2'
);
