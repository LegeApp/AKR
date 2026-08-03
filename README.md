# AKR — Agent Knowledge Records

A versioned project-knowledge ledger for AI-agent-driven software projects, with a
human-readable text serialization, a deterministic compiler, and generated views.

**Status: design only.** This repository currently contains specifications, worked
examples, and conformance fixtures. There is no implementation yet; the Rust
workspace lands in phase P1 (see `docs/13-implementation-roadmap.md`).

## The problem

A project that is worked by agents accumulates Markdown. The pile grows, and nothing
in it carries enforceable meaning about:

- **Authority** — is this a decision, a proposal, or somebody's note?
- **Scope** — what does it govern, and where does it stop applying?
- **Currency** — does it describe reality now, or reality as intended?
- **Evidence** — what observation supports this, and when was that observed?
- **Supersession** — what replaced it, and what happened to the unfinished parts?
- **Invalidation** — what change to the code would make it wrong?

Markdown cannot answer those questions mechanically, so agents re-derive context from
prose, trust stale statements, and re-litigate settled decisions. AKR replaces the pile
with a typed, versioned ledger that a compiler can check.

## The model

Three layers, with different trust and mutability rules:

| Layer | Contents | Canonical? | Written by |
| --- | --- | --- | --- |
| **Scratch** | Disposable agent working notes (`.agent/scratch/`) | No | Agents, freely |
| **Ledger** | Typed records in `.akr` source files | **Yes** | Humans and agents, via validated operations |
| **Views** | Generated Markdown/HTML (`docs/generated/`) | No | `akr build`, never by hand |

The unit is a **record**, not a document. Records have stable semantic keys
(`lege.viewer.renderer-boundary`), numbered revisions (`@key/2`), addressable claims
(`@key/2#renderer-boundary`), lifecycle states, declared scope, and typed relations with
mechanical consequences.

AKR is a compiler and build system for project knowledge, not a retrieval store. The
build is deterministic and contains no language model: parse, type-check, link, resolve,
index, emit. Language models may draft records, propose imports, and rank search
results; they never determine authority, head resolution, cycles, or acceptance.

## Document map

Frozen spine (do not edit without a recorded decision):

| Path | Contents |
| --- | --- |
| `docs/DECISIONS.md` | D-001..D-025 — every open question, resolved |
| `spec/tables/vocabulary.json` | Machine-readable spine: kinds, slots, lifecycles, relations, rules |
| `spec/exemplar.akr` | Frozen syntax specimen; the only source of quotable syntax forms |
| `examples/save-your-skin/MANIFEST.md` | Frozen record inventory and synthetic git history for the worked example |
| `spec/diagnostics/README.md` | Diagnostic code scheme and prefix ownership |

Specifications:

| Path | Contents | Status |
| --- | --- | --- |
| `docs/00-overview.md` | Problem, model, anti-goals | complete |
| `docs/01-architecture.md` | Layers, pipeline overview, trust and LLM boundary | complete |
| `docs/02-data-model.md` | Record kinds, slots, lifecycles, relations, scope, claims, acceptance | complete |
| `docs/03-syntax.md` | Lexical structure, grammar walkthrough, canonical formatting | complete |
| `docs/04-references-and-versioning.md` | Keys, revisions, heads, ref modes, supersession, lock semantics | complete |
| `docs/05-validation-rules.md` | Rule catalog V-001..V-024 with codes and examples | complete |
| `docs/06-compiler-pipeline.md` | Stage contracts A–F, hashing, incrementality | complete |
| `docs/07-cli.md` | Command reference, exit codes, JSON output | complete |
| `docs/08-mcp.md` | Agent tool surface and `AGENTS.md` protocol | complete |
| `docs/09-context-assembly.md` | Deterministic context assembly; search as ranking only | complete |
| `docs/10-freshness-and-git.md` | `observed_at`, watches, staleness, impact propagation | complete |
| `docs/11-projections.md` | Generated view catalog and rendering rules | complete |
| `docs/12-migration.md` | Legacy Markdown import and disposition workflow | complete |
| `docs/13-implementation-roadmap.md` | Phases P1–P9, crate layout, dogfood acceptance test | complete |
| `docs/14-glossary.md` | Terminology anchor | complete |
| `spec/grammar/akr.ebnf` | Formal grammar | complete |
| `spec/schema/akr-lock.md` | `akr.lock` format specification | complete |
| `spec/schema/index.sql` | SQLite index DDL sketch | complete |
| `spec/diagnostics/codes-lang.md` | `AKR-P/F/T/L/R` registry | complete |
| `spec/diagnostics/codes-runtime.md` | `AKR-I/E/X/G/C/M` registry | complete |
| `examples/save-your-skin/` | Worked example: `.akr` sources, lock, generated views, transcripts | complete |
| `fixtures/` | Parse, format, and validation conformance fixtures | complete |
| `tools/check-design.py` | Design-set coherence checker | complete |

## Anti-goals

No Markdown-plus-frontmatter. No generic wiki. No RDF authoring surface. No
newest-wins conflict resolution. No automatic deletion of knowledge. No line-number
citations as identity. Not every agent output is durable. And no published standard
before AKR has been dogfooded on two or three real projects.

## Licence

Not yet chosen.
