# The compression-schedule port: the clamp bought one micron at a time

The mode-26 rung anatomy measured where a clamped-sheet ladder's 12-95 seconds
go and turned the answer into a design: the clamp is already a *proxy-tier
parameter* — `boundary_penalty` takes the depth as an argument at all eleven of
its call sites and every candidate generator derives its sampling box from the
same scalar — so buying depth does not need a rebuilt pipeline per rung, it
needs a **clock**. This round builds that clock and gates it.

Two results, and they are on opposite sides of the design.

* **The port wins the matched-arm gate, decisively.** Twelve pinned
  coordinator parents, both arms from the same fixture at the same seed, one
  legacy short mode-26 ladder against the schedule at the same allowance:
  the schedule publishes in **12 of 12** cells against the ladder's 10, at a
  median **12.110 mm** below its parent against the ladder's **0.876 mm**, and
  it wins **12 of 12** cells paired — at the control's own *measured* spend as
  well as at the shared allowance. Every one of the twelve publications was
  re-confirmed in a separate process by the pristine default-feature binary
  with zero repair applied. The best cell reaches **160.985 mm**, which is
  **8.266 mm below the previous best-from-request layout on this request**
  (169.251 mm, opportunity-ledger arm C).
* **The design's own rollback contract is what nearly sank it.** With the
  rollback armed at 32 steps — the value this round chose before measuring —
  the same schedule publishes 8 of 12 at a median 0.359 mm and **loses** to the
  ladder in 9 of 12 cells. Disarming it is worth a median **11.751 mm**. That
  is the anatomy's own headline finding about mode 26, reproduced one level
  down inside the port that was built to avoid it, and it is why
  `rollback_after_steps` now defaults to `0`.

