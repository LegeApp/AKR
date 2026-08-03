# AKR Syntax

This document defines the concrete syntax of `.akr` files: how the model in
`docs/02-data-model.md` is written down, and exactly what `akr fmt` guarantees. The
formal grammar is `spec/grammar/akr.ebnf`; every production there is cross-referenced to
a section here. The frozen specimen is `spec/exemplar.akr`, and it is the only source of
quotable syntax forms.

---

## 1. Principles

**Boring on purpose.** The syntax has no macros, no includes, no variables, no
interpolation, no loops, no conditionals, and no expressions. A `.akr` file means what
it says, and reading one requires no evaluation. Every feature that would let two files
say the same thing differently was left out.

**One way to write each thing.** There is a single canonical formatting, and `akr fmt`
produces it. Where a choice exists — array on one line or several, slot order, prose
indentation — the formatter decides, not the author. This is not a style preference; it
is what makes a diff mean something.

**Braces, not indentation.** Structure is explicit. Indentation is a formatting output,
never an input to parsing. A file with mangled whitespace parses identically and
reformats cleanly.

**Comments survive.** Comments are how a record explains an edit to the next reader, so
the formatter preserves them with defined attachment rules (§3.2).

**Diagnostics over cleverness.** Every construct that could be ambiguous is instead an
error with a code and a span. The grammar prefers to reject something than to guess.

---

## 2. Files and encoding

| Property | Rule |
| --- | --- |
| Encoding | UTF-8, no BOM. A BOM is `AKR-P002`. |
| Line endings | LF only. CR anywhere is `AKR-P003`. |
| Final newline | Required, exactly one. |
| Extension | `.akr` |
| Case | Identifiers are lowercase ASCII; the grammar has no case-insensitive construct. |

Prose content may be any valid UTF-8, including scripts other than Latin. Identifiers may
not, which keeps identity free of normalisation and homoglyph questions (D-005).

A file is one of three profiles, distinguished by its header:

| Header | Profile | Contents |
| --- | --- | --- |
| `akr 0.1` | record file | `project` line, then records |
| `akr 0.1` | project file | `project` line, `namespace` lines, `defaults` block |
| `akr-lock 0.1` | lock file | Generated resolution entries (`spec/schema/akr-lock.md`) |

All three use the same lexer, the same parser, and the same formatter.

---

## 3. Lexical structure

### 3.1 Whitespace

Spaces, tabs, and newlines separate tokens and are otherwise insignificant. Tabs are
legal between tokens and illegal inside prose-block indentation (§3.7). The formatter
emits only spaces and newlines.

### 3.2 Comments

`#` begins a comment that runs to the end of the line. There are no block comments and
no doc-comment convention (D-006).

Comments attach to items and are preserved:

- A comment on its own line attaches as **leading trivia** to the next item in the same
  brace scope. Re-emitted above that item, at that item's indentation, in order.
- A comment after a value on the same line attaches as **trailing trivia** to that item.
  Re-emitted after exactly two spaces.
- Comments at the end of a brace scope with no following item attach as trailing trivia
  to the enclosing block, re-emitted on their own lines before the closing brace.
- Comments before the file header attach to the file and are re-emitted at the top.

```
# Leading trivia for the record below.
record sys.policy.tandem-work/1 : policy {
    state active  # trailing trivia for the state slot
    # leading trivia for the rule slot
    rule """
        ...
        """
    # trailing trivia for the record body
}
```

Attachment is total: every comment in a well-formed file belongs to exactly one item, so
round-tripping is well defined.

### 3.3 Identifiers

Two shapes, lexically distinguishable so that no context is needed to tell them apart
(D-005):

| Shape | Pattern | Used for |
| --- | --- | --- |
| **Segment** | `[a-z][a-z0-9]*(-[a-z0-9]+)*` | Key segments, enum values, claim and check anchors, `topic` |
| **Name** | `[a-z][a-z0-9_]*` | Slot names, block names |

Segments use hyphens and never underscores. Names use underscores and never hyphens.
`observed_at` is a slot; `playable-day` is a key segment; neither can be mistaken for the
other.

### 3.4 Keys

A key is two to eight segments joined by `.`:

