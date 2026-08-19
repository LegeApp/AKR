# Handoff — external sources (P10) and the AKR ↔ git bridge (P11)

**Date:** 2026-08-07
**Working from:** `sources/akr-ingest-and-mcp-fix-advice.md` (second half) and
`sources/context-reduction.md`.

## What was already in the tree before this pass

Worth stating, because roughly the first half of the advice document had been implemented
already and re-doing it would have been waste:

- The immutable source library skeleton: `sources/catalog.json`, `akr source
  add|list|get|verify|supersede`, `AKR-S021`.
- `akr build --check` and generated-view currency checking.
- MCP returning readable text *and* `structuredContent` (`ToolResult::Read`).
- Context budgets that genuinely omit prose rather than annotating it falsely.
- `knowledge.start`, `knowledge.explain`, and `GoalUnresolved` returning planning
  candidates plus a ready-made next call.
- `akr papercut collate` (D-030).
- `akr ingest` deprecated in favour of `akr source add` — the correction the advice
  document's own "revised decision" section asks for.

## What this pass added

### P10 — source retrieval and citations (D-031, `docs/15-external-sources.md`)

- `crates/akr-core/src/source/chunk.rs` — a deterministic, dependency-free Markdown
  chunker. Fences are recognised before headings, code and tables are never split,
  packing targets 450–700 estimated tokens, and prose is normalised so re-wrapping a
  paragraph cannot change how it ranks. Technical identifiers are expanded into their
  searchable variants.
- `spec/schema/sources.sql` + `crates/akr-core/src/store/sources.rs` —
  `.akr/cache/sources.sqlite`, on its own generation (`corpus_hash`, `parser_version`).
  A record write does not rechunk; a registration does not re-resolve. Sync is
  incremental because registered documents are immutable.
- `akr source search` (escaped by default, `--literal`, `--fts`, `--document`,
  `--all-versions`) and `akr source get --chunk --neighbors`.
- `knowledge.source_search` and `knowledge.source_get` over the same functions.
- The `source` block gained `document`, `start_byte`, `end_byte`, `start_line`,
  `end_line`, `excerpt_hash`. `akr check` resolves citations against the registered bytes
  and reports the new `AKR-S022`. `akr get` prints source locators.
- The MCP tool count is now derived from the registry rather than written in prose —
  the "nine tools / eleven tools" drift the advice document flagged.

### P11 — the change protocol (D-032, `docs/16-change-protocol.md`)

- `crates/akr-core/src/change/` — `ChangeIntent`, `SemanticDelta`, the implementation
  digest, and deterministic commit-message generation with `AKR-*` trailers.
- Git primitives: `staged_entries`, `blob`, `write_tree`, `git_path`,
  `has_staged_changes`, `commit`, `log_grep`.
- `akr diff --staged`, `akr change begin|show|prepare|verify|abort`,
  `akr git message|commit|log|install-hooks`, `akr git-hook`.

### Tooling

- `scripts/fetch-rust-sandbox.sh` — fetch a toolchain from the npm mirror of the official
  component tarballs, for sandboxes where `static.rust-lang.org` is blocked.
- `scripts/run-tests-sliced.sh` — run the test binaries a few at a time, for environments
  that cap a command's wall clock.
- `tools/check-design.py` now skips `sources/` and `.agents/`. A registered outside report
  citing a diagnostic code AKR never had is reporting on somebody else's tree; flagging it
  would make the checker fail for saying something true.

## Verification status — read this before trusting the tree

Verified, on a 1.92-nightly toolchain with `--ignore-rust-version`:

- `cargo check --workspace --all-targets` clean.
- All **44** test binaries pass, including the two new suites
  (`source_library.rs`, 9 tests; `change_protocol.rs`, 13 tests).
- `python tools/check-design.py` — all eight checks pass.

**Not verified.** The sandbox ran out of disk immediately after the last green run, and
these edits landed after it:

1. `crates/akr-cli/src/args.rs` — the `source`/`diff`/`change`/`git` help topics and the
   top-level command list. String literals only; reviewed by eye.
2. `crates/akr-cli/src/change.rs` — `graph` in `message()`/`commit()` now comes from
   `session.resolve().source_graph` instead of a placeholder hash.
3. `AGENTS.md`, `CLAUDE.md`, `docs/13-implementation-roadmap.md` — prose.

**First thing to do:** `cargo check --workspace --all-targets && cargo test --workspace`.
Item 2 is the only one that could plausibly fail to compile.

## Follow-up — 2026-08-08

`cargo check --workspace --all-targets` and `cargo test --workspace` both came back clean
(44 binaries, all passing), so the two items flagged above as not-verified are fine —
`akr-cli/src/change.rs`'s `source_graph` wiring compiles and is covered by
`change_protocol.rs`. `python tools/check-design.py` also passes all eight checks.

