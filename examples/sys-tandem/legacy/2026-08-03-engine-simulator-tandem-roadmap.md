> STATUS: LIVE (implementation landed; manual sign-off pending) — M1–M5 code,
> record projections, product shell, and automated acceptance lanes landed
> 2026-08-03. The remaining acceptance item is a designer's native full-day
> sign-off; the continuous tracks in §4 and sound-plan step-6 polish are not
> milestone blockers. This document remains the master orientation roadmap.
> Last reviewed: 2026-08-03

# Engine ⇄ Simulator tandem roadmap (2026-08-03)

## 0. Nomenclature (binding for planning prose)

Everything is Rust-side now, so the old engine/frontend words are ambiguous.
From this date, planning prose uses exactly two nouns for the two halves:

| Word | Means | Lives in |
|---|---|---|
| **SIMULATOR** | the backend court simulation — the deterministic world, records, and rules. The `save_your_skin` root crate, including its `src/engine/` modules. | repo root `src/` |
| **ENGINE** | the SYS presentation engine — graphics, GUI, and sound. Everything the player sees and hears. | `SYSEngine/` workspace |

The seam between them is the **bridge** (`SYSEngine/crates/sys_game_bridge`),
which reads simulator records and drives the engine; it invents nothing
(`show-dont-tell.md`). Code identifiers are NOT being renamed for this ruling
(`src/engine/` keeps its path); this is a prose/planning vocabulary, like the
time-vs-ticks ruling in `AGENTS.md`.

The TUI and classic CLI remain simulator test surfaces only (designer ruling
2026-08-02 in `AGENTS.md` §3): keep them compiling, do not extend them.

## 1. Where each half actually stands

Read these two documents first; this section only orients.

- **Engine state**: `SYSEngine/docs/BUILD-STATUS.md` (verified tree state) and
  `SYSEngine/docs/2026-08-02-mid-engine-assessment.md` (the gap register).
  Summary: castle, lighting, bodies, stage, frame discipline, and the
  perception seam are strong and audited; of ~24 player-relevant simulator
  systems, ~6 are well represented, ~8 arrive flattened to strings, ~10 have
  no channel at all. The engine currently shows *a castle with people in it,
  not a court*. Sound is decided but not heard (no mixer/device).
- **Simulator state**: the day runs deterministically without a player
  (muster, rota, metabolism, measurement→report→consultation→audit, suits,
  hospitality, ledger). Its open work is listed in
  `2026-07-19-normal-day-plan.md` (N3 partial, N4–N6 open),
  `currently-canonical/nice-version-plan-3.md` (Orders of the Day, day-kind
  calendar, NPC guest/news stream), `currently-canonical/redesign-ledger.md`
  (open defects), and `currently-canonical/PLAYABILITY_FIX_LIST.txt`.

**The central fact of this roadmap:** most engine gaps are *projection gaps*
(the simulator already computes the thing; the bridge/present layer drops it),
and the two engine channels built most recently — threshold murmurs and
conversation formations — are proven idle because the *simulator* never closes
a door and never emits a two-party percept. Neither half can be finished on
its own terms: each next engine slice needs a simulator slice, and the
simulator's open phases only become judgeable when the engine can show them.
Hence: tandem, in fixed pairs, below.

## 2. The operating rule for tandem work

Every milestone below is one simulator slice + one engine slice that
*consume each other*, with the acceptance instrument named up front. A slice
lands with its audit in the same change (`AGENTS.md` §Map-audit,
mid-engine-assessment §9 closing note). Do not start milestone N+1's
simulator half before milestone N's engine half can display what N built —
that display is how N is judged. Lighting (§6 below) is the standing
exception: it is self-contained inside the engine and proceeds continuously
in the background.

## 3. Milestones, in order

### M1 — The court speaks (simulator-led; engine is already done)

The two idle presentation channels come alive. Plan of record:
`2026-08-02-closed-doors-and-court-conversation-plan.md` (Part A closed-door
`CourtSession` records, then Part B two-party calm-day percepts).

- Simulator: mint `CourtSession` from existing convene points; derive door
  state; emit two-actor events at the calm-day interaction sites.
- Engine: **nothing to build** — the bridge drives `StageWorld` door state and
  the formation/murmur machinery that already exists.
- Acceptance: un-ignore the two `#[ignore]`d assertions in
  `SYSEngine/crates/sys_game_bridge/tests/squelch_audit.rs`; the new
  session audits from that plan's §A4.

### M2 — The castle works visibly (engine-led, one simulator table)

The rota becomes legible without prose. Plans of record:
`SYSEngine/docs/plans/pathing.md` §3 and mid-engine-assessment §3, §8.

- Engine: queue pockets as compiled geometry (the data already flows in
  `SpatialBinding`); visible yield from precedence records; workplace idle
  sets; carried objects in hands (the report a carrier bears is the game's
  core object and currently renders as nothing).
