# AKR Design Decisions

This file records every question the planning notes left open or answered
inconsistently, together with the resolution the specification set is built on. Each
entry is normative. Specification documents implement these decisions; they do not
re-open them.

**Frozen.** Nothing in this file changes as a side effect of writing a specification
document. If a specification document cannot be written consistently with a decision
here, the correct move is to report the conflict and amend this file deliberately, in
its own commit, updating every document listed under **Honored by**.

Companion frozen artifacts: `spec/tables/vocabulary.json` (machine-readable form of
D-001, D-002, D-004, D-010, D-012, D-016), `spec/exemplar.akr` (the only quotable
source of syntax forms), `examples/save-your-skin/MANIFEST.md` (the worked example
inventory), `spec/diagnostics/README.md` (D-013).

---

## D-001 — The record kind vocabulary is exactly twelve kinds

**Question.** The first planning draft listed six kinds (requirement, decision,
observation, evidence, plan, question); the second listed twelve, dropping `plan` and
adding `term`, `policy`, `constraint`, `assessment`, `milestone`, `work`, `track`.
Which list is canonical?

**Resolution.** Twelve kinds, and no more before the first dogfood completes:

`term`, `requirement`, `policy`, `constraint`, `decision`, `observation`, `evidence`,
`assessment`, `milestone`, `work`, `track`, `question`.

`plan` is **not** a kind. A plan is a `work` record designated as the plan of record
for a milestone or track through the `plan_of_record` relation. The relation carries
the meaning that a separate kind would have carried, and it is checkable (D-004,
V-018).

**Rationale.** A plan differs from other work only in its authority over a milestone,
which is a relational fact. Encoding it as a kind would have produced two ways to say
the same thing and a second head-resolution rule. Keeping the vocabulary small is a
stated goal; the burden of proof is on additions.

**Honored by.** `docs/02-data-model.md`, `spec/tables/vocabulary.json`,
`docs/09-context-assembly.md`, `docs/11-projections.md`, all fixtures and examples.

---

## D-002 — Kinds are grouped into four classes, and the classes carry the rules

**Question.** Lifecycles, validation rules, and context ordering were specified
kind-by-kind, producing twelve near-duplicate rule sets.

**Resolution.** Every kind belongs to exactly one class:

| Class | Kinds | What the class means |
| --- | --- | --- |
| **normative** | `term`, `requirement`, `policy`, `constraint`, `decision` | States what *ought* to be true; binds future work |
| **empirical** | `observation`, `evidence`, `assessment` | States what *was found* to be true, at a stated point in history |
| **planning** | `milestone`, `work`, `track` | States what is *intended*, in what order, and when it is done |
| **inquiry** | `question` | States what is *not yet known* |

Lifecycle state sets, state-transition graphs, relation domains, staleness behaviour,
and context-assembly ordering are defined per class. Kind-specific rules exist only
where a kind genuinely differs (for example, `observation` requires `observed_at`).

**Rationale.** Four state machines instead of twelve. New kinds, if any are ever
added, join a class and inherit its rules rather than inventing a fifth.

**Honored by.** `docs/02-data-model.md`, `docs/05-validation-rules.md`,
`docs/09-context-assembly.md`, `spec/tables/vocabulary.json`.

---

## D-003 — `needs-review` is derived, never authored

**Question.** The planning notes listed `needs-review` as a lifecycle state for
observations and evidence, and separately said git integration "marks records
needs-review when watched paths change". Those two statements together require the
build to write source files.

**Resolution.** `needs-review` is **not** a lifecycle state. Authored empirical
states are `verified`, `disproven`, `superseded`, `withdrawn`. Staleness is a
*derived* property of the pair (record, current commit), computed in resolve
(stage D), materialised in the index, and surfaced by `akr review-queue` and
`REVIEW-REQUIRED.md`. Nothing in `akr build` ever writes a `.akr` source file.

A human or agent who acts on the review queue does so with an explicit write command
(`akr revise`, `akr evidence add`, `akr supersede`), which is a separate operation
from the build.

**Rationale.** The build must be a pure function of (sources, commit, tool version);
that is what makes it reproducible, cacheable, and safe to run in CI. A build that
mutates its own inputs has neither property. It also preserves the stated invariant
that the system never auto-declares a record false — staleness is a question raised,
not an answer given.

**Honored by.** `docs/02-data-model.md`, `docs/06-compiler-pipeline.md`,
`docs/10-freshness-and-git.md`, `docs/11-projections.md`, `docs/07-cli.md`.

---

## D-004 — One live head per key; normative exclusivity is a separate, topic-based rule

**Question.** "For a normative record type, only ONE active head per key + overlapping
scope" conflates two different checks.

**Resolution.** Two rules.

*(a) Head rule, all kinds (V-012, `AKR-R001`).* For a given logical key, at most one
revision may be in a **live** state. Every other revision must be in a terminal state
(`superseded`, `rejected`, `withdrawn`, `abandoned`, `completed`,
`closed-without-resolution`, `disproven`, as applicable to its class). Two live
revisions of one key is a build failure, never a newest-wins tiebreak.

*(b) Exclusivity rule, normative kinds only (V-013, `AKR-R002`).* Normative records
may carry an optional `topic` identifier. Two live normative records that share a
`topic` and whose `scope` sets overlap (D-010) is a build failure. Records with no
`topic` are never in conflict by this rule.

