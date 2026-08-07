# Findings: AKR papercuts about AKR itself get siloed in the reporting project's ledger

**Date:** 2026-08-07
**Trigger:** While working in `jpegxl-rs`, I logged
`jpegxl-rs.papercut.knowledge-complete-s-cited-evidence-gets-its` — a friction
report about `knowledge.complete`/`knowledge.evidence_add`'s `observed_at`
defaulting behavior (AKR's own tool behavior, not anything about jpegxl-rs).
The user asked whether recording AKR-related papercuts inside a *consuming*
project's ledger, rather than AKR's own, risks them being acted on. This
document is pure investigation — **no changes made** to AKR, jpegxl-rs, or
any ledger, beyond the papercut already logged before this question was
asked.

## Short answer

Yes, this is a real gap, and it isn't hypothetical — it has already happened
once. Papercuts are strictly per-project: each project's `.akr/` is a ledger
over *that project's own git repository*, `PAPERCUTS.md` is rendered only
from records in that same ledger, and AKR 0.1 has no cross-repository
aggregation, subscription, or notification mechanism. A papercut about AKR's
own behavior, logged while working in `jpegxl-rs`, is invisible to anyone
maintaining AKR unless they specifically go and read `jpegxl-rs`'s ledger —
which nothing prompts them to do.

## How papercuts actually work (from AKR's own spec)

- **Kind and intent** (`docs/DECISIONS.md` D-027): `papercut` is the 13th
  record kind, empirical class, added specifically so small frictions ("a
  tool call that missed and had to be retried, a confusing setup step, a
  flaky command, a stale cache, a misleading error, a non-obvious gotcha")
  have somewhere durable, typed, and searchable to go instead of being
  discarded or dumped into an untyped `PAPERCUTS.md` at the repo root.
- **Where it's written**: `akr papercut -m <agent> "message" [--namespace
  <ns>]` (CLI) or the `knowledge.papercut` MCP tool (`docs/07-cli.md` §
  "akr papercut", `docs/08-mcp.md` § "knowledge.papercut"). The key is
  allocated as `<namespace>.papercut.<slug-of-message>` — `<namespace>` is
  **one of the namespaces the current project declares in its own
  `.akr/project.akr`**, never a foreign project's namespace.
- **Where it renders**: a seventh generated view, `PAPERCUTS.md`
  (`docs/11-projections.md` § "PAPERCUTS.md" / § 12), built by `akr build`
  from "Live `papercut` records" — scoped to the ledger being built, i.e.
  the current repository. There is no project that reads another project's
  `.akr/` to build its own views.
- **Write surface is workspace-bound**: the MCP server "runs in the
  workspace" (`docs/08-mcp.md` § 1) — one `akr-mcp` instance per project,
  talking only to that project's `.akr/`. `jpegxl-rs`'s session has a
  `.mcp.json` pointing at `jpegxl-rs`'s own ledger; there is no tool call
  available in this session that can write into `AKR`'s ledger instead. (An
  agent *could* shell out to `akr papercut --dir /path/to/AKR ...` via Bash,
  since the CLI's `--dir` flag lets you point at an arbitrary workspace, but
  nothing prompts or requires this, and it isn't how the documented workflow
  in `AGENTS.md`/`CLAUDE.md` is written — those say "record durable changes
  via AKR tools" for *the current project*.)
- **The `sys` namespace is not a cross-project mailbox.** `docs/08-mcp.md`'s
  example payload uses `"namespace": "sys"`, and `docs/03-syntax.md:305`
  shows `namespace sys "Project-wide knowledge: policy, plan, milestones,
  tracks."` — but this is a *per-project naming convention* some projects
  adopt for their own "meta" namespace (see `save-your-skin --namespace sys
  --namespace sim --namespace lege` in `docs/07-cli.md`), not a reserved,
  AKR-recognized namespace that gets treated specially or routed anywhere.
  `jpegxl-rs`'s own `project.akr` declares only one namespace, `jpegxl-rs` —
  no `sys` at all. A papercut filed with `--namespace sys` in a project that
  happens to declare one still lands in *that project's own* `.akr/`.

## Confirmed: this has already happened, silently, once

`bpg-rs`'s ledger contains
`bpg.papercut.v-020-s-descendant-commit-freshness-gate-akr/1` — a detailed
friction report about `AKR-R022` / V-020's descendant-commit freshness gate
being structurally unsatisfiable for a bulk historical import (evidence
dated before the port, committed in the same commit that introduces the
records, so it can never "descend from" that commit). This is squarely a
finding about **AKR's own validation rule**, not about `bpg-rs`.

It was, eventually, acted on: `AKR/docs/DECISIONS.md` **D-028** ("Legacy-
sourced completion is exempt from the descendant-commit gate") cites it by
key, verbatim, as the "Live case" motivating the fix. But that only happened
because whoever wrote D-028 was specifically aware of and read `bpg-rs`'s
ledger — nothing in AKR's own build, view, or validation output surfaced it
automatically. If `bpg-rs`'s papercuts had gone unread, D-028 would not
exist, and V-020 would still be silently unsatisfiable for every historical
port.

This session's `jpegxl-rs.papercut.knowledge-complete-s-cited-evidence-gets-
its` is arguably the *same underlying class of gap* as D-028 fixed
(evidence necessarily predates the commit that introduces the record citing
it) but for a *different* trigger — ordinary same-session propose-then-
complete-then-commit, not a bulk historical port — so D-028's `legacy`-source
exemption does not cover it. Unless someone reads `jpegxl-rs`'s ledger the
way `bpg-rs`'s was read for D-028, this will keep recurring, silently, in
every project's ledger that completes a milestone in the same sitting it
proposes and commits it.

## Why this is structural, not an oversight

AKR's own architecture contract (`docs/01-architecture.md`) treats a ledger
as "a pure function of (source files, git commit, tool version)" for **one**
repository — the whole reproducibility guarantee is scoped to one git
history. Multi-repo aggregation is out of scope for 0.1 by design, not
missing by accident. The gap here isn't "AKR should support cross-repo
ledgers" (a much bigger ask) — it's narrower: **papercuts specifically are
the one record kind whose subject is sometimes the tool itself rather than
the project being worked on**, and nothing in the write path, the view, or
the agent-facing docs distinguishes "friction with *this project's* code"
from "friction with *AKR's own* behavior" — both get the same treatment,
silently absorbed into whichever project's ledger happened to be open when
the friction was hit.

## Options for later review (not evaluated for feasibility, not recommended)

Listed for whoever picks this up; no implementation work was done or
implied by writing this list.

1. **Convention, not tooling**: document (in `AGENTS.md`/`CLAUDE.md`
   templates, or AKR's own onboarding docs) that a papercut whose subject is
   AKR's own behavior — not the project's — should *also* be filed directly
   against the AKR repo via `akr papercut --dir <path-to-AKR> -m <agent>
   "..."`, in addition to (or instead of) the project's own ledger. Zero
   tooling change; relies on the agent noticing the distinction and knowing
   where the AKR repo lives, which won't always be true (this session only
   knew because the user runs both from sibling directories on the same
   machine).
2. **A recognized cross-cutting namespace or tag** AKR's own tooling
   understands (e.g. a papercut record can carry a `product: "akr"` slot or
   similar, independent of which repo it's filed in), plus a CLI/MCP command
   like `akr papercut --collect-upstream` that scans known sibling
   checkouts (or a configured list) for papercuts tagged that way and
   surfaces them — closer to what actually happened for D-028, but done on
   demand instead of by luck.
3. **Accept the silo, but make discovery routine**: add a periodic/CI step
   in AKR's own repo that greps sibling project checkouts' `PAPERCUTS.md`
   (or `.akr/records/*/papercuts.akr`) for mentions of `AKR-*` diagnostic
   codes or the string "akr" in the statement, surfacing candidates for
   triage the way `docs/generated/PAPERCUTS.md` already surfaces in-repo
   ones. Cheap, no protocol change, but coupled to a specific machine's
   directory layout (as this whole investigation is).
4. **Do nothing differently, but require citing precedent**: when a
   cross-project papercut like D-028's *is* found, treat it the way D-028
   did — cite it by full key from the AKR-side decision/fix, so at least the
   provenance trail is honest about where the finding actually came from,
   even without automated discovery.

None of these were implemented or started. This document only records what
is true today and how the one confirmed precedent (`bpg-rs` → D-028) actually
played out.
