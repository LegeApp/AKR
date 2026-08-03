# Worked Example — `save-your-skin`: Frozen Record Inventory

**Frozen.** This manifest is the contract between the `.akr` sources of the worked
example and everything generated from them. Writer A materialises exactly these records
into `.akr/`; Writer B renders views, transcripts and walkthroughs from exactly this
inventory. Neither may add, rename, or restate a record without amending this file
first.

`tools/check-design.py` enforces the agreement: every key here must appear exactly once
under `examples/save-your-skin/.akr/`, with the kind, revision count, head state and
file given below, and vice versa.

## 1. The project

`save-your-skin` is a small game project built by two components that must advance
together: a deterministic **engine simulator** (`sim`) and a **viewer** (`lege`) that
renders the simulation without depending on engine types. Its plan is five milestones
(M1–M5) plus three standing tracks. This is the dogfood target named in the planning
notes, encoded here at a size that fits in one reading.

Three namespaces:

| Namespace | Meaning |
| --- | --- |
| `sys` | Project-wide: goals, policies, constraints, milestones, tracks, cross-cutting work |
| `sim` | The engine simulator |
| `lege` | The viewer and its renderer |

## 2. `.akr/project.akr` (frozen)

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

## 3. File layout (frozen)

```
examples/save-your-skin/
    MANIFEST.md                        this file
    README.md                          walkthrough narrative            [Writer B]
    .akr/
        project.akr                                                     [Writer A]
        akr.lock                                                        [Writer A]
        records/
            sys/{terms,requirements,policies,constraints,decisions,
                 observations,assessments,evidence,questions,
                 milestones,tracks,work}.akr                            [Writer A]
            sim/{requirements,decisions,observations,evidence,
                 questions,work}.akr                                    [Writer A]
            lege/{terms,requirements,decisions,observations,evidence,
                  questions,work}.akr                                   [Writer A]
        archive/
            sys/policies-archived.akr                                   [Writer A]
    docs/generated/
        ROADMAP.md, CURRENT-STATE.md, ACTIVE-WORK.md,
        REVIEW-REQUIRED.md, OPEN-QUESTIONS.md, DECISION-HISTORY.md      [Writer B]
    transcripts/
        akr-check.txt, akr-context.txt, akr-impact.txt,
        akr-review-queue.txt                                            [Writer B]
```

`sys/observations.akr` is present but holds no records in this inventory; Writer A may
omit any file whose record set below is empty.

## 4. Synthetic git history (frozen)

The example repository's history is **fictional**. `.akr` sources here live in the AKR
design repository, whose real history is unrelated. Every document, transcript, and
fixture that reasons about commits, ancestry, or staleness uses the table below and
nothing else. `HEAD` is C5.

| Id | Commit | Parent | Touched paths |
| --- | --- | --- | --- |
| C1 | `git:3f0a1c9d5b7e2648a0d4f1b8c36e9752ad014b6f` | — | `sim/src/**`, `lege/src/**` (initial skeleton) |
| C2 | `git:7c41d0ba92e6f37518a3cd406b5e2f91d8074a63` | C1 | `sim/src/project/**`, `sim/src/step.rs` |
| C3 | `git:b2e58f1406c7a9d3e41b60258fa3d7c6195e0b48` | C2 | `lege/src/**`, `sim/src/step.rs` |
| C4 | `git:5d9c2a70e31f8b46c07d5924ab6e3f1074c9d285` | C3 | `sim/src/project/**`, `sim/tests/determinism.rs` |
| C5 | `git:e806b3f54a2d7091c5e13b8a26f490dc7b135e64` | C4 | `lege/src/render/**`, `docs/generated/**` |

"Today", for `review_after` evaluation everywhere in this design set, is **2026-08-03**.

Last content change per record, for the descendant-commit rule of D-016 (only the
records where it matters):

| Record | Last content change |
| --- | --- |
| `sys.milestone.m1-walking-skeleton/1` | C1 |
| `sys.milestone.m2-deterministic-sim/1` | C2 |
| `sys.milestone.m3-playable-day/1` | C3 |
| `sys.work.m3-plan/2` | C4 |

## 5. Record inventory (40 keys, 42 revisions)

