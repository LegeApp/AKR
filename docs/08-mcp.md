# 08 — The MCP Tool Surface

How an agent reaches the ledger: eleven tools, their input and output schemas, how
diagnostics become tool errors, why reads and writes are separated, what idempotency
means here, and the `AGENTS.md` text that makes agents use any of it.

Normative for tool names, schemas, and the error mapping. The semantics of each
operation are normative in [`07-cli.md`](07-cli.md); the MCP server is a transport, not a
second implementation.

---

## 1. Shape

The MCP server is `akr-mcp`, a thin adapter over the same in-process `akr-core` model the
CLI uses. It runs in the workspace, over stdio, with no network and no state:

```
   agent ──MCP──> akr-mcp ──> akr-core ──> .akr/**.akr  (source of truth)
                                    └────> .akr/cache/index.sqlite (private)
```

Two invariants make the surface trustworthy:

- **One implementation.** `knowledge.context` and `akr context` call the same function
  with the same arguments and produce the same bundle. There is no MCP-specific
  assembly, ranking, or filtering. A behaviour that cannot be reproduced from the command
  line is a bug.
- **No privileged access.** The server has exactly the capabilities the CLI has. It
  cannot skip validation, cannot write an unformatted record, and cannot read anything an
  operator could not read by running a command.

The framing is the stdio transport's: one JSON-RPC message per line, and therefore no
line break *inside* a message. A pretty-printed request is not a request — it is several
malformed ones, and the server answers each fragment with its own parse error rather than
guessing where the message was meant to end.

## 2. Tool catalogue

| Tool | Kind | CLI equivalent | Idempotent |
| --- | --- | --- | --- |
| `knowledge.search` | read | `akr search` | yes |
| `knowledge.get` | read | `akr get` | yes |
| `knowledge.context` | read | `akr context` | yes |
| `knowledge.impact` | read | `akr impact` | yes |
| `knowledge.validate` | read | `akr check` | yes |
| `knowledge.propose` | write | `akr propose` | by key |
| `knowledge.revise` | write | `akr revise` | no |
| `knowledge.supersede` | write | `akr supersede` | no |
| `knowledge.complete` | write | `akr complete` | by state |
| `knowledge.evidence_add` | write | `akr evidence add` | by key |
| `knowledge.papercut` | write | `akr papercut` | no |

Eleven tools, and the list is closed for 0.1. Notably absent:

- **No `knowledge.query`.** No arbitrary query language, and above all no SQL. Agents
  never see the SQLite cache (§6).
- **No `knowledge.build`.** Emitting views is a repository-maintenance act performed by
  a human or by CI, not by an agent mid-task.
- **No `knowledge.delete`.** Nothing deletes knowledge (`01-architecture.md` §9). The
  terminal-state transitions are reached through `revise` and `supersede`.

`knowledge.evidence_add` earns its place for the same reason `akr evidence add` does:
an evidence record has required slots a blank template cannot invent, and the first
agent to close out a milestone over MCP had to shell out to the CLI for exactly this
step. Like the command, the tool deliberately has **no field for what the evidence
verifies** (D-016) — the link is authored on the check (`verified_by`) or supplied to
`knowledge.complete`.

## 3. Read tools

### `knowledge.search`

```jsonc
// input
{
  "query": "frame budget",            // required
  "kinds": ["constraint","observation"],  // optional filter
  "states": ["active","verified"],        // optional filter
  "limit": 20                             // optional, default 20, max 100
}
// output
{
  "results": [
    { "key": "sys.constraint.frame-budget-16ms", "rev": 1, "kind": "constraint",
      "state": "active", "title": "16 ms frame budget at p99",
      "score": 0.91, "stale": false, "at_risk": false }
  ],
  "total": 2, "truncated": false
}
```

`score` is advisory and comparable only within one result set. **Search ranks; it never
authorises.** A record appearing here has no standing it did not already have, and
nothing enters a context bundle because it matched a query
([`09-context-assembly.md`](09-context-assembly.md) §1).

### `knowledge.get`

```jsonc
// input
{ "ref": "@sys.policy.tandem-work",   // any of the four forms of D-009
  "history": false, "relations": true }
// output
{
  "key": "sys.policy.tandem-work", "rev": 1, "kind": "policy",
  "class": "normative", "state": "active", "is_head": true,
  "title": "Engine and simulator advance in tandem",
  "scope": [ { "form": "all" } ],
  "topic": "tandem-work",
  "slots": { "rule": "No engine change lands without …" },
  "claims": [ { "anchor": "lag-bound", "text": "…", "retired": false } ],
  "relations": {
    "outbound": [ { "relation": "exceptions", "ref": "@sys.track.lighting/1" } ],
    "inbound":  [ { "relation": "implements", "ref": "@sys.decision.view-generation/1" } ]
  },
  "freshness": { "stale": false, "at_risk": true, "depth": 2,
                 "path": ["@sys.assessment.projection-gaps",
                          "@sim.obs.projection-gaps"] },
  "source_text": "record sys.policy.tandem-work/1 : policy {\n …"
}
```

