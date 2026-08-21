# Continuous rotation in the relaxed lane: the mechanism works, and it is not worth what it costs at ten seconds

> Design A of the continuous-rotation brief: rotation as a continuous degree of
> freedom in the relaxed lane's own candidate loop, feature flag
> `continuous-rotation`, off by default. Base commit `92a1a08` (the merged 10 s
> round). x86_64, 16 cores, box shared with one other measurement agent for the
> whole campaign, so every wall claim below is a **per-round paired** difference
> with the within-arm spread printed beside it.

## The result in one table

Anytime WALL from the bare request, both arms with `m34pconfirm=1` armed, one
binary, one key apart (`crot=0` / `crot=1`), three seeds x three rounds = nine
paired cells per request per budget. The statistic is the paired difference in
published depth, **`crot` minus `base`, so a negative number is the operator
winning**.

| request | 3 s | 10 s | 30 s |
|---|---|---|---|
| **mixed-61** | +0.000 mm, 3 of 9 better | **+3.721 mm worse, 0 of 9 better** | **+7.071 mm worse, 1 of 9 better** |
| shapes-17 | +0.000 mm, 0 of 9 | +0.000 mm, 0 of 9 | +0.000 mm, 0 of 9 |
| triangle-20 | +0.000 mm, 3 of 9 | +0.000 mm, 0 of 9 | +0.000 mm, 0 of 9 |

The round's success criterion was "mixed-61 10 s strictly better than base on
9 of 9". It is better on **0 of 9**. This is the honest negative the brief asked
for if the operator did not pay — but the decomposition is not the one the brief
anticipated. Acceptance is **not** about zero and the millimetres bought are
**not** about zero, on mixed-61:

* **655,477 rotation/mirror iterations** at ten seconds, **8.3%** of them
  improving the incumbent;
* **56.0%** of all the proxy loss the refinement removed was removed by a
  rotation or mirror move, against 44.0% by the four translation axes;
* **67.1%** of the moves the sweep committed changed the piece's rotation or
  mirror, against 8.9% in the unarmed arm;
* and the rungs reach the sheet: an armed ten-second publication carries
  **46 of 61** pieces at angles the 2.5-degree catalogue cannot express, against
  27 of 61 unarmed (`drivers/offgrid.py`).

The mechanism does what the SE(2) certificate said it would, inside the search,
at production cost. It loses anyway, and §3 is why.

**Against Sparrow** (157.971 @ 3 s / 150.165 @ 10 s, same box, mixed-61): the
base arm's medians are 179.587 @ 3 s and 172.288 @ 10 s, so the engine is
**21.6 mm** and **22.1 mm** behind. The armed arm is 179.633 and 175.219 —
**21.7 mm** and **25.1 mm** behind. The operator moves the engine *away* from
Sparrow at the budget the binding user priority names.

---

## 1. What was built

`GeneralRelaxedSettings::continuous_rotation`, compiled only under the
`continuous-rotation` feature, off by default, every existing constructor
setting it to `false`. When armed on a lane that resolves poses through the
surrogate catalogue (`RollbackTriangle` + `StructuredTrianglePoles` — see
`continuous_rotation_lane`), `refine_candidate`'s coordinate-descent axis
schedule gains two axes:

* **`CoordinateAxis::Rotation`**, in both signs, at a rung derived per piece:
  **`dtheta = dx / r`**, where `r` is the piece's bounding radius about its own
  rotation origin and `dx` is the larger of the two live translation steps of
  the same schedule. Nothing is tuned — the descent's own contraction contracts
  the rung with it — and `dx = 0.001 mm` on a 20 mm radius *is* 0.0029 degrees,
  which is the scale of the 0.0032 / 0.008 / 0.00128-degree rungs the record
  line adopted. On top of `dx / r` the family carries its own scale, contracted
  by its own outcomes and floored at 1/64, so that a run of rejected rotations
  narrows the *rotation* probe instead of shortening the translation descent —
  a lane whose translation ladder collapsed because rotation kept failing would
  not be a measurement of rotation.