| Key | Kind | Revs | Head state | File | Summary | Exercises |
| --- | --- | --- | --- | --- | --- | --- |
| `sys.term.playable-day` | term | 1 | active | `sys/terms.akr` | One in-game day, wake state to wake state, played end to end | Normative record, `scope [ all ]`, claim anchor `day-boundary` |
| `sys.term.tandem-work` | term | 1 | active | `sys/terms.akr` | Engine and simulator changes that land together | Term referenced by a policy `topic` |
| `lege.term.renderer-boundary` | term | 1 | active | `lege/terms.akr` | The line the viewer may not reach across | Cross-namespace reference target |
| `sys.req.deterministic-sim` | requirement | 1 | active | `sys/requirements.akr` | Same seed and inputs must produce the same run | `verified_by` on a non-planning kind |
| `sim.req.fixed-timestep` | requirement | 1 | active | `sim/requirements.akr` | The simulator advances on a fixed timestep | `depends_on` a constraint and a requirement |
| `lege.req.no-engine-types-in-viewer` | requirement | 1 | active | `lege/requirements.akr` | No engine type may appear in a viewer signature | Target of `implements` from a decision |
| `sys.policy.tandem-work` | policy | 1 | active | `sys/policies.akr` | Engine and simulator advance in tandem, with listed exceptions | `topic`, `exceptions`, `supported_by`; **at_risk** at depth 2 |
| `sys.policy.no-hand-edited-views` | policy | 1 | active | `sys/policies.akr` | Generated views are never edited by hand | The record D-025 gives force to |
| `sys.constraint.single-threaded-sim` | constraint | 1 | active | `sys/constraints.akr` | The simulator runs on one thread | Constraint as a `scope` ref target |
| `sys.constraint.frame-budget-16ms` | constraint | 1 | active | `sys/constraints.akr` | 16 ms frame budget at p99 | `measure` slot; `path` scope term |
| `lege.decision.renderer-boundary` | decision | 2 | active | `lege/decisions.akr` | How the viewer talks to the simulator (rev 2 replaces rev 1) | **Supersession**; `resolves` a question; `derived_from` a disproven observation |
| `sim.decision.timestep-4ms` | decision | 1 | proposed | `sim/decisions.akr` | Proposal to fix the timestep at 4 ms | `proposed` state; target of `blocks` from an open question |
| `sys.decision.view-generation` | decision | 1 | active | `sys/decisions.akr` | Views are generated and committed, never written by hand | `implements` a policy; cites evidence (V-021) |
| `sim.obs.projection-gaps` | observation | 1 | verified | `sim/observations.akr` | Projection coverage is thin at day boundaries | **STALE**: `watches` matched by C4, `observed_at` C2 |
| `sim.obs.timestep-drift` | observation | 1 | verified | `sim/observations.akr` | Long runs drift by one tick over an in-game week | **STALE**: `review_after` passed; **contradicts** evidence |
| `lege.obs.frame-budget-headroom` | observation | 1 | verified | `lege/observations.akr` | p99 frame time is 11.4 ms against a 16 ms budget | Fresh observation; `watches` not matched since `observed_at` |
| `lege.obs.viewer-imports-engine` | observation | 1 | disproven | `lege/observations.akr` | The viewer imported engine types (no longer true) | Terminal empirical record; excluded from context; legal `derived_from` target |
| `sys.assessment.projection-gaps` | assessment | 1 | verified | `sys/assessments.akr` | Projection gaps put the M3 date at risk | **at_risk** at depth 1 via `supported_by` |
| `sys.assessment.m3-readiness` | assessment | 1 | verified | `sys/assessments.akr` | M3 is one blocked work item away from ready | **at_risk** via `sim.obs.timestep-drift` |
| `sim.evidence.determinism-suite-pass` | evidence | 1 | verified | `sim/evidence.akr` | Determinism suite green at C4 | Satisfies the M2 check; target of a `contradicts` edge |
| `lege.evidence.boundary-lint-pass` | evidence | 1 | verified | `lege/evidence.akr` | Boundary lint reports no engine imports at C3 | Satisfies the M1 check; `command` and `artifact` slots |
| `sys.evidence.playable-day-demo` | evidence | 1 | verified | `sys/evidence.akr` | Recorded 41-minute full-day session at C5 | Satisfies one of M3's two checks |
| `sim.question.timestep-vs-budget` | question | 1 | open | `sim/questions.akr` | Does a 4 ms timestep fit the frame budget? | `open`; `blocks` a decision **and** a work item |
| `lege.question.text-rendering-owner` | question | 1 | resolved | `lege/questions.akr` | Which side owns text layout? | `resolved` + `resolution` slot + live `resolves` edge (V-011) |
| `sys.question.archive-legacy-docs` | question | 1 | deferred | `sys/questions.akr` | When do we archive the legacy roadmap? | `deferred` state |
| `sys.milestone.m1-walking-skeleton` | milestone | 1 | completed | `sys/milestones.akr` | Viewer and simulator boot with a clean boundary | Completed with acceptance satisfied (V-020 passing) |
| `sys.milestone.m2-deterministic-sim` | milestone | 1 | completed | `sys/milestones.akr` | The simulator is reproducible from a seed | Completed; `after` M1 |
| `sys.milestone.m3-playable-day` | milestone | 1 | active | `sys/milestones.akr` | One in-game day, playable start to finish | Two checks, one satisfied — why it is still `active` |
| `sys.milestone.m4-content-tools` | milestone | 1 | ready | `sys/milestones.akr` | Content authors can build a day without an engineer | `ready` state; unverified acceptance |
| `sys.milestone.m5-ship-demo` | milestone | 1 | proposed | `sys/milestones.akr` | A demo build a stranger can play | `proposed` state; end of the `after` chain |
| `sys.track.lighting` | track | 1 | active | `sys/tracks.akr` | Standing lighting work no milestone contains | Policy `exceptions` target; `disposition into` target |
| `sys.track.tooling-hygiene` | track | 1 | active | `sys/tracks.akr` | Keep the build and generated views honest | `cadence` slot |
| `sys.track.perf-watch` | track | 1 | active | `sys/tracks.akr` | Watch the frame budget across milestones | `scope` by `path` term |
| `sys.work.m3-plan` | work | 2 | active | `sys/work.akr` | Plan of record for M3 (rev 2 replaces rev 1) | **plan_of_record**, **supersession with two dispositions** |
| `sys.work.m3-lighting-pass` | work | 1 | ready | `sys/work.akr` | One lighting pass over the day-loop scenes | Dispositioned `carried_forward into @sys.track.lighting` |
| `sys.work.m3-audio-pass` | work | 1 | ready | `sys/work.akr` | Ambient audio for the day loop | Dispositioned `intentionally_dropped` |
| `sys.work.legacy-roadmap-import` | work | 1 | proposed | `sys/work.akr` | Import the legacy roadmap's durable claims | **Migration**: `source { kind legacy }` + per-claim acceptance checks |
| `sim.work.rewrite-projection` | work | 1 | blocked | `sim/work.akr` | Rewrite the projection pass | `blocked` state justified by a live `blocks` edge |
| `lege.work.extract-render-graph` | work | 1 | active | `lege/work.akr` | Extract the render graph behind the boundary | `part_of` a cross-namespace milestone |
| `sys.policy.weekly-demo` | policy | 1 | withdrawn | `archive/sys/policies-archived.akr` | Weekly demo build, abandoned as a practice | **Archived**: terminal record that still resolves |

