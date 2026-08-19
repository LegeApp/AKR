# Plan: Immutable Source Library (`/sources`) + MCP Correctness Fixes — Post-Advice Review

## Goal
Add a minimal, append-only **`/sources` source library** per AKR workspace (register, verify, retrieve external Markdown) and complete the **remaining MCP correctness / latency fixes** from `akr-ingest-and-mcp-fix-advice.md`. Do **not** expand the candidate-oriented ingest system (`akr ingest` / `crates/akr-core/src/ingest/**`) — per current direction it stays as-is (existing code left untouched, treated as deprecated/experimental) and is not required for the `/sources` workflow.

## Success Criteria
- `sources/` files are immutable, content-hashed, and retrievable byte-for-byte after registration.
- `akr source add <path> --id <id>` copies external Markdown to `sources/external/<id>--<short-hash>.md`, adds a catalog entry, and creates no AKR records.
- `akr source verify` / `akr check` reports `AKR-S021`-style error when a registered file is edited, and `akr source supersede` is the only supported mutation (old version preserved).
- `akr source get` and `akr source list` retrieve exact bytes/locators without needing ledger search.
- MCP `knowledge.context` returns **both** human-readable text bundle and structured metadata (no duplicated pretty-printed JSON), and `--budget` genuinely truncates rendered prose.
- `knowledge.start` (task → planning candidates) and `knowledge.explain` remain/land correctly and `GoalUnresolved` (AKR-X001) returns actionable candidates.
- No new record kinds, no per-line/per-paragraph record creation, no FTS/SinoRAG indexing for sources in this phase.

## Context and Current Facts
- **Advice file** (`akr-ingest-and-mcp-fix-advice.md`, 2842 lines) proposes three layers (source library → derived index → AKR ledger) and a full candidate-oriented ingest with manifests under `.akr/reviews/`. The user's current direction supersedes that: "we do not intend to ingest markdowns, only save them in /sources in a given project." Phase 3 advice (exhaustive review manifests) is explicitly deprioritized.
- **Existing ingest code is already present but uncommitted** (git status `??`): `crates/akr-core/src/ingest/{markdown,manifest,review,apply}.rs` (438-candidate extractor, tables rows/support, fingerprinting, dispositions `? + x = - ~ !`), `crates/akr-cli/src/ingest.rs` (`preview/start/show/mark`/`apply`/`close`), and `Command::Ingest*` in [args.rs:190-257](/mnt/Samsung980_1TB/Rust-projects/AKR/crates/akr-cli/src/args.rs:190). This matches Phase 1–2 of the old advice; it is **done but not integrated** — no `.akr/reviews/` exists on disk, no tests wire it into CI.
- **MCP correctness fixes are partially done** (uncommitted diff ~1572 insertions):
  - [context/mod.rs:781-870](/mnt/Samsung980_1TB/Rust-projects/AKR/crates/akr-core/src/context/mod.rs:781) now computes `truncated` as a `BTreeSet` and `record_tokens(..., include_prose)` so budget truly omits prose (old bug: `truncated` subtracted 20 tokens but still rendered body in [render.rs:186-191](/mnt/Samsung980_1TB/Rust-projects/AKR/crates/akr-core/src/context/render.rs:186)).
  - [tools.rs:30-66](/mnt/Samsung980_1TB/Rust-projects/AKR/crates/akr-mcp/src/tools.rs:30) now has `ToolResult::Read { text, structured }` and `context()` returns both, fixing the earlier `run_read` discarding `Output.text` (advice § 7).
  - `knowledge.start` and `knowledge.explain` added in [schema.rs:31-36](/mnt/Samsung980_1TB/Rust-projects/AKR/crates/akr-mcp/src/schema.rs:31) and [tools.rs:74-75](/mnt/Samsung980_1TB/Rust-projects/AKR/crates/akr-mcp/src/tools.rs:74), with `GoalUnresolved` candidate suggestions and `recommended_context`.
  - Remaining diff still needs verification against real MCP transport (protocol version, `structuredContent` shape, tests in `crates/akr-mcp/tests/differential.rs`).
- **No `sources/` exists yet** (`ls sources` → ENOENT). `docs/generated/` is empty of source material; `spec/schema/index.sql` is ledger-only FTS (no `source_documents`/`source_chunks`). No source catalog, no byte-range citations.
- **Workspace model**: single project `akr` at repo root (`.akr/project.akr`). Multi-project workspaces (BPG, Lege) will need per-namespace or per-workspace `sources/`— simplest is repo-root `sources/` with `sources/external/` and `sources/catalog.akr` (or `.json`), per advice §2. `AKR-S021` diagnostic for hash mismatch already described but not implemented.
- **Constraints from AGENTS.md**: durable knowledge in `.akr`, `docs/generated` is build output, no deletion of records, use `knowledge.*` tools.