```
sys.policy.tandem-work
lege.viewer.renderer-boundary
```

The first segment is the **namespace** and must be declared in `project.akr` (V-002,
`AKR-L004`). One segment is too few to be meaningful; nine is a directory tree pretending
to be a name.

Keys are identity. They are never derived from filenames, never renamed, and never
reused after retirement (D-018, `docs/04` §1).

### 3.5 References

Four forms, no others (D-009):

```
@sys.policy.tandem-work                 current head
@sys.policy.tandem-work/2               pinned revision
@sys.policy.tandem-work#lag-bound       current head, claim or check anchor
@sys.policy.tandem-work/2#lag-bound     pinned revision, anchor
```

The `@` is part of the token; there is no bare-key reference. A revision number is a
positive integer with no leading zeros. An anchor is a segment identifier naming a
`claim` block in the target record, or a `check` block in the target's `acceptance`
block.

### 3.6 Quoted strings

`"..."`, single line. Legal escapes are exactly:

| Escape | Means |
| --- | --- |
| `\"` | quote |
| `\\` | backslash |
| `\n` | newline |
| `\t` | tab |
| `\r` | carriage return |
| `\u{H...}` | Unicode scalar, 1–6 hex digits |

Any other backslash sequence is `AKR-P012`. A raw newline inside a quoted string is
`AKR-P011` — use a prose block.

### 3.7 Prose blocks

`"""..."""`, multi-line, and **raw**: there are no escape sequences at all, so a
backslash is a backslash and a `"` is a quote (D-007). Rules:

1. The opening `"""` is followed by a newline; content starts on the next line.
2. The closing `"""` is the only non-whitespace content on its line.
3. Trailing whitespace is stripped from every line.
4. The common leading-whitespace prefix of all non-blank lines is removed. Blank lines
   count as empty regardless of what whitespace they contain.
5. A tab inside the indentation prefix is `AKR-P015`. Prose indents with spaces.
6. Leading and trailing blank lines are removed from the result.

```
    statement """
        Across the projection suite the least-covered paths cluster at the
        transition from one in-game day to the next.

        Steady-state paths are at 88 percent.
        """
```

parses to the two paragraphs with no leading indentation, one blank line between them.

Prose blocks may not contain `"""`. If you need one, you are quoting AKR source, and it
belongs in a document rather than a record.

### 3.8 Scalar literals

| Type | Form | Examples | Notes |
| --- | --- | --- | --- |
| date | `YYYY-MM-DD` | `2026-08-03` | Bare, unquoted. Proleptic Gregorian. Invalid dates are `AKR-P022`. |
| timestamp | `YYYY-MM-DDThh:mm:ssZ` | `2026-08-03T14:00:00Z` | UTC only. A missing or non-`Z` zone is `AKR-P023`. |
| commit | `git:` + 40 lowercase hex | `git:3f0a1c9d...4b6f` | Abbreviations are `AKR-P021`. |
| glob | quoted string | `"sim/src/**"` | Repo-root-relative, `/` separators. |
| integer | decimal, optional `-` | `0`, `42`, `-3` | No floats, no exponents, no leading zeros. |
| boolean | `true` / `false` | | |
| enum | segment identifier | `active`, `carried_forward` | Validated against the slot's value set at stage B. |

Enum values that look like they contain underscores — `carried_forward`,
`intentionally_dropped` — are the one place the two identifier shapes visibly meet.
They are **name**-shaped tokens used as enum values, because the alternative
(`carried-forward`) reads worse next to the slot names beside them. The grammar accepts
either shape in an enum position and the type checker matches against the declared set.

**Globs** use a deliberately small subset: `*` matches within one path segment, `**`
matches any run of segments, `?` matches one character, and `[a-z0-9]` matches a
character class. There is no brace expansion and no `!` negation. Globs are matched
against repo-root-relative paths with forward slashes on every platform.

**A note on integers.** No slot in vocabulary 0.1 has integer type. The literal exists in
the grammar because omitting it would guarantee a breaking grammar change the first time
a numeric slot is justified, and because a parser that rejects `42` with "unexpected
token" is a worse experience than one that rejects it with "slot `cadence` takes a
string". This is a knowingly unused production, not an oversight.