**Rationale.** (a) is about identity and is universal. (b) is about governance and
needs an explicit, authored declaration of "these two speak to the same thing" —
inferring it from prose would require judgement the compiler does not have. `topic`
is opt-in, cheap to write, and mechanically decidable.

**Honored by.** `docs/02-data-model.md`, `docs/04-references-and-versioning.md`,
`docs/05-validation-rules.md`.

---

## D-005 — Identifier and namespace lexicon

**Question.** Character sets for keys, slot names, and namespaces were unspecified.

**Resolution.**

- **Key segment**: `[a-z][a-z0-9]*(-[a-z0-9]+)*` — lowercase ASCII, digits, internal
  hyphens.
- **Key**: two to eight segments joined by `.`, for example
  `lege.viewer.renderer-boundary`. The first segment is the **namespace**.
- **Slot and block names**: `[a-z][a-z0-9_]*` — lowercase ASCII snake_case.
- **Enum values and anchors**: key-segment form (hyphens, no underscores).
- Keys and slot names are therefore lexically distinguishable: keys never contain
  `_`, slot names never contain `-`.
- Namespaces must be declared in `.akr/project.akr`. A key whose first segment is not
  a declared namespace is an error (V-002, `AKR-L004`).
- No Unicode in identifiers. Prose may be any valid UTF-8.

**Rationale.** One shape per concept, no case rules to remember, no homoglyph or
normalisation questions in identity. Declared namespaces are the cheapest available
defence against silent typo-drift (`lege.` versus `ledge.`) creating a second
knowledge graph nobody notices.

**Honored by.** `docs/03-syntax.md`, `spec/grammar/akr.ebnf`,
`docs/04-references-and-versioning.md`, `fixtures/parse/`.

---

## D-006 — Comments: `#` to end of line, with defined attachment

**Question.** Comment syntax, and whether the canonical formatter preserves comments.

**Resolution.** `#` to end of line. No block comments, no doc-comment convention, no
nesting. Comments are preserved by the formatter with these attachment rules:

- A comment on its own line attaches as **leading trivia** to the next item (record,
  slot, block, or array element) in the same brace scope. Leading trivia is re-emitted
  above that item, at that item's indentation, in original order.
- A comment following a value on the same line attaches as **trailing trivia** to that
  item, and is re-emitted after exactly two spaces.
- Comments at the end of a brace scope with no following item attach as trailing
  trivia to the enclosing block.
- Blank lines inside a record body are not preserved; the formatter emits exactly one
  blank line between records and none within them, except that a leading-comment group
  is preceded by a blank line if it was in the input.

**Rationale.** Comments are how a record explains an edit to the next reader, so they
must survive `akr fmt`. Attachment must be total and deterministic or round-tripping
is not well defined. `#` needs no escape handling inside the rest of the grammar.

**Honored by.** `docs/03-syntax.md`, `spec/grammar/akr.ebnf`, `spec/exemplar.akr`,
`fixtures/format/`.

---

## D-007 — Strings and prose blocks

**Question.** Escaping, multi-line prose, and how indentation inside prose is
normalised.

**Resolution.** Two string forms.

*Quoted string* `"..."` — single line. Legal escapes are exactly `\"`, `\\`, `\n`,
`\t`, `\r`, and `\u{HHHH}` (1–6 hex digits, a Unicode scalar value). Any other
backslash sequence is an error (`AKR-P012`). Raw newlines are not permitted.

*Prose block* `"""..."""` — multi-line, **raw**: no escape sequences at all, so a
backslash is a backslash. Rules:

1. The opening `"""` must be followed by a newline; content starts on the next line.
2. The closing `"""` must be the only non-whitespace content on its line.
3. Trailing whitespace is stripped from every line.
4. The common leading-whitespace prefix of all non-blank lines is removed. Blank lines
   are treated as empty regardless of their whitespace.
5. Tabs inside the indentation prefix are an error (`AKR-P015`); prose is indented with
   spaces.
6. The result has no leading or trailing blank lines.

The formatter re-emits prose blocks indented one level deeper than the owning slot,
with the closing `"""` at that same indentation.

**Rationale.** Prose is the payload of most records and gets pasted, quoted, and
diffed constantly; escape processing in it would be a permanent source of surprise. A
single, fully specified dedent rule means the formatter is a fixed point and diffs
reflect meaning changes only.

**Honored by.** `docs/03-syntax.md`, `spec/grammar/akr.ebnf`, `fixtures/parse/`,
`fixtures/format/`.

---

## D-008 — Scalar literal forms

**Question.** How dates, timestamps, commits, globs, and numbers are written.

**Resolution.**

| Type | Form | Notes |
| --- | --- | --- |
| date | `2026-08-03` | Proleptic Gregorian, bare, unquoted |
| timestamp | `2026-08-03T14:00:00Z` | UTC only; the `Z` is mandatory; no offsets, no local time |
| commit | `git:` + exactly 40 lowercase hex digits | Abbreviations are rejected (`AKR-P021`) |
| glob | quoted string | Repo-root-relative, `/` separators, subset `*`, `**`, `?`, `[a-z0-9]`; no brace expansion, no `!` negation |
| integer | `0`, `42`, `-3` | No floats, no exponents, no underscores, no leading zeros |
| boolean | `true` / `false` | |
| enum | bare identifier in key-segment form | |