- Simulator: the `RoomActivityKind` ↔ work-packet mapping table — the
  designer table deliberately re-deferred out of the bridge on 2026-08-02;
  it retires the bridge's `infer_activity_kind` / `emotional_tone`
  re-derivation defect (mid-engine-assessment §8) at the source. Also route
  `submit_workbench` into the practice systems instead of the dead-end Vec.
- Acceptance: legibility rows in the squelch-audit spirit; the "Prime bell at
  the servery door" fixture from mid-engine-assessment §3.

### M3 — The first audible day (engine-led; sound design unknown resolved by placeholder)

Plan of record: `SYSEngine/docs/plans/sound.md` §9 build order — step 2 and
the §4 hearing gate are landed; what remains is steps 1, 3, 4, 5 (mixer core
+ cpal, FDN reverbs + door continuity, speech-clarity rendering, record-derived
ambience + bells), then 6 (music routing) whenever convenient after.

**The voice-acting question is deferred, not blocking.** The architecture
already contains the answer:

- Only `Clear`/`Partial` speech needs a voice at all; `Murmur` is procedural
  voice-shaped babble by design and `Inaudible` is nothing (`sound.md` §4).
- **Placeholder voices are TTS, generated OFFLINE into ordinary voice-asset
  files** by a build/bake tool (one file per rendered line or per delivery
  template), so the runtime engine just decodes assets through the existing
  `symphonia` path and no TTS crate enters the runtime dependency tree. If a
  runtime TTS dependency ever looks preferable, it needs explicit designer
  approval first (`AGENTS.md` §1 dependency rule).
- This meets every testing objective (earshot parity contract, masking at the
  source, clarity determinism) with zero art budget. Real voice direction —
  what the court *should* sound like, casting, recording — is a separate
  designer track with no scheduled milestone; swapping assets in later
  touches no code.
- Ambience needs no voices at all: it is derived from occupancy/activity
  records (`sound.md` §5). Bells, doors, footsteps are synthesizable or
  sourceable without budget.

Simulator half: none required (percepts and clarity verdicts already exist).

- Acceptance: `sound.md` §8 contracts — earshot parity (GUI, mixer, sim
  record show one verdict from one call), determinism per (seed, tick, pose),
  the Studio walk test with golden clarity verdicts.

### M4 — The day means something (tandem, the biggest pair)

The economy and the day-spine reach the screen, and the day gets its close.
Plans of record: mid-engine-assessment §7 (metabolism on stage, end-of-day
surface), `2026-07-19-normal-day-plan.md` N3-remainder/N4 (admission list,
coherence contract), `currently-canonical/nice-version-plan-3.md` (Orders of
the Day, delta ledger, day-kind calendar, NPC guest/news stream).

- Simulator: finish N3's admission list and N4's coherence contract (every
  decision traces to a precedent); build the nice-version-3 close — Orders of
  the Day generated and delivered at night, the delta ledger (rose / slipped /
  unresolved), on top of the existing `DayLegacy` compile.
- Engine: project four more record families through the bridge — flows,
  work packets, occasions, ledger (mid-engine-assessment §7). Carts at the
  gate and meals in the hall from `FlowCause`; day-spine occasions as
  formations (muster, the audience queue at the dais — the duke's ear
  finally *looks* scarce); the end-of-day position surface — the ledger's
  prose, the delta, and tomorrow's Orders as a real screen, the game's score
  screen and structural cliffhanger.
- Acceptance: normal-day plan §3 ("a normal day without a hitch"); the
  planned `--machine-audit` Court Almanac (`AGENTS.md` §Map-audit) proving
  the day is live and not inert; a squelch-audit row that the close screen
  derives only from records.

### M5 — One playable day (productization)

Definition of playable: **a person sits at the native SYS client and plays
the seeded 900/1020-tick product day from bedside wake to the Compline close,
seeing and hearing the court, with nothing reading as debug.** Remaining
work is the client shell hardening listed in `SYSEngine/docs/BUILD-STATUS.md`
"Deliberately incomplete": production input bindings, persistence
(save/resume via `DayLegacy`), font shaping, accessibility adapters,
character costume/insignia geometry for the semantic selections already in
`CharacterVisualRecord` (the art bottleneck for reading office/house at a
glance — `SYSEngine/docs/plans/models.md`), and the animation social channel
(bow depth, gait urgency, gaze — `SYSEngine/docs/plans/animation.md`).

- Simulator: bug-fix only; the playable-surface items still open in
  `currently-canonical/PLAYABILITY_FIX_LIST.txt` are triaged here.
- Acceptance: a full-day play session on three seeds with the good-day
  surface audits green and the squelch audit green; designer sign-off.

## 4. The standing tracks (continuous, not milestones)

- **Lighting** — `SYSEngine/docs/plans/lighting.md` §9–§10 is its own
  sequenced ladder (global surface atlas → retire the sun depth pass → body
  blob/capsule shadows → beam prisms → ambient probes) with per-step state.
  Proceeds whenever engine capacity exists; no milestone blocks on it. The
  one coupling: authored openings/fixtures for the *grammar-grown* citadel
  (the sun currently has real windows only where authoring passes provided
  them) should land by M4 so the working castle is lit like one
  (`SYSEngine/docs/plans/world-building.md`).
