# Worked Example — `sys-tandem`: Record Inventory

The second worked example, and the first encoded from a document somebody actually wrote.
The source is `legacy/2026-08-03-engine-simulator-tandem-roadmap.md`, committed verbatim:
270 lines orienting two halves of a game — a deterministic court simulation and a
presentation engine — around five milestones.

Unlike `examples/save-your-skin/MANIFEST.md`, this file is **not spine-frozen**. It
describes an encoding rather than fixing a contract, and it changes when the encoding
improves. `crates/akr-core/tests/example_sys_tandem.rs` is what holds it honest: every
count, state and freshness expectation below is asserted there.

## 1. What it is

| | |
| --- | --- |
| Project | `sys-tandem` |
| Source document | `legacy/2026-08-03-engine-simulator-tandem-roadmap.md` (2026-08-03) |
| Keys | **62** |
| Revisions | **65** |
| Kinds used | all twelve |
| Namespaces | `tandem`, `simulator`, `engine` |

Namespaces are deliberately disjoint from `save-your-skin`'s `sys` / `sim` / `lege`.
Projects are independent, so reuse would have been legal — D-005 scopes namespace
declarations to a project — but tooling that globs both examples at once, and
`tools/check-design.py` does, reads better when no key is ambiguous about which example it
came from.

The three namespaces are the document's own division: `tandem` for the roadmap's rulings,
plan and milestones; `simulator` for the backend court simulation; `engine` for the
presentation engine. §0 of the source exists precisely to fix that vocabulary, so the
namespaces are the first thing the encoding took from the document.

## 2. Synthetic git history

Fictional, as in the first example, and the only source for ancestry and staleness. HEAD
is C5. "Today" is **2026-08-04**, the day after the roadmap.

| Id | Commit | Parent | Date | Touched paths |
| --- | --- | --- | --- | --- |
| C1 | `git:a3f19d0c7b5e284610f8c2a94db3e0761fc58a29` | — | 2026-07-20 | `src/**`, `SYSEngine/**` (skeleton) |
| C2 | `git:4e82b6d1a09f37c5e28d4bf160a739c8b2e05d17` | C1 | 2026-07-31 | `SYSEngine/docs/**` |
| C3 | `git:9d40e1b7c85a326f0b1de94738ca5602f81b7d4a` | C2 | 2026-08-02 | `SYSEngine/docs/**`, `AGENTS.md` — the assessment and the rulings |
| C4 | `git:5cb1a4f0d2e79836ba4c17e05d3f96428a7bc10e` | C3 | 2026-08-03 | `src/**`, `SYSEngine/crates/**` — M1–M5 land |
| C5 | `git:b7e3092d6a1f48c5039be2714da86f05c93e1b6d` | C4 | 2026-08-04 | `SYSEngine/crates/sys_game_bridge/**` — a bridge follow-up |

C3 is when the observations were made and C4 is when everything landed, which is the whole
point: the document asserts a state of the world in §1 and then reports, in §7 of the same
document, the work that changed it.

## 3. Freshness expectations

Evaluated at HEAD = C5:

| Record | Status | Cause |
| --- | --- | --- |
| `engine.obs.channel-coverage/1` | **stale** | `watches` on the bridge and present crates, matched by C5; observed at C3 |
| `simulator.obs.day-runs-deterministically/1` | **stale** | `watches ["src/**"]`, matched by C4; observed at C3 |
| `tandem.assessment.central-fact/2` | at risk, depth 1 | `supported_by` to channel-coverage |
| `engine.assessment.castle-not-court/1` | at risk, depth 1 | `supported_by` to channel-coverage |
| `tandem.policy.tandem-work/1` | at risk, depth 2 | `supported_by` to central-fact to channel-coverage |
| `tandem.work.m5-plan/1` | at risk, depth 2 | `depends_on` to central-fact to channel-coverage |

Two stale, four at risk, maximum depth 2 — and the record at depth 2 is the project's
**operating rule**. That is the scenario worth having: a measurement nobody re-took, two
hops from the policy that orders all the work.

`simulator.obs.day-runs-deterministically` is the quieter finding. §1 asserts the day runs
deterministically; §7 then lands admission lists, action precedents, `DayLegacy` v5 Orders
and delta, and typed Compline composition. Nobody re-ran the determinism check, and
nothing in the document notices. The ledger does.