**Rationale.** Abbreviated hashes are not stable identity — they collide as history
grows, and resolving them would make parsing depend on repository state. Local time in
a ledger read by agents in unknown timezones is a bug generator. Floats have no use in
this vocabulary and invite formatting non-determinism.

**Honored by.** `docs/03-syntax.md`, `spec/grammar/akr.ebnf`,
`docs/10-freshness-and-git.md`, `spec/schema/akr-lock.md`.

---

## D-009 — Exactly four reference forms

**Question.** Reference syntax and which resolution modes exist.

**Resolution.** Four forms, no others:

| Form | Meaning |
| --- | --- |
| `@key` | Current head — resolves to whichever revision is live at build time |
| `@key/2` | Pinned — resolves to revision 2, always |
| `@key#anchor` | Current head, claim or check anchor |
| `@key/2#anchor` | Pinned revision, claim or check anchor |

No `@key/latest`, no revision ranges, no wildcards, no cross-project references in
0.1. Every current-head resolution performed by a build is written to `akr.lock`
(D-014), so a build is reproducible from (sources, lock) alone.

Guidance, not enforced: pin when citing evidence or narrating history; float when
referring to governing policy you intend to keep following.

**Rationale.** Two resolution modes are the minimum that supports both "follow the
current rule" and "this is what I relied on". Anything more expressive turns reference
resolution into a query language and the lock file into a query cache.

**Honored by.** `docs/03-syntax.md`, `docs/04-references-and-versioning.md`,
`spec/schema/akr-lock.md`, `docs/09-context-assembly.md`.

---

## D-010 — Scope is a set of terms with a conservative overlap test

**Question.** The notes wrote `scope @sys.goal.playable-day`, referring to a `goal`
kind that does not exist, and never defined what scope overlap means.

**Resolution.** `scope` is an array of **scope terms**. A term is one of:

- `all` — project-wide.
- `ref @key` — organisational scope. The target must be a `milestone`, `track`, or
  `constraint` (V-005).
- `path "glob"` — code scope, repo-root-relative.

Two scopes overlap if any term of one overlaps any term of the other:

- `all` overlaps everything.
- Two `ref` terms overlap if they are equal, or if one is reachable from the other
  through `part_of` edges.
- Two `path` terms overlap if their literal prefixes (the portion before the first
  wildcard) are prefix-comparable, treating `**` as matching any sequence of segments
  and `*`/`?` as matching within one segment.
- A `ref` term and a `path` term never overlap by themselves. A record that must be
  compared against both should declare both.

The test is deliberately **conservative**: it may report an overlap where none exists
in practice, and it must never miss one. False positives are resolved by narrowing
scope or removing a `topic`; false negatives would silently permit contradictory
governance.

**Rationale.** Overlap decides D-004(b), so it must be decidable, cheap, and stable
across implementations. Full glob-intersection is neither. There is no `goal` kind;
the example in the planning notes becomes `scope [ ref @sys.milestone.playable-day ]`.

**Honored by.** `docs/02-data-model.md`, `docs/05-validation-rules.md`,
`spec/tables/vocabulary.json`, `examples/save-your-skin/`.

---

## D-011 — Claims are versioned with their record; retirement is explicit

**Question.** Whether claim anchors are independently versioned, and what happens to
a reference to a claim that a later revision drops.

**Resolution.** A claim is a `claim <anchor> { ... }` block inside a record. Claims
are **not** independently versioned; a claim belongs to the revision that contains it,
and `@key/2#anchor` is a reference into revision 2.

Anchor ids are stable across revisions when the claim's meaning is unchanged; a
changed meaning requires a new anchor id. A revision that drops an anchor present in
the previous revision must list it in `retired_claims [ ... ]`. A current-head
reference to a retired anchor then produces the specific diagnostic "anchor retired at
revision N" (V-004, `AKR-L012`) rather than a generic "not found", and points the
reader at the revision that dropped it.

**Rationale.** Independently versioned claims would give a record two version numbers
and a partial-order problem. Explicit retirement costs one line at the moment of
authorship and buys a precise error at the moment of confusion, which is the trade the
whole design keeps making.

**Honored by.** `docs/02-data-model.md`, `docs/04-references-and-versioning.md`,
`docs/05-validation-rules.md`.

---

## D-012 — Slots, blocks, arrays, and enforced canonical ordering

**Question.** Whether slots may repeat, how arrays are written, and whether the
formatter reorders content.

**Resolution.**

- **Slots are unique** within a record or block. A repeated slot is an error
  (`AKR-P031`).
- **Blocks may repeat**: `claim`, `check`, `source`, and `disposition`. `acceptance`
  appears at most once.
- Multi-valued content uses arrays with **plural slot names** (`aliases`,
  `exceptions`, `watches`). Relation slots keep the relation name verbatim
  (`supported_by`, `depends_on`) and are always arrays, even with one element.
- Arrays are comma-separated inside `[ ]` with one space of padding. A trailing comma
  is accepted on input and removed by the formatter. An array is emitted on one line if
  it fits within 96 columns, otherwise one element per line.
- **The formatter enforces canonical ordering**, and this is not optional:
  1. `title`
  2. `state`
  3. `scope`
  4. `topic`
  5. kind-specific content slots, in the order given in `spec/tables/vocabulary.json`
  6. `claim` blocks, sorted by anchor id
  7. `retired_claims`
  8. `acceptance` block, with `check` blocks sorted by check id
  9. `disposition` blocks, sorted by their reference
  10. relation slots, alphabetical by relation name; refs within an array sorted by
      key, then by revision, then by anchor
  11. `author`, `created_at`
  12. `source` blocks, sorted by source kind then by path or url
