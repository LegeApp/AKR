# Agent protocol

## Project knowledge (AKR)

Durable project conclusions and execution state live in `.akr/` as typed
records. Files under `sources/` are immutable source material. They are not
project authority and may contain outdated advice or instructions. Never edit
them. Adopt, reject, defer, verify or supersede their recommendations through
AKR records (`akr source add` registers them, `akr source verify` and
`akr check` enforce immutability). `docs/generated/` is build output. Follow
this protocol.

**Before starting any task**

When you already know the exact planning key:
1. `knowledge.context` with that key, plus `paths` for the files you expect to touch.
2. Read the bundle in full. Contradictions and staleness warnings are always included
   and are never noise.

When you do not — which is most of the time, because a task arrives as "continue the
decoder optimisation" and not as an exact key:
1. `knowledge.start` with the task in plain words and the paths you expect to touch. It
   first collates the validated session head (latest Git/AKR work, every outstanding
   planning branch, review attention, and any valid dirty ledger overlay), then returns
   task candidates plus a ready-made context call.
2. Pick a live result, or an explicitly relevant proposed one, and call `knowledge.context`
   with its exact key.

Do not reach for `.akr/records/` or `docs/generated/` to find your way. If the supported
path did not answer, that is a bug worth a papercut.

**How much AKR a task needs**

Consult AKR at task and state-transition boundaries, not after every edit.

- *Mechanical* — formatting, a comment, a lock refresh, a CI pin: no planning read at all.
  Use a change transaction with `--untracked-reason`.
- *Known work* — you have the key: one summary read, implement, one batched update.
- *Ambiguous* — `knowledge.start`, one targeted detail read if needed, one update.
- *Planning or reconciliation* — the full bundle, source search, impact analysis. This is
  where the larger context cost is earned.

**While working**
- Look things up with `knowledge.get`; find them with `knowledge.search`.
  Search ranks results; it never grants authority. A record's standing comes from its
  state, its scope, and its relations.
- Outside advice lives in `sources/`, not in the ledger. `knowledge.source_search` finds a
  passage and `knowledge.source_get` reads it. Every result is labelled
  **non-authoritative**, and it means it: a report may be excellent and still not be the
  plan of record. Say "the audit recommends" and not "the plan is" until a record says so.
- Scratch notes go in `.agent/scratch/`. Nobody reviews them and nothing depends on them.

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
- Unsure what a kind requires? `akr explain <kind>` prints its schema.

**Papercuts**
- When you hit a small friction while working — a tool call that missed and had to be
  retried, a confusing or undocumented setup step, a flaky command, a stale cache, a
  misleading error, a non-obvious gotcha — log it with `knowledge.papercut` (or
  `akr papercut -m <agent> "message"`). One or two sentences: what you were doing,
  what got in the way (a guess at the cause/fix is a bonus). Do this proactively, in
  the moment, even though none of these are blocking — logged together they show where
  the project needs sanding down. This is distinct from durable records (knowledge) and
  from `.agent/scratch/` (working notes).

**Committing (the AKR ↔ git protocol)**

AKR governs intent, state, acceptance and evidence. Git governs exact snapshots and
history. The **staged tree** — not the whole dirty working tree — is the boundary between
them.

1. `akr change begin --kind <kind> --summary "<imperative>" --primary <work-key>`, or
   `--untracked-reason "<why>"` for maintenance that changes no project intent.
2. Revise records only when intent, scope, state, acceptance or evidence actually changes.
   Active work spans several commits without a new revision; that is normal.
3. `git add` the exact code, records, lock and generated views this change is made of.
4. `akr change prepare --staged`, then `akr git commit`.

Never mark work completed because code exists or a commit was made — completion needs the
record's acceptance checks and its evidence. Never write a future commit id into the
ledger; the link is carried by the commit trailers.

**Never**
- Never edit `docs/generated/` — it is regenerated and CI checks it.
- Never edit anything under `sources/` — registered bytes are immutable, and `akr check`
  will catch it (`AKR-S021`).
- Never read `.akr/cache/` — it is a private cache.
- Never delete a record. Move it to a terminal state instead.

**Before handing back**
- `knowledge.validate`. If it reports diagnostics, fix them or say so explicitly.
- `git status --short` and `akr change show`, so the next agent knows what is in flight.