No milestone is flagged: `after` and `part_of` do not carry staleness (D-024), which is
what stops the warning from covering half the project.

## 4. The acceptance gap

The document's STATUS banner reads *"LIVE (implementation landed; manual sign-off
pending)"*. The ledger computes that state rather than asserting it:

| Milestone | State | Checks | Satisfied |
| --- | --- | --- | --- |
| M1 — the court speaks | completed | 2 | 2 |
| M2 — the castle works visibly | completed | 2 | 2 |
| M3 — the first audible day | completed | 3 | 3 |
| M4 — the day means something | completed | 3 | 3 |
| M5 — one playable day | **active** | 3 | **2** |

M5's unsatisfied check is `three-seed-designer-signoff`. Setting M5 to `completed` raises
`AKR-R022` naming that check, which
`example_sys_tandem.rs::completing_m5_fails_on_the_one_unverified_check` asserts. The
banner's prose and the ledger's arithmetic agree — but only one of them can be checked,
and only one of them stays right when somebody edits the milestone.

## 5. Record inventory (63 keys, 66 revisions)

| Key | Kind | Revs | Head state | File | Title |
| --- | --- | --- | --- | --- | --- |
| `engine.assessment.castle-not-court` | assessment | 1 | verified | `engine/assessments.akr` | The engine shows a castle with people in it, not a court |
| `engine.decision.placeholder-tts-offline` | decision | 1 | active | `engine/decisions.akr` | Placeholder voices are TTS baked offline into voice assets |
| `engine.evidence.dawn-capture-review` | evidence | 1 | verified | `engine/evidence.akr` | The native dawn capture passed visual review |
| `engine.evidence.lane-audits-green` | evidence | 1 | verified | `engine/evidence.akr` | Every automated acceptance lane is green |
| `engine.evidence.sound-contracts` | evidence | 1 | verified | `engine/evidence.akr` | The sound contracts hold |
| `engine.evidence.squelch-audit-unignored` | evidence | 1 | verified | `engine/evidence.akr` | The calm two-actor squelch assertion is un-ignored and passes |
| `engine.obs.channel-coverage` | observation | 1 | verified | `engine/observations.akr` | Two thirds of player-relevant simulator systems have no engine channel |
| `engine.obs.sound-decided-not-heard` | observation | 1 | disproven | `engine/observations.akr` | Sound is decided but not heard |
| `engine.question.voice-direction` | question | 1 | deferred | `engine/questions.akr` | What should the court sound like, and who records it? |
| `engine.req.no-debug-surfaces` | requirement | 1 | active | `engine/requirements.akr` | Nothing in the product day reads as debug |
| `engine.work.citadel-openings` | work | 1 | ready | `engine/work.akr` | Authored openings and fixtures for the grammar-grown citadel |
| `engine.work.client-shell-hardening` | work | 1 | active | `engine/work.akr` | Harden the native client shell |
| `engine.work.four-record-families` | work | 1 | completed | `engine/work.akr` | Project flows, work packets, occasions and the ledger |
| `engine.work.mixer-core` | work | 1 | completed | `engine/work.akr` | Mixer core, room response and clarity rendering |
| `engine.work.queue-pockets` | work | 1 | completed | `engine/work.akr` | Queue pockets, visible yield, idle sets and carried objects |
| `simulator.decision.wild-threshold-ignored` | decision | 1 | active | `simulator/decisions.akr` | The wild threshold assertion stays ignored |
| `simulator.evidence.court-almanac-seed-1042` | evidence | 1 | verified | `simulator/evidence.akr` | Court Almanac production run, seed 1042 |
| `simulator.evidence.court-almanac-seed-1735` | evidence | 1 | verified | `simulator/evidence.akr` | Court Almanac production run, seed 1735 |
| `simulator.evidence.court-almanac-seed-2901` | evidence | 1 | verified | `simulator/evidence.akr` | Court Almanac production run, seed 2901 |
| `simulator.evidence.court-session-audits` | evidence | 1 | verified | `simulator/evidence.akr` | CourtSession audits pass |
| `simulator.evidence.prime-bell-fixture` | evidence | 1 | verified | `simulator/evidence.akr` | The Prime-bell-at-the-servery-door fixture passes |
| `simulator.obs.day-runs-deterministically` | observation | 1 | verified | `simulator/observations.akr` | The day runs deterministically without a player |
| `simulator.obs.idle-channels` | observation | 1 | disproven | `simulator/observations.akr` | The two newest engine channels are idle for want of simulator events |
| `simulator.question.wild-threshold` | question | 1 | resolved | `simulator/questions.akr` | Should the wild threshold assertion be un-ignored? |
| `simulator.work.a5-a7-a8-defects` | work | 1 | ready | `simulator/work.akr` | Redesign-ledger defects A5, A7 and A8 |
| `simulator.work.classic-play-debug-split` | work | 1 | ready | `simulator/work.akr` | Fix the classic Play/Debug split |
| `simulator.work.court-session-records` | work | 1 | completed | `simulator/work.akr` | Mint CourtSession and emit two-actor percepts |
| `simulator.work.day-close` | work | 1 | completed | `simulator/work.akr` | The day's close: admission list, coherence contract, Orders and delta |
| `simulator.work.delete-phase-enum` | work | 1 | ready | `simulator/work.akr` | Delete the Phase enum |
| `simulator.work.playability-triage` | work | 1 | active | `simulator/work.akr` | Triage the remaining playability fixes |
| `simulator.work.rebase-digest-pins` | work | 1 | ready | `simulator/work.akr` | Re-base the four fast digest pins on a chosen day length |
| `simulator.work.room-activity-table` | work | 1 | completed | `simulator/work.akr` | The RoomActivityKind to work-packet mapping table |
| `tandem.assessment.acceptance-gap` | assessment | 1 | verified | `tandem/assessments.akr` | The implementation landed; acceptance did not complete |
| `tandem.assessment.central-fact` | assessment | 2 | verified | `tandem/assessments.akr` | Projection gaps remain the shape of the work; the idle channels are closed |
| `tandem.constraint.deferred-tool-suite` | constraint | 1 | active | `tandem/constraints.akr` | The tool suite is deferred until there is a visible day to direct |
| `tandem.constraint.no-identifier-renames` | constraint | 1 | active | `tandem/constraints.akr` | Code identifiers are not renamed for the nomenclature ruling |
| `tandem.constraint.no-new-features` | constraint | 1 | active | `tandem/constraints.akr` | This roadmap orders existing plans and schedules no new features |
| `tandem.constraint.no-runtime-tts` | constraint | 1 | active | `tandem/constraints.akr` | No text-to-speech crate enters the runtime dependency tree |
| `tandem.constraint.out-of-scope-systems` | constraint | 1 | active | `tandem/constraints.akr` | War dials, the 30k city and multiplayer are out of scope |
| `tandem.decision.nomenclature` | decision | 1 | active | `tandem/decisions.akr` | Planning prose uses exactly two nouns for the two halves |
| `tandem.decision.tui-divergence` | decision | 1 | active | `tandem/decisions.akr` | The classic Play/Debug split is deprioritised |
| `tandem.milestone.m1-court-speaks` | milestone | 2 | completed | `tandem/milestones.akr` | M1 — the court speaks |
| `tandem.milestone.m2-castle-works` | milestone | 1 | completed | `tandem/milestones.akr` | M2 — the castle works visibly |
| `tandem.milestone.m3-audible-day` | milestone | 1 | completed | `tandem/milestones.akr` | M3 — the first audible day |
| `tandem.milestone.m4-day-means-something` | milestone | 1 | completed | `tandem/milestones.akr` | M4 — the day means something |
| `tandem.milestone.m5-one-playable-day` | milestone | 1 | active | `tandem/milestones.akr` | M5 — one playable day |
| `tandem.papercut.search-after-write-stale` | papercut | 1 | verified | `tandem/papercuts.akr` | akr search right after a write returned stale results |
| `tandem.policy.same-change-audit` | policy | 1 | active | `tandem/policies.akr` | A slice lands with its audit and its documents in the same change |
| `tandem.policy.tandem-work` | policy | 1 | active | `tandem/policies.akr` | Milestones are one simulator slice and one engine slice that consume each other |
| `tandem.policy.test-surfaces-only` | policy | 1 | active | `tandem/policies.akr` | The TUI and classic CLI are simulator test surfaces only |
| `tandem.question.step-6-polish` | question | 1 | open | `tandem/questions.akr` | When does sound step-6 polish land? |
| `tandem.req.bridge-invents-nothing` | requirement | 1 | active | `tandem/requirements.akr` | The bridge invents nothing |
| `tandem.term.bridge` | term | 1 | active | `tandem/terms.akr` | Bridge |
| `tandem.term.engine` | term | 1 | active | `tandem/terms.akr` | ENGINE |
| `tandem.term.playable-day` | term | 1 | active | `tandem/terms.akr` | Playable day |
| `tandem.term.simulator` | term | 1 | active | `tandem/terms.akr` | SIMULATOR |
| `tandem.track.defect-retirement` | track | 1 | active | `tandem/tracks.akr` | Defect retirement |
| `tandem.track.doc-discipline` | track | 1 | active | `tandem/tracks.akr` | Doc discipline |
| `tandem.track.lighting` | track | 1 | active | `tandem/tracks.akr` | Lighting |
| `tandem.work.defect-retirement-plan` | work | 2 | active | `tandem/work.akr` | Defect retirement schedule |
| `tandem.work.m1-plan` | work | 1 | completed | `tandem/work.akr` | M1 plan of record |
| `tandem.work.m5-plan` | work | 1 | active | `tandem/work.akr` | M5 plan of record |
| `tandem.work.roadmap-import` | work | 1 | proposed | `tandem/work.akr` | Import the tandem roadmap's durable claims |

