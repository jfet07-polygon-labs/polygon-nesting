# CurrentPoseOverlay: isolating the 2.5-degree snap, and the composability prize that didn't land

Sol review 5 §3 (`docs/sol-review-5-se2-and-pose-freedom.md`) asked for one thing
before any `CurrentAssignment`/`DirectionalPenetration` comparison is trusted:
build `StructuredGrid + CurrentPoseOverlay` so the entry-damage claim
(`+0.448 mm` median, `0.825 mm` at the 155.4 depth, both from the record-line
and orientation-floor rounds) can be measured **without also swapping the
catalogue, the pair-NFP table and the pressure model** the way
`CurrentAssignment` does today. This round builds that overlay, campaigns it
against the grid and against the existing other engine, and reports what
happened — including a real bug the campaign itself caught, and a negative on
the prize the review actually cared about.

## 0. What was built

`GeneralRelaxedSettings::current_pose_overlay: bool` (compiled only under
`compression-schedule`, off by default, every existing constructor sets it to
`false`). When armed, `drive_compression_schedule` (mode 34):

* builds the `StructuredGrid` catalogue exactly as before —
  `build_surrogate_catalog`'s `StructuredGrid` branch is untouched, so the
  candidate space `random_candidate`/`seed_angle` can propose is the one it
  always proposed;
* separately computes `CurrentPoseOverlay`: one [`OrientedSurrogate`] per piece
  whose parent rotation is not already a 2.5-degree grid angle, keyed the same
  way the grid keys its own entries (`build_current_pose_overlay`), and layers
  it onto a *clone* of the grid catalogue's orientation table
  (`catalog_with_current_pose_overlay`) — the original `Arc<SurrogateCatalog>`
  is never mutated, so any other holder of it still sees the pure grid table;
* seeds `initialize_complete_state` with the parent's own continuous rotation
  instead of `canonical_angle`-snapping it, for every combination this used to
  snap (not only `DirectionalPenetration`/`ContinuousUniform`/`CurrentOnly` -
  those already kept continuous angles);
* keeps the pressure model at `StructuredTrianglePoles` and the collision
  backend at `RollbackTriangle` throughout. No path to `DirectionalPenetration`
  opens because the overlay is armed.

Every consumer of `catalog.orientations` in the file is a point `.get()`/
`.contains_key()` — nothing enumerates the map — so layering extra entries
into a clone changes nothing about what a fresh `StructuredGrid` build
produces or what candidate a lane can propose; it only lets a placement that
still holds its exact parent pose *resolve* instead of erroring. That is the
literal meaning of "a separate lookup used only for warm-start/repair": once a
candidate's own rotation is accepted for a piece, `seed_angle` has already
snapped it back onto the grid, and the piece is grid-native again for the rest
of the run.

### 0.1 The bug the campaign caught

The first campaign run reported **bit-identical** entry loss between the grid
and overlay arms on every one of 15 parents, despite `currentPoseOverlayEntries`
correctly counting 8-50 off-grid pieces per parent. The cause: rotation-key
derivation for the non-directional backend ran through `derive_rotation_key`,
whose `else` branch calls `canonical_angle` **unconditionally** — so even
though `initialize_complete_state` correctly seeded a placement's
`rotation_deg` at the continuous angle, every lookup that turned that
placement into a catalogue key re-snapped it before ever reaching the overlay
entry. The overlay's own entries sat in the map, unread; every score was
still computed from the grid-snapped shape.

The fix is `continuous_rotation_keys(relaxed_settings)`, a single predicate
that governs *key derivation only* (never which pressure model's whole
scoring branch runs — that stays gated on `uses_directional_pressure()`
exactly as before): true for `DirectionalPenetration`, and now also true when
`current_pose_overlay` is armed. Six call sites fed a `directional` bool into
`rotation_key`/`derive_rotation_key`; all six now go through this predicate.
A regression test
(`compression_schedule_without_overlay_snaps_continuous_parent_rotation`)
pins that a hand-built parent with one piece at a continuous, off-grid
rotation resolves correctly under the overlay (`currentPoseOverlayEntries ==
1`, `parentProxyFeasible == true`, no `missing_orientation` error) and that
disarming the flag is a no-op on that same fixture. See
`crates/polygon-nesting-core/src/search/general_relaxed.rs` around
`continuous_rotation_keys`, `build_current_pose_overlay`,
`catalog_with_current_pose_overlay`.

