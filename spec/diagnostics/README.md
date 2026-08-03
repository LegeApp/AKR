# Diagnostics: Scheme and Ownership

**Frozen.** This file fixes how AKR numbers its diagnostics and who may mint which
numbers. It is the mechanism behind D-013. The codes themselves live in the two
registries named below; this file defines nothing but the scheme.

## 1. Code form

```
AKR-<stage-letter><nnn>
```

One uppercase letter naming the pipeline stage that raises the diagnostic, followed by
exactly three digits. `AKR-L012`, `AKR-R022`, `AKR-G004`. No other form is valid, and
the letter is never omitted: a reader who sees `AKR-R022` in a CI log knows the failure
came from resolve before reading a word of the message.

## 2. Stages and registry ownership

| Letter | Stage | Registry | Author |
| --- | --- | --- | --- |
| `P` | Parse — lexing, grammar, literal forms | `spec/diagnostics/codes-lang.md` | Writer A |
| `F` | Format — canonicalisation and `akr fmt --check` | `spec/diagnostics/codes-lang.md` | Writer A |
| `T` | Type-check — kind schema, required slots, enum values | `spec/diagnostics/codes-lang.md` | Writer A |
| `L` | Link — reference, anchor, namespace and kind-correctness resolution | `spec/diagnostics/codes-lang.md` | Writer A |
| `R` | Resolve — heads, graphs, acceptance, sealing, contradictions | `spec/diagnostics/codes-lang.md` | Writer A |
| `I` | Index — building the SQLite cache | `spec/diagnostics/codes-runtime.md` | Writer B |
| `E` | Emit — projections and view freshness | `spec/diagnostics/codes-runtime.md` | Writer B |
| `X` | Context — context assembly and search | `spec/diagnostics/codes-runtime.md` | Writer B |
| `G` | Git / freshness — watches, `observed_at`, impact | `spec/diagnostics/codes-runtime.md` | Writer B |
| `C` | CLI / config — invocation, `project.akr`, workspace layout | `spec/diagnostics/codes-runtime.md` | Writer B |
| `M` | Migration — `akr import` and legacy disposition | `spec/diagnostics/codes-runtime.md` | Writer B |

Every code is defined in **exactly one** registry. The two registries are disjoint by
stage letter, so they are never edited by both authors and never merge-conflict.

Lock-file integrity is deliberately a **resolve** concern, not a CLI one: sealing
(D-015) is checked while resolving heads, so `AKR-R051` and `AKR-R052` sit in the `R`
range with the rest of the graph checks. `C` codes cover only how the tool was invoked
and how the workspace is configured.

## 3. Severity

Two severities, and no third:

| Severity | Meaning |
| --- | --- |
| `error` | The ledger is not well formed, or a stated invariant is broken. The build produces no output. |
| `warning` | The ledger builds, but something is very likely wrong. |

The default profile is `--strict`, in which **warnings are errors** and the build fails.
`--lenient` downgrades warnings to their stated severity and exists for exactly one
purpose: `akr import` on legacy material (D-022). A warning that never fails a build is
a warning nobody fixes.

Staleness and at-risk flags are **not** diagnostics. They never carry an `AKR-*` code,
never enter the diagnostic stream, and never change an exit status (D-024). They are
build facts, reported by `akr review-queue`, `REVIEW-REQUIRED.md`, and context bundles.

## 4. Registry entry shape

Every code in either registry carries all six fields:

| Field | Content |
| --- | --- |
| Code | `AKR-L012` |
| Title | A noun phrase naming the fault, not the fix |
| Severity | `error` or `warning` |
| Rule | The `V-nnn` rule that raises it, or `—` for faults with no rule (most `P` codes) |
| Message | The template, with `{placeholders}` |
| Cause and fix | One or two sentences, plus a minimal reproducing source where the fault is not obvious from the message |

## 5. Rendered form

Diagnostics render as one block, in this shape, sorted by file then by span:

```
error[AKR-R001]: two live revisions of one key
  --> .akr/records/sys/work.akr:48:1
   |
48 | record sys.work.m3-plan/2 : work {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ this revision is active
   |
note: revision 1 is also active
  --> .akr/records/sys/work.akr:12:1
help: supersede revision 1, or withdraw it (see V-012)
```

Spans are byte offsets internally and are rendered as 1-based line and column. Every
diagnostic points at a span in a source file; none is emitted without one. `akr explain
AKR-R001` prints the registry entry.

## 6. Numbering conventions

- Numbers are grouped in tens by topic within a stage, leaving gaps: `AKR-L001`–`L009`
  reference resolution, `L011`–`L019` anchors, `L021`–`L029` terminal-record
  references, `L031`–`L039` kind correctness.
- Codes are **never renumbered and never reused**. A retired code stays in the registry
  marked retired, with a pointer to whatever replaced it. Codes appear in logs, commit
  messages, and agent transcripts; recycling one falsifies history.
- A code must be cited by at least one specification document, and a specification
  document must not cite a code that is not registered. `tools/check-design.py` enforces
  both directions.

## 7. Rule identifiers

Validation **rules** use a separate namespace, `V-nnn`, so that the rule prefix is never
confused with the resolve stage letter `R`:

| Range | Catalogue | Author |
| --- | --- | --- |
| `V-001`–`V-099` | `docs/05-validation-rules.md` — language and graph rules | Writer A |
| `V-101`–`V-149` | `docs/10-freshness-and-git.md`, `docs/11-projections.md`, `docs/09-context-assembly.md` — freshness, emission and context rules | Writer B |

A rule names the code it raises; a code names the rule that raises it. The frozen list
of `V-001`–`V-024` with their codes is in `spec/tables/vocabulary.json` under `rules`.