Two corrections to the design document fall out of the same evidence, both
measured: the exact confirmation costs **4.83 ms**, not the 0.491 ms the
anatomy budgeted; and every parent in the 171-179 mm band arrives at the
relaxed lane already proxy-infeasible, with 26-38 colliding pairs.

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_8687e703-3d9-2` |
| base commit | `fccda7f` (merge lane stage 2) |
| request | `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`, sha256 `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3` |
| contract | true 5.0/5.0 exact clearance, search-offset allowance **`0.002`** for the gate band, `0.0005` for the record-line contrast |
| pristine gate binary (`jagua-experimental`, before the change) | sha256 `d17533dc47e0686e9ea3809e1fd0b29ed8817f4c88cf644211a6df2aa7e3d7c5` |
| final gate binary (`jagua-experimental`, after the change) | sha256 `a772b82f36bff6f1a2c065c77d05fbc11d25f30bbf75784d261e7b9c45a3548f` |
| schedule binary (`jagua-experimental,compression-schedule`, final tree) | sha256 `7bd05065448af0659eed09e1180fa840987d2932b2801fe3714ef6493d386a79` |
| gate-run schedule binary (rollback default `32`, every arm's rollback set explicitly) | sha256 `1bc56db51b605dc35701b2fa0934ae56106333015c8e619f4970daddaf10fd6e` |
| box | x86_64, 16 cores, engine pinned at 8 threads, **shared with another measurement agent** |

The allowance is `0.002`, which is coordinator v2's and the opportunity
ledger's, **not** the four pinned gates' `0.0005`. Every depth in the gate band
is therefore comparable to 174.208 / 176.056 / 179.006 and to the ledger's
169.251 / 171.739, and **is not** comparable to the 159.079 / 164.038 record
lineage.

The two schedule binaries differ only in the compiled-in default of
`rollback_after_steps` (32 before the measurement, 0 after) and in doc comments;
the gate drives every schedule arm with an explicit `rollback=` key, and the
final binary reproduces the gate's no-rollback arm on seed 0 field for field —
same raw depth `164.008`, same fingerprint
`ec7cc0bf8a085e0e8908546b02bac3ddd64a935d4c63824c6369f53710ac8457`, same step
and confirmation counts, differing only in the two wall-clock fields.

## 1. What was built

One feature, `compression-schedule`, off by default. The five pieces the
anatomy's §2.2 listed as missing, all five built:

| piece | where | what it is |
|---|---|---|
| **(a)** a lane-owned schedule driving `strip_depth_mm` per sweep | `search/compression_schedule.rs`, and one call at the top of `LaneSearch::move_sweep` | `CompressionSchedule` owns the depth in **canonical grid units** and `move_sweep` writes `schedule.depth_mm()` into the state at entry. That one `f64` write reaches all eleven `boundary_penalty` call sites and all four candidate generators, because every one of them already reads that scalar. Zero new geometry, exactly as the anatomy priced it. |
| **(b)** a monotone floor in the proxy tier | `CompressionSchedule::floor_mm` | The depth of the deepest *confirmed* layout. It only ever decreases, it lives on the **lane** rather than on the state — a state can be cloned, projected and restored by paths that predate the schedule — and `depth_grid <= floor_grid` is a `debug_assert` at every mutation, not an argument. |
| **(c)** a deepest-confirmed slot | `drive_compression_schedule`'s `confirmed_state` / `published_placements` | The incumbent/frontier asymmetry at the finer grain: the frontier may be proxy-infeasible for as long as it likes, and it is for 82% of its steps; the slot is written only by an exact confirmation of the frontier *itself*. A layout that only a repair pass could rescue is published but does **not** move the floor, because the floor's layout has to be the layout the lane is holding. |
| **(d)** an affordable repair from the existing violating-pair queue | the sweeps themselves, plus one `micro_legalize` on a refused confirmation | A step that makes `k` pieces protrude puts exactly those `k` pieces into the next sweep's active set through `PairTracker::collision_pairs` and `piece_is_active`. No new selection logic was written. |
| **(e)** a rollback contract that survives a moving depth | `CompressionSchedule::rollback_to_floor` + the caller's paired restore | It declines to inherit the coupled separator's rollback rescore entirely, which is the anatomy's own mitigation. What replaces it: the only restorable state is the deepest-confirmed slot, and that slot is a `(layout, depth)` **pair** written by one confirmation at one instant. After an accepted confirmation the floor *is* the frontier (`debug_assert_eq!`), so the depth a rollback restores is exactly the depth the layout was confirmed at. |

The step is derived, not chosen: `canonical_grid_step_mm()` is `from_grid(1.0)`
through the canonical-grid authority, one unit of the lattice `snap_mm` rounds
every translation to. Depths are carried in grid units rather than millimetres
throughout, because a schedule that takes 35,000 steps and subtracts `0.001`
each time stops one step early and reports depths that are not on the pose
lattice.

Mode 34 drives it. It is the same shape as mode 26 — validate the parent,
measure it, walk down, publish the deepest exact-valid layout with the parent
as the floor — and it never touches `fast_settings.sheet_long_axis_mm`: the
clamp is a proxy-tier scalar, and every candidate is validated against the real
request.

## 2. Regression

The four pinned gates were run on four binaries: the pristine tree's
default-feature binary (`base`), this tree's default-feature binary with the
feature *off* (`after`), the gate-run `compression-schedule` binary (`armed`),
and the final tree's `compression-schedule` binary (`final`) — the last two with
the feature compiled in and the schedule unarmed, which is what every
invocation other than mode 34 does in a schedule-capable build.

| gate | pinned | `base` | `after` | `armed` | `final` |
|---|---|---|---|---|---|
| g1 mode 20 | 206.869 / `8a7737381238fa4d` | hit | hit | hit | hit |
| g2 mode 22 | 159.09233022733062 / `fa01012af1d559ae` | hit | hit | hit | hit |
| g3 mode 22 | 159.07876040364795 / `e28fba007f8031d4` | hit | hit | hit | hit |
| g4 mode 22 | 164.0375677990678 / `49f094d7e59a9008` | hit | hit | hit | hit |

Whole-document comparison against the pristine binary, wall-clock and
build-identity fields removed and listed in `evidence/gates-docdiff.json`:

| comparison | fields compared (g1/g2/g3/g4) | differences |
|---|---|---|
| `after` vs `base` | 3,263 / 3,244 / 3,244 / 3,244 | **0** |
| `final` vs `base` | 3,263 / 3,244 / 3,244 / 3,244 | **0** |

All four are `exactValid` and `contractValid` in every run. The default build is
byte-identical in behaviour and the *feature-compiled-in* build is too, which is
the stronger statement: the feature adds a field that no existing caller
constructs, one `#[cfg]`-paired call whose disabled half has no body, and one
match arm.