- Indentation is four spaces per level. Files are UTF-8 without BOM, LF line endings,
  and end with exactly one newline.

**Rationale.** Canonical ordering is what makes a diff mean something: a reordered
record produces no diff, and a real change produces a small one. It also removes an
entire category of review comment. The cost — the tool moves your lines — is paid once.

**Honored by.** `docs/03-syntax.md`, `spec/exemplar.akr`, `fixtures/format/`,
`spec/tables/vocabulary.json`.

---

## D-013 — Diagnostic and rule identifier scheme

**Question.** How errors are numbered, who owns which numbers, and what severity
means.

**Resolution.** Diagnostics are `AKR-<stage><nnn>`, where stage is one letter:

| Letter | Stage | Registry |
| --- | --- | --- |
| `P` | parse | `spec/diagnostics/codes-lang.md` |
| `F` | format / canonicalisation | `spec/diagnostics/codes-lang.md` |
| `T` | type-check | `spec/diagnostics/codes-lang.md` |
| `L` | link | `spec/diagnostics/codes-lang.md` |
| `R` | resolve | `spec/diagnostics/codes-lang.md` |
| `I` | index | `spec/diagnostics/codes-runtime.md` |
| `E` | emit / projection | `spec/diagnostics/codes-runtime.md` |
| `X` | context assembly | `spec/diagnostics/codes-runtime.md` |
| `G` | git / freshness | `spec/diagnostics/codes-runtime.md` |
| `C` | cli / config | `spec/diagnostics/codes-runtime.md` |
| `M` | migration / import | `spec/diagnostics/codes-runtime.md` |

Every code is defined in exactly one registry, is cited by at least one specification
document, and carries: title, severity, message template, a minimal reproducing
source, and fix guidance. Codes are never renumbered or reused.

**Validation rules** are numbered `V-nnn` — a separate namespace from the stage letter
`R`, to avoid two meanings for one prefix. `V-001`–`V-099` are the language and graph
rules catalogued in `docs/05-validation-rules.md`. `V-101`–`V-149` are reserved for
freshness, emission, and context rules catalogued in `docs/10-freshness-and-git.md`,
`docs/11-projections.md`, and `docs/09-context-assembly.md`. A rule cites the code it
raises; a code names the rule that raises it.

**Severity.** `error` and `warning` only. The default profile is `--strict`, in which
warnings are errors and the build fails. `--lenient` downgrades warnings and exists
for exactly one purpose: `akr import` on legacy material (D-022).

**Rationale.** Stage-letter codes tell a reader where in the pipeline a failure
happened before they read the message. Strict-by-default is the only setting under
which a warning ever gets fixed.

**Honored by.** `spec/diagnostics/README.md`, both registries, every specification
document that cites a code.

---

## D-014 — `akr.lock` is written in AKR syntax and is committed

**Question.** Lock file format — TOML, JSON, or something else — and what it contains.

**Resolution.** `akr.lock` is written in the AKR grammar, with the header
`akr-lock 0.1` in place of `akr 0.1`. It is committed to the repository. It contains,
in a fixed order and sorted deterministically:

1. Tool version and grammar version.
2. The git commit the build resolved against.
3. The source-graph hash.
4. One entry per source file: path and content hash.
5. One entry per current-head reference resolved during the build: referring revision,
   referenced key, resolved revision, and that revision's content hash.
6. One entry per sealed revision: key, revision, content hash (D-015).

Hash definitions live in `spec/schema/akr-lock.md`: a revision content hash is
SHA-256 over the canonically formatted text of that record; the source-graph hash is
SHA-256 over the sorted list of (path, file hash) pairs.

**Rationale.** Reusing the grammar means one parser, one formatter, one determinism
story, and one set of diff-readability properties. A generated file that a reviewer
must read during supersession review should not be in a second syntax. TOML or JSON
would each add a dependency and a canonicalisation question already answered here.

**Honored by.** `docs/04-references-and-versioning.md`, `spec/schema/akr-lock.md`,
`docs/06-compiler-pipeline.md`, `docs/07-cli.md`.

---

## D-015 — Sealing gives revision immutability teeth

**Question.** "Accepted bodies are immutable; changes require a new revision" — how is
that enforced without a server?

**Resolution.** A revision in any non-`proposed` state is **sealed**. Its content hash
is recorded in `akr.lock`. `akr check` recomputes the hash of every sealed revision
and fails with `AKR-R051` ("sealed revision modified; create a new revision instead")
on mismatch, naming the key, the revision, and the expected hash. A revision whose
resolution is absent from an otherwise-current lock raises `AKR-R052`.

`proposed` revisions are not sealed and may be edited freely, which is what makes
`proposed` useful.

**Rationale.** The rule is enforced by the same commit-and-review machinery the
project already has: changing a sealed record shows up as a lock diff, which is
exactly the thing a reviewer should be looking at. No daemon, no signatures, no
central authority.

**Honored by.** `docs/04-references-and-versioning.md`, `docs/05-validation-rules.md`,
`spec/schema/akr-lock.md`.

---

