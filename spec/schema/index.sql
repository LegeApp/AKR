-- spec/schema/index.sql — DDL for the AKR Stage E index cache.
--
-- WHAT THIS IS
--   The schema of `.akr/cache/index.sqlite`, the output of pipeline stage E
--   (docs/06-compiler-pipeline.md §7). It is a materialisation of the resolved
--   model, written by `akr build` and read by `akr get`, `akr search`,
--   `akr context`, `akr impact` and `akr review-queue`.
--
-- WHAT THIS IS NOT (D-019)
--   * NOT authoritative. The `.akr` source files are the only source of truth.
--     Every row here is derivable from them, and nothing here is derivable from
--     anything else.
--   * NOT committed. `.akr/cache/` is gitignored by `akr init`. Deleting this
--     file is always safe and costs one rebuild.
--   * NOT a public interface. Agents reach knowledge through the CLI or the MCP
--     surface and never open this database. That boundary is what keeps the
--     schema free to change; the moment something reads it directly it acquires
--     compatibility obligations the design has not promised.
--   * NOT stable. A `schema_version` bump means a FULL REBUILD: every table is
--     dropped and repopulated. There is no migration path and none is needed.
--
-- INVALIDATION
--   A full rebuild is triggered by a change in any of `meta.schema_version`,
--   `meta.source_graph_hash`, `meta.tool_version`, `meta.commit` or
--   `meta.today`, or by an absent or corrupt file. Routine invalidation is
--   silent; diagnostics in this stage (AKR-I001..AKR-I032) mean the cache could
--   not be built, never that it had to be.
--
-- VOCABULARY
--   Every kind, class, state, relation, outcome and enum literal constrained
--   below is copied from spec/tables/vocabulary.json and must agree with it
--   exactly. tools/check-design.py enforces the agreement.
--
-- CONVENTIONS
--   * Identity is (key, revision), never a rowid. Rowids never escape the cache.
--   * `commit` columns hold 40 lowercase hex digits WITHOUT the `git:` prefix.
--   * `hash` columns hold 64 lowercase hex digits WITHOUT the `sha256:` prefix.
--   * Booleans are INTEGER 0/1.
--   * Rows are inserted in canonical order (key, then revision) so that a dump
--     is stable across builds (docs/06-compiler-pipeline.md §11).

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

-- ---------------------------------------------------------------------------
-- meta — one row per key. Read before anything else; drives invalidation.
-- ---------------------------------------------------------------------------

CREATE TABLE meta (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
) WITHOUT ROWID;

-- Required keys, all written in one transaction at the end of stage E:
--   schema_version      integer, bumped on any change to this file
--   tool_version        semver of the akr binary that wrote the cache
--   grammar_version     from the `akr <version>` file headers
--   vocabulary_version  from spec/tables/vocabulary.json
--   source_graph_hash   sha256 over sorted (path, file-hash) pairs (D-014)
--   commit              the commit the build resolved against
--   today               the date used for review_after comparisons
--   built_at            UTC timestamp, informational only, never an input

-- ---------------------------------------------------------------------------
-- sources — one row per `.akr` file read by the build.
-- ---------------------------------------------------------------------------

CREATE TABLE sources (
    path        TEXT PRIMARY KEY,   -- repository-relative, LF-normalised
    file_hash   TEXT NOT NULL,      -- sha256 of the raw bytes
    byte_len    INTEGER NOT NULL,
    archived    INTEGER NOT NULL DEFAULT 0   -- lives under .akr/archive/ (D-018)
) WITHOUT ROWID;

-- ---------------------------------------------------------------------------
-- records — one row per logical key.
-- ---------------------------------------------------------------------------