### 3.9 Reserved words

`akr`, `akr-lock`, `project`, `namespace`, `record`, `all`, `ref`, `path`, `true`,
`false`. They are reserved only in the positions where they are meaningful; a key segment
named `path` is legal, if unwise.

---

## 4. File structure

### 4.1 Record files

```
akr 0.1
project save-your-skin

record <key>/<revision> : <kind> {
    <body>
}

record ...
```

The header is the first non-comment line. The `project` line names the project and must
match `project.akr` (`AKR-L005`). Then zero or more records.

### 4.2 Record bodies

A body is a sequence of slots and blocks:

```
record sys.term.playable-day/1 : term {
    title "Playable day"
    state active
    scope [ all ]
    definition """
        One in-game day, wake state to wake state.
        """
    aliases [ "playable day", "day-loop build" ]
    claim day-boundary {
        text """
            A day boundary is the morning wake state, not midnight.
            """
    }
    author "dkoepke"
    created_at 2026-01-14
}
```

Slot names are validated against the kind at stage B (V-008): an unknown slot is
`AKR-T002`, a missing required slot is `AKR-T001`, a duplicate slot is `AKR-P031`.

### 4.3 Arrays

```
aliases [ "playable day", "day-loop build" ]
scope [ ref @sys.milestone.m3-playable-day, path "sim/**" ]
supported_by [ @sim.obs.projection-gaps ]
```

Comma-separated inside `[ ]` with one space of padding. A trailing comma is accepted on
input and removed by the formatter. An empty array `[ ]` is legal and means the same as
omitting the slot; the formatter removes it.

Array element types are homogeneous per slot and declared in
`spec/tables/vocabulary.json`.

### 4.4 Blocks

```
claim lag-bound { ... }              # head is an anchor identifier
check determinism-suite-green { ... } # head is a check identifier
acceptance { ... }                    # no head
source { ... }                        # no head
disposition @sys.work.m3-audio-pass { ... }   # head is a reference
```

A block head is an identifier, a reference, or a quoted string (the last only in lock
files). Blocks contain slots, and in the single case of `acceptance`, `check` blocks.
There is no other nesting.

### 4.5 Project files

`project.akr` uses the same header and grammar with different top-level items:

```
akr 0.1
project save-your-skin

namespace sys "Project-wide knowledge: policy, plan, milestones, tracks."
namespace sim "Engine simulator."
namespace lege "Viewer and renderer."

defaults {
    review_after_days 90
    view_output "docs/generated"
}
```

`namespace <segment> <string>` declares a namespace and describes it. Every key's first
segment must appear here. `defaults` is a block of project-wide settings; unknown keys
in it are an error rather than being ignored, so a typo in a setting name is caught
rather than silently doing nothing.

### 4.6 Plural naming for array slots

Content slots holding arrays are named in the plural: `aliases`, `watches`,
`exceptions`, `retired_claims`. Relation slots keep the relation's own name verbatim and
are always arrays even with one element: `supported_by`, `depends_on`, `part_of`.

The planning notes wrote `exception @sys.track.lighting` in the singular with a bare
value. The plural array form replaces it: one shape for one concept, and adding a second
exception never changes the slot's syntax.

---

## 5. Grammar walkthrough

`spec/grammar/akr.ebnf` is the normative grammar. Its shape:

```
file            = header , project_decl , { top_level_item } ;
header          = ( "akr" | "akr-lock" ) , version ;
top_level_item  = record | namespace_decl | defaults_block | lock_item ;
record          = "record" , revision_id , ":" , kind , body ;
revision_id     = key , "/" , integer ;
body            = "{" , { slot | block } , "}" ;
slot            = name , value ;
block           = name , [ block_head ] , "{" , { slot | block } , "}" ;
value           = scalar | reference | array | prose ;
```

Three properties are worth naming because they are what keep the language small:

**The grammar is LL(1) after the header.** Every construct is decided by its first
token: `record` starts a record, `namespace` a namespace declaration, `@` a reference,
`[` an array, `"""` a prose block. No backtracking, no lookahead tables, no ambiguity
that a precedence rule has to resolve.