Release suite, `cargo test --release`: **1,238 passed, 0 failed, 2 ignored**
with `jagua-experimental` — unchanged from the count before the change — and
**1,250 passed, 0 failed, 2 ignored** with
`jagua-experimental,compression-schedule`. The twelve new tests are the
schedule's eight invariant tests, three end-to-end mode-34 tests on the same
two-piece fixture the mode-26 ladder tests use, and one that pins an *unarmed*
lane leaving the depth and the tracker untouched.

## 3. The parent band

The anatomy measured mode 26's cost at 159 mm and 164 mm parents and its yield
not at all — zero of eight ladders published — and the opportunity ledger then
measured the same operator at the coordinator's own 174-179 mm parents and got
two publications in three. A gate run at 159/164 would compare two zeros, so
this one is run where the control publishes.

Twelve parents, one per seed, each the published incumbent of the pinned
coordinator run from the bare request at `work=120,000,000`, written out as a
pinned fixture. Both arms of every cell descend from *that file*.

| seed | raw depth (mm) | work spent | dual gate | in 174-179 band |
|---:|---:|---:|---|---|
| 0 | **174.20812003998896** | 32,393,757 | true | yes |
| 1 | **176.05599999999998** | 31,957,935 | true | yes |
| 2 | **179.006** | 27,938,867 | true | yes |
| 3 | 176.061 | 31,395,350 | true | yes |
| 4 | 171.64953207726535 | 31,897,492 | true | below |
| 5 | 179.05182605364416 | 28,895,961 | true | yes |
| 6 | 179.6200102363703 | 31,130,162 | true | above |
| 7 | 179.52233303152792 | 35,396,236 | true | above |
| 8 | 178.93200000000002 | 33,901,660 | true | yes |
| 9 | 174.96558182288433 | 27,877,624 | true | yes |
| 10 | 176.3622237458826 | 29,620,324 | true | yes |
| 11 | 171.6141235046606 | 32,775,448 | true | below |

Seeds 0, 1 and 2 reproduce the opportunity ledger's three depths **to the
digit**, from a different worktree and a different binary, which is the check
that the parents are the parents that round measured.

## 4. The matched-arm gate

Twelve cells. Both arms start from the same pinned parent at the same relaxed
seed, and both are given the same allowance: **33,413,789 work units**, one
measured mode-26 rung (32,246,564 candidate queries + 5 x 233,445 exact pair
tests). The statistic is the raw source depth of the best exact-valid
publication, with the parent as the floor for both arms — which is the contract
both modes already publish under, so an arm that finds nothing scores zero
rather than being dropped.

The arms, all four measured:

* `m26` — one legacy short mode-26 ladder to `parent - 0.3 mm`, the anatomy's
  shortest sampled drop and the ledger's arm C, minus the coordinator-level
  mode-31 rung that followed it there (that rung is a second, outer legalizer,
  not part of the operator under comparison).
* `sched` — the schedule at the same allowance, asked for the same drop and
  allowed to continue past it, with the rollback armed at 32 steps.
* `sched-noroll` — the same, rollback disarmed.
* `sched10` / `sched10-noroll` — the same two at **3,341,379** units, 10% of a
  rung, the middle of the 5.9-11.7% band the anatomy's 0.5-1.0 s design slice
  works out to. These are the *cost* arms.

### 4.1 The table