This is worth stating plainly: **without this fix, a `CurrentPoseOverlay`
that "worked" (compiled, ran, produced valid output, never errored) would
have silently done nothing.** The bug was caught only because the campaign
compared arms and found a suspiciously exact match, not because anything
failed loudly. Every number in §2-§3 below is from the binary with the fix
in place; the pre-fix numbers are not reported because they are, by
construction, identical to arm A's.

## 1. Campaign setup

Fifteen parents, all mode 34 (compression schedule), all from the same
request (`tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`):

* the twelve compression-schedule port parents (171.61-179.62 mm,
  `docs/experiments/compression-schedule/evidence/parents.json`), replayed at
  the from-request allowance `0.002`;
* the three true-contract pins `156.9188`, `156.0914`, `155.4223`
  (`docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/record-line-cascade/`),
  replayed at the record lineage's own allowance `0.0005`.

Two arms, equal work:

* **A - `grid`**: `StructuredGrid`, today's default (`current_pose_overlay =
  false`).
* **B - `overlay`**: `StructuredGrid + CurrentPoseOverlay`
  (`current_pose_overlay = true`), same pressure model, same catalogue, same
  candidate order.

Both arms run mode 34 with `POLYGON_NESTING_COMPRESSION_SCHEDULE=past=1,
rollback=32,work=3341379` (compression-schedule's own "design slice": 10% of
one measured mode-26 rung, the middle of the 5.9-11.7% band its 0.5-1.0 s
design point works out to — chosen so this campaign's own claim transfers to
the ten-second envelope rather than an offline record-chasing budget), asked
for the same `0.3 mm` drop below each parent's own reported depth and allowed
to continue past it. `work_cap_queries` is the schedule's own currency
(candidate queries + 5 x exact pair tests); every arm stops itself at the same
budget regardless of engine, which is what makes this a work-budget
comparison rather than a wall-clock race on a shared box.

Driver: `docs/experiments/current-pose-overlay/drivers/campaign.py`. Raw
per-run output lives under `/var/lib/t3/tmp/current-pose-overlay/campaign/`
(not committed - 38 MB of full benchmark dumps); the reduced evidence table
is `docs/experiments/current-pose-overlay/evidence/ab-campaign.json`, built by
`drivers/summarize.py`.

## 2. A vs B: entry damage, not a promised gain

`currentPoseOverlayEntries` (pieces off the 2.5-degree grid) ranges 0-50
across the fifteen parents, median 28. The `155.4223` pin shows **49** - the
exact number the record-line-cascade evidence and Sol's own review already
reported for that parent ("49 rotazioni su 61 fuori dalla griglia 2.5°"),
which is an independent cross-check that the overlay's count is measuring the
right thing.

| metric (overlay - grid, 15 parents) | median delta | direction |
|---|---:|---|
| entry loss (proxy tier's own units) | **-1420.4** | reduced on 11, increased on 3, unchanged on 1 (the one parent with 0 overlay entries) |
| entry boundary violations (count) | **-2** | reduced on 11, increased on 0, unchanged on 4 |
| entry collision pairs (count) | **+9** | reduced on 0, increased on **14**, unchanged on 1 |
| entry proxy-feasible (bool) | — | **false on all 15, both arms** |

So: **not zero, and not uniformly a gain.** Fixing the rotation snap reduces
boundary overflow and (mostly) reduces total entry loss magnitude — the
translation each parent carries was calibrated for its own continuous
rotation, so undoing the snap on the rotation half of the pose recovers some
of that fit. But it makes the proxy tier's own *pairwise collision-pair count*
worse on 14 of 15 parents, every time it changes at all. This is not
noise or a tie broken by rounding: the median entry-loss deltas are on the
order of 10% of the ~12,000-20,000-unit baseline, and the collision-pair
increase is monotonic in direction if not in size. The most likely reading is
that the `StructuredTrianglePoles` pole/triangle proxy's own approximation
error is not uniform across rotation - the round 2.5-degree angles the
catalogue always builds are not obviously a worse operating point for that
specific proxy than an arbitrary continuous one, even though they are a worse
operating point for the *exact* geometry the parent actually has. Sol's
warning said this could come back as "zero, a larger gain, or a regression";
the honest answer is that it is a **mixed** result along two different axes
of the same entry-feasibility measurement, and this campaign does not have
enough resolution to say the trade nets positive or negative in general - only
what it did on these fifteen parents.

Downstream, at equal work, arm B still comes out ahead on the numbers that
matter for publication:

| | grid (A) | overlay (B) |
|---|---:|---:|
| parents publishing below their own parent | 9 / 15 | **12 / 15** |
| total drop summed over publishing parents | 4.136 mm | **5.984 mm** |
| confirmations attempted (= accepted, both arms) | 1,807 | 1,394 |
| median process queries/s | 1,224,000 | 1,213,300 |

Three parents (`port-seed6`, `port-seed11`, `true-156.091`) flip from "grid
never beats its own parent at this budget" to "overlay does"; none flip the
other way. Queries/s is within 1% between arms, as expected for the same work
budget on the same box - this is a work-budget comparison, not a wall-clock
one, and no wall-clock claim is made here. Fewer confirmation attempts under
overlay is consistent with the schedule spending more of its fixed budget
repairing a state whose *collision-pair count* the overlay just made larger
(§ above), and still coming out ahead on where the frontier ends up.

Per-parent table: `docs/experiments/current-pose-overlay/evidence/ab-campaign.json`
(`perParent`), one row per parent with both arms' entry counts, loss, and
publication outcome side by side.

## 3. The composability prize: negative, on this campaign

The prize the review actually named is not the entry-loss number - it is
whether an already-published state that fails `parentProxyFeasible` under the
grid *passes* under the overlay, because that is what would make an
`m33`/`m22`-produced continuous state directly composable with mode 34 instead
of paying the snap tax it pays today.

**Zero of fifteen parents flip.** Every parent that is proxy-infeasible under
the grid (all fifteen are) remains proxy-infeasible under the overlay too -
`entryCollisionPairs` never reaches zero on either arm for any parent tested.
This is not a contradiction of §2: the collision-pair *count* moving from,
say, 27 to 38 is exactly the kind of change that cannot cross a
feasibility threshold of zero from a starting count that was never within a
handful of it. None of the fifteen parents here happen to be close to
proxy-feasible in the first place - the compression-schedule README already
recorded that the 171-179 mm port band "arrives at the relaxed lane already
proxy-infeasible, with 26-38 colliding pairs," and the true-contract pins are
no closer (28-35). The overlay removes the *rotation-snap* contribution to
infeasibility; it does nothing about the much larger contribution from asking
a 0.3 mm compression a parent was not already sitting on.

Per the task's own condition ("if yes, one demonstration"): since no parent
flipped, there is no demonstration to run. The composability question is
answered, and the answer on this campaign is no - not because the overlay is
broken (§0.1's fix and regression test rule that out), but because none of
the fifteen parents tested were ever close enough to the feasibility boundary
for a sub-1.25-degree correction to cross it.

## 4. Arm C: architecturally unreachable, not run

The plan asked for `C: CurrentAssignment + DirectionalPenetration, the
existing other engine, for reference`. It could not be run, and the reason is
itself a finding worth recording plainly rather than working around:

* Mode 34 (and every other persistent-vacancy mode) is reachable from the CLI
  through exactly one path: `dispatch_persistent_vacancy_mode`, called only
  from `run_coupled_dynamic_separator_experiment`, which is gated by
  `coupled_separator_configuration_error`. That gate **unconditionally**
  requires `pressure_model == StructuredTrianglePoles` (plus
  `RollbackTriangle`, `StructuredGrid` seed policy, and the exact protected
  8-lane/40-sweep/10-10-sample/5-refinement shape) - for *both* the control
  and the treatment arm, regardless of which mode is asked for. Setting
  `relaxed-pressure-model=directional` with `persistent-vacancy=34` produces
  no `persistentVacancyPopulation` at all; the JSON instead carries
  `treatment.skippedReason: "coupled dynamic separator requires the
  protected 8-lane, 40-sweep, 10/10-sample, 5-refinement structured route"`.