- **Defect retirement** — `currently-canonical/redesign-ledger.md` open
  entries, scheduled: re-base the four fast digest pins on a deliberately
  chosen day length (S3; before M4 lands, since M4 deliberately changes the
  product day and must be caught by pins that watch it); delete the `Phase`
  enum per the archived consolidation table (S3, any time); favour A5/A7/A8
  (fold into M4's simulator half where they touch the close); the classic
  Play/Debug split is DEPRIORITIZED under the 2026-08-02 TUI-divergence
  ruling — the classic surface is a test harness now.
- **Doc discipline** — every slice flips banners and updates
  `SYSEngine/docs/BUILD-STATUS.md` + the mid-engine gap register in the same
  change (`tests/doc_audit.rs` enforces the root/canonical banners).

## 5. What this roadmap deliberately does not schedule

- New features of any kind — this document orders existing plans only.
- The deferred tool suite (`game-tool-design.md`: Day/Occasion Director,
  Office Designer, Journey Board, Crowd Stager, Semantic Presentation
  Studio) — revisit after M4, when there is a visible day to direct.
- Real voice direction/casting (see M3 — placeholder TTS carries testing).
- Anything in `do-not-implement-not-canonical-save-for-later/`, the war/
  wrongness dials, the 30k city, multiplayer (`AGENTS.md` §2.6–2.7).
- The courtier-practice extension professions beyond the landed program
  (`courtier-work-dev-area/` holds their plans for later).

## 6. Index of the live plans this roadmap references

Simulator side (root + `currently-canonical/`):

- `2026-08-02-closed-doors-and-court-conversation-plan.md` — M1
- `2026-07-19-normal-day-plan.md` — M4 (N3 remainder, N4; N5–N6 after)
- `currently-canonical/nice-version-plan-3.md` — M4 (the close)
- `currently-canonical/redesign-ledger.md` — standing defect track
- `currently-canonical/PLAYABILITY_FIX_LIST.txt` — M5 triage
- `2026-07-16-good-day-playing-surface-plan.md` — standing design law
- `currently-canonical/3k.md`, `currently-canonical/duke-scope-decision.md` —
  binding scope; consulted, never scheduled
- `game-tool-design.md` — deferred (§5)

Engine side (`SYSEngine/docs/`):

- `2026-08-02-mid-engine-assessment.md` — the gap register; M2/M4 source
- `BUILD-STATUS.md` — verified state; updated every slice
- `RENDERING-NEXT.md` — renderer boundary (atlas, then per its list)
- `plans/pathing.md` — M2
- `plans/sound.md` — M3
- `plans/lighting.md` — standing track
- `plans/models.md`, `plans/animation.md` — M5
- `plans/gui.md`, `plans/show-dont-tell.md` — binding presentation charter
- `plans/world-building.md` — grammar-citadel openings (standing track)
- `plans/textures.md` — folded into the renderer boundary as capacity allows

Archived as landed history on 2026-08-03 (provenance, do not implement):
`llm-docs/llm-docs-archive/2026-07-30-courtier-professions-implementation-plan.md`,
`llm-docs/llm-docs-archive/2026-07-31-wgpu-rendering-handoff.md`,
`llm-docs/llm-docs-archive/2026-08-01-sysengine-citadel-base-design-plan.md`.

## 7. 2026-08-03 implementation record

- **M1:** `CourtSession` owns the rank rule, participant entry gate, lifecycle,
  and audits; calm two-actor percepts activate conversation formations. The
  wild threshold assertion remains ignored by explicit designer ruling
  because ordinary generation need not schedule a nearby qualifying council.
- **M2:** simulator-authored room activity and typed practice submission feed
  queue pockets, visible yields, workplace idles, and carried objects.
- **M3:** CPAL callback/mixer buses, FDN room response, smooth continuity,
  one-verdict clarity rendering, and record-derived work/weather/bell sources
  are live. A hearing-capability result alone no longer fabricates a voice.
  Decoded line assets and SoundFont PCM routing remain step-6 polish.
- **M4:** audience admission, action precedents, `DayLegacy` v5 Orders and
  delta, flow/work/occasion/muster projection, embodied carts/meals, occasion
  formations, and typed Compline composition landed. The Court Almanac passed
  production 900-tick runs on seeds 1042, 1735, and 2901 with 8/10
  institutions live on each.
- **M5:** production keyboard/focus contracts, seed selection, DayLegacy JSON
  save/resume, semantic accessibility output, complete medium costume
  selection, proportional shipped-atlas shaping, and simulator/stage-driven
  bow/gait/gaze landed. World labels now share one production visual budget,
  expose activity on deliberate selection, and reserve semantic HUD chrome
  instead of painting beneath it. Good-day vocabulary, document, checker,
  focused audio, perception, animation, costume, client, render/presentation,
  and squelch lanes are green; the native dawn capture passed visual review.

The native shell now accepts `--seed <u64>` so the required three manual
product-day sessions are reproducible. A human designer still has to perform
and sign off those real-time sessions; code cannot self-certify that judgment.