* **`CoordinateAxis::Mirror`**, the discrete companion: the flip at the
  incumbent's angle and the flip one rung away from it, both recentred so the
  flipped pose keeps the unflipped one's bounds centre. Without the recentring a
  mirror is a teleport rather than a candidate, which is why the record line's
  155.456 → 155.422 step needed a ~130x50 mm relocation to go with its flip.

Each is pushed **once** into the axis list, against the legacy rotation ladder's
two, so a rotation iteration is one in six rather than one in three. The
asymmetry is a price, not a preference: a translation probe costs a candidate
scan and a rotation probe costs a candidate scan *plus*, on a cache miss, an
`OrientedSurrogate` build.

**The surrogate at a continuous angle** is built on demand through
`build_oriented_surrogate` — the construction path the `CurrentPoseOverlay`
round hardened, never a catalogue clone and never an enumeration of the angle
space — into a lane-local map with the brief's cache shape: a **per-piece pinned
slot** for the pose the lane's state holds, an in-flight **hold set** for what a
refinement is carrying, and a **48-entry LRU** for everything else. Eviction
happens in exactly one place, `release_rotation_holds`, at the top of
`search_piece`, so nothing can be evicted while anything can still resolve it —
and because eviction takes `&mut self`, the borrow checker proves no resolved
`&OrientedSurrogate` is alive across it. `resolve_surrogate` consults the
catalogue first and the lane map only on a miss.

### 1.1 The publication path, checked before the operator was written

The brief's step 2. `validate_and_measure_placements` (`general_fast.rs:3546`,
body `validate_and_measure_placements_inner` at `general_fast.rs:3582`) rebuilds
each placement by `piece.polygon.transformed(placement.rotation_deg, ...)` and
enforces exactly two rotation rules: the angle must be finite, and
`!piece.allow_rotation` requires `angle_key(placement.rotation_deg) == 0`. There
is **no snap**: `canonical_angle` is defined at `general_relaxed.rs:18024` and
appears nowhere outside that file, so nothing between an accepted rung and the
sheet re-quantises it. `to_fast_placements` (`general_relaxed.rs:16545`) copies
`rotation_deg` verbatim.

The warm-start snap the brief warned about is
`initialize_complete_state`'s `_ => canonical_angle(existing.rotation_deg)` arm
(`general_relaxed.rs:16313`), and it runs **before** the lane starts, on the
parent's placements. The operator's angles are introduced during the lane and
never pass through it again.

Pinned by `the_exact_validator_accepts_a_continuous_rotation`, which publishes at
13.37, 0.0032 and 47.00812 degrees and asserts each is off the grid first so the
test cannot go vacuous.

### 1.2 The one thing that had to change in a hot path

An accepted rung puts a continuous angle **into the lane's state**, so
`derive_rotation_key` has to derive that piece's key from the exact angle from
then on. `continuous_rotation_keys` therefore returns true for an armed lane —
the same correction the overlay round needed, for the same reason: without it
the placement would carry 13.37 degrees and every score would be computed at
12.5, silently.

The consequence is the cost §3 is mostly about. Every neighbour resolution for an
off-grid piece **misses the catalogue and then hits the lane map** — two
ordered-map descents where the unarmed lane does one — on the file's
most-called path, plus one more to materialise each rotation candidate before it
can be scored.

---

## 2. The anytime battery

`drivers/battery.py`, `evidence/curve-mixed61.json`, `curve-shapes17.json`,
`curve-triangle20.json`, summarised by `drivers/summarize.py` into
`evidence/curves-summary.json`.

### mixed-61 — the request the campaign's records are on

| budget | base median | crot median | paired median | rounds crot better | paired range | base spread | crot spread |
|---|---|---|---|---|---|---|---|
| 3 s | 179.587 | 179.633 | +0.000 mm | 3 of 9 | [−0.019, +0.627] | 0.627 | 0.065 |
| **10 s** | **172.288** | **175.219** | **+3.721 mm** | **0 of 9** | [+0.396, +7.880] | 5.572 | 3.325 |
| **30 s** | **163.927** | **169.067** | **+7.071 mm** | **1 of 9** | [−0.739, +8.409] | 5.788 | 3.985 |

The within-arm spread is larger than the median delta at both losing budgets, so
the unpaired numbers would not carry this claim. The **paired** count does: in
9 of 9 rounds at ten seconds and 8 of 9 at thirty, the same seed in the same
round did better without the operator. The nine ten-second pairs, in full:

| seed | round | base | crot | delta |
|---|---|---|---|---|
| 0 | 0 | 172.288 | 175.219 | +2.931 |
| 0 | 1 | 172.288 | 175.219 | +2.931 |
| 0 | 2 | 170.453 | 175.219 | +4.766 |
| 1 | 0 | 168.708 | 175.925 | +7.217 |
| 1 | 1 | 168.708 | 176.588 | +7.880 |
| 1 | 2 | 168.708 | 176.588 | +7.880 |
| 2 | 0 | 174.280 | 174.676 | +0.396 |
| 2 | 1 | 174.280 | 174.676 | +0.396 |
| 2 | 2 | 174.280 | 178.001 | +3.721 |

At **three seconds** neither arm runs a mode-34 slice at all, and the two arms
differ only through mode 22: three rounds by −0.019 mm and three by +0.627 mm,
median zero. The three-second tier is not evidence either way.

### shapes-17 and triangle-20 — no corpus regression, and no effect

| request | budget | base median | crot median | paired median | rounds better |
|---|---|---|---|---|---|
| shapes-17 | 3 / 10 / 30 s | 200.349 | 200.349 | +0.000 mm | 0 of 9 |
| triangle-20 | 3 s | 70.747 | 70.747 | +0.000 mm | 3 of 9 |
| triangle-20 | 10 s | 70.730 | 70.731 | +0.000 mm | 0 of 9 |
| triangle-20 | 30 s | 70.727 | 70.727 | +0.000 mm | 0 of 9 |

Both requests saturate: their mode-34 slice publishes on **0 of 9** runs in
*both* arms, so the operator has nothing it can change about the incumbent. What
it does change is the slice's wall — shapes-17 10.1 s → 12.9 s and triangle-20
16.4 s → 22.3 s over the nine runs — bought and thrown away. The corpus
does not regress; it also does not move.

---

## 2.5 The equal-WORK gate, which is where the operator stops losing

`drivers/workgate.py`, `evidence/workgate-band.json` and `workgate-deep.json`.
Both arms replay **the same pinned parent at the same seed** through mode 34
under the same schedule work cap (the anatomy's design slice, 3,341,379
queries), on the same binary, differing only in
`POLYGON_NESTING_CONTINUOUS_ROTATION`. Work rather than wall is the point: the
operator's builds are not candidate queries, so at a fixed query cap the armed
arm gets the *same number of proxy questions* as the unarmed one and has to pay
for its rungs in quality rather than in seconds. It is the fairest test the
operator can be given, and it removes exactly the throughput loss §3 is about.

**The twelve 171-179 mm parents, drop 0.3 mm, allowance 0.002:**

| statistic | value |
|---|---|
| paired median | **+0.005 mm** |
| crot better / base better | **6 / 6** |
| range | [−1.681, +0.144] |
| median wall | 4.49 s → 6.59 s (**1.47x**) |
| median process work units | 9.99 M → 11.70 M (**1.17x**) |

Median zero, six all-square, and a **fat left tail**: on seed 3 the unarmed arm
publishes *nothing at all* — it returns its parent, 176.061 — and the armed arm
descends to **174.380**, a 1.681 mm win on a cell where translation alone found
no legal step. Seeds 11 and 8 are smaller versions of the same thing (−0.406,
−0.332). Every loss in the table is under 0.15 mm.

So the honest statement of the round is two-sided, and the sides do not
contradict each other:

* **at equal wall the operator is a clear loss** (§2), because it spends the
  clock that the coordinator's other actions were going to spend better;
* **at equal work it is a wash with a good tail** — median +0.005 mm, one cell
  where it is the difference between a publication and none.

The 1.17x on process work units is worth naming rather than hiding: the cap is
on the *schedule's* candidate queries and the process meter also counts exact
pair tests, so the armed arm's extra confirmations are not inside the cap. The
arms are equal in what the cap governs and the armed one spends 17% more of
what it does not.