## D-016 — Acceptance is a block of checks; evidence never points at what it verifies

**Question.** How acceptance criteria are expressed, and in which direction the
evidence link runs.

**Resolution.** `milestone` records require, and `work` records may carry, an
`acceptance` block containing one or more `check` blocks:

- A `check <id>` has a `statement` (prose), a `method`
  (`manual` | `command` | `observation`), an optional `command`, and a `verified_by`
  array of references to `evidence` records.
- A check is **satisfied** when at least one referenced evidence record has
  `result pass` and an `observed_at` commit that is a descendant of the last commit
  that changed the content of the work item's current revision.
- Completing a `milestone` or `work` record with an unsatisfied check is an error
  (V-020, `AKR-R022`).

The `verified_by` relation runs in exactly one direction: from the thing being
verified to the evidence. An `evidence` record never declares what it verifies. Its
own slots describe only the observation: `result`, `method`, `observed_at`, optional
`command` and `artifact`.

**Rationale.** A two-directional link is two sources of truth and a reconciliation
rule nobody wants to write. Putting `verified_by` on the check keeps acceptance
readable in one place — the milestone tells you what "done" means and what proved it —
and makes evidence records reusable across several checks. The descendant-commit
condition is what stops a passing test from 200 commits ago closing a milestone whose
definition changed yesterday.

**Honored by.** `docs/02-data-model.md`, `docs/05-validation-rules.md`,
`docs/10-freshness-and-git.md`, `docs/11-projections.md`.

---

## D-017 — Supersession must dispose of unfinished children

**Question.** Where disposition is recorded and what the outcomes are.

**Resolution.** The **superseding** record carries one `disposition` block per
unfinished child of the record it supersedes:

```
disposition @sys.work.m3-lighting-pass {
    outcome carried_forward
    into @sys.track.lighting
}
```

`outcome` is one of `carried_forward`, `completed_elsewhere`, `intentionally_dropped`,
`still_required_separately`. `into` is required for `carried_forward` and
`completed_elsewhere`, optional for `still_required_separately`, and forbidden for
`intentionally_dropped`. An optional `note` may explain the choice.

"Unfinished child" means any record in a live planning state related to the superseded
record by `part_of`. A superseding planning record that omits a disposition for any
such child fails with V-017, `AKR-R014`.

**Rationale.** This is the single most valuable check in the system. Dropped work
silently disappearing across a replan is the failure mode that makes long-running
agent projects untrustworthy. The cost is one block per unfinished item at exactly the
moment the author knows the answer.

**Honored by.** `docs/02-data-model.md`, `docs/04-references-and-versioning.md`,
`docs/05-validation-rules.md`, `examples/save-your-skin/`.

---

## D-018 — Files are containers; identity never comes from paths

**Question.** How records map onto files, and what `archive/` means.

**Resolution.** A `.akr` file may contain any number of records. Identity comes from
the key alone; nothing in the compiler derives meaning from a file's name or location
below `.akr/records/`.

Two conventions, one of them enforced:

- *Convention:* one file per namespace subtree and kind group, for example
  `.akr/records/sys/policies.akr`.
- *Enforced (V-003, `AKR-L006`):* every revision of one key lives in one file, so a
  key's whole history is reviewable in one place and one diff.

`.akr/archive/` holds files in which every record is in a terminal state. Archived
records still resolve, so historical references never break, but they are excluded
from ordinary context assembly and from every generated view except
`DECISION-HISTORY.md`. Moving a file to `archive/` is a filesystem operation with no
semantic effect beyond that exclusion; state is what makes a record terminal.

**Rationale.** Filename-as-identity is the failure the whole project exists to avoid.
The one-file-per-key rule is a review ergonomics rule, not a semantic one, and is
cheap to satisfy.

**Honored by.** `docs/04-references-and-versioning.md`,
`docs/05-validation-rules.md`, `docs/09-context-assembly.md`,
`examples/save-your-skin/MANIFEST.md`.

---

## D-019 — The SQLite index is a cache, and agents never read it

**Question.** Status of the generated index, and who may touch it.

**Resolution.** `.akr/cache/index.sqlite` is a rebuildable cache. It is gitignored. It
carries `schema_version` and `source_graph_hash` in a `meta` table; a mismatch in
either triggers a full rebuild. Deleting it is always safe.

Agents access knowledge only through the CLI or MCP surface. `AGENTS.md` says so
explicitly. All writes go through validated source-record operations that produce
canonically formatted `.akr` text; nothing writes to the index except `akr build`.

**Rationale.** The moment anything reads the cache directly, the cache becomes a
schema with compatibility obligations, and the ledger stops being the single source of
truth. Keeping the boundary absolute keeps the index free to change.

**Honored by.** `docs/06-compiler-pipeline.md`, `spec/schema/index.sql`,
`docs/08-mcp.md`, `docs/09-context-assembly.md`.

---

## D-020 — No language model participates in a build

**Question.** Where the LLM boundary sits.

**Resolution.** Stages A through F contain no model inference of any kind. A build is
a pure function of (source files, git commit, tool version) and is byte-identical
across machines.

Models are welcome on the other side of the boundary: drafting record bodies for human
or agent review, proposing imports from legacy documents, summarising a context bundle,
and ranking search results. They may never determine authority, head resolution,
scope overlap, cycle detection, staleness, acceptance, or supersession. Every one of
those has a defined algorithm in this specification set, and the algorithm is the
answer.