`source_text` is the canonically formatted record. An agent that wants to reason about
the ledger's own syntax reads that, not a file.

### `knowledge.context`

```jsonc
// input
{ "goal": "sys.milestone.m3-playable-day",   // required; milestone|work|track
  "paths": ["sim/src/project/**"],           // optional
  "budget_tokens": 8000,                     // optional
  "format": "json" }                         // "json" | "text"
// output
{
  "goal": { "key": "…", "rev": 1, "title": "…" },
  "commit": "e806b3f54a2d7091c5e13b8a26f490dc7b135e64",
  "sections": [
    { "id": "goal",            "records": [ … ] },
    { "id": "milestone",       "records": [ … ] },
    { "id": "work-items",      "records": [ … ] },
    { "id": "plan-of-record",  "records": [ … ] },
    { "id": "normative",       "records": [ … ] },
    { "id": "dependencies",    "records": [ … ] },
    { "id": "acceptance",      "checks":  [ … ] },
    { "id": "observations",    "records": [ … ] },
    { "id": "questions",       "records": [ … ] },
    { "id": "contradictions",  "pairs":   [ … ] },
    { "id": "staleness",       "warnings":[ … ] }
  ],
  "excluded": { "superseded": 2, "archived": 1, "terminal": 4,
                "out_of_scope": 12 },
  "truncated_prose": [], "estimated_tokens": 5120
}
```

Sections appear in the fixed order above, always, whether or not they are empty. The
membership of each is computed by the algorithm of `09-context-assembly.md` §4 — a pure
function of (ledger, commit, request).

### `knowledge.impact`

```jsonc
// input — exactly one of `ref` or `git_diff`
{ "ref": "@sim.obs.projection-gaps", "depth": null }
{ "git_diff": "5d9c2a70e31f8b46c07d5924ab6e3f1074c9d285..e806b3f54a2d7091c5e13b8a26f490dc7b135e64" }
// output
{
  "mode": "ref",
  "dependents": [
    { "key": "sim.work.rewrite-projection", "rev": 1, "depth": 1,
      "via": "depends_on", "path": ["@sim.obs.projection-gaps"] },
    { "key": "sys.assessment.projection-gaps", "rev": 1, "depth": 1,
      "via": "supported_by", "path": ["@sim.obs.projection-gaps"] }
  ],
  "newly_stale": [], "newly_at_risk": []
}
```

The tool an agent calls **before** proposing a supersession, to see what it is about to
disturb.

### `knowledge.validate`

```jsonc
// input
{ "review_clean": false }
// output
{ "ok": true, "diagnostics": [],
  "counts": { "records": 40, "revisions": 42, "stale": 2, "at_risk": 4 } }
```

Runs stages A–D over the ledger as it stands on disk. An agent calls it after a batch of
writes to confirm the ledger is still coherent, and before handing work back to a human.

## 4. Write tools

All four writes go through the pipeline of [`07-cli.md`](07-cli.md) §4 — parse, apply,
validate the *result*, canonically format, write atomically — and all four fail
completely rather than partially. A rejected write leaves the working tree byte-identical.

### `knowledge.propose`

```jsonc
// input
{ "key": "sim.obs.projection-rewrite", "kind": "observation",
  "title": "Projection pass rewritten; coverage measured again",
  "slots": { "statement": "…", "observed_at": "git:e806b3f5…",
             "watches": ["sim/src/project/**"] },
  "relations": { "derived_from": ["@sim.obs.projection-gaps/1"] },
  "claims": [ { "anchor": "coverage-restored", "text": "…" } ],
  "sources": [ { "kind": "internal", "path": "sim/src/project/mod.rs" } ] }
// output
{ "key": "…", "rev": 1, "state": "verified", "path": ".akr/records/sim/observations.akr",
  "written": true, "lock_stale": true, "content_hash": "sha256:…" }
```

Creates revision 1 of a **new** key, in its class's initial state. An existing key is an
error — the tool will not silently turn a proposal into a revision.

A milestone requires a non-empty `acceptance` block to exist at all (V-008), so
`knowledge.propose` accepts one directly — an array of `{id, statement, method, command?,
verified_by?}`, one entry per check:

```jsonc
{ "key": "product.milestone.m2", "kind": "milestone", "title": "M2: …",
  "acceptance": [
    { "id": "full-day-demo", "statement": "…", "method": "manual" } ] }
```

`knowledge.revise` accepts the same field to replace a record's acceptance block; omit it
to keep the head's checks.

All four writes return this payload, and three of its fields describe the write rather
than the request:

- `rev` is the revision the write **produced**, not the one it started from. A revision
  and a supersession each touch two revisions of the key — the retired head and its
  successor — and it is the successor an agent's next `base_rev` has to name.
- `state` and `content_hash` describe the record as it landed on disk, read back after
  the write rather than predicted from the request. For a lifecycle move they are the
  only way the agent learns the move took.
- `lock_stale` is `true` after every write, because no write operation may invent a build
  (D-014). Saying so here is the difference between an expected `AKR-R052` on the next
  `knowledge.validate` and a confusing one.

### `knowledge.revise`

```jsonc
{ "key": "sim.obs.projection-gaps",
  "slots": { "observed_at": "git:e806b3f5…" },
  "state": null,                       // optional lifecycle move
  "retired_claims": ["old-anchor"],
  "base_rev": 1 }                      // required: optimistic concurrency
```

`base_rev` must equal the current head revision. If another writer has revised the key
since the agent read it, the tool fails with a conflict rather than clobbering
(`AKR-C033`). This is the only concurrency control the surface has, and it is enough
because the underlying store is a git working tree that a human is also watching.

### `knowledge.supersede`

```jsonc
{ "old_key": "sys.work.m3-plan", "new_key": "sys.work.m3-plan",
  "slots": { "intent": "…" },
  "dispositions": [
    { "child": "@sys.work.m3-lighting-pass", "outcome": "carried_forward",
      "into": "@sys.track.lighting", "note": "Lighting is standing work." },
    { "child": "@sys.work.m3-audio-pass", "outcome": "intentionally_dropped" }
  ] }
```

If any unfinished `part_of` child lacks a disposition, the tool fails with `AKR-R014` and
**lists the children in the error payload**, so the agent's next message can name them.
That is the moment the design cares most about (D-017), and the API is shaped to make
answering easy and skipping impossible.

### `knowledge.complete`

```jsonc
{ "key": "sys.milestone.m3-playable-day",
  "checks": { "no-placeholder-assets": "@sys.evidence.asset-audit/1" } }
```

Fails with `AKR-R022` naming each unsatisfied check, including whether the failure was
"no passing evidence" or "evidence predates the last content change" (D-016).

### `knowledge.evidence_add`

```jsonc
{ "key": "sys.evidence.asset-audit",
  "result": "pass",                       // pass | fail | inconclusive
  "method": "command",                    // manual | command | observation
  "command": "cargo run -p tools -- audit-assets",
  "summary": "Zero placeholder assets on the day-loop path",
  "observed_at": "e806b3f54a2d7091c5e13b8a26f490dc7b135e64" }  // defaults to HEAD
```

Creates an `evidence` record, exactly as `akr evidence add` does. There is no field for
what the evidence verifies, and that absence is the tool doing its job (D-016): the
check names its evidence in `verified_by`, or `knowledge.complete` supplies the link —
one direction, one source of truth. The typical closing sequence is `evidence_add`,
then `complete` with `checks` citing the returned revision.

### `knowledge.papercut`

```jsonc
{ "agent": "claude",
  "message": "Ran knowledge.search right after a write and got stale results;               akr build in between fixed it.",
  "namespace": "sys" }   // needed only when the project declares several
```

Logs a small friction as a `papercut` record (D-027): what you were doing, what got in
the way, and — as a bonus — a guess at the cause or fix. The message is the whole
ceremony: the key, the commit, the author and the date are filled in by the tool. Not
idempotent, deliberately: a log never refuses an entry, so the same message twice is two
records with distinct keys. The aggregate renders to `PAPERCUTS.md` on the next build.

## 5. Error mapping

Every failure is an MCP tool error whose payload is the JSON diagnostic array of
[`07-cli.md`](07-cli.md) §5, plus a coarse class the agent can branch on without knowing
the code table:

```jsonc
{ "error": {
    "class": "invariant",
    "summary": "superseding plan does not dispose of an unfinished child",
    "diagnostics": [ { "code": "AKR-R014", "severity": "error", "rule": "V-017",
                       "message": "…", "path": "…", "line": 61, "column": 1,
                       "help": "add a disposition block" } ],
    "retryable": false,
    "wrote": false } }
```