**Slots and blocks are distinguished by the token after the name.** `{` means block,
anything else means slot. That is the entire disambiguation rule.

**Value types are not in the grammar.** The parser accepts any scalar in any slot; that a
`date` slot got a string is a stage-B type error with a good message, not a parse error
with a bad one. This is why `AKR-T*` codes exist and why the parse stage has so few of
them.

---

## 6. Canonical formatting

`akr fmt` rewrites a file into exactly one form. `akr fmt --check` verifies without
writing and is the CI gate (`AKR-F001`).

### 6.1 Layout

| Aspect | Rule |
| --- | --- |
| Indentation | 4 spaces per level. Never tabs. |
| Slots | One per line. |
| Braces | `{` on the same line as the name and head, `}` on its own line at the opening indentation. |
| Blank lines | Exactly one between top-level items; none within a record body, except that a leading-comment group keeps a preceding blank line if it had one. |
| Line width | Soft target 96 columns; only arrays and prose wrap. |
| Trailing whitespace | Removed. |

### 6.2 Record order within a file

Records are emitted **sorted by key, then by revision ascending**. A file is therefore
a stable index of its own contents, and adding a record does not move unrelated ones.
Since all revisions of a key live in one file (V-003), sorting also puts a key's history
in order wherever it appears.

### 6.3 Slot order within a record

Enforced, not merely suggested (D-012):

1. `title`
2. `state`
3. `scope`
4. `topic`
5. Kind-specific content slots, in the order declared in `spec/tables/vocabulary.json`
6. `claim` blocks, sorted by anchor
7. `retired_claims`
8. `acceptance`, with `check` blocks sorted by check id
9. `disposition` blocks, sorted by their reference
10. Relation slots, alphabetical by relation name
11. `acknowledged`
12. `author`, `created_at`
13. `source` blocks, sorted by `kind` then `path`/`url`

Within a relation array, references are sorted by key, then revision, then anchor. Within
a scope array, terms are sorted `all`, then `ref` terms by key, then `path` terms
lexically.

This is the rule people resist and then stop noticing. Its payoff is that a reordered
record produces no diff and a changed record produces a small one, so review attention
lands where the meaning changed.

### 6.4 Arrays

An array is emitted on one line if the whole slot fits in 96 columns:

```
    watches [ "sim/src/project/**" ]
```

Otherwise one element per line, indented one level, with the closing bracket at the slot
indentation:

```
    scope [
        ref @sys.milestone.m3-playable-day,
        path "sim/src/project/**",
        path "sim/src/step.rs"
    ]
```

No trailing comma in output, regardless of input.

### 6.5 Prose

Prose blocks are re-emitted with content indented one level deeper than the owning slot
and the closing `"""` at that same content indentation. Content is written back exactly
as parsed: no rewrapping, no reflowing, no trailing-space reintroduction. The formatter
does not have opinions about line length inside prose.

### 6.6 What the formatter never does

It never changes prose text, never reorders array elements whose order is meaningful
(there are none — all array slots are sets), never adds or removes records, never
resolves a reference, never touches a `.akr` file during `akr build`, and never rewrites
a file that already matches its canonical form (so `akr fmt` on a formatted tree is a
no-op and touches no mtimes).

---

## 7. Round-trip invariants

Three properties, all fixture-tested:

**Idempotence.** `format(format(s)) == format(s)`, byte for byte. Fixtures in
`fixtures/format/` include an already-canonical input for every construct.

**Semantic preservation.** `parse(format(parse(s))) == parse(s)`, where equality is AST
equality *including* comment trivia and its attachment. A formatter that dropped or
re-homed a comment would satisfy a weaker invariant and would be wrong.

**Parse totality on canonical output.** Any AST produced by the parser formats to text
that reparses to the same AST. There is no AST the formatter cannot print.

Not an invariant: `format(s) == s` for arbitrary input. The formatter reorders slots and
sorts records, so it is a normaliser, not a pretty-printer.

---

## 8. Rejected syntax

Each of these was considered and rejected. Recording why keeps them rejected.