### Multi-revision keys

| Key | Rev | State | Note |
| --- | --- | --- | --- |
| `lege.decision.renderer-boundary` | 1 | superseded | Viewer called the simulator directly |
| `lege.decision.renderer-boundary` | 2 | active | Viewer consumes a frame snapshot; `supersedes` rev 1; `resolves` `lege.question.text-rendering-owner`; `implements` `lege.req.no-engine-types-in-viewer`; `derived_from` `lege.obs.viewer-imports-engine/1` |
| `sys.work.m3-plan` | 1 | superseded | Sim first, then renderer, then lighting, then audio |
| `sys.work.m3-plan` | 2 | active | Renderer boundary first, then sim step, then the asset audit; carries both `disposition` blocks |

`sys.work.m3-audio-pass` deliberately remains `ready` while dispositioned
`intentionally_dropped`. The disposition records the decision; abandoning the record is
a separate write operation. The example shows the ledger between those two steps, which
is the state a reviewer actually encounters.

## 6. Acceptance and evidence map (frozen)

| Milestone | Check | Method | Verified by | Satisfied? |
| --- | --- | --- | --- | --- |
| M1 | `viewer-boundary-clean` | command | `@lege.evidence.boundary-lint-pass/1` | yes (C3 descends from C1) |
| M2 | `determinism-suite-green` | command | `@sim.evidence.determinism-suite-pass/1` | yes (C4 descends from C2) |
| M3 | `full-day-demo` | observation | `@sys.evidence.playable-day-demo/1` | yes (C5 descends from C3) |
| M3 | `no-placeholder-assets` | command | — | **no** — this is why M3 is `active` |
| M4 | `content-day-without-engineer` | manual | — | no |
| M5 | `stranger-can-play` | manual | — | no |