## Constraints and Non-goals
- **Non-goals (explicitly deferred):**
  - Full ingest review lifecycle (pending/promote/verified/declined, `manifest.json`, `.akr/reviews/`, one-char CLI marks, `akr ingest apply/close`). Leave existing ingest code unexported/deprecated; do not remove it, do not extend it.
  - Source chunking, `source_chunks`/`source_chunks_fts` FTS5 tables, token-packing, symbol normalization, BM25 weights, `SourceSearchBackend`, SinoRAG TF-IDF/PhraseIndex, embeddings (advice §§ 4–6).
  - Persistent `WorkspaceRuntime` snapshot / lazy Git `ReadNeeds` ladder (advice §6) and `source_corpus_hash` incremental indexing — keep as separate performance follow-up; current fix is limited to budget/text correctness already in diff.
  - MCP protocol dual-version (`2026-07-28` vs `2024-11-05`), `server/discover`, `outputSchema`/`resultType` hardening — note as follow-up, not required for source library.
- **Constraints:**
  - No new Rust dependencies; hand-rolled markdown scanning is acceptable for future `source search` but not needed now (advice permits deterministic subset parser).
  - `sources/` immutability is file-system + hash-checked, not git-immutable; git `show <commit>:<path>` retrieval is optional — primary guarantee is `content_hash` check.
  - Record `Source { kind, path/url, excerpt }` extended later with `document` + `range` (advice § 4) is deferred; this phase only ensures source files are stable citables (hash + path) without changing record schema.

## Key Decisions
| Decision | Choice | Why | Rejected alternative |
|---|---|---|---|
| `sources` location | Repo-root `sources/` with `sources/external/` + `sources/catalog.akr` (or `catalog.json`) | Matches advice §2 `sources/external/*.md` and user's "/sources in a given project" (project root = workspace root); visible, not hidden in `.akr`, not in `docs/` which implies maintained docs | `.akr/sources/` (hides from humans, violates append-only visibility) or per-namespace subfolders (premature) |
| Catalog format | `sources/catalog.akr` with `document` blocks as in advice example, plus deterministic JSON export for tooling; `id` is `SourceId` string, `content_hash = sha256:…`, `byte_len`, `added_at`, `observed_at` optional | Reuses existing AKR syntax infra; `akr check` can parse it; easy to extend with `supersedes` | Standalone `catalog.json` only (loses AKR validation) or SQLite table (overkill without index) |
| Hashing | `Sha256` from [crates/akr-core/src/hash/mod.rs](/mnt/Samsung980_1TB/Rust-projects/AKR/crates/akr-core/src/hash/mod.rs) already used for `source_graph`; short hash `7a2d3c1e` in filename, full `sha256:` in catalog | Single hash impl, matches ledger conventions; advice requires stable, not `DefaultHasher` | Blake3/new dep |
| Content address | `sources/external/<date>-<slug>--<short-hash>.md` copy + original path kept as provenance `path` field; `source get` reads from copy | File is immutable snapshot even if original deleted/moved; filename encodes hash for human dedupe | Symlink or original path only (breaks after deletion) |
| Ingest disposition | Keep `crates/akr-core/src/ingest` and `akr ingest` as deprecated/experimental, hidden from docs/help by default, no removal | Avoid churn, preserve work, satisfy "some parts already done" note; user says not important | Delete ingest code (loses audit work, causes diff noise) |
| MCP scope | Finish only: budget fix, text+structured return, `knowledge.start`/`explain`, `GoalUnresolved` candidates (already in diff) — verify via existing differential tests; no new protocol upgrade | Small delta, high value per advice Priority order 1–3; diff already implements it | Full Phase 1 performance runtime + protocol `2026-07-28` now (too large, blocks sources) |
| Provenance | `akr source add` records `path` + optional `observed_at` (git commit or URL), no auto record creation | Advice: "Register immutable source material, but do not translate it into AKR records by default." | Auto-create tracking `work` per import (recreates old flooding problem) |