**The two record-lineage depth-reach parents (156.418 and 155.42229074464285,
`''` 0.0005):** both arms publish **nothing** below either parent, so the
delta is 0.000 mm on 2 of 2 and this instrument has no resolution at the record
line at a design-slice cap. What the armed arm did there is still worth
recording, because it is the highest acceptance rate anywhere in the round:
**356,153 rotation iterations, 33.0% of them improving the incumbent** — four
times the coordinator's ten-second rate — for 3.83 s of surrogate builds and no
publication. Deep states have the most to gain from rotation and this operator
cannot convert it.

---

## 3. Where the millimetres went

The operator does not lose at the proxy tier. It loses the *clock*, and the
coordinator's own action log is where that is visible rather than inferred. One
ten-second run of mixed-61 at seed 0, the two arms side by side
(`evidence/smoke-*.json`):

| | base (`crot=0`) | armed (`crot=1`) |
|---|---|---|
| operator calls in the run | **9** | **7** |
| m22 call, mean | 0.77 s | 0.98 s |
| m34 slice | 0.89 s | 1.14 s and **3.13 s** |
| run incumbent | **172.288** | 175.219 |

The descent the base arm gets to make with its extra actions is the whole
difference: 179.59 → 176.11 → 175.39 → 175.14 → 173.58 → **172.29**, six
productive m22 publications, against the armed arm's 179.57 → 175.22 and then a
stall.

Aggregated over the nine paired ten-second rounds, the same effect in slices:
the base arm ran **30 m34 slices** and published on 30; the armed arm ran
**11** and published on 8. Per slice, 0.87 s becomes 1.94 s.

**Only a sixth of that is the surrogate builds.** They are measured directly:
3.51 s over the nine armed runs, **0.32 s per slice**, 5.4 microseconds per
rotation iteration at an **89.4% cache hit rate**, `rotationBuildsRefused` zero
everywhere. The other five sixths is the resolution tax of §1.2 — an armed lane
whose layout is 46/61 off-grid pays two ordered-map descents per neighbour on
the hottest loop in the engine, and a third to materialise each rotation
candidate before it can be scored.

That is the honest shape of the negative: **the operator's own geometry is
cheap and its effect on everything else's lookups is not.**

### 3.1 The first cut was four times worse, and the measurement is why the rungs are in the fine pass only

The first working version offered the derived axes in **every** refinement pass,
including the pre-refinement pass that runs once per start at
`PRE_REFINEMENT_INITIAL_RATIO = 0.25` of the piece's minimum dimension and owns
three quarters of the budget. A rung derived from a step that large is a
*reorientation* — tens of degrees, clamped at 45 — which is the move
`random_candidate`'s own angle sampling already makes for free out of the
catalogue, and every one of them costs a build the next start throws away.

Measured, mixed-61 seed 0, ten seconds, one cell each:

| | rungs in every pass | rungs in the fine pass only | unarmed |
|---|---|---|---|
| surrogate builds | 476,735 | 169,772 | 0 |
| build wall | **2.07 s** | 0.70 s | 0 |
| m34 slice wall | **4.21 s** | 1.14 s + 3.13 s | 0.89 s |
| m34 slice depth | 178.178 mm | 178.194 mm | **178.180 mm** |
| operator calls in the run | **3** | 7 | 9 |
| run incumbent | 178.178 | 175.219 | **172.288** |

Two microns of slice depth for 3.3 extra seconds. Both variants are negative;
the committed one is the better of the two, and the left column is kept because
"we measured it and it did not work" is a stronger statement when the thing
measured is written down. **These are one-cell probes, not a battery** — the
battery in §2 was run only on the committed variant.

### 3.2 A defect the smoke run caught, before any battery ran

The first armed run failed outright:

```
compression schedule: relaxed surrogate job may contain at most 524288 generated cells
```

`build_oriented_surrogate` enforces `MAX_CELLS_PER_JOB` against the
**cumulative** `generated_cells` of the counters it is handed. That cap is the
*catalogue's* guard — a catalogue is built once and is resident for the whole
job — and the operator's surrogates are the opposite: transient, evicted,
bounded by the LRU. Charging them to a lifetime counter fails the slice the
moment enough rungs have been proposed, which is what happened: one m34 slice,
then the coordinator's incumbent fell back to the m22 arm at 179.634.

