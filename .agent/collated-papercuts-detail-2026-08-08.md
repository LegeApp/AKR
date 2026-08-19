# Collated sister-project papercuts — full detail

**Why this file exists.** `akr papercut collate --projects /mnt/Samsung980_1TB/Rust-projects`
already ran (2026-08-07, see D-030) and correctly reports "nothing new to collate" — the
work is done. Its output lives in
`akr.papercut.collated-18-papercuts-from-kitchen-concept-lege/1` in
`.akr/records/akr/papercuts.akr`, but that record deliberately stores only a one-line
summary per item plus a `collated` slot of source keys ("see the owning project's ledger
for the full statement" — D-030's design, so the collation doesn't duplicate content across
repos). An agent sandboxed to this repo can't follow those keys out to the six sister
repos to read the full statements. This file inlines all 18 in full, read directly from
each sister project's own `.akr/records/*/papercuts.akr`, so the work can proceed without
that access.

Do not treat this file as ledger truth — it's a read-only convenience snapshot. The
authoritative pointer is still the `collated` record in `akr.papercut.collated-18-...`.

---

## Already addressed by AKR itself (verify before acting further)

Three of the 18 turn out to be the *same* root cause AKR already fixed on this branch —
sister repos just haven't picked up the fix yet (they'd need updated `akr` on their
machine / a re-run once they do):

- **`bpg.papercut.v-020-s-descendant-commit-freshness-gate-akr`** — this is the exact case
  D-028 was written to resolve (the decision text cites this papercut by name). A
  `completed` record with a `source { kind legacy ... }` block is now exempt from the
  descendant-commit comparison. Action for bpg-rs: add `legacy` source blocks to the ~19
  affected records, rebuild with current AKR, re-check.
- **`jpegxl-rs.papercut.knowledge-complete-s-cited-evidence-gets-its`** — likely improved
  by D-029 (the descendant gate now hashes only *definitional* content, excluding `state`
  and `verified_by`), since adding evidence + completing no longer counts as "the last
  content change" the way it did before. Not a byte-identical match to the papercut's
  scenario (that one is specifically about `observed_at` being stamped from a
  pre-commit HEAD), so treat as "probably better, verify on a fresh case" rather than
  fully closed.
- **`jpegxl-rs.papercut.akr-mcp-install-setup-akr-mcp-sh-cp-fails-with`** — AKR's own
  ledger recorded and resolved the identical failure
  (`akr.papercut.calling-the-knowledge-papercut-mcp-tool/2`): `scripts/setup-akr-mcp.sh`
  now installs by rename instead of `cp`, avoiding `ETXTBSY` when a client holds the old
  binary open. Sister repos get this for free once they re-run the updated install script.

