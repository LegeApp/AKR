# `sys-tandem` — encoding a roadmap somebody actually wrote

`examples/save-your-skin` was built to exercise the format: every mechanism appears once,
at a size that fits in one reading. This example is the opposite experiment. The source is
a real orientation roadmap — 270 lines, written 2026-08-03, committed verbatim in
`legacy/` — and the question is what happens when the format meets a document that was
never written with it in mind.

Read `MANIFEST.md` for the inventory and the expected outcomes. This file is about what
the encoding is *for*, and what it turned up.

## 1. What this demonstrates that the first example cannot

**An acceptance gap that is computed, not asserted.** The roadmap's banner says
"implementation landed; manual sign-off pending" — a true and careful sentence that no
tool can check, in a document whose §7 then reads as an unbroken list of things that
landed. Encoded, the same fact is arithmetic: four milestones `completed`, one `active`
with two of three checks satisfied and the third naming a designer's judgement that code
cannot self-certify. Mark M5 complete and `AKR-R022` names the missing check. The banner
and the ledger agree today; only one of them still agrees after somebody edits the
milestone.

**Freshness at a scale where propagation means something.** The first example's staleness
chain was three records long and built to be one. Here the chain is the same shape but
load-bearing: a mid-engine assessment made at C3, a bridge change at C5 that its `watches`
globs match, and two hops later the project's **operating rule** — the tandem policy that
orders every milestone — is flagged at risk. Nobody would have noticed by reading. The
second stale record is quieter and worse: §1 asserts the day runs deterministically, §7
lands a great deal of simulator work, and no one re-ran the check.

**Knowledge shapes the synthetic example simplified away.** A milestone whose acceptance
had to be *revised* rather than met. An assessment that contradicts its own document. A
replan that scatters four work items in four directions. Twenty referenced plan documents
that exist only as citations. Open questions that deliberately block nothing.

**A migration worked example.** Everything here came out of one Markdown file by hand,
which is what P8's tooling will have to automate. `tandem.work.roadmap-import` is the
tracking record D-022 prescribes, with one acceptance check per section of the source. It
is `proposed`, because this is a first pass and saying otherwise would be the exact
failure the format exists to prevent.

## 2. Original document to records

| Source | Captured as |
| --- | --- |
| STATUS banner — "implementation landed; manual sign-off pending" | `tandem.assessment.acceptance-gap`, and structurally by M5's unsatisfied check |
| §0 nomenclature table — SIMULATOR, ENGINE | `tandem.term.simulator`, `tandem.term.engine`, `tandem.decision.nomenclature` |
| §0 — the bridge "invents nothing" | `tandem.term.bridge` (claim `invents-nothing`), `tandem.req.bridge-invents-nothing` |
| §0 — identifiers are not renamed | `tandem.constraint.no-identifier-renames` |
| §0 — TUI and classic CLI are test surfaces only (designer ruling 2026-08-02) | `tandem.policy.test-surfaces-only`, with the ruling's provenance in a legacy `source` block |
| §1 — engine state, the ~6/~8/~10 of ~24 counts | `engine.obs.channel-coverage`, counts as claim `coverage-counts` |
| §1 — "a castle with people in it, not a court" | `engine.assessment.castle-not-court` |
| §1 — "sound is decided but not heard" | `engine.obs.sound-decided-not-heard` (now `disproven`) |
| §1 — simulator runs a day deterministically | `simulator.obs.day-runs-deterministically` (now **stale**) |
| §1 — the central fact: projection gaps, idle channels | `tandem.assessment.central-fact` (two revisions; see §3 below) |
| §2 — the tandem operating rule, lighting excepted | `tandem.policy.tandem-work`, `exceptions [ @tandem.track.lighting ]` |
| §2, §4 — a slice lands with its audit and docs in one change | `tandem.policy.same-change-audit` |
| §3 M1 — the court speaks | `tandem.milestone.m1-court-speaks` (2 revisions), `tandem.work.m1-plan`, `simulator.work.court-session-records` |
| §3 M2 — the castle works visibly | `tandem.milestone.m2-castle-works`, `engine.work.queue-pockets`, `simulator.work.room-activity-table` |
| §3 M3 — the first audible day | `tandem.milestone.m3-audible-day`, `engine.work.mixer-core` |
| §3 M3 — the voice-acting deferral and its answer | `engine.decision.placeholder-tts-offline`, `tandem.constraint.no-runtime-tts`, `engine.question.voice-direction` (deferred) |
| §3 M4 — the day means something | `tandem.milestone.m4-day-means-something`, `simulator.work.day-close`, `engine.work.four-record-families` |
| §3 M5 — one playable day, and its definition | `tandem.milestone.m5-one-playable-day`, `tandem.term.playable-day`, `engine.req.no-debug-surfaces`, `engine.work.client-shell-hardening`, `simulator.work.playability-triage`, `tandem.work.m5-plan` |
| §3 — the named acceptance instruments | Acceptance checks: `squelch-audit-calm-assertion`, `session-audits`, `prime-bell-fixture`, `legibility-rows`, `earshot-parity`, `clarity-determinism`, `studio-walk-golden`, `normal-day-without-hitch`, `court-almanac-machine-audit`, `close-screen-from-records`, `good-day-surface-audits-green`, `squelch-audit-green`, `three-seed-designer-signoff` |
| §4 — lighting track, and its one M4 coupling | `tandem.track.lighting`, `engine.work.citadel-openings` |
| §4 — defect retirement, with its four scheduled items | `tandem.track.defect-retirement`, `tandem.work.defect-retirement-plan` (2 revisions), and the four `simulator.work.*` children |
| §4 — the classic Play/Debug split deprioritised | `tandem.decision.tui-divergence`, and the `intentionally_dropped` disposition |
| §4 — doc discipline | `tandem.track.doc-discipline` |
| §5 — no new features | `tandem.constraint.no-new-features` |
| §5 — tool suite deferred until after M4 | `tandem.constraint.deferred-tool-suite` |
| §5 — war dials, 30k city, multiplayer, extension professions | `tandem.constraint.out-of-scope-systems` |
| §6 — index of live plans | **No records.** Legacy `source` blocks on the records that derive from each plan; see §4 below |
| §7 M1–M5 implementation record | Milestone states, plus `simulator.evidence.court-almanac-seed-{1042,1735,2901}`, `engine.evidence.lane-audits-green`, `engine.evidence.sound-contracts`, `engine.evidence.squelch-audit-unignored`, `simulator.evidence.court-session-audits`, `simulator.evidence.prime-bell-fixture`, `engine.evidence.dawn-capture-review` |
| §7 M1 — the wild threshold assertion stays ignored | `simulator.question.wild-threshold` (resolved), `simulator.decision.wild-threshold-ignored`, and M1's revision |
| §7 M3 — step-6 polish remains | `tandem.question.step-6-polish` (open, blocking nothing) |
| §7 closing — a human must still sign off | M5's `three-seed-designer-signoff` check, verified by nothing |

