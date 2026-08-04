You are helping migrate an assistant's planning output into AKR, a typed
project-knowledge ledger. Instead of prose, AKR stores typed records in a small
declarative language (`.akr`). Your job is to convert my raw notes directly into valid
`.akr` record source that I will drop into `.akr/records/<namespace>/`, then check with the
compiler. You are drafting for a human reviewer and for a validator; you decide nothing,
and everything you emit lands in a non-authoritative state.

Output **only** the `.akr` source — no preamble, no explanation, no fences.

## File shape

Begin the file with these two lines, then one `record` block per durable claim:

```
akr 0.1
project <PROJECT-NAME>
```

Replace `<PROJECT-NAME>` with the `project` name from my `.akr/project.akr`, and use one
of my declared namespaces (below) for every key.

- Declared namespace(s): `<NAMESPACE>`
- The document these notes came from: `<SOURCE-PATH>`

A record looks like this:

```
record <namespace>.<kind>.<slug>/1 : <kind> {
    title "One-line human label"
    state <state>
    scope [ all ]
    <required content slot> """
        The claim, in my words. Multi-line prose uses triple quotes.
        """
    source {
        kind legacy
        path "<SOURCE-PATH>"
        excerpt """
            The exact sentence(s) from my notes this record is a structured form of.
            """
    }
}
```

- **Keys** are `namespace.kind.slug`. Slugs are lowercase letters, digits and hyphens
  (`no-fraud-score`, `milestone-1-audit`); a digit may not start a segment.
- **`/1`** is the first revision. Everything you draft is revision 1.
- **`scope`** is required on normative kinds (term, requirement, policy, constraint,
  decision). Use `scope [ all ]` unless my notes point at specific code, in which case use
  `scope [ path "crate-or-dir/**" ]`.
- **`source { kind legacy … }`** goes on every record, so a reviewer can check your
  drafting against the original. The `excerpt` is copied verbatim from my notes — never
  paraphrased.

## Kinds you may use

| kind | required content slot | state to use | use it for |
| --- | --- | --- | --- |
| `decision` | `decision` | `proposed` | a choice made or recommended, with the reasoning |
| `requirement` | `statement` | `proposed` | something that must hold ("must", "shall") |
| `policy` | `rule` | `proposed` | a standing rule ("always", "never", "every") |
| `constraint` | `statement` | `proposed` | a hard limit or boundary the work must respect |
| `term` | `definition` | `proposed` | a word this project uses in a specific sense |
| `work` | `intent` | `proposed` | a unit of intended work; the safe default |
| `milestone` | `intent` | `proposed` | a named goal; add an `acceptance { … }` block if my notes list how it is judged |
| `track` | `intent` | `proposed` | standing work no milestone owns ("standing", "ongoing") |
| `question` | `question` | `open` | something not yet known (note: state is `open`, not `proposed`) |

Optional content slots you may add when my notes support them: `rationale` (requirement,
policy, constraint), `context` and `consequences` (decision), `target` as an ISO date
(milestone, work), `cadence` (track), `aliases` (term), `resolution` (question).

**Do not** emit `observation`, `evidence`, or `assessment`. Each needs a specific git
commit (`observed_at` / `as_of`) that you cannot know; leave any such note as a `work`
record whose intent says a person should re-observe it at HEAD.

## An acceptance block, when a milestone or work item lists how it is judged

```
    acceptance {
        check <slug> {
            statement """
                What must be true for this check to pass.
                """
            method manual
        }
    }
```

`method` is `manual`, `command`, `observation`, or `instrumented`. Use `manual` unless my
notes give an exact shell command, in which case use `method command` and a `command "…"`
line.

## Relations, only when my notes state them plainly

Add these as `slot [ @key, @key ]` lines; skip any you are unsure of.
`implements` (work/decision → requirement/policy/constraint/decision),
`part_of` (work → milestone/track/work), `depends_on`, `after` (work/milestone →
work/milestone), `blocks`, `resolves` (→ question). Point only at keys you also emit.

## Discipline

- **Durable only.** A claim earns a record if it is worth knowing in six months. Drop
  status, progress percentages, and notes to a person.
- **No invention.** No facts, dates, commits, or file paths my notes do not contain.
- **When unsure of the kind, use `work`** and say in the intent what it might become.

Now convert the following notes into `.akr` source:

<paste your assistant output here>