The fix is a **residency** guard over the cells the lane's cache is actually
holding, checked against the same constant, with the catalogue's own counter
left exactly where the catalogue build put it. Pins are at most one per piece
and probes at most 48, so about 110 surrogates at 512 cells is an order of
magnitude under the ceiling — the guard never binds, and the battery confirms
it: `rotationBuildsRefused` is **0** in every armed cell of every request. When
it would bind, `prepare_continuous_candidate` proposes the incumbent's own pose
instead, so the lane stops offering rungs rather than failing.

### 3.3 A second defect, found by review rather than by a run

`CoordinateAxis::Mirror`'s second probe is the flip *one rung away* from the
incumbent's angle. Offered to a piece the request forbids rotating, it proposes
a rotated pose — which the proxy tier scores happily, the sweep commits happily,
and `validate_and_measure_placements` then refuses at publication with "piece
uses a forbidden rotation", failing the whole slice. **Every piece of all three
campaign requests allows both transforms**, so no measurement in this round
could have caught it. The mirror companion is now conditioned on
`allow_rotation` as well as `allow_mirror`, and
`a_piece_forbidden_to_rotate_is_never_offered_the_mirror_companion` pins it.

Because the conjunction is a no-op wherever `allow_rotation` is universally
true, and because the residency guard of §3.2 provably never binds
(`rotationBuildsRefused == 0` in every measured cell), the binary the §2 battery
ran on and the binary committed here are behaviourally identical on every
measured cell. Both fixes were applied after the battery started and neither can
have changed a number in it.

---

## 4. Attribution: what the rungs actually did

Per m34 slice, from the run's own report rather than from a profile
(`scheduleSlice.rotation*`, aggregated by `drivers/summarize.py`). All nine runs
of the armed arm, per request and budget:

| | mixed-61 10 s | mixed-61 30 s | shapes-17 10 s | triangle-20 10 s |
|---|---|---|---|---|
| rotation + mirror iterations | 655,477 | 1,870,559 | 436,998 | 1,336,518 |
| improved the incumbent | **8.3%** | 11.3% | **0.1%** | **0.7%** |
| share of proxy loss bought by rotation | **56.0%** | 55.5% | 93.3% | 83.0% |
| accepted moves that changed the pose | **67.1%** | 72.2% | 22.2% | 3.7% |
| ... in the unarmed arm | 8.9% | 16.9% | — | — |
| surrogate builds per rotation iteration | 0.73 | 0.71 | 0.75 | 1.19 |
| build wall per rotation iteration | 5.4 us | 6.0 us | 9.2 us | 6.3 us |
| cache hit rate | 89.4% | 88.7% | 88.3% | **52.5%** |
| builds refused by the residency guard | 0 | 0 | 0 | 0 |

Four readings.

**The rungs are accepted, and on mixed-61 they buy the majority of the loss.**
"Rotation bought millimetres" is not a claim this round has to hedge: the same
instrument measured the same quantity for the translation axes in the same
iterations, and rotation removed more of it than all four translation axes
together. The SE(2) certificate's claim — that rotation finds more room than
translation on these parents — reproduces inside the search at production cost.

**Millimetres of proxy loss are not millimetres of depth.** The armed arm
removes more weighted loss per accepted move and still publishes shallower
layouts, because part of the loss it removes is loss its own extra freedom
created: a layout with 46 pieces at off-grid angles has more repair to do at the
next depth step than a grid-native one, and the schedule's clock is spent doing
it. That is a different finding from "rotation is useless", and it is the one
this round actually has.

**Acceptance is a property of the request, not of the operator.** 8.3% on
mixed-61, 0.7% on triangle-20, **0.1%** on shapes-17. On the two requests where
almost nothing is accepted, the operator still pays for every proposal: 4.0 s
and 8.4 s of surrogate builds over nine runs, for zero millimetres. A rule that
withdrew the rungs after a measured acceptance rate would pay for itself on two
of three requests — and that rule is not design A.

**triangle-20 breaks the cache**, at a 52.5% hit rate against ~89% elsewhere and
1.19 builds per iteration: twenty distinct geometry classes generate distinct
`(class, angle, mirror)` keys faster than a 48-entry window can hold them. The
window is a parameter, but the shape of the miss is not: a per-piece continuous
angle space has no reuse across pieces by construction.

---

## 5. Gates, suites, determinism