## 3. What the encoding surfaced

Four things the format forced into the open. None was hidden — each is visible to a
careful reader of the original — but each was something prose let stay soft.

**The document contradicts itself about the central fact.** §1 says the two newest engine
channels "are proven idle" and builds the whole tandem argument on it. §7, in the same
document on the same day, reports M1 landing and activating them. Both sentences are true
of different moments and the document never says which moment it is describing. AKR cannot
hold both: an assessment is either current or superseded. So `central-fact` has two
revisions, and revision 2 says in its own body what happened. The general claim — most
gaps are projection gaps — survives; the specific instance retires.

**M1's acceptance was changed, not met.** §3 names the acceptance instrument precisely:
un-ignore *the two* `#[ignore]`d assertions in `squelch_audit.rs`. §7 reports M1 landed,
and adds that one of the two "remains ignored by explicit designer ruling". Under prose
that reads as a footnote. Under acceptance checks it is a different fact: the milestone as
originally defined was not met, and the definition was narrowed by a ruling. The encoding
has revision 1 with the two-assertion check, revision 2 with one, and
`derived_from` pointing at the ruling. This is the good case for D-016's
descendant-commit rule, too: because acceptance changed, the evidence had to post-date the
change, and it does.

**Nobody re-checked determinism.** §1 asserts the simulator runs a day deterministically.
§7 lands admission lists, action precedents, `DayLegacy` v5, delta ledgers and typed
Compline composition — a great deal of simulator change. The observation's `watches` cover
`src/**`, so the ledger flags it stale. The document has no mechanism to notice, and
doesn't.

**Two open questions deliberately block nothing.** `voice-direction` is deferred with no
scheduled milestone; `step-6-polish` is explicitly not a milestone blocker. My own
`docs/02` §4.12 lists "a question with no `blocks` edge" under common mistakes, on the
grounds that it never surfaces where it matters. This document shows the guidance is too
strong: a question can be genuinely unscheduled and still worth recording, and
`OPEN-QUESTIONS.md` is where it surfaces. The guidance should say so.

## 4. What was deliberately not encoded

§6 indexes roughly twenty plan documents — `sound.md`, `pathing.md`, `nice-version-plan-3.md`,
the redesign ledger, and the rest. None is in this repository. Each is cited with a
`source { kind legacy, path "..." }` block on the record that derives from it, and no
record stands in for the document itself.

That restraint is the point. A `decision` record for `sound.md` would look like knowledge
and be a guess, and every later reader would have to discover that the record was invented
by whoever ran the import. The legacy `source` block says exactly what is true: this claim
came from that file, which you will have to open.

The same applies to the archived plans §6 lists as "landed history, do not implement" —
they are provenance, and they are cited where relevant rather than resurrected as records.

## 5. Running it

```
cargo test --test example_sys_tandem
```

Nine tests: the corpus resolves with no diagnostics, the inventory matches the manifest,
every source file is canonical, the four completed milestones and the one active one are
as described, completing M5 fails on the named check, M1's acceptance narrowing is
recorded, staleness reaches the policy at depth 2, doubt travels only along the three
relations that carry it, and the committed lock matches what the build produces.