The ledger records this pass should have written now exist: `akr.work.p10-external-sources`
and `akr.work.p11-change-protocol`, both `completed` with acceptance checks mirroring the
roadmap's exit criteria and `verified_by` evidence for each; five `evidence` records citing
the suites above; and `akr.decision.external-sources-immutable-library` /
`akr.decision.change-transaction-not-commit-kind`, mirroring D-031/D-032 and cited by the
matching work record's `implements` edge. `akr build` regenerated the lock and views; `akr
check --review-clean --views-current` reports no diagnostics.

One environment note for whoever picks this up next: `akr propose` / `akr evidence add` went
through `./target/debug/akr` directly rather than the `knowledge.*` MCP tools — the running
`akr-mcp` process (`~/.local/bin/akr-mcp`) was a stale binary from before this pass's model
changes (missing the papercut `collated` slot), so it refused every write with a spurious
`AKR-T002`. `~/.local/bin/{akr,akr-mcp}` have now been reinstalled from this build, but the
already-running MCP server process still has the old binary loaded in memory — an MCP
reconnect or session restart is needed before `knowledge.*` tools reflect it.

## Not done

- **Registering `sources/*.md` in the catalog.** The two advice documents are still loose
  files rather than registered sources. `akr source add sources/akr-ingest-and-mcp-fix-advice.md`
  would copy them under `sources/external/` with a hash suffix; that reorganises the
  user's folder, so it was left as an explicit decision rather than done quietly.
- From `context-reduction.md`, deliberately out of scope this pass (the other two clusters
  the user chose not to take): `knowledge.get` detail levels, hard per-tool output budgets
  with cursors, the read-only MCP surface, MCP JSONL telemetry and `akr mcp stats`, batched
  `knowledge.update`, `ReadNeeds` load levels, and the persistent MCP workspace snapshot.
- `akr git verify-range`, `akr git prepare-squash`, and the derived `git-links.sqlite`
  index. `akr git log` covers the common query by grepping trailers, which is what the
  index would have cached.

## Follow-up — 2026-08-08, second pass: the collated papercuts

### What the collated record actually contained

`akr.papercut.collated-18-papercuts-from-kitchen-concept-lege/1` stored the eighteen source
**keys** and a **truncated title** each, ending "see the owning project's ledger for the
full statement". The sister checkouts are not reachable from this session, so fifteen of the
eighteen could not be read here at all — which is the same reason nobody had acted on them.

**Fixed at the root:** `akr papercut collate` now carries each source papercut's full
statement into the master record (D-033). **Re-run it** — `akr papercut collate --projects
/mnt/Samsung980_1TB/Rust-projects` — and the eighteen become readable and workable in this
repository. The existing master record already absorbed those keys, so a plain re-run is a
no-op; either revise it, or withdraw it first so the keys are collated again.

### AKR's own seven papercuts — all resolved and retired

Six were already fixed and still rendering as live, which is why `PAPERCUTS.md` read as a
backlog when it was mostly history:

| Papercut | Resolution |
| --- | --- |
| D-028 incomplete: completing a committed non-legacy milestone always fails | D-029, the definitional-change gate |
| Historical-port design gap (V-020 unsatisfiable for a port) | D-028 |
| `cargo fmt --check` red on the committed tree | commit `7cf53ce`, which landed *after* the report |
| MCP dropped the `help:` line from `AKR-C011` | commit `8866c42` |
| `knowledge.*` failed at the repo root: no ledger of AKR's own | commit `047a684` |
| `knowledge.papercut` "unsupported call": stale installed binary | **this pass** — see below |

Each is now `superseded` with a `RESOLVED:` line naming the fix, so `PAPERCUTS.md` shows
what still hurts rather than what used to.

### Frictions fixed in code this pass

- **The stale MCP binary, which recurred twice and was twice diagnosed as a ledger bug.**
  `setup-akr-mcp.sh` installs by rename instead of `cp` (no `ETXTBSY` over a running
  server); the server publishes its vocabulary version in `serverInfo`; and
  `crates/akr-mcp/src/skew.rs` attaches `AKR-X042` to any failing call when the server and
  the workspace lock disagree — naming the reinstall *and* the reconnect, which is the half
  that gets forgotten.
- **`akr propose --from` rejecting slot-only bodies** (jpegXL-rs). A `--from` file may now
  be a bare slot list; the header and record line come from the key and kind already given
  on the command line. A file with its own header is still taken verbatim.
- **`cargo fmt --check` has no gate.** `scripts/verify-distribution.sh` now runs it, and
  `akr source verify`, so neither can rot unnoticed again.
- **A collation rendered as an unreadable wall** in `PAPERCUTS.md`. It now renders as one
  summary line; the statements live in the record.

### Token reduction (D-034)

`knowledge.get` gained `detail: summary|body|canonical` defaulting to `body`, so canonical
AKR source text is an explicit request. Per-tool output budgets withhold an oversized result
rather than shortening it, returning its counts and a ready-made narrower call; both halves
of the payload count towards the limit, because a client that renders `content` and one that
parses `structuredContent` each pay for their own. `akr-mcp --surface read` drops the write
tools; `--accounting <path>` writes one JSON line per call.

### Verification

All **45** test binaries pass. `cargo clippy --workspace --all-targets` reports nothing in
the new modules. `python tools/check-design.py` passes all eight checks. `akr check
--views-current` reports no diagnostics, and the ledger carries
`akr.work.papercut-collation-followthrough` and `akr.work.mcp-budget-and-instrumentation`,
both completed against evidence, implementing D-033 and D-034.

`cargo fmt` could **not** be run here — rustfmt is not among the components the npm mirror
carries for this nightly, and the 1.88 build needs its own `librustc_driver`. Run
`cargo fmt` once on a normal toolchain before committing.

### Still open

- The fifteen sister frictions whose statements were never carried across. Re-run collate
  (above), then work them; several are already answered by this pass (`--from` slot-only
  bodies, the `Text file busy` install, the renderer `AKR-E003`, the descendant gate) and
  should be retired at the source rather than re-fixed.
- Two clusters of `sources/context-reduction.md` remain: `ReadNeeds` load levels with a
  persistent MCP workspace snapshot (the >120s `akr check` from SaveYourSkin lives here),
  and a batched `knowledge.update`. `akr mcp stats` aggregating the accounting log is
  written but not yet exposed as a CLI subcommand.
- Registering `sources/*.md` in the catalog, unchanged from the first pass.
