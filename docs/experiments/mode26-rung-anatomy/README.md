# Mode-26 rung anatomy, and what a per-move compression schedule would cost

This is a measurement round, not an engine change. It answers one question with
numbers: **where do the 12-95 seconds of a mode-26 (clamped-sheet ladder
compression) call actually go**, and it turns that answer into a concrete
porting design for a compression schedule that runs at kernel frequency inside
`move_sweep`, against the 0.5-1.0 second slice a production budget can pay for.

The only code change is diagnostics: a wall-clock anatomy block on the mode-26
ladder, rung and arm diagnostics, compiled in **only** under
`--features search-profiling`. The default build has no field, no clock read
and no snapshot; the four pinned regression gates reproduce, and their
documents are identical field for field apart from build identity and wall
clock (see *Regression* below).

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_028f78e1-e59-3` |
| base commit | `b522373` (coordinator v2 + inner certificate merged) |
| request | `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`, sha256 `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3` |
| contract | true 5.0/5.0 exact clearance, search-offset allowance `0.0005` |
| profiled binary | sha256 `143650aaf15010d2e8b3d28285eb9ac43a8c31fc5037b63c4da8b08f376e41ee` (`jagua-experimental,search-profiling`) |
| gate binary | sha256 `ad25149662e256b39f17282dc81c95c0d730ef52a9b441633a48c9d0d1b154c7` (`jagua-experimental`) |
| box | x86_64, 16 cores, engine pinned at 8 threads |

Parents: `pinned-parent-159.079.json` (raw 159.07876040364795, the linear
lineage that holds the record) and `pinned-fs-parent-164.0376.json`
(raw 164.0375677990678, the from-scratch lineage), both from
`docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/finer-ladder/`.

Sample: 8 mode-26 ladders — drops of 0.3, 0.55 and 1.0 mm below the parent at
seeds 0 and 1 for the linear parent, plus drops of 0.3 and 1.0 mm at seed 0 for
the from-scratch parent. That is **35 rungs and 171 rung arms**, 330.73 s of arm
wall time.

> **The wall times here are from a profiling build.** `search-profiling` costs
> about 4.5% of a deep-operator stream. Everything below is a *decomposition* —
> shares, ratios, calls and nanoseconds per call. No number here is a
> production wall-clock claim; a wall-clock claim needs the paired interleaved
> A/B, and this round does not make one.

## 1. The measured decomposition

### 1.1 The band reproduces, and the rung is not where it lives

| scope | n | min | median | max |
|---|---:|---:|---:|---:|
| one mode-26 operator call (ladder region) | 8 | 9.98 s | 38.43 s | 81.13 s |
| one rung | 35 | 4.66 s | 10.53 s | 13.80 s |
| one rung arm | 171 | 1.23 s | 1.74 s | 4.03 s |

Process wall for the same eight runs: 12.51 - 83.25 s. The review's "12-95
seconds of operator work" is therefore the **whole ladder**, not one rung: a
rung is 4.7-13.8 s and an arm is 1.2-4.0 s.

### 1.2 Orchestration between rungs is exactly zero

| scope | measured |
|---|---|
| ladder time outside its rungs | 0.010 - 0.045 **ms** per ladder (4.5e-7 of the ladder) |
| rung time outside its arms | 0.20 - 1.73 ms per rung, median 1.01 ms (0.0095% of a rung) |
| sum of arm wall vs sum of rung wall | 330,734.0 ms vs 330,769.0 ms |

Fingerprinting both warm starts, cloning the placement vectors and the
publication bookkeeping between attempts cost **35.0 ms in total across all 35
rungs**. There is nothing to optimise between rungs. All of it is inside the
arms.

### 1.3 Inside an arm: 90.1% is one call

| component | total | arms that ran it | share of arm wall | per arm when run (min / median / max) |
|---|---:|---:|---:|---|
| clamped mode-0 pipeline (`improve_complete_layout_under_rollback_comparison`) | 298.14 s | 171 | **90.14%** | 1229.2 / 1737.1 / 2310.0 ms |
| repair tier 4: global program (mode 31, Hildreth) | 19.01 s | 25 | 5.75% | 168.2 / 888.3 / 1006.7 ms |
| repair tier 3: joint multi-piece re-placement | 12.22 s | 25 | 3.69% | 252.1 / 495.6 / 735.1 ms |
| repair tier 2: single-piece re-placement | 1.33 s | 25 | 0.40% | 30.2 / 51.5 / 82.9 ms |
| repair tier 1: `micro_legalize` | 0.021 s | 25 | 0.01% | 0.708 / 0.808 / 1.024 ms |
| `validate_and_measure_placements` (publication gate) | 0.0071 s | 25 | 0.002% | 0.021 / 0.304 / 0.445 ms |
| `count_exact_overlap_pairs` (1830 pairs) | 0.0040 s | 25 | 0.001% | 0.142 / 0.158 / 0.172 ms |
| `coupled_independent_source_depth` | 0.0012 s | 25 | 0.000% | 0.041 / 0.049 / 0.057 ms |

**One full exact confirmation of the 61-piece layout — depth, exact overlap
census and the publication gate together — costs 0.491 ms mean (0.213 - 0.664,
n=25).** That number is the hinge of the porting design in section 3.

### 1.4 85.4% of the arms produce nothing, and they are 75.5% of the time

| arm fate | n | share | wall each (min / median / max) | wall total |
|---|---:|---:|---|---:|
| aborted on a rollback-tracker disagreement, **no state at all** | 146 | 85.4% | 1229.2 / 1687.4 / 2233.1 ms | **249.81 s (75.53%)** |
| produced the separator's terminal (infeasible) state | 25 | 14.6% | 2641.4 / 3181.9 / 4030.9 ms | 80.92 s |
| produced an exact-valid state | **0** | 0% | — | — |

Every one of the 171 arms attempted exactly **one** contraction target
(`armTargetsAttempted == 1` for all 171, against `COUPLED_SEPARATOR_TARGETS = 32`)
and accepted **zero**; `epochsImproved == 0` for all 171. An m26 arm on this
fixture is 1.74 s spent to attempt one 0.1% contraction and fail it.

### 1.5 The abort is a 0-6 f32-ulp disagreement judged at one f64 ulp

All 146 aborts are the same comparison — the per-piece **incident-loss vector**,
never the boundary total and never a pair pressure:

| statistic | value |
|---|---|
| aborts by kind | `incidentLoss` 146, `boundaryLoss` 0, other 0 |
| gap in f32 ulps | min 0, median 2, mean 2.21, max 6 |
| gap, relative | 3.93e-8 to 4.93e-7 |
| pair-pressure comparisons *tolerated* per rung | 4 - 40, median 18 (644 total) |
| widest gap any rung saw, in f32 ulps | 2 - 6, against a budget of 64 |

The mechanism is exact and is in the code. `CoupledRollbackComparison::ToleratesPoleRounding`
grants a 64-f32-ulp budget to `RollbackMagnitude::PairPressure`, because a pole
pressure reaches `f64` through `f64::from(f32)`. The per-piece incident sums are
`RollbackMagnitude::NativeF64` — an `f64` sum of `f32`-valued terms is not
itself an `f32`, so `derived_losses_agree` falls back to `equal_within_one_ulp`
at **one f64 ulp**. At the magnitudes involved (~4.2e2) one f64 ulp is 5.7e-14
and the observed gaps are ~1.2e-4: four f32 ulps, nine orders of magnitude
outside the rule that judges them and well inside the budget the same policy
already grants the terms being summed.

This is a measurement, not a proposal: widening that rule is a search-trajectory
change and would need its own gate. It is recorded here because it is where
three quarters of mode 26's wall time goes.

### 1.6 Where the nanoseconds are: the ladder is proxy-bound, not exact-bound

Leaf phases summed over all 171 arms — 1,322.88 **thread**-seconds against
330.73 s of wall (effective leaf parallelism 4.00 on 8 workers). Enclosing
phases are excluded from the share column, as `Phase::is_enclosing` requires.

| phase | thread-s | calls | ns/call | leaf share |
|---|---:|---:|---:|---:|
| `pairCollide` | 351.26 | 3,899,437,721 | 90.1 | 26.55% |
| `pairPressure` | 284.28 | 1,646,949,939 | 172.6 | 21.49% |
| `hazardPressure` | 269.54 | 392,246,000 | 687.2 | 20.38% |
| `hazardQuery` | 195.02 | 235,698,265 | 827.4 | 14.74% |
| `boundaryPenalty` | 99.86 | 1,175,941,640 | **84.9** | 7.55% |
| `hazardCommit` | 50.92 | 1,970,352 | 25,845.5 | 3.85% |
| `hazardPoseBounds` | 30.46 | 303,492,333 | 100.4 | 2.30% |
| `exactOverlapTest` | 15.56 | 8,170,586 | 1,904.8 | **1.18%** |
| `updateAfterMove` | 12.88 | 10,025,921 | 1,284.8 | 0.97% |
| `pieceIndexBuild` | 8.03 | 441,664 | 18,184.5 | 0.61% |
| `collisionPolygonBuild` | 4.98 | 1,200,402 | 4,149.3 | **0.38%** |
| `publicationValidate` | 0.08 | 65 | 1,154,367.3 | 0.01% |
| `sheetFitTest` | 0.0001 | 2,633 | 43.4 | 0.00% |
| *(enclosing)* `moveSweep` | 1,731.35 | 441,664 | 3,920,058.9 | — |
| *(enclosing)* `scorePlacement` | 1,604.42 | 1,128,629,723 | 1,421.6 | — |
| *(enclosing)* `fullRescore` | 12.34 | 122,588 | 100,685.8 | — |
| *(enclosing)* `vacancyProposals` | 11.22 | 4,112 | 2,727,569.6 | — |
| *(enclosing)* `vacancyExactRows` | 10.27 | 1,200,402 | 8,557.8 | — |
| *(enclosing)* `auditorScore` | 0.48 | 1,011 | 478,219.8 | — |

Grouped: proxy collider + boundary penalty **55.59%**, dynamic hazard adapter
**41.27%**, exact tier (`exactOverlapTest` + `collisionPolygonBuild`)
**1.55%**. All 8,170,586 exact pair tests and 1,200,402 collision polygon
builds are inside the repair tiers — the clamped separator itself performs
essentially none.

Counters over the whole sample:

| counter | total | derived |
|---|---:|---|
| `candidateQueries` | 1,128,629,723 | 32.25M per rung |
| `effectivePieceMoves` = `acceptedMoves` | 10,025,921 | 286,455 per rung; **112.57 candidate queries per effective move** |
| `fullRescores` | 122,588 | 1 per 81.79 accepted moves; 100.7 µs each |
| `exactPairTests` | 8,170,586 | 233,445 per rung |
| `collisionPolygonBuilds` | 1,200,402 | 34,297 per rung |
| `publicationAttempts` | 65 | 1.9 per rung |
| move sweeps | 441,664 | 22.70 accepted moves and 2,555.4 candidate queries per sweep |

### 1.7 The inner loop is already at production speed

Per ladder, against the ladder's own wall time:

| ladder | drop | seed | rungs | arms | ladder wall | candidate queries/s | effective moves/s | published |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| `lin-d0.3-s0` | 0.30 | 0 | 2 | 6 | 9.98 s | 3.814M | 34,059 | none |
| `lin-d0.3-s1` | 0.30 | 1 | 2 | 9 | 16.20 s | 3.536M | 31,617 | none |
| `lin-d0.55-s0` | 0.55 | 0 | 4 | 18 | 34.75 s | 3.388M | 30,178 | none |
| `lin-d0.55-s1` | 0.55 | 1 | 4 | 21 | 42.10 s | 3.328M | 29,611 | none |
| `lin-d1.0-s0` | 1.00 | 0 | 7 | 36 | 72.65 s | 3.401M | 30,199 | none |
| `lin-d1.0-s1` | 1.00 | 1 | 7 | 39 | 81.13 s | 3.352M | 29,749 | none |
| `fs-d0.3-s0` | 0.30 | 0 | 2 | 6 | 11.06 s | 3.313M | 29,245 | none |
| `fs-d1.0-s0` | 1.00 | 0 | 7 | 36 | 62.90 s | 3.493M | 30,962 | none |

`docs/next-generation-engine-plan.md` records the m22 replay at **3.775M
candidate evaluations/s at ~265 ns each and 33.9K effective moves/s**. The
mode-26 ladder sustains 3.31-3.81M queries/s and 29.2-34.1K moves/s **under a
profiling build that costs ~4.5%**. Within measurement, the clamped ladder's
inner loop is the same loop at the same rate.

**So the 12-95 seconds are not slow geometry. They are 38-272 million candidate
evaluations spent to move one bound by 0.159 mm; 75.5% of that work is thrown
away by a rollback comparison, only 1.55% of the leaf is the exact tier at all,
and the exact *confirmations* the ladder performs on its own states are 12.3 ms
of 330.73 s (0.0037%).**

## 2. What is already in the proxy tier, and what is missing

Read against `search/general_relaxed.rs`, `search/kernel/`, and
`search/portfolio.rs` at `b522373`.

### 2.1 Already expressible in the proxy tier — no new geometry required

1. **The clamp itself.** `boundary_penalty(&placement, strip_depth_mm)`
   (general_relaxed.rs) is pure `f64` arithmetic on the placement's cached
   axis-aligned bounds against `[inset, sheet_short - inset] x [inset,
   strip_depth - inset]`. It **already takes the depth as a parameter** at
   every one of its 11 call sites, and every one of them passes
   `state.strip_depth_mm`. Measured cost 84.9 ns/call over 1.176 billion calls,
   7.55% of leaf. A per-sweep or per-move schedule `s(t)` is a *substitution at
   the call site*: zero additional geometry.
2. **Candidate generation under the clamp.** `random_candidate`,
   `random_directional_candidate`, `directional_inner_fit` and
   `repair_contact_candidates` all derive their sampling box from
   `max_y = strip_depth_mm - inset - local.max_y`. Tightening `s` narrows the
   proposal box automatically.
3. **The violating-pair queue.** `PairTracker.collision_pairs` plus
   `piece_is_active` already select exactly the pieces incident to a colliding
   pair or to a boundary overflow, and `move_sweep` already builds its work
   order from that set (plus `legacy_forced_blockers` when it is empty). A
   compression step that makes `k` pieces protrude puts precisely those `k`
   pieces into the next sweep's active set for free. **This is the repair queue
   the design needs; it exists.**
4. **Incremental scoring.** `MovedRowDelta` + `update_after_move` maintain the
   tracker per accepted move at 1,284.8 ns; a whole-layout rescore already runs
   only once per 81.79 accepted moves.
5. **The pair question.** `ExplorationKernel::pair_row` is the swappable proxy
   tier and carries 48.04% of leaf (`pairCollide` + `pairPressure`). The exact
   tier is off the trait by construction (`exact::ExactAuthority`), so a
   compression schedule written against `K: ExplorationKernel` *cannot* reach an
   exact answer by accident.

### 2.2 Genuinely missing for a per-move clamped-sheet schedule

**(a) A depth schedule object.** `RelaxedState.strip_depth_mm` is one scalar,
written in exactly five non-test places, and every one of them is a
*whole-pipeline* decision: the initial state's construction from the incumbent
(`shelf_y.max(incumbent.used_long_axis_depth_mm)`), an exact-accepted epoch
improvement (`working.strip_depth_mm = metrics.used_long_axis_depth_mm`), a
checkpoint projection to a target depth, and the two `compress_state_at_split`
sites. Nothing owns it per sweep. Missing: a lane-owned
`CompressionSchedule { floor_mm, step_mm, sweeps_per_step }` that `move_sweep`
advances, and a rule for what happens when the schedule outruns the layout.

**(b) A monotone floor in the proxy tier.** Today the only thing that stops the
layout relaxing back out is `fast_settings.sheet_long_axis_mm` in the **exact**
tier — which is exactly why mode 26 had to build a whole clamped-sheet pipeline
per rung. `boundary_penalty` has no memory: relax `s` and a protruding piece's
penalty returns to zero. Missing: the schedule must be monotone non-increasing
within a slice and its floor must live in the lane, so no rollback, no epoch
acceptance and no rescore can restore a looser depth.

**(c) A best-feasible slot at the tightest depth.** Mode 26 gets this by
publishing an arm's exact-valid state through the ladder's `published_*` pair.
The fast loop has `ExactLaneValidation` once per epoch and nothing at
finer grain. Missing: a lane-local "deepest confirmed" slot plus a cheap
confirmation trigger (costed in section 3).

**(d) A repair that answers the residue the schedule will make.** Measured:
`micro_legalize` — the only tier cheap enough for a per-move loop at 0.83 ms —
published in **none** of the 25 arms that produced a state. The tiers that can
answer this residue are joint re-placement (0.496 s) and the global Hildreth
program (0.888 s), and neither fits in a 0.5-1.0 s slice more than once. This
is the design's biggest open question, and section 3 answers it by making the
step small enough that the residue should stay inside the micro-legalizer's
translation-only model — which is a hypothesis this round does **not** verify.

**(e) A rollback contract that survives a moving depth.** The coupled
separator's rollback compares a from-scratch rescore against the incrementally
tracked minimum, and 146/171 arms died there (section 1.5). A moving `s` makes
the boundary term of that comparison depth-dependent, so restoring a state
would also have to restore its depth. Missing: either the schedule declines to
inherit that rollback (mode 0's own accept/reject discipline has no such
rescore) or the schedule becomes part of the snapshot.

### 2.3 What the exact tier must still confirm, and how often

The proxy tier decides ranking; it may never decide feasibility. So the exact
tier must confirm exactly one thing: **that the layout the schedule has reached
is publishable at the depth it claims.** Measured cost of that confirmation on
this fixture:

| step | measured (n=25) |
|---|---|
| `coupled_independent_source_depth` (61 pieces) | 0.049 ms median |
| `count_exact_overlap_pairs` (1,830 pairs) | 0.158 ms median |
| `validate_and_measure_placements` (the publication gate) | 0.304 ms median |
| **all three** | **0.491 ms mean, 0.213 - 0.664 range** |

Nothing on the *candidate* path needs the exact tier: `exactOverlapTest` is
1.18% of this sample's leaf and all of it is inside the repair tiers. Frequency
budget, at 5% of the production slice:

| slice | 5% budget | confirmations affordable | one per N effective moves |
|---|---:|---:|---:|
| 0.5 s | 25 ms | 50 | 340 |
| 1.0 s | 50 ms | 101 | 340 |

That is ~100 confirmations per second. This ledger's quality-curve trace
records mode 0 running **16 epoch scopes in 1.193 s** on the pinned mode-20
seed-0 stream — 13.4 epochs/s, and an epoch is where its exact lane validation
happens. The proposed rate is therefore about **7.5x** more often than mode 0
confirms today, and it fits inside the review's own 5% rule for
optimizer-internal exact geometry.

## 3. The porting design

### 3.1 The budget, stated in the engine's own units

The production slice this must live in is 0.5-1.0 s of a single arm. At the
plan's measured production rate (3.775M candidate queries/s, 33.9K effective
moves/s) and this sample's measured sweep shape (2,555.4 queries and 22.70
accepted moves per sweep):

| quantity | 0.5 s slice | 1.0 s slice | one m26 rung (mean of 35) |
|---|---:|---:|---:|
| candidate queries | 1.89M | 3.78M | **32.25M** |
| effective moves | 16,950 | 33,900 | **286,455** |
| move sweeps | 739 | 1,478 | ~12,619 |
| exact pair tests | budgeted below | budgeted below | **233,445** |

**The production slice can afford 5.9% - 11.7% of a single mode-26 rung.** That
is the whole reason the port cannot be "run the rung faster": it must be a
schedule that produces compression *per move* rather than per bound.

### 3.2 The schedule

One m26 rung is `step_mm = parent_depth * 0.001` = 0.159079 mm on this parent.
Spread over the 739-1,478 sweeps a slice affords, that is 0.11-0.22 µm per
sweep — below the canonical 1/1000 mm grid the engine snaps translations to. So
the schedule must be **quantised to the canonical grid**:

* step = **1 µm** (one canonical grid unit) — 159 steps buys one m26 rung's
  worth of depth;
* **4-9 sweeps of repair per step** (739/159 = 4.6 at the 0.5 s slice,
  1,478/159 = 9.3 at 1.0 s);
* exact confirmation every **4th step** — 40 confirmations x 0.491 ms = 19.6 ms
  = 2.0% of a 1.0 s slice, 3.9% of a 0.5 s slice, inside the review's 5% rule
  for optimizer-internal exact geometry;
* the schedule is monotone: `floor_mm` only ever decreases, and it is restored
  to the last **confirmed** depth (not the last attempted one) when a
  confirmation fails.

Marginal cost of making the depth per-sweep, derived from the measurement:
`boundary_penalty` runs 1,175,941,640 times against 1,128,629,723 candidate
queries — 1.042 per query — and costs 84.9 ns whether the piece protrudes or
not, because it is a bounds-box comparison with no early exit that depends on
the verdict. **Advancing `s` costs one `f64` write per sweep and zero
additional geometry.** The schedule's real price is the extra *moves* the
tightened boundary makes active, which is the thing the quality gate has to
measure rather than predict.

### 3.3 What code changes

| file | change |
|---|---|
| `search/general_relaxed.rs` | `CompressionSchedule` struct (floor, step, sweeps-per-step, confirm-every); `LegacyLaneSearch` owns one; `move_sweep` advances it at entry and passes `schedule.depth_mm()` instead of `state.strip_depth_mm` to `boundary_penalty`, `random_candidate`, `directional_inner_fit` and `repair_contact_candidates`; a lane-local `deepest_confirmed: Option<(Vec<GeneralFastPlacement>, f64)>` |
| `search/general_relaxed.rs` | `GeneralRelaxedSettings::compression_schedule: Option<CompressionSchedule>`, default `None` |
| `search/general_relaxed.rs` | the confirmation trigger: every `confirm_every` steps, `coupled_independent_source_depth` + `validate_and_measure_placements` on the current state; on success install into `deepest_confirmed` and lower `floor_mm`; on failure restore `floor_mm` to the confirmed depth and leave the layout alone |
| `search/portfolio.rs` | the `compression` phase gains a schedule arm beside its mode-22 arm, charged in the same `PortfolioBudget::Work` currency |
| `profiling.rs` | no change — `boundaryPenalty`, `moveSweep` and `publicationValidate` already carry the counters this needs |

**Feature flag: `compression-schedule`, off by default.** With the setting
`None` — which is what every existing caller constructs — `move_sweep` passes
`state.strip_depth_mm` exactly as it does today, so the four pinned gates must
reproduce bit-for-bit. That is not a hope: this round's own diagnostics change
demonstrates the pattern, with all four gates identical field for field apart
from build identity and wall clock.

### 3.4 The quality gate

Matched-arm, equal budget, endpoints by raw depth:

* **Endpoints:** `pinned-parent-159.079.json` (159.07876040364795) and
  `pinned-fs-parent-164.0376.json` (164.0375677990678). Both arms start from
  the same parent at the same seed.
* **Arms:** `A` = the fast schedule inside one mode-0 pipeline run;
  `B` = a mode-26 short ladder (one or two rungs) — the control this round
  measured.
* **Budget:** `PortfolioBudget::Work` units (candidate queries +
  `WORK_UNITS_PER_EXACT_PAIR_TEST = 5` x exact pair tests), so the comparison
  is reproducible rather than wall-bound. Equal budget = **one measured m26
  rung**: 32,246,564 candidate queries + 5 x 233,445 exact pair tests =
  **33,413,789 work units**. Giving the control less than a rung means giving
  it a budget it structurally cannot publish in.
* **Statistic:** paired delta of the best raw depth each arm reaches, one pair
  per (endpoint, seed), over at least 10 seeds. The schedule wins only if the
  paired median delta is strictly negative **and** no arm publishes worse than
  its parent (the parent is the floor for both, as it already is in mode 26).
* **Regression:** all four pinned gates reproduce with the flag off —
  206.869/`8a7737381238fa4d`, 159.09233022733062/`fa01012af1d559ae`,
  159.07876040364795/`e28fba007f8031d4`, 164.0375677990678/`49f094d7e59a9008`.

### 3.5 The three biggest risks

1. **The rollback contract is the dominant cost and a moving depth makes it
   worse.** 146/171 arms and 75.5% of all arm wall died on a 0-6 f32-ulp
   disagreement in an `f64` incident-loss sum. A per-move schedule makes the
   boundary term of that comparison depth-dependent. *Mitigation:* the schedule
   must not inherit the coupled separator's rollback rescore. It should keep
   mode 0's own accept/reject discipline, which has no such rescore, and take
   its safety from the periodic exact confirmation instead. If the schedule is
   nonetheless run inside the separator, the rollback snapshot must carry the
   schedule state, and the *first* thing to measure is whether the abort rate
   moves.
2. **The residue may not be rounding-scale, and the affordable repair only
   handles rounding-scale.** `micro_legalize` (0.83 ms) published in 0 of 25
   arms; the tiers that answered were joint re-placement (0.496 s) and the
   global Hildreth program (0.888 s), each of which is 50-180% of the entire
   production slice. The 1 µm step is chosen precisely so the residue stays
   inside the micro-legalizer's translation-only model — **this round does not
   verify that**, and it is the first thing the implementation round must
   measure: residue magnitude per step, against `micro_legalize`'s acceptance.
3. **This sample measured mode 26's cost honestly and its yield not at all.**
   Zero of 8 ladders published; zero of 171 arms produced an exact-valid state.
   The 159.079 record was reached by mode 26 at other seeds, so the mechanism
   works — but at seeds 0 and 1, at drops of 0.3/0.55/1.0 mm, 330.73 s of arm
   work produced no improvement. A matched-arm gate run at these seeds would
   compare two zeros. The gate must be run at enough seeds that the *control*
   publishes, or its verdict is vacuous.

Two smaller ones worth naming: the boundary penalty is an **axis-aligned bounds
box**, not the material or the miter-joined envelope, so tightening it pulls on
a proxy for the thing that actually binds; and the hazard adapter is 41.27% of
this sample's leaf, so a schedule that increases the active set increases hazard
queries at 827.4 ns each, not only pair tests at 90.1 ns.

## Regression

The four pinned gates were run on the default-feature binary **before and
after** the instrumentation, from this worktree:

| gate | pinned | reproduced |
|---|---|---|
| g1 mode 20 | 206.869 / `8a7737381238fa4d` | yes, both |
| g2 mode 22 | 159.09233022733062 / `fa01012af1d559ae` | yes, both |
| g3 mode 22 | 159.07876040364795 / `e28fba007f8031d4` | yes, both |
| g4 mode 22 | 164.0375677990678 / `49f094d7e59a9008` | yes, both |

Comparing the two runs' documents field by field after stripping the volatile
set, the **only** differences are `executableSha256`,
`relevantSourceTreeSha256`, `engineWorktreeStatus` and the five wall-clock
quartile fields. Every search-visible field is identical. (The g3/g4 wall times
halved between the two runs; that is box contention in the earlier run, not the
change — the documents are identical.)

A third run against the final tree (`gates-final.json`) reproduces all four
again.

`cargo check` passes for the lib with no features, with `search-profiling`
alone, for the example with `jagua-experimental` and with
`jagua-experimental,search-profiling`, and for
`--tests -p polygon-nesting-core --features jagua-experimental,search-profiling`.

`cargo test --release --features jagua-experimental`: **1238 passed, 0
failed**, exit 0.

## Files

* `drivers/lib.py` — the shared runner and the four pinned gates, `ROOT`
  repointed at this worktree.
* `drivers/gates.py` — the four gates against one binary.
* `drivers/rungs.py` — the eight profiled mode-26 ladders.
* `drivers/summarize.py` — the rung, arm-component, phase and abort-census
  tables.
* `rung-anatomy-evidence.json` — every table above, per ladder, per rung, per
  arm, plus the abort census, as measured.
* `ladder-runs.json` — one row per ladder with its whole-process
  `searchProfile` block, as the driver emitted it.
* `gates-before-instrumentation.json`, `gates-after-instrumentation.json`,
  `gates-final.json` — the four pinned gates on the default-feature binary
  either side of the change and against the final tree.

Reproduce:

```
cargo build --release --example general_request_benchmark \
  --features jagua-experimental                                  # gate binary
cargo build --release --example general_request_benchmark \
  --features jagua-experimental,search-profiling                 # profiled binary
python3 drivers/gates.py <label> <gate-binary>
python3 drivers/rungs.py <profiled-binary> <outdir>
python3 drivers/summarize.py <outdir> rung-anatomy-evidence.json
```