CREATE TABLE records (
    key         TEXT PRIMARY KEY,
    namespace   TEXT NOT NULL,      -- first key segment; declared in project.akr
    kind        TEXT NOT NULL,
    class       TEXT NOT NULL,
    path        TEXT NOT NULL,      -- every revision of a key is in one file (V-003)
    head_rev    INTEGER,            -- NULL when no revision is live
    rev_count   INTEGER NOT NULL,
    CHECK (kind IN ('term', 'requirement', 'policy', 'constraint', 'decision',
                    'observation', 'evidence', 'assessment', 'papercut',
                    'milestone', 'work', 'track',
                    'question')),
    CHECK (class IN ('normative', 'empirical', 'planning', 'inquiry')),
    FOREIGN KEY (path) REFERENCES sources(path) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX records_by_kind      ON records(kind, key);
CREATE INDEX records_by_class     ON records(class, key);
CREATE INDEX records_by_namespace ON records(namespace, key);

-- ---------------------------------------------------------------------------
-- revisions — one row per (key, revision). The centre of the schema.
-- ---------------------------------------------------------------------------

CREATE TABLE revisions (
    key           TEXT NOT NULL,
    rev           INTEGER NOT NULL,
    kind          TEXT NOT NULL,
    class         TEXT NOT NULL,
    state         TEXT NOT NULL,
    live          INTEGER NOT NULL,   -- state is live for the class (D-004a)
    sealed        INTEGER NOT NULL,   -- state <> 'proposed' (D-015)
    title         TEXT NOT NULL,      -- required slot; every view heading (D-012)
    topic         TEXT,               -- normative only (D-004b)
    content_hash  TEXT NOT NULL,      -- sha256 over canonically formatted text
    -- kind-specific content slots, flattened; NULL where not applicable
    body          TEXT,               -- definition | statement | rule | decision |
                                      -- intent | question, the kind's required prose
    rationale     TEXT,
    context       TEXT,
    consequences  TEXT,
    resolution    TEXT,               -- question, required when state = 'resolved'
    measure       TEXT,               -- constraint
    cadence       TEXT,               -- track
    confidence    TEXT,               -- assessment
    result        TEXT,               -- evidence
    method        TEXT,               -- observation | evidence
    command       TEXT,               -- evidence
    artifact      TEXT,               -- evidence
    summary       TEXT,               -- evidence
    observed_at   TEXT,               -- observation | evidence (required)
    as_of         TEXT,               -- assessment
    review_after  TEXT,               -- observation
    target        TEXT,               -- milestone | work
    author        TEXT,
    created_at    TEXT,
    acknowledged  INTEGER NOT NULL DEFAULT 0,   -- D-023
    span_start    INTEGER NOT NULL,   -- byte offset of `record` in its file
    span_end      INTEGER NOT NULL,
    PRIMARY KEY (key, rev),
    CHECK (rev >= 1),
    CHECK (state IN ('proposed', 'active', 'rejected', 'superseded', 'withdrawn',
                     'verified', 'disproven',
                     'ready', 'blocked', 'completed', 'abandoned',
                     'open', 'deferred', 'resolved', 'closed-without-resolution')),
    CHECK (result IS NULL OR result IN ('pass', 'fail', 'inconclusive')),
    CHECK (method IS NULL OR method IN ('manual', 'command', 'observation',
                                        'instrumented')),
    CHECK (confidence IS NULL OR confidence IN ('low', 'medium', 'high')),
    CHECK (live IN (0, 1) AND sealed IN (0, 1) AND acknowledged IN (0, 1)),
    FOREIGN KEY (key) REFERENCES records(key) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX revisions_live      ON revisions(live, kind, key);
CREATE INDEX revisions_by_state  ON revisions(state, key, rev);
CREATE INDEX revisions_by_topic  ON revisions(topic, key) WHERE topic IS NOT NULL;
CREATE INDEX revisions_sealed    ON revisions(sealed, key, rev) WHERE sealed = 1;
CREATE INDEX revisions_observed  ON revisions(observed_at) WHERE observed_at IS NOT NULL;
CREATE INDEX revisions_review    ON revisions(review_after) WHERE review_after IS NOT NULL;

-- ---------------------------------------------------------------------------
-- claims — `claim <anchor> { ... }` blocks. Versioned with their record (D-011).
-- ---------------------------------------------------------------------------

CREATE TABLE claims (
    key       TEXT NOT NULL,
    rev       INTEGER NOT NULL,
    anchor    TEXT NOT NULL,        -- key-segment form
    text      TEXT NOT NULL,
    retired   INTEGER NOT NULL DEFAULT 0,  -- listed in this revision's retired_claims
    ord       INTEGER NOT NULL,     -- position in canonical (anchor-sorted) order
    PRIMARY KEY (key, rev, anchor),
    CHECK (retired IN (0, 1)),
    FOREIGN KEY (key, rev) REFERENCES revisions(key, rev) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX claims_by_anchor ON claims(anchor, key, rev);

-- ---------------------------------------------------------------------------
-- relations — one row per typed edge occurrence, exactly as authored.
-- ---------------------------------------------------------------------------

CREATE TABLE relations (
    from_key    TEXT NOT NULL,
    from_rev    INTEGER NOT NULL,
    relation    TEXT NOT NULL,
    to_key      TEXT NOT NULL,
    to_rev      INTEGER,            -- NULL for a current-head reference (D-009)
    to_anchor   TEXT,               -- NULL unless the reference carried one
    resolved_rev INTEGER NOT NULL,  -- what to_rev resolved to at build time
    ord         INTEGER NOT NULL,   -- position within the relation's array
    PRIMARY KEY (from_key, from_rev, relation, to_key, ord),
    CHECK (relation IN ('supported_by', 'depends_on', 'supersedes', 'contradicts',
                        'implements', 'resolves', 'derived_from', 'part_of',
                        'after', 'blocks', 'verified_by', 'plan_of_record')),
    FOREIGN KEY (from_key, from_rev) REFERENCES revisions(key, rev) ON DELETE CASCADE,
    FOREIGN KEY (to_key) REFERENCES records(key) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX relations_forward  ON relations(from_key, from_rev, relation);
CREATE INDEX relations_reverse  ON relations(to_key, resolved_rev, relation);
CREATE INDEX relations_by_name  ON relations(relation, from_key, to_key);

-- ---------------------------------------------------------------------------
-- scopes — one row per scope term (D-010). Overlap is computed, not stored.
-- ---------------------------------------------------------------------------

CREATE TABLE scopes (
    key        TEXT NOT NULL,
    rev        INTEGER NOT NULL,
    form       TEXT NOT NULL,       -- 'all' | 'ref' | 'path'
    ref_key    TEXT,                -- form = 'ref': a milestone, track or constraint
    glob       TEXT,                -- form = 'path'
    prefix     TEXT,                -- form = 'path': literal part before the first wildcard
    ord        INTEGER NOT NULL,
    PRIMARY KEY (key, rev, ord),
    CHECK (form IN ('all', 'ref', 'path')),
    CHECK ((form = 'all'  AND ref_key IS NULL AND glob IS NULL)
        OR (form = 'ref'  AND ref_key IS NOT NULL AND glob IS NULL)
        OR (form = 'path' AND ref_key IS NULL AND glob IS NOT NULL)),
    FOREIGN KEY (key, rev) REFERENCES revisions(key, rev) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX scopes_by_prefix ON scopes(prefix) WHERE form = 'path';
CREATE INDEX scopes_by_ref    ON scopes(ref_key) WHERE form = 'ref';

-- ---------------------------------------------------------------------------
-- watches — one row per `watches` glob, plus the derived freshness verdict.
-- The verdict is a BUILD FACT, not a diagnostic (D-024).
-- ---------------------------------------------------------------------------

CREATE TABLE watches (
    key           TEXT NOT NULL,
    rev           INTEGER NOT NULL,
    glob          TEXT NOT NULL,
    prefix        TEXT NOT NULL,    -- literal part before the first wildcard
    matched_by    TEXT,             -- commit that touched a matching path, or NULL
    matched_path  TEXT,             -- the path that matched, for the explanation
    ord           INTEGER NOT NULL,
    PRIMARY KEY (key, rev, ord),
    FOREIGN KEY (key, rev) REFERENCES revisions(key, rev) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX watches_matched ON watches(matched_by) WHERE matched_by IS NOT NULL;

-- ---------------------------------------------------------------------------
-- checks — `check <id>` blocks inside an `acceptance` block (D-016).
-- ---------------------------------------------------------------------------

CREATE TABLE checks (
    key          TEXT NOT NULL,     -- the milestone or work record
    rev          INTEGER NOT NULL,
    check_id     TEXT NOT NULL,
    statement    TEXT NOT NULL,
    method       TEXT NOT NULL,
    command      TEXT,
    satisfied    INTEGER NOT NULL,  -- derived; see evidence_links
    ord          INTEGER NOT NULL,  -- position in canonical (check-id-sorted) order
    PRIMARY KEY (key, rev, check_id),
    CHECK (method IN ('manual', 'command', 'observation')),
    CHECK (satisfied IN (0, 1)),
    FOREIGN KEY (key, rev) REFERENCES revisions(key, rev) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX checks_unsatisfied ON checks(satisfied, key, rev) WHERE satisfied = 0;

-- ---------------------------------------------------------------------------
-- evidence_links — the one-directional `verified_by` edge from a check to an
-- evidence record, with the descendant-commit verdict that decides satisfaction.
-- Evidence never declares what it verifies (D-016).
-- ---------------------------------------------------------------------------

CREATE TABLE evidence_links (
    key            TEXT NOT NULL,   -- the verified record
    rev            INTEGER NOT NULL,
    check_id       TEXT NOT NULL,
    evidence_key   TEXT NOT NULL,
    evidence_rev   INTEGER NOT NULL,
    result         TEXT NOT NULL,   -- copied from the evidence revision
    observed_at    TEXT NOT NULL,
    last_change    TEXT NOT NULL,   -- last commit that changed the verified content
    descends       INTEGER NOT NULL,-- observed_at descends from last_change
    satisfies      INTEGER NOT NULL,-- result = 'pass' AND descends = 1
    PRIMARY KEY (key, rev, check_id, evidence_key, evidence_rev),
    CHECK (result IN ('pass', 'fail', 'inconclusive')),
    CHECK (descends IN (0, 1) AND satisfies IN (0, 1)),
    FOREIGN KEY (key, rev, check_id) REFERENCES checks(key, rev, check_id)
        ON DELETE CASCADE,
    FOREIGN KEY (evidence_key, evidence_rev) REFERENCES revisions(key, rev)
        ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX evidence_links_by_evidence ON evidence_links(evidence_key, evidence_rev);

-- ---------------------------------------------------------------------------
-- dispositions — what happened to each unfinished child at supersession (D-017).
-- ---------------------------------------------------------------------------

CREATE TABLE dispositions (
    key         TEXT NOT NULL,      -- the SUPERSEDING record carrying the block
    rev         INTEGER NOT NULL,
    child_key   TEXT NOT NULL,
    child_rev   INTEGER,            -- NULL for a current-head reference
    outcome     TEXT NOT NULL,
    into_key    TEXT,               -- required for carried_forward,
                                    -- completed_elsewhere; optional for
                                    -- still_required_separately; forbidden for
                                    -- intentionally_dropped
    note        TEXT,
    ord         INTEGER NOT NULL,
    PRIMARY KEY (key, rev, child_key),
    CHECK (outcome IN ('carried_forward', 'completed_elsewhere',
                       'intentionally_dropped', 'still_required_separately')),
    CHECK ((outcome IN ('carried_forward', 'completed_elsewhere') AND into_key IS NOT NULL)
        OR (outcome = 'intentionally_dropped' AND into_key IS NULL)
        OR (outcome = 'still_required_separately')),
    FOREIGN KEY (key, rev) REFERENCES revisions(key, rev) ON DELETE CASCADE,
    FOREIGN KEY (child_key) REFERENCES records(key) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX dispositions_by_child ON dispositions(child_key, key, rev);

-- ---------------------------------------------------------------------------
-- resolutions — stage D's verdict per key, plus the derived freshness flags.
-- One row per (key, rev) that the build had an opinion about.
-- ---------------------------------------------------------------------------

CREATE TABLE resolutions (
    key            TEXT NOT NULL,
    rev            INTEGER NOT NULL,
    is_head        INTEGER NOT NULL,
    superseded_by  TEXT,             -- key of the superseding record, if any
    stale          INTEGER NOT NULL DEFAULT 0,
    stale_cause    TEXT,             -- 'watch' | 'review_after'
    stale_detail   TEXT,             -- the glob and commit, or the passed date
    at_risk        INTEGER NOT NULL DEFAULT 0,
    at_risk_depth  INTEGER,          -- propagation distance from the stale source
    at_risk_path   TEXT,             -- '@a -> @b -> @c', so a reader sees why
    PRIMARY KEY (key, rev),
    CHECK (is_head IN (0, 1) AND stale IN (0, 1) AND at_risk IN (0, 1)),
    CHECK (stale_cause IS NULL OR stale_cause IN ('watch', 'review_after')),
    CHECK (stale = 0 OR stale_cause IS NOT NULL),
    FOREIGN KEY (key, rev) REFERENCES revisions(key, rev) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX resolutions_heads   ON resolutions(is_head, key) WHERE is_head = 1;
CREATE INDEX resolutions_stale   ON resolutions(stale, key) WHERE stale = 1;
CREATE INDEX resolutions_at_risk ON resolutions(at_risk, at_risk_depth, key)
    WHERE at_risk = 1;

-- ---------------------------------------------------------------------------
-- diagnostics — everything collected in stages A-F for this build.
-- Present so that `akr check --format json` and the MCP `validate` tool read
-- from one place. Empty after a clean build.
-- ---------------------------------------------------------------------------

CREATE TABLE diagnostics (
    seq         INTEGER PRIMARY KEY,   -- emission order: path, span, code
    code        TEXT NOT NULL,         -- AKR-<letter><nnn>
    severity    TEXT NOT NULL,
    stage       TEXT NOT NULL,
    rule        TEXT,                  -- V-nnn, or NULL
    message     TEXT NOT NULL,
    path        TEXT NOT NULL,
    span_start  INTEGER NOT NULL,
    span_end    INTEGER NOT NULL,
    key         TEXT,
    rev         INTEGER,
    CHECK (severity IN ('error', 'warning')),
    CHECK (stage IN ('parse', 'format', 'type', 'link', 'resolve',
                     'index', 'emit', 'context', 'git', 'cli', 'migration')),
    CHECK (code GLOB 'AKR-[A-Z][0-9][0-9][0-9]')
);

CREATE INDEX diagnostics_by_code ON diagnostics(code, seq);
CREATE INDEX diagnostics_by_path ON diagnostics(path, span_start);

-- ---------------------------------------------------------------------------
-- records_fts — full-text search over live revisions only.
-- Search RANKS; it never authorises (docs/09-context-assembly.md §1). Nothing
-- enters a context bundle because it matched a query.
-- ---------------------------------------------------------------------------

CREATE VIRTUAL TABLE records_fts USING fts5 (
    key       UNINDEXED,
    rev       UNINDEXED,
    kind      UNINDEXED,
    title,
    body,
    claims,               -- concatenated claim text
    aliases,              -- term aliases, so a synonym finds the term
    tokenize = 'unicode61 remove_diacritics 2'
);

-- Populated from live revisions in (key, rev) order. Not a content-linked table:
-- the cache is rebuilt wholesale, so incremental FTS maintenance would buy
-- nothing and cost a trigger set that has to stay in step with `revisions`.

-- ---------------------------------------------------------------------------
-- Convenience views. Read-only; the CLI uses them, nothing outside the tool
-- does.
-- ---------------------------------------------------------------------------

CREATE VIEW heads AS
    SELECT r.key, r.rev, r.kind, r.class, r.state, r.title, r.topic,
           res.stale, res.stale_cause, res.at_risk, res.at_risk_depth
      FROM revisions r
      JOIN resolutions res ON res.key = r.key AND res.rev = r.rev
     WHERE res.is_head = 1;

CREATE VIEW review_queue AS
    SELECT key, rev, 'stale' AS reason, stale_cause AS cause, 0 AS depth,
           NULL AS via
      FROM resolutions WHERE stale = 1
    UNION ALL
    SELECT key, rev, 'at_risk', NULL, at_risk_depth, at_risk_path
      FROM resolutions WHERE at_risk = 1 AND stale = 0;
-- Ordering is applied by the caller: stale before at_risk, then depth, then key
-- (docs/10-freshness-and-git.md §7).