`sys.work.legacy-roadmap-import` carries three checks, one per durable claim found in
the legacy roadmap (`m3-scope-claim`, `lighting-standing-claim`,
`weekly-demo-claim`), none yet verified.

## 7. Freshness expectations (frozen)

Evaluated at HEAD = C5, today = 2026-08-03:

| Record | Status | Cause |
| --- | --- | --- |
| `sim.obs.projection-gaps/1` | stale | `watches ["sim/src/project/**"]` matched by C4; `observed_at` C2 |
| `sim.obs.timestep-drift/1` | stale | `review_after 2026-07-15` has passed |
| `sys.assessment.projection-gaps/1` | at_risk | `supported_by` → `sim.obs.projection-gaps` |
| `sys.assessment.m3-readiness/1` | at_risk | `supported_by` → `sim.obs.timestep-drift` |
| `sys.policy.tandem-work/1` | at_risk | `supported_by` → `sys.assessment.projection-gaps` → `sim.obs.projection-gaps` |
| `sim.work.rewrite-projection/1` | at_risk | `depends_on` → `sim.obs.projection-gaps` |

Two stale records, four at risk, propagation depth 2. Nothing else is flagged;
`lege.obs.viewer-imports-engine/1` is terminal and is not evaluated.

## 8. Feature coverage

| Mechanism | Demonstrated by |
| --- | --- |
| All twelve kinds | The inventory above uses every kind |
| All four lifecycle classes | normative / empirical / planning / inquiry rows |
| Supersession of a normative record | `lege.decision.renderer-boundary` 1 → 2 |
| Supersession with disposition (D-017) | `sys.work.m3-plan` 1 → 2, two `disposition` blocks |
| Claim anchors and `retired_claims` (D-011) | Anchors: `sys.term.playable-day#day-boundary`, `sys.policy.tandem-work`; `retired_claims`: `lege.decision.renderer-boundary/2` retires `direct-calls` from `/1` |
| All four reference forms (D-009) | Head, pinned, head+anchor, pinned+anchor across `sys/policies.akr` and `sys/work.akr` |
| Scope terms and overlap (D-010) | `all`, `ref`, and `path` terms across policies, constraints and tracks |
| `topic` exclusivity (D-004b) | `sys.policy.tandem-work` carries `topic tandem-work`; nothing else claims it |
| Acceptance and evidence (D-016) | M1, M2, M3 acceptance map above |
| Staleness by watched path (D-024a) | `sim.obs.projection-gaps` |
| Staleness by review date (D-024b) | `sim.obs.timestep-drift` |
| Reverse propagation (D-024) | The two at-risk assessments, the at-risk policy, and the at-risk work item |
| Declared contradiction (D-023) | `sim.obs.timestep-drift` contradicts `sim.evidence.determinism-suite-pass`, `acknowledged true` |
| Blocking (`blocks`) | `sim.question.timestep-vs-budget` blocks a decision and a work item |
| Archived terminal records (D-018) | `sys.policy.weekly-demo` |
| Legacy migration (D-022) | `sys.work.legacy-roadmap-import` with `source { kind legacy }` |
| Cross-namespace relations | `lege.work.extract-render-graph` `part_of` `sys.milestone.m3-playable-day` |

## 9. Expected tool outcomes (frozen)

Writer B's transcripts must agree with these:

- `akr check` — exits **0**. The example is a valid ledger; every V-rule passes.
- `akr review-queue` — 2 stale records, 4 at risk, as in section 7.
- `akr build` — emits six views; `ROADMAP.md` shows M1 and M2 complete, M3 active with
  one of two checks satisfied, M4 ready, M5 proposed, plus three standing tracks.
- `akr context --goal sys.milestone.m3-playable-day --paths "sim/src/project/**"` —
  returns M3, its plan of record `sys.work.m3-plan/2`, the live in-scope policies and
  constraints, the blocked work item with its blocking question, both M3 checks with
  the satisfied one marked, `sim.obs.projection-gaps` with a staleness warning, and the
  acknowledged contradiction. It must **not** return `sys.work.m3-plan/1`,
  `lege.decision.renderer-boundary/1`, or `sys.policy.weekly-demo`.
- `akr impact --git-diff C4..C5` — reports no newly stale records, since C5 touches only
  `lege/src/render/**`, which `lege.obs.frame-budget-headroom` already observes at C5.

## 10. Deliberate failure fixtures

The example project is valid by construction. Every failure mode is demonstrated
separately under `fixtures/validate/err/`, which Writer A owns, so that the worked
example never has to be a broken project to be instructive.