## Recommended Approach
1. **Source library (CLI + core):** Add `crates/akr-core/src/source.rs` (or `crates/akr-core/src/sources/mod.rs`) with `SourceDocument`, `SourceId`, `SourceCatalog` (parse/write/verify), `SourceRange` helper (start/end byte/line for future citations, not used now). Add `crates/akr-cli/src/source.rs` with commands `akr source add/list/get/verify/supersede`. `source add` validates workspace-relative path vs absolute/outside (reuses `resolve_workspace_file` pattern from advice § MCP #File-access), computes `Sha256`, copies to `sources/external/`, appends to catalog, prints `sha256:` and path. `source verify` and `akr check` recompute hashes; `source supersede <old-id> <new-file>` adds new document with `supersedes <old-id>` and preserves old file.
2. **MCP parity (minimal):** Ensure `knowledge.start`/`explain` and `context` text+structured are wired through [protocol.rs](/mnt/Samsung980_1TB/Rust-projects/AKR/crates/akr-mcp/src/protocol.rs) and `TOOLS` count is derived, not hard-coded (fixes advice "nine tools" drift). Add `knowledge.source_get`/`source_search` only as stubs returning "use `akr source get` / file read" or defer entirely — per user, no source indexing now, so MCP search over sources is out of scope.
3. **Docs & AGENTS:** Update `AGENTS.md` line "Durable project knowledge lives in `.akr`, not in Markdown." → "Durable conclusions live in `.akr`; files under `sources/` are immutable source material … Never edit them." Add `docs/15-external-sources.md` (or `docs/XX-sources.md`) describing `sources/` layout, `akr source add/verify/supersede/get` flows, and that `sources/` is append-only.
4. **Deprecate ingest:** Mark `akr import` as legacy alias and `akr ingest` help as "(experimental — for exhaustive review only; prefer `akr source add`)" without removing code; add warning when `akr import` used under `sources/external/`.

## Work Plan
### Phase A — Measure & stage (no ledger change)
- A0: Record decision `akr source library uses /sources` (or revise existing D-0xx) distinguishing legacy migration vs external intake; mark old line-by-line ingest advice superseded per `akr-ingest-and-mcp-fix-advice.md` Revised decision table.
- A1: Add instrumentation placeholder for future latency work (defer runtime snapshot) — only ensure existing `context` budget fix has test `context_budget_removes_prose_from_rendered_text` (already in diff intent).

### Phase B — Immutable source catalog (core + CLI, no index)
- B1: Core types: `SourceDocument`, `SourceId`, `ContentHash`, `SourceOrigin (external/internal-reference)`, `SourceCatalog` (load/save from `sources/catalog.akr`), `hash_file`, `verify_file`, `catalog_path()`. Reuse `akr_core::hash::Sha256`. Tests: round-trip catalog, hash mismatch error `AKR-S021`, supersede preserves old.
- B2: CLI `akr source add <path> [--id <id>] [--title <t>] [--origin external] [--observed-at git:<sha>]` — canonicalizes path, checks not absolute, not outside workspace (`canonical_root` check from advice), rejects if under `sources/` already, copies to `sources/external/`, updates catalog. Returns json with `id`, `content_hash`, `path`. Dry-run via existing `EnvError` pattern.
- B3: CLI `akr source list [--all-versions]`, `akr source get <id> [--whole|--lines a:b|--section "…"]` (section via simple heading scan, not full chunker), `akr source verify`, `akr source supersede <old-id> <new-path>`. `akr check` calls `source::verify_catalog` and surfaces `AKR-S021`.
- B4: Render support: `akr get` shows `sources: kind external path … content_hash …` when record cites a source (future; stub now shows catalog entry for `source_get`).

### Phase C — MCP / context hardening (verify & ship diff)
- C1: Verify `crates/akr-mcp/src/tools.rs::context` text+structured path (diff lines 242-273) matches `commands::run` envelope and `protocol.rs` wraps `ToolResult::Read` as `{content:[{type:"text",text}], structuredContent}`. Fix `TOOLS` count derived from `TOOLS.len()` in help text.
- C2: Validate `knowledge.start` planning-candidate logic (search limited to `milestone|work|track`, `path_overlap` via `scopes_overlap`) and `GoalUnresolved` → `candidates` + `recommended_context` payload (diff lines 124-224). Ensure `knowledge.explain` proxies `Command::Explain`.
- C3: Budget correctness: confirm `apply_budget` in [context/mod.rs:781](/mnt/Samsung980_1TB/Rust-projects/AKR/crates/akr-core/src/context/mod.rs:781) with `truncated: BTreeSet` and `total_tokens(..., truncated)` and `render_records` omits prose for truncated IDs. Add regression test from advice (sentinel prose assertion).

### Phase D — Docs, validation, rollout
- D1: Update `AGENTS.md`, add `docs/15-external-sources.md`, update `docs/07-cli.md` with `akr source *` subcommands. Ensure `akr source verify --help` and `akr build --check`-style `akr check` path documented.
- D2: `tools/check-design.py --strict` passes; add `scripts/verify-distribution.sh` step 6 check ("source verify" after modification). Add `cargo test` golden for `sources/` round-trip.
- D3: Manual: `cargo run --bin akr -- source add fixtures/sample.md --id sample-2026-08-06`, edit file, `cargo run --bin akr -- source verify` → `AKR-S021`, `akr source supersede`.

## Validation Plan
- **Unit:** `cargo test -p akr-core -- source` — catalog parse, hash verify, supersede, `verify_detects_edited_source`. `cargo test -p akr-core -- context::tests::context_budget_removes_prose_from_rendered_text` — render with budget 900 excludes sentinel.
- **Integration:** `cargo test -p akr-cli -- source` — `source add` creates file + catalog, `source verify` passes, edit → fails with `S021`, `check` surfaces it. `cargo test -p akr-mcp -- differential` — updated `differential.rs` expects `text` + `structuredContent` for `knowledge.context`/`search`/`start`/`explain`.
- **Manual CLI:** `akr source add README.md --id test-1 --title "Test" && akr source get test-1 --whole | diff - sources/external/test-1--*.md` → no diff. `echo x >> sources/external/test-1--*.md && akr source verify` → error `AKR-S021`. `akr ingest` still present but help marks experimental.
- **E2E:** `akr build --check` (or `cargo run --bin akr -- build --check`) passes; `python tools/check-design.py --strict` passes; `scripts/verify-distribution.sh` (extract to tmp, `cargo test`, `python tools/check-design.py --strict`) passes.

## Risks / Rollback
- **Scope creep (ingest):** Advice proposes 400+ candidate manifests; deferring prevents ledger flood but existing `ingest` code may be discovered by agents. Mitigation: hide/demote in help, add deprecation warning, do not delete — rollback is trivial (re-expose).
- **Hash collision / filename length:** Short hash (7–8 hex) is display only; catalog holds full hash — collision handled by full-hash equality. Keep filename under 255 chars (truncate slug).
- **Workspace detection:** `sources/` at repo root assumes single workspace; multi-ledger repos (BPG) need `sources/` per ledger root. Mitigation: `sources/catalog.akr` path resolved via `Session::locate` akr_dir sibling, not hard-coded `PathBuf::from("sources")`; search upward for `sources/` or use `<akr_dir>/../sources`.
- **Catalog edit races:** `akr source add` should hold same `.akr` lock/file lock as ledger writes; reuse `akr_core::lock` or atomic rename. Rollback: if catalog corrupts, `akr source verify` rebuilds from file hashes? No—catalog is source of truth; keep backup `catalog.akr.bak`.
- **MCP differential breakage:** Changing `ToolResult` enum changes wire format. Rollback: keep `call()` returning `Result<Value,ToolError>` with backward-compat shim if clients expect old shape (tests in `crates/akr-mcp/tests/writes.rs` will catch).

## Open Questions
- **Per-workspace vs per-project `sources/`:** Should `sources/` live at git root, at each `.akr` parent, or at `<namespace>/sources/`? Repo currently has one project `akr`; Lege ecosystem may need per-ledger `sources/` (e.g., `lege-codecs/jp2lam/sources/`). Decision needed before B1 path canonicalization.
- **Catalog syntax choice:** `catalog.akr` (AKR syntax) vs `catalog.json` — `catalog.akr` aligns with existing parser but needs schema; `catalog.json` is simpler for tooling. Confirm with `docs/02-data-model.md`.
- **MCP `knowledge.source_*` tools:** User says only save in `/sources` — confirm we should **not** add `knowledge.source_search/source_get` MCP tools in this phase (file read + `akr source get` suffices).
- **Superseded visibility:** Should `akr source list` hide superseded by default and `knowledge.start` exclude them? Advice says `--all-versions` opt-in — confirm.

---
*No file was overwritten; this plan was saved to `.agents/plans/2026-08-06-sources-and-mcp-fix.md`. Existing `crates/akr-core/src/ingest/*` remains as experimental reference implementation.*