Everything else below is a genuine open item for whoever owns that project (or for AKR
itself, where the papercut is about AKR's own behavior).

---

## Kitchen-Concept (3)

### `kitchen.papercut.porting-a-12-document-40kb-source-planning`
Porting a 12-document, ~40KB-source planning corpus by hand-authoring `.akr` directly
(rather than `akr import`) worked well but gave no tooling checkpoint between "wrote N
records" and "akr check" at the end — no per-file dry-run/lint equivalent to
`import --dry-run` for hand-authored source. A lightweight `akr check <file>` scoped to
one new file would let an agent validate incrementally while drafting a large batch
instead of writing all ~80 records before the first syntax feedback.
*(observed_at git:7c757c2b2ee0f7185bbf67d202e6168c6db28eae, author sonnet, 2026-08-05)*

### `kitchen.papercut.relation-domain-range-e-g-resolves-work`
Relation domain/range (e.g. `resolves`: work/decision/observation/evidence -> question,
not the reverse) isn't spelled out in `exemplar.akr` or the migration/data-model prose;
the author placed `resolves` on the question record first and had to grep
`spec/tables/vocabulary.json` directly to find the correct direction. A one-line
relation-direction cheat sheet in `docs/02-data-model.md` §7 (or a hint in the
akr-source prompt's relations section) would have saved a check-fail round trip.
*(observed_at git:7c757c2b2ee0f7185bbf67d202e6168c6db28eae, author sonnet, 2026-08-05)*

### `kitchen.papercut.two-policy-records-with-the-same-topic-and`
Two policy records with the same topic and overlapping scope silently produced AKR-R002
only at `akr check` time, not at write time. When hand-authoring many records in one
batch (as opposed to one `akr propose` at a time), a lint/fmt-time warning for topic
collisions across the whole workspace would surface this before running the full check
pass.
*(observed_at git:7c757c2b2ee0f7185bbf67d202e6168c6db28eae, author sonnet, 2026-08-05)*

---

## Lege-ecosystem (1)

### `lege-ecosystem-perf.papercut.imported-lege-ecosystem-perf-audit-work-records`
Imported lege-ecosystem-perf audit work records (2026-08-05) are stale vs code: their
P1/Phase-1..3 defects (nonzero tile origins disabling optimized DWT, per-row 9/7 scratch,
full planar staging, missing packed Rgbx8/Bgra8 formats) are already fixed in
lege-codecs/jp2lam. Determining "unfinished" work required reading source per-item since
ledger states are all still `proposed`. Consider reconciling/closing those imported
records against the implementation.
*(observed_at git:5780178af9f0d10bc4a8457259dfa141c2f804de, author fugu, 2026-08-06)*

This one is not an AKR tooling papercut — it's a to-do inside Lege-ecosystem's own ledger
(records need their state reconciled against code). Action belongs to that project, not AKR.

---

## SaveYourSkin (2)

### `saveyourskin.papercut.akr-check-akr-build-akr-lock-update-each-took`
`akr check` / `akr build` / `akr lock --update` each took >120s (timed out the 120s bash
default and had to background) on saveyourskin, a ~360-record, ~330-commit repo —
ancestry-facts computation (`git merge-base --is-ancestor` per evidence citation, per
D-016/D-028) seems to dominate. Every write-then-validate cycle during a large port
session pays this cost repeatedly; a cached/batched ancestry check (e.g. one
`git rev-list --ancestry-path` walk instead of O(citations) merge-base calls) would make
interactive porting sessions much faster.
*(observed_at git:f053287dc4d933ffa031f8e37eee8833d2230a32, author sonnet, 2026-08-05)*

### `saveyourskin.papercut.akr-check-build-validate-the-whole-ledger-with`
`akr check`/`build` validate the whole ledger with no way to scope to just the records
being added; a pre-existing AKR-R022 failure in `milestones.akr` (unrelated real-engine
work, not part of the markdown-porting task) blocked `akr build` and
`akr check --views-current` for 300+ new records even though they were themselves clean,
and there was no flag like `--only <path>` to confirm additions in isolation.
*(observed_at git:b9dfebbf9eadda0accf0b809d5bb9b02587f1ec2, author sonnet, 2026-08-05)*

Note: these two are performance/scoping asks against AKR's own `check`/`build`/`lock`
commands — real AKR feature-request material (batched ancestry check, `--only <path>`
scoping), not something the sister project can fix on its own.

---

## bpg-rs (2)

### `bpg.papercut.task-brief-for-a-historical-port-asked-for-docs`
Task brief for a historical port asked for `docs/generated/DECISION-HISTORY.md`
alongside `ROADMAP.md` to read as the development story, but this akr 0.1.0 build only
implements the roadmap and papercuts view renderers — decision-history, current-state,
active-work, review-required, and open-questions all fail with AKR-E003 ("the X renderer
arrives with a later phase"). `akr build` and `akr check --views-current` silently treat
the unimplemented views as vacuously current instead of warning, so the gap is only
visible by running `akr view <name>` directly. Worth a note in the CLI help or check
output that these renderers are stubs in 0.1.0.
*(observed_at git:fdc89eb2143ec53f48a9376e95aac45c94993f97, author sonnet, 2026-08-05)*

Check current AKR state before treating as open: the roadmap doc
(`docs/13-implementation-roadmap.md`) says the implementation is now well past P8 (through
P10/P11), so some of these renderers may already exist — worth a quick `akr view
DECISION-HISTORY` / `akr view CURRENT-STATE` smoke test against current AKR rather than
assuming the gap is still there. The silent-vacuous-current behavior on unimplemented
views is the more durable part of this papercut either way.

### `bpg.papercut.v-020-s-descendant-commit-freshness-gate-akr`
V-020's descendant-commit freshness gate (AKR-R022) is structurally incompatible with a
single-commit historical/bulk port: once the `.akr` records are committed, that commit
becomes "the last commit that changed the content" for every completed milestone/work
record, and no historical evidence (dated before the port, which is the whole point of a
historical port) can ever be a descendant of it. `akr check --views-current` was clean on
the working tree before committing the ledger; the same check failed with ~19 AKR-R022
diagnostics immediately after committing, for every completed record with an
evidence-backed acceptance check. Possible product answers considered: exempt records
sourced from legacy transcription from the descendant check, document that bulk
historical imports should land as one commit per original doc/commit, or accept this as
an expected one-time "stale on arrival, re-verify to close" state and document it.
*(observed_at git:6934e2f49f1434278f89c5222f4639e8889dae2e, author sonnet, 2026-08-05)*

**Resolved at the AKR level** — see "Already addressed by AKR itself" above (D-028).
Remaining action is on bpg-rs: annotate the affected records with `source { kind legacy
... }` and re-check against current AKR.

---

## jpegXL-rs (8)

### `jpegxl-rs.papercut.akr-check-strict-exits-1-on-akr-g004-alone-when`
`akr check --strict` exits 1 on AKR-G004 alone when watched paths have uncommitted edits
during a disposition; lenient is clean. Makes "akr check is clean" acceptance awkward
mid-session before commit.
*(observed_at git:788a3b4ebce146cacbe3412b12de6f712da58304, author grok-build, 2026-08-06)*

### `jpegxl-rs.papercut.akr-import-of-performance-md-dry-run-proposed`
`akr import` of PERFORMANCE.md `--dry-run` proposed every section as kind `work`
(including rules and measurements), so manual propose/revise was required for
policy/decision/observation disposition.
*(observed_at git:788a3b4ebce146cacbe3412b12de6f712da58304, author grok-build, 2026-08-06)*

### `jpegxl-rs.papercut.akr-mcp-install-setup-akr-mcp-sh-cp-fails-with`
`akr-mcp` install: `setup-akr-mcp.sh` `cp` fails with "Text file busy" when an MCP client
holds the old binary open; atomic `mv` over a `.new` file works. Setup script should
install via rename.
*(observed_at git:788a3b4ebce146cacbe3412b12de6f712da58304, author grok-4.5, 2026-08-06)*

**Resolved at the AKR level** — see "Already addressed by AKR itself" above.

### `jpegxl-rs.papercut.akr-propose-from-rejects-slot-only-bodies`
`akr propose --from` rejects slot-only bodies; requires full `akr 0.1` project header +
`record { ... }` even though help says "a file holding the record body." First attempt
failed with AKR-C031.
*(observed_at git:788a3b4ebce146cacbe3412b12de6f712da58304, author grok-build, 2026-08-06)*

### `jpegxl-rs.papercut.akr-view-active-work-current-state-etc-returned`
`akr view ACTIVE-WORK/CURRENT-STATE/etc` returned AKR-E003 (renderer arrives later) on an
intermediate binary while roadmap worked; full `akr build` needed for other views.
Confusing when only roadmap is available.
*(observed_at git:788a3b4ebce146cacbe3412b12de6f712da58304, author grok-4.5, 2026-08-06)*

Same "check against current AKR state" caveat as bpg's docs papercut above — implementation
has moved past P8 since this was observed.

### `jpegxl-rs.papercut.hybrid-intel-plain-cycles-u-records-cpu-atom`
Hybrid Intel: plain `cycles:u` records `cpu_atom`+`cpu_core`; atom samples can dominate
collapsed stacks (vardct-rate first pass looked like only `synthetic_rgb8`). Always use
`-e cpu_core/cycles/u` and `taskset` to P-cores for reusable flamegraphs.
*(observed_at git:788a3b4ebce146cacbe3412b12de6f712da58304, author grok-4.5, 2026-08-06)*

This one is a jpegXL-rs profiling-methodology note, not an AKR papercut — no AKR action
implied, just perf-tooling hygiene for that project.

### `jpegxl-rs.papercut.jpxl-bench-fingerprints-outputs-with-fnv-1a-64`
`jpxl bench` fingerprints outputs with FNV-1a-64, but PERFORMANCE.md baselines require a
cryptographic hash of codestream and binary; need `--sha256` or a post-pass so promotion
is not hand work.
*(observed_at git:788a3b4ebce146cacbe3412b12de6f712da58304, author grok-4.5, 2026-08-06)*

Also a jpegXL-rs-internal tooling gap (its own `jpxl bench`), not an AKR papercut.

### `jpegxl-rs.papercut.knowledge-complete-s-cited-evidence-gets-its`
`knowledge.complete`'s cited evidence gets its `observed_at` commit hash from the current
git HEAD at write time, which is the *parent* commit if the record and its evidence are
still uncommitted — so `akr check`/`knowledge.validate` after the eventual `git commit`
flags V-020 "evidence predates last content change" even though the evidence genuinely
verifies the committed content. Workaround used twice: after committing, add a second
evidence record (re-running the same cheap check) and re-run `knowledge.complete` citing
it, then `akr build`. Suggested cleaner fix: an explicit `observed_at` override accepted
post-hoc, or docs telling agents to commit code before the propose/evidence/complete
sequence rather than after.
*(observed_at git:d11219f6cc0b81eaa13bbe59c291213dce429910, author claude, 2026-08-07)*

**Likely improved, not confirmed closed** — see "Already addressed by AKR itself" above
(D-029 narrows the descendant gate to definitional changes only). Worth a fresh repro on
current AKR before deciding whether the `observed_at`-override idea is still needed.

### `jpegxl-rs.papercut.opt-f-multiplicity-counters-are-still-only`
Opt-F multiplicity counters are still only listed in PERFORMANCE.md — no feature-gated
`EncodeStats` in policy/encode — so flamegraphs cannot yet separate repeated DCT from
expensive DCT.
*(observed_at git:788a3b4ebce146cacbe3412b12de6f712da58304, author grok-4.5, 2026-08-06)*

jpegXL-rs-internal implementation gap, not an AKR papercut.

---

## raw-autotune (1)

### `raw-autotune.papercut.knowledge-search-and-knowledge-start-via-the`
`knowledge.search` and `knowledge.start` via the akr MCP tool repeatedly failed with
FTS5 errors (e.g. `"fts5: syntax error near ','"` on a comma-containing query, and
`"no such column: default"` on the query `'HDR slice 6 non-default feature'`). Queries
with bare punctuation or reserved-looking words need escaping/quoting, or the tool should
sanitise them. Fell back to grepping `.akr/records` directly.
*(observed_at git:ff74d3b248f65ea98bde93578a3b95c80a647181, author fugu, 2026-08-07)*

Genuine open AKR bug: FTS5 query strings built from raw user/agent text aren't escaped
before being handed to SQLite's MATCH. Likely lives in `crates/akr-core/src/store/mod.rs`
or wherever `knowledge.search`/`akr search` builds the FTS5 query string — needs
quoting/escaping of the query term (or a fallback to a LIKE-based search on FTS5 parse
error) before it reaches SQLite.

---

## Suggested next AKR-side work items (genuinely open, AKR's to fix)

1. **FTS5 query escaping** (`raw-autotune`) — quote/escape query text before MATCH, or
   catch the FTS5 parse error and retry with an escaped/simplified query.
2. **Ancestry-check performance at scale** (`saveyourskin` #1) — batch the
   `git merge-base --is-ancestor` calls (e.g. one `git rev-list --ancestry-path` walk)
   instead of one process spawn per evidence citation; matters once a ledger has
   hundreds of records/commits.
3. **Scoped validation** (`saveyourskin` #2) — a `--only <path>` or similar flag so
   `akr check`/`akr build` can confirm a subset of records without being blocked by
   pre-existing unrelated failures elsewhere in the ledger.
4. **`akr propose --from` accepting slot-only bodies** (`jpegxl-rs`) — either loosen the
   parser to match the documented behavior, or fix the help text to say a full
   `akr 0.1` + `project` + `record { ... }` file is required.
5. **`akr import` kind classification** (`jpegxl-rs`) — PERFORMANCE.md-style docs mixing
   rules/measurements/narrative currently all import as `work`; needs section-content
   heuristics for policy/decision/observation.
6. **`akr check --strict` + AKR-G004 alone** (`jpegxl-rs`) — decide/document whether
   uncommitted watched-path edits alone should fail strict mode mid-session.
7. **Unimplemented-view UX** (`bpg-rs`, `jpegxl-rs`) — re-verify against current AKR
   first (implementation has advanced past P8); if still relevant, stop treating
   unimplemented renderers as vacuously current in `--views-current`, and/or say in CLI
   help which views are stubs in the current build.
8. Sister-repo-only follow-ups that don't need AKR code changes: bpg-rs adding `source {
   kind legacy }` to its 19 affected records (D-028), Lege-ecosystem reconciling stale
   `proposed` audit records against fixed code.

---

## Disposition — 2026-08-08 (AKR side)

Worked from this file. Recorded as D-035, `akr.work.sister-papercut-fixes` (completed),
and verified by `akr.evidence.sister-papercut-fixes`.

### Fixed

| # | Papercut | What changed |
| --- | --- | --- |
| 1 | `raw-autotune` FTS5 escaping | `akr search` escapes its query into quoted terms by default; raw FTS5 moved behind `--fts`, and is never used over MCP. All three field queries (`budget, tokens`; `HDR slice 6 non-default feature`; `DecodeRequest::default()`) were reproduced failing and now return results. A malformed `--fts` expression says `drop --fts to search for the words themselves`. |
| 2 | `saveyourskin` ancestry perf | `ancestry_over` used a comparison sort whose comparator forked `git merge-base --is-ancestor` — O(n log n) processes — and filtered its input with one `rev-parse` per commit. Now one `cat-file --batch-check` and one `rev-list --topo-order`: two processes total, same order. |
| 4 | `jpegxl-rs` `--from` slot-only bodies | A `--from` file may now be a bare slot list; the header and record line come from the key and kind already on the command line. A file with its own header is still verbatim. |
| 6 | `jpegxl-rs` strict + `AKR-G004` | `AKR-G004` is now exempt from the strict promotion, for the same reason staleness never changes an exit code (D-024). A clean `akr check --strict` is reachable mid-task. |
| — | `jpegxl-rs` `setup-akr-mcp.sh` `Text file busy` | Installs by rename; the server also publishes its vocabulary version and reports a skew as `AKR-X042` with the reconnect step named. |

### Verified stale — no action needed

**#7, unimplemented views** (bpg-rs and jpegXL-rs). All seven renderers answer on current
AKR: ROADMAP, CURRENT-STATE, ACTIVE-WORK, REVIEW-REQUIRED, OPEN-QUESTIONS,
DECISION-HISTORY, PAPERCUTS. Landed in `30579ae`, after both papercuts were observed. The
"vacuously current" concern is moot with no stub renderers left; if a future view is added
as a stub, that is the moment to revisit it.

### Judged answered by design, not by code

**#5, `akr import` kind classification.** Everything imports as `work` because
heading-oriented import *guesses*, and D-031 concluded that guessing is the wrong operation
for an outside document: register it with `akr source add` and create records only for what
the project adopts. `akr import` remains the legacy-migration path and keeps its behaviour.
No heuristics were added — better keyword rules would make the guess wrong less often
rather than right.

### Still open, and not AKR's

- **#3, scoped validation (`--only <path>`)** — genuine AKR feature request, not done this
  pass. A pre-existing failure elsewhere in the ledger still blocks confirming new records
  in isolation.
- **jpegXL-rs #16, `observed_at` from a pre-commit HEAD** — probably improved by D-029; the
  scenario is not a byte-identical match, so it wants a fresh repro before closing.
- **bpg-rs**: annotate the ~19 affected records with `source { kind legacy }` and re-check
  (D-028).
- **Lege-ecosystem**: reconcile the stale `proposed` audit records against the code that
  already fixed them.
- **jpegXL-rs internal**: hybrid-Intel `cycles:u` profiling hygiene, `jpxl bench` FNV-1a-64
  vs SHA-256 baselines, Opt-F multiplicity counters. Not AKR's to fix.