### The three multi-revision keys

Each exists for a different reason, which is why all three are worth having:

| Key | Why it was revised |
| --- | --- |
| `tandem.milestone.m1-court-speaks` | **Acceptance was narrowed.** Revision 1 required both `#[ignore]`d squelch assertions un-ignored; a designer ruled the wild-threshold one out of scope, so revision 2 names one. The roadmap records the ruling and not the revision. |
| `tandem.assessment.central-fact` | **The document contradicted itself.** §1 asserts the two newest engine channels are idle; §7 of the same document reports M1 landing and closing them. Revision 2 keeps the general claim and retires the specific one. |
| `tandem.work.defect-retirement-plan` | **A replan with dispositions.** §4's defect bullet re-schedules four items in four different directions; revision 2 disposes of each. |

### The disposition demonstration

`tandem.work.defect-retirement-plan/2` supersedes `/1` and disposes of all four of its
unfinished children, using three of the four outcomes on real content:

| Child | Outcome | Into |
| --- | --- | --- |
| `simulator.work.rebase-digest-pins` | `carried_forward` | `@tandem.track.defect-retirement` |
| `simulator.work.delete-phase-enum` | `carried_forward` | `@tandem.track.defect-retirement` |
| `simulator.work.a5-a7-a8-defects` | `completed_elsewhere` | `@simulator.work.day-close` |
| `simulator.work.classic-play-debug-split` | `intentionally_dropped` | — |