| Class | Codes | What the agent should do |
| --- | --- | --- |
| `usage` | `AKR-C001`–`AKR-C005`, `AKR-C041`, `AKR-X041` | Fix the call. Never retry unchanged. |
| `not_found` | `AKR-L001`, `AKR-L004`, `AKR-X001`, `AKR-E003` | The reference is wrong. Search or re-read. |
| `schema` | `AKR-P***`, `AKR-T***` | The proposed content is malformed. Fix and resubmit. |
| `invariant` | `AKR-R***`, `AKR-L006`, `AKR-L012`, `AKR-L021`, `AKR-L031` | The ledger would become incoherent. This usually needs a *design* decision, not another attempt — surface it to the human. |
| `conflict` | `AKR-C032`, `AKR-C033` | Re-read the head and rebase the edit. Retryable once. |
| `environment` | `AKR-C011`, `AKR-C012`, `AKR-G001`, `AKR-G003`, `AKR-I003`, `AKR-I031`, `AKR-I032` | Not the agent's fault and not fixable by it. Stop and report. |
| `degraded` | `AKR-X033`, `AKR-G004`, `AKR-X012`, `AKR-X022` | Warnings under `--lenient`; the call succeeded with a caveat that belongs in the agent's report. |

`wrote` is always present on a write tool's error and is always `false`. An agent never
has to guess whether a failed write left something behind.

## 6. Why agents never see SQLite

D-019 in one paragraph, because it is the boundary most likely to be eroded by
convenience.

`.akr/cache/index.sqlite` is a private implementation detail of pipeline stage E. It is
gitignored, rebuilt whenever the schema version or the source-graph hash changes, and
safe to delete at any instant. Exposing it — even read-only, even "just for search" —
would convert its schema into a public interface with compatibility obligations, and the
ledger would acquire a second source of truth that is sometimes newer and sometimes older
than the first. Every `AKR-I` diagnostic in
[`../spec/diagnostics/codes-runtime.md`](../spec/diagnostics/codes-runtime.md) exists
because the cache is allowed to fail loudly and be rebuilt; none of that is true of an
interface someone depends on.

The practical consequence for tool design: whenever an agent wants something the nine
tools cannot express, the answer is a new tool with a defined contract, never a query
hole. `knowledge.search` is the deliberate, narrow escape valve, and it returns records,
not rows.

## 7. Read/write separation and idempotency

**Separation.** Read tools never touch `.akr/records/`. They may rebuild the index cache
as a side effect (that is what a cache is), and under `--no-rebuild` they will not even
do that. A read tool's effect on the repository's committed content is always nil.

**Idempotency.** The read tools are idempotent in the strong sense: called twice against
the same (sources, commit, tool version), they return byte-identical results. That
follows directly from the determinism contract (`01-architecture.md` §4) and is what lets
an agent cache a bundle for a session.

The write tools are idempotent to the extent the operation allows, and the schema is
shaped to make the difference explicit:

- `knowledge.propose` is idempotent **by key**: a second call with the same key fails
  rather than creating a second record.
- `knowledge.revise` is not idempotent — that is what `base_rev` is for. A retry with a
  stale `base_rev` fails with `conflict`; a retry with the new one applies the edit
  again, which is usually not what was wanted.
- `knowledge.supersede` and `knowledge.complete` are idempotent **by state**: superseding
  an already-superseded record, or completing an already-completed one, fails with an
  invariant error rather than doing it twice.

There is no transaction spanning multiple tool calls. An agent that needs several records
to land together proposes them one at a time and calls `knowledge.validate` at the end;
if the ledger is incoherent in between, the intermediate `propose` calls will have failed
already, because every write validates the *resulting* ledger.

## 8. The `AGENTS.md` protocol text

This is the recommended minimal `AGENTS.md` section. It is deliberately protocol only —
no philosophy, no data model, no examples — because an agent reads it every session and
every extra line competes with the task. Everything it needs to know beyond this is
reachable through the tools themselves.

