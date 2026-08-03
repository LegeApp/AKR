# `save-your-skin` — the worked example

A complete AKR ledger for a small game project, at a size that fits in one reading.
Forty records across forty-two revisions, three namespaces, five milestones, three
standing tracks, two stale observations and one acknowledged contradiction.

Everything here is derived from [`MANIFEST.md`](MANIFEST.md), which is frozen. The
manifest fixes the record inventory (§5), the synthetic git history (§4), the acceptance
map (§6), the freshness expectations (§7) and the exact tool outcomes (§9). Nothing in
this directory may disagree with it, and `tools/check-design.py` checks that nothing
does.

## The project

Two components that must advance together: a deterministic **engine simulator** (`sim`)
and a **viewer** (`lege`) that renders the simulation without depending on engine types.
Project-wide knowledge lives under `sys`.

| Namespace | Meaning |
| --- | --- |
| `sys` | Project-wide: policies, constraints, milestones, tracks, cross-cutting work |
| `sim` | The engine simulator |
| `lege` | The viewer and its renderer |

## What is here

```
MANIFEST.md              frozen inventory, history, and expected outcomes
README.md                this file
.akr/
    project.akr          namespaces and defaults
    akr.lock             head resolutions and content hashes
    records/             the ledger, by namespace and kind group
    archive/             terminal records that still resolve
docs/generated/          the six views, exactly as `akr build` emits them
transcripts/             expected output of four commands
```

## The synthetic history

The example repository's history is **fictional** (`MANIFEST.md` §4). This directory
lives inside the AKR design repository, whose real history is unrelated and must never be
used to reason about staleness here. Five commits, C1 through C5, with C5 as `HEAD`;
"today" is 2026-08-03 everywhere in this design set.

| Id | Commit | Touched |
| --- | --- | --- |
| C1 | `3f0a1c9d…` | `sim/src/**`, `lege/src/**` |
| C2 | `7c41d0ba…` | `sim/src/project/**`, `sim/src/step.rs` |
| C3 | `b2e58f14…` | `lege/src/**`, `sim/src/step.rs` |
| C4 | `5d9c2a70…` | `sim/src/project/**`, `sim/tests/determinism.rs` |
| C5 | `e806b3f5…` | `lege/src/render/**`, `docs/generated/**` |

## What each feature is demonstrated by

### Supersession, and what happens to the unfinished parts (D-017)

`sys.work.m3-plan` has two revisions. Revision 1 sequenced the sim step first; revision 2
put the renderer boundary first. Two work items were left under revision 1, and revision 2
carries a `disposition` block for each — `carried_forward` into `@sys.track.lighting` for
the lighting pass, `intentionally_dropped` for ambient audio, each with a note saying why.

This is the single most valuable check in the system. Without it, the lighting pass simply
stops being mentioned and reappears as a surprise in November.

Two details worth noticing:

- **`sys.work.m3-audio-pass` is still `ready`.** It has been dispositioned
  `intentionally_dropped` and not yet abandoned, because the disposition records the
  decision and abandoning the record is a separate write. The example deliberately shows
  the ledger between those two steps, which is the state a reviewer actually encounters.
- **Both children still say `part_of [ @sys.work.m3-plan/1 ]`**, pinned to the superseded
  revision. `part_of` pins to a plan revision so that "the children of revision 1" is a
  well-defined set at the moment revision 1 is superseded. They remain reachable and are
  shown in `akr context` and `ACTIVE-WORK.md` with the disposition that governs them.