**Rationale.** The value proposition is that the ledger's mechanical claims are
trustworthy. A probabilistic step anywhere in the build destroys that for every claim
downstream of it, and the failures would be quiet.

**Honored by.** `docs/01-architecture.md`, `docs/06-compiler-pipeline.md`,
`docs/08-mcp.md`, `docs/12-migration.md`.

---

## D-021 — Language versioning before 1.0

**Question.** What `akr 0.1` versions, and what compatibility is promised.

**Resolution.** The file header versions the **grammar**, not the tool and not the
vocabulary. `spec/tables/vocabulary.json` carries its own `vocabulary_version`, which
moves independently.

Before 1.0: an unknown minor version is a warning (and therefore an error under the
default strict profile, fixable with `--lenient`); an unknown major version is a hard
error. No forward compatibility, no deprecation windows, and no migration tooling for
grammar changes are promised until AKR has been dogfooded on two or three real
projects. Breaking changes before 1.0 are expected and are handled by `akr fmt`
upgrades shipped with the tool.

**Rationale.** Promising stability before the design has met a second and third real
project is how a format acquires permanent mistakes.

**Honored by.** `docs/03-syntax.md`, `docs/13-implementation-roadmap.md`,
`spec/grammar/akr.ebnf`.

---

## D-022 — Migration adds no kinds

**Question.** The notes referred to "legacy-source records" without saying whether
that is a kind.

**Resolution.** It is not. Legacy provenance is a repeatable `source` block:

```
source {
    kind legacy
    path "docs/legacy/ROADMAP.md"
    excerpt """
        M3 — playable day. Ship the day loop.
        """
}
```

`kind` is `legacy`, `external`, or `internal`. Each legacy document being migrated
gets one tracking `work` record whose acceptance checks enumerate the disposition of
its durable claims. The legacy document is archived only when that work record reaches
`completed`, which by V-020 requires every check satisfied. `akr import --lenient` is
the only place warnings are downgraded, and everything it produces lands in `proposed`
state for review.

**Rationale.** Migration is a workflow, not a category of knowledge. A thirteenth kind
would outlive the migration it was created for. Reusing `work` plus acceptance means
migration progress shows up in `ACTIVE-WORK.md` like any other work.

**Honored by.** `docs/12-migration.md`, `docs/02-data-model.md`,
`docs/07-cli.md`, `examples/save-your-skin/`.

---

## D-023 — Contradictions are declared, with one inferred check

**Question.** Whether the compiler detects contradictions.

**Resolution.** Primarily, no — contradiction is **declared** with the `contradicts`
relation, which is treated as symmetric regardless of which side declares it. A
declared contradiction must be dispositioned: either resolved (one side reaches a
terminal state) or explicitly `acknowledged true` on the declaring record. An
undispositioned contradiction fails with V-023, `AKR-R041`.

The single inferred check is D-004(b): two live normative records sharing a `topic`
with overlapping scope.

Contradictions are **always** surfaced in `akr context`, including when one side has
been superseded, and they are never suppressed by relevance ranking.

**Rationale.** Detecting semantic contradiction in prose requires judgement, which
belongs to the human or the agent, not the compiler. What the compiler can do is
guarantee that a contradiction someone noticed is never quietly lost — which is the
part that actually goes wrong.

**Honored by.** `docs/02-data-model.md`, `docs/05-validation-rules.md`,
`docs/09-context-assembly.md`.

---

## D-024 — Staleness propagates along three relations and flags, never overwrites

**Question.** How far "this record is at risk" travels.

**Resolution.** A record is **stale** if it is empirical and either (a) a commit
reachable from HEAD but not from its `observed_at` commit touched a path matching one
of its `watches` globs, or (b) its `review_after` date has passed.

Staleness propagates from a stale record to its dependents along exactly three
relations, in the dependent direction: `supported_by`, `depends_on`, `derived_from`.
Propagation is transitive, cycle-safe, and unbounded in depth. Dependents are flagged
`at_risk`, with the propagation path recorded so a reader can see why.

Neither flag ever changes a record's state, its content, or the truth value of any
claim (D-003). The flags appear in `akr review-queue`, in `REVIEW-REQUIRED.md`, and
as warnings in a context bundle.

Staleness is a **build fact, not a diagnostic**: it never enters the `AKR-*` diagnostic
stream and never affects the exit status of `akr check` or `akr build`. A project with
stale knowledge still builds — that is the point, since building is how you find out.
Projects wanting a hard gate opt in with `akr check --review-clean`.

**Rationale.** Three relations are the ones that mean "my correctness rests on yours".
Propagating along `part_of` or `after` would flag half the project every time a file
changes, and a warning that always fires is not a warning.

**Honored by.** `docs/10-freshness-and-git.md`, `docs/09-context-assembly.md`,
`docs/11-projections.md`, `spec/schema/index.sql`.

---

## D-025 — Generated views are committed build outputs, and CI enforces it

**Question.** Whether generated Markdown lives in the repository, and how the
never-hand-edit rule is enforced.

**Resolution.** `akr build` writes views to `docs/generated/` and they are committed,
so that people and tools reading the repository on the web see current knowledge
without running anything. Every generated file opens with:

```
<!-- GENERATED BY AKR — DO NOT EDIT
     source-graph: sha256:<hash>
     commit: <40-hex>
     tool: akr <version>
-->
```