All four remain `ready`. `completed_elsewhere` names a *destination*, not a past-tense
fact: V-017's child set is live-only, so a child already terminal is not an unfinished
child and needs no disposition at all. The disposition records the decision; terminating
each child is a separate write, and the example sits between the two steps deliberately.

## 6. `akr.lock`

Generated by the toolchain, not hand-written: `ResolvedModel::to_lock().render()` over
this corpus, with real content hashes. `example_sys_tandem.rs` re-derives it on every run
and fails if the committed file drifts, so it cannot rot the way a hand-written lock can.

64 seals — every revision except the three `proposed` ones — and 58 resolutions.

## 7. Expected outcomes

- `akr check` — exits **0**. Every V-rule passes on all 66 revisions.
- `akr review-queue` — 2 stale, 4 at risk, as in §3.
- Completing M5 — fails with `AKR-R022` on `three-seed-designer-signoff`.
- Reformatting — a no-op; every source file is already canonical.

## 8. What is deliberately absent

No record is invented for a document this project does not hold. §6 of the roadmap indexes
roughly twenty plan documents; none is in this repository, so each is cited with a legacy
`source` block on the record that derives from it, and nowhere else. Inventing a record
for `sound.md` would assert knowledge nobody here has, which is the anti-goal D-022
exists to prevent.

`tandem.work.roadmap-import` is the migration tracking record D-022 prescribes: six
acceptance checks, one per section of the source document, none yet verified. The legacy
file is archived only when that work record completes. It is `proposed`, which is the
honest state — this encoding is a first pass, not a signed-off migration.