**The four pinned gates, on rebuilt binaries, exits captured directly.** Run on
three binaries: the base commit's measurement build, the patched build *without*
the feature compiled, and the patched build *with* it compiled and the flag
unset.

| binary | g1 206.869 | g2 159.09233022733062 | g3 159.07876040364795 | g4 164.0375677990678 |
|---|---|---|---|---|
| `base-meas` (92a1a08) | hit | hit | hit | hit |
| `final-gate` (`jagua-experimental`) | hit | hit | hit | hit |
| `final-meas` (+ `continuous-rotation`, flag off) | hit | hit | hit | hit |
| `commit-gate` (rebuilt from the committed tree) | hit | hit | hit | hit |
| `commit-meas` (rebuilt from the committed tree) | hit | hit | hit | hit |

All four fingerprints reproduce, and — the stronger check — the **whole-document
digest** is identical across all five binaries on all four gates
(`fba9b4da1f768970`, `95d78aacdf6c3d3c`, `ecd10a4e6a03381e`, `474d45d8e72e6e67`),
using the repaired `doc_digest` with the elapsed-derived statistics and the
worktree fields stripped. `evidence/gates-*.json`.

The last two rows exist because the tree moved after the first four gates ran:
one whitespace-only reformat inside `prepare_continuous_candidate`. The gated
binary and the commit now correspond exactly.

**Flag-off document reproduction against the base commit.** `drivers/reproduce.py`,
whole documents at a 40 M work budget through the coordinator, three requests x
three seeds: **9 of 9 identical**, `allEqual=true`. `evidence/reproduce.json`.

> The first run of this driver reported 4 of 18 cells differing, and the driver
> was wrong rather than the build: it inherited `m34-wall-price`'s arm, which
> forces `m34wall=0,...,m34bit=0` on the new side only. Those keys were new and
> default-off in *that* round; here they are default-**on** inside v3, so the
> comparison was between two different configurations. Both arms now run the
> plain default spec, which is the claim this round actually makes: with `crot`
> unset the new binary is the base binary.

**Determinism across two processes, armed, hard gate.** `drivers/determinism.py`,
`crot=1,m34lanes=1,m34pconfirm=1`, 40 M work budget, three requests x three
seeds, two processes per cell, whole documents: **9 of 9 equal**,
`allEqual=true`. `evidence/determinism-crot.json`. The operator's cache is
lane-local and its eviction order is a deterministic function of the lane's own
sequence of ensures, so nothing it does depends on the clock or on which thread
ran what.

**Both suites, exits captured directly.**

| suite | result |
|---|---|
| `--features jagua-experimental` | **1,261 passed, 0 failed** |
| `--features jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation` | **1,293 passed, 0 failed** |

The 32-test difference is this round's own tests plus the schedule features'.
No rerun was needed; `free_material_multi_eviction...` did not flake in either
run. `evidence/suite-*.log`.

**The operator's own regression tests** (all in
`crates/polygon-nesting-core/src/search/general_relaxed.rs`, on the overlay
round's deliberately rotation-sensitive, *interacting* fixtures rather than on
symmetric squares):

| test | what it pins |
|---|---|
| `continuous_rotation_rung_is_the_arc_length_derivation` | `dtheta = dx / r` in degrees, the record line's own scale, the 45-degree cap, the degenerate radius |
| `continuous_rotation_arms_only_the_catalogue_resolved_lane` | the predicate refuses dynamic-hazard and directional lanes, and an armed lane derives continuous keys |
| `rotation_surrogate_is_built_once_and_then_cached` | one build, then hits; the built shape equals a directly constructed surrogate at that angle and differs from its grid snap; a grid angle costs nothing |
| `rotation_cache_evicts_probes_but_never_a_pinned_pose` | a flood of twice the window evicts probes, leaves the pinned pose resolvable, and evicts it once released — a pin is a hold, not a leak |
| `a_foreign_state_with_continuous_poses_scores_without_raising` | the fan-out / rollback case: a state no lane built resolves, with exactly two builds |
| `the_exact_validator_accepts_a_continuous_rotation` | the publication path, at 13.37 / 0.0032 / 47.00812 degrees, each asserted off-grid first |
| `an_armed_sweep_proposes_rungs_and_leaves_a_publishable_state` | end to end: the armed lane proposes and pays, the unarmed one leaves every rotation on the grid, and the exact tier accepts what the armed one left |
| `a_piece_forbidden_to_rotate_is_never_offered_the_mirror_companion` | §3.3's defect |