**Markdown with YAML frontmatter.** The most obvious option and the one the project
exists to replace. Frontmatter carries no relations, no revisions, and no scope; the body
stays unstructured prose; and the tooling ecosystem encourages exactly the document-shaped
thinking that produces stale piles.

**Significant indentation.** Deletes a brace and adds a class of bug where a copy-paste
changes meaning. Braces make the formatter's job total and reformatting safe.

**JSON or YAML.** JSON has no comments and no multi-line strings, both of which are
load-bearing here. YAML has too many ways to write the same thing, and its type coercion
rules are a recurring source of production incidents. Neither reads well in a review
diff.

**RDF, Turtle, or PROV-N as the authoring surface.** PROV-N is cited in the planning
notes as precedent for human-readable formal provenance, and it is a good precedent. But
authoring in a triple language pushes the modelling burden onto every author, and the
model here is deliberately closed. An export mapping to PROV or RDF is a later,
mechanical addition; an import surface is not planned.

**A query or expression language in values.** `scope path("sim/**") and not
path("sim/tests/**")` reads well and turns scope resolution into an evaluator, overlap
into a satisfiability problem, and the formatter into a printer for an AST with
precedence. Scope stays a flat set of terms.

**Includes, macros, or templates.** They make identical-looking records mean different
things depending on context, and they make a record impossible to read in isolation.
Repetition across records is acceptable; a record that cannot be read alone is not.

**Bare-key references without `@`.** Ambiguous against enum values and unreadable in
prose-adjacent slots. One sigil, always.

---

## 9. Worked file

A complete, canonically formatted record file. This parses, formats to itself, and
passes `akr check` in the context of the worked example.

```
akr 0.1
project save-your-skin

# Both revisions of a key live in one file (V-003), sorted by revision.
record lege.decision.renderer-boundary/1 : decision {
    title "The viewer calls the simulator directly"
    state superseded
    scope [ path "lege/**" ]
    topic renderer-boundary
    decision """
        The viewer calls simulator entry points directly at each tick boundary.
        """
    context """
        Chosen for the walking skeleton, where there was no frame snapshot to read.
        """
}

record lege.decision.renderer-boundary/2 : decision {
    title "The viewer consumes a frame snapshot"
    state active
    scope [ path "lege/**" ]
    topic renderer-boundary
    decision """
        The viewer reads an immutable frame snapshot produced by the simulator at each
        tick boundary. It does not call into the simulator and does not name engine
        types in any signature.
        """
    context """
        Revision 1 put engine types in viewer signatures and made the viewer
        untestable in isolation.
        """
    consequences """
        One extra allocation and copy per frame, measured at 0.4 ms against the 16 ms
        budget.
        """
    claim no-engine-types {
        text """
            No engine type appears in any viewer function signature.
            """
        supported_by [ @lege.evidence.boundary-lint-pass ]
    }
    derived_from [ @lege.obs.viewer-imports-engine/1 ]
    implements [ @lege.req.no-engine-types-in-viewer ]
    resolves [ @lege.question.text-rendering-owner ]
    supersedes [ @lege.decision.renderer-boundary/1 ]
    author "dkoepke"
    created_at 2026-05-22
}
```

Read the canonical order off it: title, state, scope, topic, content slots in vocabulary
order, claim blocks, relations alphabetically (`derived_from`, `implements`, `resolves`,
`supersedes`), then metadata.

---

## 10. Version compatibility

The header versions the **grammar**, not the tool and not the vocabulary
(`spec/tables/vocabulary.json` carries its own `vocabulary_version`).

Before 1.0: an unknown minor version is a warning, and therefore an error under the
default strict profile, fixable with `--lenient`. An unknown major version is a hard
error. Breaking grammar changes are expected and are handled by `akr fmt` upgrades
shipped with the tool. No deprecation windows are promised until AKR has been dogfooded
on two or three real projects (D-021).

---

## 11. See also

- `spec/grammar/akr.ebnf` — the normative grammar.
- `spec/exemplar.akr` — the frozen specimen; every construct, canonically formatted.
- `docs/02-data-model.md` — what the slots mean.
- `docs/05-validation-rules.md` — what happens after parsing succeeds.
- `fixtures/parse/`, `fixtures/format/` — conformance fixtures for everything above.