`akr check --views-current` rebuilds views in memory and compares; any difference,
whether a hand edit or a stale build, fails with an emission diagnostic. That check is
the CI gate, and it is what gives the `sys.policy.no-hand-edited-views` record actual
force rather than good intentions.

**Rationale.** Committing generated output is a real cost — merge conflicts on
regenerated files — paid for by the ledger being legible to every reader and tool that
will never install AKR. The banner plus the CI gate makes the cost bounded and the
rule self-enforcing.

**Honored by.** `docs/11-projections.md`, `docs/06-compiler-pipeline.md`,
`docs/07-cli.md`, `examples/save-your-skin/docs/generated/`.

---

## D-026 — Planning kinds carry an optional `note` slot

*Amendment, 2026-08-04. Lead decision, taken on the P6a report; see the rationale below
for what prompted it. Unlike D-001..D-025 this entry postdates the spine's freezing, and
it landed with its implementation rather than in a commit of its own.*

**Question.** `docs/07` §6 said `akr abandon --reason` "lands in a `note`". No kind had a
`note` slot — only `disposition` blocks did — so the reason had nowhere to go. The P6
implementation wrote it as a leading comment, which works and is unsatisfying.

**Resolution.** `work`, `milestone` and `track` gain an optional `note` prose slot:
free-form operator commentary, informational only, with **no validation consequence**. No
rule reads it, nothing is required to set it, and nothing fails if it is absent or
nonsense. Views render it for records in terminal states, so an abandonment reason
appears in `DECISION-HISTORY.md` and the work projections rather than sitting in a
comment nobody renders.

`akr abandon --reason` writes it. Other operations may set it through an ordinary edit.

In canonical order it is the **last content slot of its kind** — `intent`, `target`,
`note` for milestones and work; `intent`, `cadence`, `note` for tracks — which puts it at
the end of the content group, immediately before claims and acceptance. That is as close
to the metadata group as a kind-specific slot can sit without inventing a new ordering
rank in D-012, and it reads correctly: the commentary comes after the thing commented on.

**Rationale.** A comment was the wrong home for two reasons. It is excluded from the seal
hash by D-015 — which is right for commentary and wrong for a reason somebody will later
need — and it is invisible to every generated view, so the operator who abandons a plan
on Tuesday leaves nothing the Thursday reader of `ACTIVE-WORK.md` can see. An
abandonment reason is durable knowledge and deserves a rendered slot.

Scoping it to the planning kinds is deliberate. Normative and empirical records already
have a place for every kind of prose they should carry — `rationale`, `context`,
`consequences`, `summary` — and a general-purpose commentary slot on them would become
the metadata bag `docs/02` §12 refuses to have. Planning records are the ones that get
abandoned, carried forward and re-scheduled by operators mid-flight, and that is the
commentary this slot is for.

**Honored by.** `spec/tables/vocabulary.json`, `docs/02-data-model.md` §4.9–§4.11,
`crates/akr-core/src/model/kind.rs`, `crates/akr-core/src/ops`, `docs/07-cli.md` §6
(Writer B, P6c), `docs/11-projections.md`.

---

## D-027 — A `papercut` kind, logged in the moment, with its own generated view

*Amendment, 2026-08-05. Like D-026 this entry postdates the spine's freezing and lands
with its implementation. It consciously extends D-001's closed set of twelve kinds to
thirteen; D-022 ("migration adds no kinds") is untouched — this kind comes from a
recorded decision, not from an import.*

**Question.** Agents hit small frictions while working — a tool call that missed and had
to be retried, a confusing setup step, a flaky command, a stale cache, a misleading
error, a non-obvious gotcha. None of them blocks; none of them is worth a work item; all
of them are worth knowing in aggregate, because logged together they show where the
project needs sanding down. Where do they go? Scratch is discarded, an `observation`
carries watch/staleness ceremony the moment does not want, and a Markdown file at the
repository root would be exactly the untyped pile AKR exists to replace.

**Resolution.** A thirteenth kind, `papercut`, in the **empirical** class: it records
what was found to be true at a stated point in history, which is precisely what a
friction report is. Two content slots, both filled automatically by the tooling:
`statement` (required prose — what you were doing, what got in the way, and a guess at
the cause or fix as a bonus) and `observed_at` (required commit, defaulted to HEAD). The
agent that hit it goes in the common `author` slot; the date in `created_at`. No
`watches`, so a papercut never goes stale and never enters the review queue; no
relations are required, so logging one is a single call.

The write surface is `akr papercut -m <agent> "message"` and the `knowledge.papercut`
MCP tool. Both allocate the key (`<namespace>.papercut.<slug-of-message>`), fill every
slot, and run the ordinary write pipeline — a papercut is a first-class record that
happens to cost one line to create.

The aggregate lives in a seventh generated view, `PAPERCUTS.md`, newest first, emitted
only once the ledger contains at least one papercut — a project that never logs one
never grows the file.

**Rationale.** The alternative of a free-form `PAPERCUTS.md` at the repository root was
rejected because it recreates the prose pile: no author an agent can trust, no commit,
no dedup handle, invisible to `akr search` and to the index. Making the record typed
costs nothing at the call site — the tooling fills every slot — and buys search,
provenance, and the one thing a papercut log is for: a reviewable aggregate.