The last one was checked against the bug rather than assumed to catch it:
re-introducing `let mirror = armed && ...` verbatim fails it with
`left: 4, right: 0` on the mirror-proposal counter.

---

## 6. What this round does **not** claim

* It does not claim rotation is worthless in the search. It claims **design A —
  rotation rungs in the relaxed lane's candidate loop, priced at production
  cost — is negative**: −3.721 mm at 10 s and −7.071 mm at 30 s on mixed-61,
  0 mm on the other two requests, with the mechanism working as designed.
* It does not claim the price is irreducible. Five sixths of the per-slice
  slowdown is the *resolution* tax of §1.2, not the builds, and the obvious
  attack on it is a catalogue whose keys are continuous by construction rather
  than a grid with a lane-local overflow beside it. That is a different design
  from the one this round was asked to build. A smaller, cheaper item in the
  same place: `touch_rotation_probe` and `acquire_rotation_key` linear-scan a
  48-entry deque, and at ~1.2 M cache hits per armed ten-second run that is
  tens of millions of comparisons — bounded, but not free, and removable with an
  index.
* It does not claim anything about designs B (rungs inside the compression
  schedule when the clamp binds) or C (witness-driven, from the SE(2) dual).
  Both propose *far* fewer rotations than A's one-in-six, and A's own numbers —
  8.3% acceptance, 56% of the loss, 5.4 us a rung, and a 2.2x per-slice wall —
  are the first production measurement of what one of those rotations is worth
  and what it costs.

## 7. Files

* `drivers/battery.py` — the paired interleaved anytime battery, from
  `m34-wall-price/drivers/battery.py` plus the operator's per-slice attribution.
* `drivers/summarize.py` — the §2 and §4 tables.
* `drivers/workgate.py` — the equal-work matched-arm gate on the pinned parents.
* `drivers/offgrid.py` — how many published rotations are off the 2.5-degree
  grid: the publication-tier half of the decomposition.
* `drivers/smoke.py` — one run per arm with the operator's counters printed.
* `drivers/gates.py`, `drivers/gatelib.py`, `drivers/reproduce.py`,
  `drivers/determinism.py`, `drivers/runlib.py`, `drivers/docdiff.py` — from
  `m34-wall-price/drivers/`, `ROOT` repointed at this worktree.
* `evidence/*.json`, and the suite logs.

Reproduce:

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental                                   # gate binary
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,parallel-compression-schedule,continuous-rotation

D=docs/experiments/continuous-rotation/drivers
OFF='m34lanes=1,m34pconfirm=1,crot=0'
ON='m34lanes=1,m34pconfirm=1,crot=1'

for R in mixed-61:mixed61 shapes-17:shapes17 triangle-20:triangle20; do
  python3 $D/battery.py "curve-${R##*:}" 3 "${R%%:*}" 0,1,2 \
      "baseat3:wall:3000:1:$OFF"   "crotat3:wall:3000:1:$ON" \
      "baseat10:wall:10000:1:$OFF" "crotat10:wall:10000:1:$ON" \
      "baseat30:wall:30000:1:$OFF" "crotat30:wall:30000:1:$ON"
done
python3 $D/summarize.py evidence/curves-summary.json evidence/curve-*.json

python3 $D/gates.py final-meas <measurement-binary> <outdir>
python3 $D/reproduce.py reproduce <base-binary> <measurement-binary> \
    mixed-61,shapes-17,triangle-20 0,1,2 40000000
python3 $D/determinism.py determinism-crot mixed-61,shapes-17,triangle-20 \
    0,1,2 40000000 'm34lanes=1,m34pconfirm=1,crot=1'
python3 $D/workgate.py <outdir> <measurement-binary> \
    docs/experiments/parallel-compression-schedule/evidence/parents.json 0.3 0.002
python3 $D/workgate.py <outdir> <measurement-binary> \
    evidence/deep-parents.json 0.3 0.0005
```