Work is the operator's own: the whole-process counter minus the identical
mode-0 preamble the same cell measured (6.8-11.9M units, the coupled
separator's own arms, which every arm in a cell pays).

| arm | publishes | median Δ (mm) | mean Δ | best Δ | median work | **mm / M units** |
|---|---:|---:|---:|---:|---:|---:|
| `m26` (control) | 10 / 12 | 0.876 | 2.435 | 8.172 | 14,755,710 | 0.168 |
| `sched` (rollback 32) | 8 / 12 | 0.359 | 0.962 | 3.258 | 32,468,155 | 0.031 |
| **`sched-noroll`** | **12 / 12** | **12.110** | 11.807 | 15.618 | 17,481,265 | **0.623** |
| `sched10` (rollback 32) | 6 / 12 | 0.019 | 0.281 | 1.406 | 2,705,476 | 0.119 |
| **`sched10-noroll`** | **11 / 12** | **1.104** | 1.072 | 1.663 | **869,133** | **1.013** |

Paired per cell, schedule minus control:

| pairing | median advantage (mm) | schedule wins | tied | ladder wins |
|---|---:|---:|---:|---:|
| `sched-noroll` − `m26` | **+7.479** | **12** | 0 | 0 |
| `sched10-noroll` − `m26` | −0.505 | 4 | 0 | 8 |
| `sched` − `m26` | −0.562 | 2 | 1 | 9 |
| `sched10` − `m26` | −0.663 | 1 | 1 | 10 |

And per cell, the number each arm published:

| seed | parent | `m26` | `sched-noroll` | `sched10-noroll` |
|---:|---:|---:|---:|---:|
| 0 | 174.208 | 169.655 | **164.008** | 173.380 |
| 1 | 176.056 | 172.143 | **164.103** | 174.576 |
| 2 | 179.006 | 179.006 | **164.304** | 177.343 |
| 3 | 176.061 | 175.059 | **168.582** | 176.061 |
| 4 | 171.650 | 171.074 | **164.155** | 171.082 |
| 5 | 179.052 | 179.052 | **165.659** | 177.563 |
| 6 | 179.620 | 179.520 | **164.002** | 178.403 |
| 7 | 179.522 | 179.482 | **164.060** | 178.264 |
| 8 | 178.932 | 170.760 | **166.666** | 177.280 |
| 9 | 174.966 | 169.669 | **164.127** | 173.974 |
| 10 | 176.362 | 171.544 | **160.985** | 175.394 |
| 11 | 171.614 | 170.865 | **164.716** | 170.868 |

### 4.2 At the control's own measured spend

Equal *allowance* is the primary statistic — it is the opportunity ledger's,
and an operator that stops early should be rewarded rather than padded out —
but the schedule records the queries every step cost and the depth every
confirmation measured, so it can also be read at the moment the control
stopped. That is a reconstruction of something the run really did, not an
interpolation.

| statistic | value |
|---|---|
| median advantage at the control's own spend | **+4.340 mm** |
| cells the schedule wins | **12 of 12** |

The equal-allowance comparison is if anything generous to the *control*: the
schedule's self-meter over-charges its own exact tier by about 18x (see §6), so
`sched-noroll` stopped at a median 17.5M of the coordinator's units against a
33.4M allowance — 52% of a rung.

### 4.3 Independent confirmation of every publication

Each published layout was written out as a pinned-parent fixture and replayed
through **mode 27** — the micro-legalization probe, the mode meant to be
pointed at states that may not validate — in a separate process, from the
**pristine default-feature binary**, which contains no compression-schedule
code and no mode 34 at all.

| arm | published | confirmed `exactValid` **and** `contractValid`, fingerprint unchanged, zero pieces moved |
|---|---:|---:|
| `sched-noroll` | 12 | **12** |
| `sched10-noroll` | 11 | **11** |

`evidence/confirmations-noroll.json` and
`evidence/confirmations-sched10-noroll.json` carry all twenty-four rows,
including the one cell that published nothing, because `0 of 12` is a result.

## 5. The rollback is 97% of the port's depth

This is the round's structural finding and it was not predicted.

`rollback_after_steps = 32` — 32 one-micron steps without an accepted
confirmation and the frontier is given back to the deepest-confirmed layout —
was this round's own choice, made from the anatomy's design and before any
measurement. Against `0`, on the same twelve cells at the same allowance:

| | rollback 32 | rollback 0 |
|---|---:|---:|
| publications | 8 / 12 | **12 / 12** |
| median Δ | 0.359 mm | **12.110 mm** |
| mm / M units | 0.031 | **0.623** |
| median rollbacks per arm | 685 | 0 |
| median confirmations accepted | 128 | **1,838** |

The mechanism is visible in the step rows. The frontier is proxy-infeasible for
**82%** of its steps by construction — that is what a compression frontier
*is* — so a rollback that fires after 32 steps without an accepted confirmation
fires almost every time it can, and the schedule spends its budget descending 32
microns, being thrown back, and descending again. It never accumulates.

The mode-26 anatomy's headline was that 85.4% of legacy rung arms abort on a
rollback and burn 75.5% of the arm wall. The port was designed specifically to
avoid inheriting that rollback — and then introduced its own, and the new one
cost 97% of the depth. The lesson generalises past this port: **a rollback whose
trigger is "the frontier has not been publishable lately" is a rollback that
fires on the normal state of a compression frontier.**

The mechanism is kept, correct and tested, and defaults to off. `0` is now the
compiled-in default of `CompressionScheduleSettings`, and the doc comment on the
field carries these numbers.

## 6. Cost, and two corrections to the design

### 6.1 One exact confirmation costs 4.83 ms, not 0.491 ms

The anatomy's §2.3 called 0.491 ms "the hinge of the porting design" and
budgeted 40 confirmations at 19.6 ms, 2.0% of a 1.0 s slice. Measured here,
over 23,176 confirmations across the twelve cells and both budgets:

| statistic | value |
|---|---|
| mean ms per confirmation, `sched-noroll` pooled | **4.828** |
| mean ms per confirmation, `sched10-noroll` pooled | **4.717** |
| range over cells | 4.18 - 5.65 |

The anatomy's own phase table implies exactly this and the discrepancy is
arithmetic, not a measurement conflict: a confirmation the validator **accepts**
asks all `61 * 60 / 2 = 1,830` pairs, and at that round's own 1,904.8 ns per
`exactOverlapTest` that is 3.485 ms, plus 61 collision-polygon builds at
4,149.3 ns = 0.253 ms, plus 0.049 ms of depth — 3.79 ms before anything else.
**The 0.491 ms figure is the cost of a confirmation that fails**, which exits at
the first violating pair; the anatomy's 25 samples were all of rejections,
because zero of its 171 arms produced an exact-valid state.

The consequence for the design budget is real. At the designed cadence the
exact tier would be 20% of a 1.0 s slice, not 2%. What saves it is the second
clause of the confirmation trigger, which the design did not name: a layout the
proxy tier already calls infeasible is never offered to the exact validator.
That suppresses **82%** of the confirmations the cadence makes due in the
rung-allowance arm and 69% in the design-slice arm, and the achieved exact-tier
share of the schedule's own wall is 24.6% and 51.9% respectively.

**So the port's exact tier is over the anatomy's 5% rule by a wide margin, on
the clock.** It is not over on the work meter, which is what the gate is
denominated in, and that difference is itself a finding: see §6.3.

### 6.2 The design slice buys 6.0x the ladder's depth per unit of work

The anatomy's design budget is a 0.5-1.0 s slice, 5.9-11.7% of a rung. The
`sched10-noroll` arm is capped at 10% of a rung in the schedule's own
conservative currency and spends a median of **869,133** of the coordinator's
units — **2.6% of a rung** — for a median **1.104 mm**:

| arm | median operator work | as % of one rung | median Δ | mm / M units | vs the ladder |
|---|---:|---:|---:|---:|---:|
| `m26` | 14,755,710 | 44.2% | 0.876 mm | 0.168 | 1.0x |
| `sched10-noroll` | 869,133 | **2.6%** | 1.104 mm | **1.013** | **6.0x** |
| `sched-noroll` | 17,481,265 | 52.3% | 12.110 mm | 0.623 | 3.7x |

The schedule is more efficient per unit of work than the ladder at both budgets,
and it is *most* efficient at the small one — the marginal value of its work
decreases, which is the shape an anytime operator should have and the ladder
does not (a ladder either publishes a rung or does not).

### 6.3 Equal work is not equal wall, and the gap is the exact tier

Three separate reasons the wall clock and the work meter disagree here, all
measured, none of them a reason to prefer either number on its own:

1. **The schedule is one lane; the mode-26 pipeline is eight.** A rung arm runs
   `run_independent_lanes` across the job pool, so its candidate queries are
   spread over eight workers. The schedule's are not. Median process wall was
   7.19 s for `m26`, 35.25 s for `sched-noroll` and 4.58 s for
   `sched10-noroll` (most of which is the shared preamble) — but the box was
   shared with another measurement agent for part of this campaign and **no
   wall-clock claim is made here**.
2. **The work meter counts the narrow phase of the exact tier only.**
   `Counter::ExactPairTests` is incremented past the broad-phase bounds reject
   (`kernel::exact`), so a 1,830-pair confirmation charges about 99 tests, ~493
   units — while it costs 4.8 ms. On the schedule's arms the exact tier is
   24-52% of the wall and about 4% of the metered work.
3. **The schedule's self-meter deliberately over-charges.** It charges all
   1,830 asked pairs, ~18x the coordinator's meter, because the cap has to be
   deterministic and load-independent without a profiling build. That is
   conservative in the direction that hurts the schedule: `sched-noroll` used
   52% of the allowance in the coordinator's currency.

## 7. What the schedule found out about the lane it runs in

Two measurements that are about the pipeline rather than about this port, and
that no previous round could make because no previous round asked the proxy
tier what it thought of a *parent*.

### 7.1 Every 171-179 mm parent is proxy-infeasible on arrival

| statistic over the 12 parents | value |
|---|---|
| parents the proxy tier calls feasible | **0 of 12** |
| colliding pairs at the parent | 26 - 38 |
| boundary violations at the parent | 4 - 11 |

Every one of these parents is `exactValid` and `contractValid`. The cause is
upstream of both modes and is not something the schedule introduces:
`initialize_complete_state` (`general_relaxed.rs:15363`) maps a warm start's
rotations through `canonical_angle` (`general_relaxed.rs:15397`,
`general_relaxed.rs:17107`), which snaps every angle onto the structured
surrogate's `SURROGATE_ANGLE_STEP_DEG = 2.5` grid. Seventeen of the 61 poses in
seed 0's parent are off that grid; all 61 of the 159.079 record parent's are.

### 7.2 The entry transform costs a median of 0.448 mm

The first depth an exact validator accepts after the entry transform, against
the parent's own:

| statistic | value |
|---|---|
| median entry loss | **+0.448 mm** |
| range | −6.031 to +0.999 mm |
| cells measured | 11 of 12 (one never confirmed) |

It is an upper bound on the snap alone, because the steps before the first
confirmation also ran repair. It is *not* a criticism of the port — mode 26
pays exactly the same cost through exactly the same function — but it is the
first thing to fix if this band is to be pushed further, and it is a protected
shared path: changing `canonical_angle`'s treatment of a warm start changes
mode 0, mode 22, mode 23 and mode 26 trajectories at once, so it is out of
scope here and named rather than touched.

## 8. The record-line contrast

For contrast only. The record lineage is a different search envelope —
allowance `0.0005`, not `0.002` — so nothing here is comparable to §4.

| parent | arm | exact / contract | published | Δ | work |
|---|---|---|---:|---:|---:|
| `pinned-parent-159.079` (159.07876040364795) | `m26` | true / true | 159.07876040364795 | 0.000 | 50,258,652 |
| `pinned-parent-159.079` | `sched` | true / true | 159.07876040364795 | 0.000 | 41,053,511 |
| `pinned-fs-parent-164.0376` (164.0375677990678) | `m26` | true / true | 164.0375677990678 | 0.000 | 43,228,124 |
| `pinned-fs-parent-164.0376` | **`sched`** | true / true | **159.668** | **−4.370** | 39,697,926 |

The linear record parent is a fixpoint for both operators, which reproduces the
anatomy's 0-of-171. The from-scratch parent is not: the schedule takes it 4.370
mm down to **159.668 mm**, independently confirmed through mode 27 on the
pristine binary at the same `0.0005` allowance with the fingerprint unchanged
and zero pieces moved. That is not a record — the linear lineage holds
159.07876040364795 — but it closes most of the gap between the two lineages that
the descent campaign left open.

## 9. Honest limits

* **One request, twelve seeds.** Nothing here says anything about shapes-17,
  triangle-20 or a fourth request. Coordinator v2's generality finding applies
  with full force.
* **No wall-clock claim is made, and the box was shared.** Every quality and
  cost comparison is in work units, which is why the work-budget mode exists.
  The wall numbers are reported because §6.3 needs the reader to see that they
  disagree with the meter, not as a measurement of anything.
* **The two work readings disagree by a factor that depends on the operator.**
  The coordinator's meter counts narrow-phase exact tests; the schedule's own
  meter counts asked pairs. Both are reported, the mm-per-unit table uses the
  coordinator's for both arms, and the disagreement is quantified in §6.3
  rather than smoothed over.
* **The 0.3 mm drop is the control's request, and neither arm honours it.** The
  ledger already measured mode 26 publishing 4.25 mm below a 0.174 mm rung; the
  schedule is explicitly allowed to continue past its bound for the same
  reason. Both arms are therefore compared on an allowance, not on a target,
  and "the same drop" is a statement about how they were *asked*, not about
  what they did.
* **`micro_legalize` was never invoked, and so is untested here.** Zero of
  23,176 confirmations were refused: at a one-micron step, a layout the proxy
  tier calls feasible was *always* exact-valid. The anatomy's second-biggest
  risk — that the residue is not rounding-scale and the affordable repair only
  handles rounding-scale — is answered on this fixture by the residue never
  reaching the confirmation, but the tier itself carries no evidence from this
  round.
* **The rollback verdict is a verdict about one trigger.** `0` and `32` steps
  were measured; 128, 512 and a trigger keyed on something other than
  confirmation success were not. The claim is that *this* trigger is harmful,
  not that no rollback can help.
* **The schedule is single-lane.** Everything above is one lane's trajectory
  from one parent. What eight schedule lanes with different seeds would do, and
  whether the coordinator should spend a phase on them, is not measured.
* **Nothing here is a schedule change.** No default outside the new feature was
  moved, the portfolio coordinator has no compression-schedule phase, and mode
  34 is reachable only from an explicit CLI mode with the feature compiled in.

## Files

* `drivers/lib.py`, `drivers/gates.py` — the shared runner and the four pinned
  gates, `ROOT` repointed at this worktree.
* `drivers/runlib.py` — the pinned CLI tail, the salt sets and the `0.002`
  allowance, copied from the opportunity ledger.
* `drivers/parents.py` — the twelve coordinator parents.
* `drivers/gate.py` — the matched-arm gate, all five arms plus the preamble.
* `drivers/merge.py` — joins two gate documents over the same parents.
* `drivers/summarize.py` — the paired-delta and cost-per-mm tables.
* `drivers/curve.py` — the depth-versus-work curve and the read at the
  control's own spend.
* `drivers/confirm.py`, `drivers/confirmall.py` — the independent mode-27
  confirmation of every publication.
* `drivers/records.py` — the two record-lineage parents at `0.0005`.
* `drivers/docdiff.py` — the whole-document gate comparison.
* `evidence/gate.json` — every arm of every cell as the driver emitted it.
* `evidence/gate-summary.json` — the §4 tables.
* `evidence/gate-curve.json` — the §4.2 curves and the §7 entry-loss rows.
* `evidence/parents.json` — the twelve parents.
* `evidence/confirmations-noroll.json`,
  `evidence/confirmations-sched10-noroll.json` — the twenty-four replays.
* `evidence/records.json` — the record-line contrast.
* `evidence/gates-*.json` — the four pinned gates on four binaries, and the
  two whole-document diffs.
* `evidence/suite.log`, `evidence/suite-armed.log` — the release suite with the
  feature off (1,238 passed, 0 failed, 2 ignored) and on (1,250 passed, 0
  failed, 2 ignored).

Reproduce:

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental                          # gate binary
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,compression-schedule     # schedule binary

python3 drivers/gates.py base  <pristine-binary>  <gatesdir>
python3 drivers/gates.py after  <gate-binary>     <gatesdir>
python3 drivers/gates.py final  <schedule-binary> <gatesdir>
python3 drivers/docdiff.py <gatesdir> base after
python3 drivers/docdiff.py <gatesdir> base final

python3 drivers/parents.py 0,1,2,3,4,5,6,7,8,9,10,11 <gate-binary> <parentsdir>
python3 drivers/gate.py <parentsdir> <gatedir> <schedule-binary> <gate-binary> \
    preamble,m26,sched,sched-noroll,sched10,sched10-noroll
python3 drivers/summarize.py <gatedir>/gate.json gate-summary.json
python3 drivers/curve.py <gatedir>/gate.json <gatedir> sched-noroll \
    gate-curve.json
python3 drivers/confirmall.py <gatedir> sched-noroll <confirmdir> \
    <pristine-binary> confirmations-noroll.json
python3 drivers/records.py <recordsdir> <schedule-binary> <gate-binary>
```

The schedule's knobs are read from `POLYGON_NESTING_COMPRESSION_SCHEDULE`
(`sweeps=`, `confirm=`, `rollback=`, `work=`, `past=`, `repair=`) for the same
reason profiling is read from the environment: the positional argument list is a
pinned contract that replay drivers depend on, and a new knob may not change
what a replayed command means. Mode 34's *bound* is the ordinary positional
target-depth slot, argument 45 — the same slot mode 26 reads.