Where to look: `.akr/records/sys/work.akr`,
[`docs/generated/ACTIVE-WORK.md`](docs/generated/ACTIVE-WORK.md#m3-plan-of-record).

### Acceptance under the descendant-commit rule (D-016)

`sys.milestone.m3-playable-day` has two checks. `full-day-demo` is satisfied:
`@sys.evidence.playable-day-demo/1` reports `pass` at C5, and C5 descends from C3, the
last commit that changed the milestone's content. `no-placeholder-assets` has no evidence
at all. **That is why M3 is `active` and not `completed`**, and `akr complete` refuses
with `AKR-R022`.

M1 and M2 show the satisfied case, each with one check closed by one evidence record.
Nowhere does an evidence record declare what it verifies; the `verified_by` edge runs one
way only, from the check to the evidence.

Where to look: `.akr/records/sys/milestones.akr`,
[`docs/generated/ROADMAP.md`](docs/generated/ROADMAP.md).

### Staleness, and the two ways it arises (D-024)

- **By watched path.** `sim.obs.projection-gaps/1` was observed at C2 and watches
  `sim/src/project/**`. C4 touched that path. Stale.
- **By review date.** `sim.obs.timestep-drift/1` carries `review_after 2026-07-15`, which
  has passed. Stale, even though the path it watches has not moved.

And one record that is deliberately **not** stale: `lege.obs.frame-budget-headroom/1`
watches `lege/src/render/**`, which C5 touched — but its `observed_at` *is* C5, so it
already accounts for the change.

### Reverse propagation

Two stale observations flag four dependents, along `supported_by` and `depends_on`:

```
       sim.obs.projection-gaps/1                sim.obs.timestep-drift/1
       STALE                                    STALE
         │                    │                        │
         │ depends_on         │ supported_by           │ supported_by
         ▼                    ▼                        ▼
sim.work.rewrite-     sys.assessment.          sys.assessment.
projection/1          projection-gaps/1        m3-readiness/1
AT RISK depth 1       AT RISK depth 1          AT RISK depth 1
                              │ supported_by
                              ▼
                      sys.policy.tandem-work/1
                      AT RISK depth 2
```

Two of those edges carry the argument for propagation, in opposite directions.

`sys.policy.tandem-work` is a live governance rule nobody has touched, and its support
rests on an assessment that rests on an observation made before the projection code
changed. No reader of the policy would have known.

`sim.work.rewrite-projection` is the work item somebody is about to pick up. It declares
`depends_on [ @sim.obs.projection-gaps ]` — the coverage measurement that motivated it —
and that measurement is stale, so the premise of the work is stale before anyone starts.
Note the kind: propagation is not restricted to records that make claims about the world.
Anything declaring that its correctness rests on something stale is flagged.

Where to look: [`docs/generated/REVIEW-REQUIRED.md`](docs/generated/REVIEW-REQUIRED.md),
[`transcripts/akr-review-queue.txt`](transcripts/akr-review-queue.txt).

### Declared contradiction (D-023)

`sim.obs.timestep-drift/1` declares `contradicts [ @sim.evidence.determinism-suite-pass/1 ]`
and `acknowledged true`. The suite reports byte-identical state across 512 seeds; the
observation reports a one-tick divergence after seven in-game days. Both stand, because
the suite runs 10 000 ticks and the divergence is outside what it covers.

The compiler did not detect this. Somebody declared it, and the compiler's job is to
guarantee that a contradiction somebody noticed is never quietly lost: it appears in every
context bundle that touches either side, and it is never suppressed by ranking or
budgeting.

### Archived records that still resolve (D-018)

`sys.policy.weekly-demo/1` is `withdrawn` and lives in
`.akr/archive/sys/policies-archived.akr`. It still parses, still resolves, and still
satisfies any historical reference. It is excluded from every generated view except
[`DECISION-HISTORY.md`](docs/generated/DECISION-HISTORY.md) and from every context bundle.

Its `topic demo-cadence` is now unclaimed, so a future policy may take it without tripping
V-013 — exclusivity is between live records only.

### Migration (D-022)

`sys.work.legacy-roadmap-import` is a `work` record with a `source { kind legacy }` block
and three acceptance checks, one per durable claim found in `docs/legacy/ROADMAP.md`. None
is verified yet, so the tracking record is `proposed` and the legacy file cannot be
archived. Migration adds no kinds; it is a workflow over the existing model.

Where to look: `.akr/records/sys/work.akr`,
[`../../docs/12-migration.md`](../../docs/12-migration.md) §7.

### The rest, in one table

| Mechanism | Where |
| --- | --- |
| All twelve kinds | The inventory uses every one |
| All four lifecycle classes | normative / empirical / planning / inquiry |
| Claim anchors and `retired_claims` | `sys.term.playable-day#day-boundary`; `lege.decision.renderer-boundary/2` retires `direct-calls` |
| All four reference forms | `sys/work.akr`, `sys/policies.akr`, `lege/decisions.akr` |
| Scope terms and overlap | `all`, `ref` and `path` terms across policies, constraints and tracks |
| `topic` exclusivity | `sys.policy.tandem-work` carries `topic tandem-work`; nothing else claims it |
| Blocking | `sim.question.timestep-vs-budget` blocks a decision and a work item |
| Cross-namespace relations | `lege.work.extract-render-graph` `part_of` a `sys` milestone |
| Two-tier head resolution | `after [ @sys.milestone.m2-deterministic-sim ]` resolves although M2 is completed |

## Expected tool outcomes

These match `MANIFEST.md` §9 exactly. Full output in
[`transcripts/`](transcripts/).

| Command | Outcome |
| --- | --- |
| [`akr check`](transcripts/akr-check.txt) | **Exits 0.** 40 records, 42 revisions, no diagnostics. The example is a valid ledger and every V-rule passes. |
| [`akr check --review-clean`](transcripts/akr-check.txt) | Exits 1 with `AKR-G041`. The opt-in gate, shown to make the difference visible. |
| [`akr check --views-current`](transcripts/akr-check.txt) | Exits 0. The six committed views match what the sources render. |
| [`akr review-queue`](transcripts/akr-review-queue.txt) | **2 stale, 4 at risk**, maximum depth 2. Exits 0. |
| [`akr context --goal sys.milestone.m3-playable-day --paths "sim/src/project/**"`](transcripts/akr-context.txt) | 23 records across 11 sections. Includes M3, `sys.work.m3-plan/2`, the in-scope policies and constraints, the blocked work item with its blocking question, both M3 checks with the satisfied one marked, `sim.obs.projection-gaps` with a staleness warning, and the acknowledged contradiction. **Excludes** `sys.work.m3-plan/1`, `lege.decision.renderer-boundary/1` and `sys.policy.weekly-demo`. |
| [`akr impact --git-diff 5d9c2a70..e806b3f5`](transcripts/akr-impact.txt) | **No newly stale records.** C5 touches only `lege/src/render/**` and `docs/generated/**`, and the one record watching the former already observes at C5. |
| `akr build` | Emits the six views in [`docs/generated/`](docs/generated/); a second run changes nothing. |

## Reading order

1. [`docs/generated/ROADMAP.md`](docs/generated/ROADMAP.md) — where the project is.
2. [`docs/generated/REVIEW-REQUIRED.md`](docs/generated/REVIEW-REQUIRED.md) — what should
   not be trusted, and why. The shortest route to understanding the freshness model.
3. [`transcripts/akr-context.txt`](transcripts/akr-context.txt) — what an agent actually
   receives. The point of the whole system is in this file.
4. `.akr/records/sys/work.akr` — supersession with disposition, in source form.
5. [`MANIFEST.md`](MANIFEST.md) — the contract everything above is checked against.