```markdown
## Project knowledge (AKR)

Durable project knowledge lives in `.akr/` as typed records, not in Markdown.
`docs/generated/` is build output. Follow this protocol.

**Before starting any task**
1. `knowledge.context --goal <milestone|work|track>` for the thing you are working on.
   Add `--paths` for the files you expect to touch.
2. Read the bundle in full. Contradictions and staleness warnings are always included
   and are never noise.

**While working**
- Look things up with `knowledge.get`; find them with `knowledge.search`.
  Search ranks results; it never grants authority. A record's standing comes from its
  state, its scope, and its relations.
- Scratch notes go in `.agent/scratch/`. Nobody reviews them and nothing depends on them.
- When you hit a small friction — a retried tool call, a confusing setup step, a flaky
  command, a stale cache, a misleading error — log it with `knowledge.papercut`, in the
  moment. One or two sentences; a guess at the cause/fix is a bonus.

**When something becomes durable**
- New knowledge: `knowledge.propose`. Observations need `observed_at` and, if they can
  go out of date, `watches`.
- Changed knowledge: `knowledge.revise`. Never edit a `.akr` file directly, and never
  edit a record that is not `proposed`.
- Replacing a plan: `knowledge.supersede`, with a disposition for every unfinished
  child. The tool will list them; answer each one.
- Finishing work: record what you observed with `knowledge.evidence_add`, then
  `knowledge.complete` with evidence for every acceptance check. Evidence records
  state what was observed; they never state what they verify.

**Never**
- Never edit `docs/generated/` — it is regenerated and CI checks it.
- Never read `.akr/cache/` — it is a private cache.
- Never delete a record. Move it to a terminal state instead.

**Before handing back**
- `knowledge.validate`. If it reports diagnostics, fix them or say so explicitly.
```

That is the whole protocol. Three commands to start, three to write, three prohibitions,
one to finish.

## 9. Walkthrough: an agent working on M3

Against [`../examples/save-your-skin/`](../examples/save-your-skin/), whose inventory is
frozen in its `MANIFEST.md`. The agent has been asked to rewrite the projection pass.

**1. Get context.**

```jsonc
→ knowledge.context { "goal": "sys.milestone.m3-playable-day",
                      "paths": ["sim/src/project/**"] }
```

The bundle returns M3, its plan of record `sys.work.m3-plan/2`, the live in-scope
policies and constraints, `sim.work.rewrite-projection` in `blocked` state with the
question that blocks it, both M3 acceptance checks with `full-day-demo` marked satisfied,
`sim.obs.projection-gaps` with a staleness warning, and the acknowledged contradiction
between `sim.obs.timestep-drift` and `sim.evidence.determinism-suite-pass`. It does not
return `sys.work.m3-plan/1`, `lege.decision.renderer-boundary/1`, or
`sys.policy.weekly-demo` — superseded, superseded, and archived respectively.

The agent now knows three things it could not have learned from any Markdown file: the
work item is blocked and by what; the observation it would naturally rely on is stale and
why; and one of the two acceptance checks is already met.

**2. Check what the blocker is.**

```jsonc
→ knowledge.get { "ref": "@sim.question.timestep-vs-budget", "relations": true }
```

An `open` question — "does a 4 ms timestep fit the frame budget?" — with `blocks` edges
to `sim.decision.timestep-4ms` and to `sim.work.rewrite-projection`. The agent cannot
proceed past it without an answer, and it now knows to say so rather than guessing.

**3. See what the rewrite would disturb.**

```jsonc
→ knowledge.impact { "ref": "@sim.obs.projection-gaps" }
```

Three dependents: at depth 1, `sim.work.rewrite-projection` itself (via `depends_on` —
the agent's own work item rests on the stale observation) and
`sys.assessment.projection-gaps` (via `supported_by`); at depth 2,
`sys.policy.tandem-work`. Rewriting the projection pass will require re-observing, and
three records downstream will need review when it does.

**4. Record what it found.**

```jsonc
→ knowledge.propose {
    "key": "sim.obs.projection-rewrite-scope", "kind": "observation",
    "title": "The projection pass has three callers, all inside sim",
    "slots": { "statement": "…", "observed_at": "git:e806b3f5…",
               "watches": ["sim/src/project/**"] },
    "relations": { "derived_from": ["@sim.obs.projection-gaps/1"] } }
```

Note the pinned `derived_from`: the new observation records what it was derived from at
the revision it actually read, which is what makes the provenance auditable later. The
`watches` glob means this observation will itself go stale when the code moves again —
the agent is writing knowledge that knows how to expire.

**5. Hand back.**

```jsonc
→ knowledge.validate { }
← { "ok": true, "diagnostics": [],
    "counts": { "records": 41, "revisions": 43, "stale": 2, "at_risk": 4 } }
```

Still 2 stale and 4 at risk: the new observation is current, and nothing it touched
became stale. The agent reports that the rewrite is blocked on
`@sim.question.timestep-vs-budget` and stops — which is the outcome the whole design
exists to produce, in place of a confident rewrite built on a stale observation.

---

Next: [`09-context-assembly.md`](09-context-assembly.md) for exactly what step 1
computed, or [`07-cli.md`](07-cli.md) for the same operations from a shell.