* The obvious workaround - warm-start the base engine from the same parent
  fixture (`warm-start-fixture`, argument 46) with `pressure-model=directional`
  and clamp the sheet to the target depth (`sheet-long-axis-override-mm`,
  argument 15) - also fails, and not for a reason specific to
  `DirectionalPenetration`: `load_pinned_vacancy_parent` validates a fixture's
  recorded sheet dimensions against the settings *after* the override is
  applied, so **any** sheet-long-axis override makes **every** pinned or
  warm-start fixture fail to load (`"parent fixture settings mismatch:
  sheetLongAxisMm fixture=2700 effective=173.908"`), independent of pressure
  model.

So today there is no CLI-reachable way to ask "the existing other engine" to
compress an already-complete parent toward a target depth at all - not a
narrow gap specific to mode 34, but a property of how the coupled-separator
gate and the fixture-settings check compose. This sharpens rather than
contradicts Sol's own framing: the review already said `CurrentAssignment`
"cambia insieme catalogo, pair-NFP e pressure model," making a direct
A/B/C comparison a comparison of two engines; the campaign here shows the
entanglement runs deeper still - the compression *operator itself* is not
available to the other engine under any settings this round could reach from
production code, only from the free functions the existing unit tests call
directly. Opening that path is explicitly next-round work (Sol review 5 §3:
"per arrivare verso 150 inizierei subito a rimuovere la barriera
StructuredGrid/CurrentAssignment"), not this one's.

## 5. Gates, suite, determinism

* **Four pinned regression gates**, on the exact gate binary
  (`cargo build --release --example general_request_benchmark --features
  jagua-experimental`, `current_pose_overlay` compiled out entirely because
  it lives behind `compression-schedule`, which this build does not carry):
  all four hit (`206.869/8a7737381238fa4d`, `159.09233022733062/fa01012af1d559ae`,
  `159.07876040364795/e28fba007f8031d4`, `164.0375677990678/49f094d7e59a9008`).
  A whole-document diff against a binary built from this same worktree at the
  pre-round commit (`f32c629`, `jagua-experimental` only, unmodified) differs
  in exactly 8 fields per gate, all of them expected build/run artefacts
  rather than engine output: `executableSha256`, `relevantSourceTreeSha256`,
  `engineWorktreeStatus` (this worktree has uncommitted changes; the baseline
  does not), and the four elapsed-time quartiles (wall-clock, two separate
  runs). Every semantic field - every depth, every fingerprint, every
  diagnostic count - is identical.
* Also re-checked with `compression-schedule` compiled in but
  `current_pose_overlay` left at its default `false`: all four gates still
  hit. The flag changes nothing when it is off, in either build.
* **Suite**: `cargo test --release --features jagua-experimental`, redirected
  to a log with `echo EXIT=$?` as a separate command afterward -
  **`EXIT=0`, 55 test binaries, 0 failed** (log: `evidence/suite.log`).
  `free_material_multi_eviction_shrinks_retained_container_capacity` (the
  known-flaky test) passed on the first attempt; no rerun was needed.
* New unit tests: `compression_schedule_without_overlay_snaps_continuous_parent_rotation`
  (the overlay/no-overlay resolution contrast, §0.1) plus the three pre-existing
  mode-34 tests, all passing under `jagua-experimental,compression-schedule`.

## 6. Honest limits

* Fifteen parents is what the task specified, not a claim about generality;
  none of them happened to be near the feasibility boundary, so this campaign
  cannot say what the overlay does to a parent that *is* near it.
* The entry-collision-pair regression (§2) is measured, not explained beyond
  a plausible mechanism (proxy approximation error is not rotation-uniform).
  Confirming that mechanism would need a per-pair breakdown this round did not
  build.
* Work-budgeted throughout; no wall-clock comparison is made, and the box is
  shared with other agents during this round.
* Arm C is a documented non-result (§4), not a negative measurement - it was
  never run, because it cannot currently be run.

## Files

* `drivers/campaign.py` - the A/B campaign (fifteen parents, both arms, equal
  work).
* `drivers/summarize.py` - reduces the campaign's raw per-run dumps to the
  evidence table.
* `drivers/gates.py`, `drivers/lib.py` - the four pinned regression gates,
  copied from `constructor-inner-certificate` with `ROOT` repointed at this
  worktree.
* `evidence/ab-campaign.json` - the reduced campaign table: per-arm summary,
  per-metric delta histograms, per-parent rows, composability-prize count.
* `evidence/gates-flag-off-final.json`, `evidence/gates-compression-schedule-flag-off.json`
  - gate runs on the `jagua-experimental`-only and
  `jagua-experimental,compression-schedule` builds respectively, flag off in
  both.
* `evidence/suite.log` - the full test suite.