Logging is proactive and in the moment. Mining a whole session for papercuts afterwards
is a language-model act, so it lives outside the tool (a harness command that reads the
transcript and calls `akr papercut` per finding), user-triggered, never in stages A–F
(D-020).

**Honored by.** `spec/tables/vocabulary.json`, `crates/akr-core/src/model/kind.rs`,
`crates/akr-core/src/papercut`, `crates/akr-core/src/render`, `docs/07-cli.md` §6,
`docs/08-mcp.md`, `docs/11-projections.md`.

---

## D-028 — Legacy-sourced completion is exempt from the descendant-commit gate

*Amendment, 2026-08-05.*

**Question.** D-016 / V-020 requires a `completed` record's acceptance evidence to have
an `observed_at` commit that descends from the last commit that changed the record's
content — the condition that stops a test from 200 commits ago closing a milestone
redefined yesterday. A historical port authors the record today, citing genuinely old
evidence commits from before the port existed: the record's own introduction to this
repository is necessarily the *newest* commit touching it, so its evidence can never
descend from it. That is not a data error to be fixed by re-running the check; it is
structurally impossible for a transcription of history to satisfy. Live case: `bpg-rs`'s
ledger carries 19 `AKR-R022` at HEAD for exactly this reason (`bpg.papercut.v-020-s-
descendant-commit-freshness-gate-akr/1`).

**Resolution.** When a `completed` record carries at least one `source { kind legacy
... }` block, the descendant-commit comparison of D-016 / V-020 is waived for its
acceptance evidence. Nothing else is: the cited reference must still resolve, the
evidence must still record `result pass`, and — whenever git facts are available at all
— its `observed_at` commit must still be one the repository actually has. Only the
comparison between that commit and the record's last content change is skipped. A record
with no `legacy` source keeps the full gate, unchanged.

The same exemption applies to `docs/11-projections.md`'s acceptance-verdict computation
(`akr-core::resolve::citation_facts`), which mirrors V-020's selection so that a rendered
view and the diagnostic it corresponds to never disagree about why a check is or is not
satisfied.

**Rationale.** A legacy-sourced record is a transcription of history: its git
introduction date says when it was *ported*, not when the work it describes happened.
Gating on descendancy from that introduction date would make every legacy port permanently
`AKR-R022`, forever, regardless of how solid its cited evidence is — a false alarm with no
action that clears it. The evidence commits are still the real, checkable claim about
when the work happened, so they remain required, must resolve, must pass, and must be
commits the repository can find: only the comparison that is structurally impossible for
a port to satisfy is waived.

**Honored by.** `crates/akr-core/src/validate/rules.rs` (`v020_acceptance_satisfied`,
`descends`), `crates/akr-core/src/resolve/mod.rs` (`citation_facts`),
`docs/05-validation-rules.md` (V-020), `docs/10-freshness-and-git.md` (the descendant
rule), `crates/akr-core/tests/v_rules.rs`.

## D-029 — The descendant gate measures the last *definitional* change, not the last transition

*Amendment, 2026-08-05.*

**Question.** D-016 / V-020 gates a `completed` record's acceptance evidence on descending
from "the last commit that changed the record's content." D-028 waived that comparison for
legacy ports, but the same wording bites ordinary, non-legacy work once the ledger is
committed. `akr complete` writes the record: it sets `state` to `completed` and adds a
`verified_by` to each satisfied check. Committing that completion is therefore, by the
literal reading, the record's *newest* content change — and the evidence, created before
the completion, can never descend from it. Every committed non-legacy milestone completion
would fail with `AKR-R022`, proven end to end: define a milestone, add passing evidence,
`akr complete` and commit, and `akr check` reports "evidence predates the last content
change." That defeats the verb the gate exists to serve.

**Resolution.** "Content change" in D-016 means a change to what the record *requires* —
its definition — not to its lifecycle bookkeeping. The commit a record's evidence must
descend from is the last commit that changed the record's **definitional** text: the
canonical record with the `state` slot, every acceptance-check `verified_by`, and the
D-026 `note` removed. `crates/akr-core/src/git/last_change_of` hashes that projection
(`resolve::definitional_record_text`) instead of the full canonical text, so a completion,
an abandonment, or a later note does not move `last_change`, while any change to `intent`,
a check's `statement`/`method`/`command`, `target`, or any other definitional slot still
does. The D-015 seal is untouched: it keeps hashing the whole record, because a seal
attests the literal bytes, not the definition.

D-028 stands and is still needed: a legacy port's *definition* is authored at the port
commit, so its older evidence still cannot descend and still relies on the legacy waiver.
D-029 narrows what counts as a definitional change; D-028 waives the comparison for
transcriptions of history. They are complementary.

**Rationale.** The gate's stated purpose is to stop a test from 200 commits ago closing a
milestone *redefined* yesterday. A state transition or an evidence citation is not a
redefinition, so counting it made the rule stricter than its purpose to the point of
forbidding the normal completion path. Hashing the definitional projection restores the
intended meaning without weakening it: real redefinitions still move the gate.

**Honored by.** `crates/akr-core/src/resolve/source.rs` (`definitional_record_text`),
`crates/akr-core/src/git/mod.rs` (`last_change_of`, `hash_at`),
`docs/05-validation-rules.md` (V-020), `docs/10-freshness-and-git.md` (the descendant
rule), `crates/akr-core/tests/git_queries.rs`.
