# A sound broad phase for the exact-clearance contract validator

> The §3 correction in docs/experiments/parallel-compression-schedule/ found
> where a mode-34 confirmation's milliseconds are and did not spend them: 97.9%
> of an accepted 4.83–5.03 ms confirmation is
> `validation::general_polygon::validate_publication`'s all-pairs
> `minimum_boundary_distance` loop, which has **no broad phase of any kind** and
> had never been measured. This round adds one, and the point of the design is
> that it is a *proof* rather than an approximation.
>
> Measured on x86_64, 16 cores, base `92a1a08`. The box was shared with one
> other measurement agent for the whole round, so every wall claim below is a
> paired interleaved difference with the within-arm spread printed beside it.

## The headline

| | flag-off | flag-on | |
|---|---:|---:|---|
| per accepted confirmation, serial (`pconfirm=0`) | **4.824 ms** | **0.861 ms** | **5.574x**, 110/110 cells |
| pairs the exact loop must examine | 1,830 | **~73** | 96.02% proved clear |
| mixed-61 at a 10 s wall, from the bare request | 172.288 mm | **169.572 mm** | **+2.716 mm**, 3/3 seeds |
| m34 slices per 10 s run (total, 9 runs) | 30 | **49** | the mechanism |
| `publicationValidate` share of a 10 s run | 3.914% | **0.496%** | |
| four pinned gates | 4/4 pinned | 4/4 pinned | **whole-document digests identical** |

The second row is the mechanism and the last is the licence to believe the
first. The filter proves 96% of the 1,830 pairs are far enough apart that the
exact loop's verdict on them is already known, and the four pinned gates hash to
the *same document* with the flag on — not the same pinned scalars, the same
document, field for field.

The third row is the one that matters to the binding priority, and the fourth is
why it happened: a confirmation that costs a fifth as much lets the same ten
seconds buy 49 mode-34 slices instead of 30. **Against Sparrow's 150.165 @ 10 s
on the same box this is still 19.4 mm short**, and §5 says so without dressing
it up.

---

## 1. What the loop must produce, decided before anything was designed

The task's first requirement is the one that decides the whole design: does any
caller consume the exact minimum distance **value**, or only its pass/fail
against the contract clearance? A prefilter is allowed to skip a pair only when
it can prove the skip changes no consumed output, and the two cases license
completely different filters.

`minimum_boundary_distance` is module-private and has **exactly one call site**,
`crates/polygon-nesting-core/src/validation/general_polygon.rs:181` (post-patch;
`:147` on the base commit), inside `validate_publication_inner`'s `scan_row`:

```rust
let distance = minimum_boundary_distance(first, second);
if !distance.is_finite() || distance < pair_clearance {
    return Some(PublicationValidationError::new(format!(
        "pieces {} and {} violate the required clearance", ...
    )));
}
```

The value is bound, tested, and dropped. It is not reported, not returned, not
stored, and not compared anywhere else — `grep -rn minimum_boundary_distance`
over the tree returns the definition, that one call, and two doc comments. **This
is case (a): only the threshold verdict is consumed.**

That matters because the campaign's pinned raw depths compare at 1 ULP, and a
reader is right to be nervous about touching anything near them. They are not
near this: the pinned depth is `raw_source_long_axis_depth_mm`
(`general_polygon.rs:292`), a `max` over transformed ring points that never
calls the distance loop at all. Nothing this round touches can move it, and §4
shows it did not.

### 1.1 Where the minimum *value* is consumed — and why it is not this function

Case (b) does exist in this engine, and naming it is part of showing case (a)
was established rather than assumed.
`search/general_micro_legalization.rs:1827`'s `measure_approach` computes the
same quantity and *does* consume the value, for a witness direction. It already
carries bound-based pruning against a running minimum, and its own doc comment
states the soundness argument this round reuses in a different form:

> Pruning is exact — a segment pair whose bounding boxes are further apart than
> the running minimum can neither improve it nor intersect — so whenever the
> result is below `ceiling` it is identical to the validator's measurement.

That is the in-repo precedent for what follows. It is a different function on a
different path; this round does not touch it.

### 1.2 The two things a skip has to prove

The scan row runs **two** tests per pair, and a skip bypasses both, so it has to
prove both:

1. `material_sets_overlap(first, second)` is false; and
2. `minimum_boundary_distance(first, second)` is finite **and** `>= pair_clearance`.

The finiteness half is not decoration. `minimum` starts at `f64::INFINITY`, and
`f64::min` ignores `NaN`, so the only way the result is non-finite is that *no
segment pair existed at all* — and that case is a **rejection**, not an
acceptance. A filter that skipped a pair on the reasoning "far apart, therefore
fine" would invert that verdict on degenerate input. §2.2 says how the design
forecloses it.

---

## 2. The design: four separating axes in the validator's own millimetres

`ClearanceSlabs` stores one material set's extent along four directions —
`(1,0)`, `(0,1)`, `(1,1)`, `(1,-1)` — and `ClearanceBroadPhase::provably_clear`
skips a pair when some direction's gap clears the pair clearance plus a margin.
A gap along an unnormalised direction `d` is `|d|` times the gap along its unit
normal, so the diagonal thresholds are scaled by `√2`; the threshold is scaled
up rather than the gap down, which keeps the test on the strict side.

`true` is a proof; `false` carries no information and costs four subtractions
and four comparisons before the exact loop runs unchanged.

### 2.1 Why `GridSlabs` could not be reused

The obvious move — reuse the constructor's certified separation shield,
`geometry/general_polygon.rs:579`'s `GridSlabs::separated`, which is the same
four-direction discrete oriented polytope — is wrong here, and the reason is the
reason this module exists.

`GridSlabs` projects the **canonical integer Clipper grid**. This module
deliberately reads `PolygonRing::source_points`, the untouched `f64` ring,
because — in `transform_source_ring`'s own words — "the search's own geometry is
quantized to the grid, so a validator built on it could not see a sub-grid
violation". Importing `GridSlabs` would import exactly that blindness into the
publication authority. There is a committed test for the property at stake,
`sub_grid_source_overlap_is_not_hidden_by_search_snapping`, and a grid-based
prefilter is precisely how one would break it.

So the slabs are rebuilt on the validator's own transformed `f64` points. The
structure is borrowed; the numbers are not.

### 2.2 Why the margin makes it a proof, and what it dominates

Skipping claims the exact loop would have computed `distance >= pair_clearance`.
Three error sources stand between the stored projections and that claim:

* **the projections.** `x` and `y` are stored coordinates and exact; `x + y` and
  `x - y` are one rounded operation each, so a slab can be reported at most
  `2^-53 * (|x| + |y|)` too far apart;
* **the gap.** One further correctly-rounded subtraction;
* **the exact loop's own arithmetic.** The claim is about the *computed* value,
  and `segment_distance`'s coordinate differences and `hypot` carry a few ulps of
  the coordinate magnitude.

Each is bounded by a small multiple of `2^-53 * extent ≈ 1.1e-16 * extent`. The
margin is `1e-9 mm + 1e-12 * extent` — four orders above the worst of them at
sheet-sized coordinates, and still six orders below the tightest clearance the
engine is asked for (`0.0005 mm`), so it costs the filter nothing it could have
had.

Two structural guards finish the argument:

* **finiteness.** `ClearanceSlabs::of` returns `None` for a set with no points,
  and for any set whose projections are not finite. A skip therefore requires
  both sets to have at least one point, hence one ring, hence one segment pair,
  hence a finite minimum — the `!is_finite()` rejection of §1.2 can never be
  skipped past. (`transform_source_ring` already rejects non-finite coordinates
  at `:266`, so this is belt and braces.)
* **`NaN`.** Every comparison is a `>=` against a positive threshold, and `>=`
  is false on `NaN`, so poisoned operands answer "no proof" and fall through.

### 2.3 The overlap half, including the case a reader should check

A positive gap along any direction puts the two sets in disjoint half-planes, so
`rings_properly_cross` is false and an interior sample of one set — which lies
inside its own polygon, hence inside its own slab interval — cannot be inside the
other.

The case worth stating is **containment**: a region sitting strictly inside
another's outer ring has a large positive boundary distance and is nevertheless
an overlap the validator must reject. It can never be skipped, because
containment makes one slab interval a subset of the other in *every* direction,
so every gap is negative and no proof is available. `a_contained_piece_is_never_proved_clear`
pins this with a pair whose exact boundary distance is a comfortable 4 mm — a
distance-only filter would wave it through, and the test asserts the verdict is
still the overlap rejection.

### 2.4 The debug arm

With `debug_assert` live, every skip re-runs **both** bypassed tests and
requires the verdict it claimed. Because `cargo test` is a debug build, the
whole suite — including the 142-second integration test — is a soundness run for
this filter, not merely a regression run. Both suites pass with it armed (§4.3).

---

## 3. Measurement

### 3.1 The census: what the filter can actually prove

`drivers/census.py`, mode 34 from the pinned 171–179 mm parents, serial
validator, `evidence/census.json`:

| | validator calls | pairs offered | proved clear | skip rate |
|---|---:|---:|---:|---:|
| all twelve parents | **3,243** | **5,934,690** | **5,698,534** | **96.02%** |

Per-seed the range is tight — 95.49%, 95.63%, 95.66%, 95.82%, 95.83%, 95.85%,
96.19%, 96.32%, 96.35%, 96.42%, 96.68%, 97.07% — so this is a property of a
mixed-61 layout at this density, not of one lucky parent.

1,830 pairs per call becomes about **73**. Note what this measures: a *count*,
taken by a separate `O(n²)` pass that runs only under
`POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS`, so the loop being described is never
the loop being timed, and it reports on **stderr** rather than into the result
document — see §4.1 for why that is not a stylistic choice.

### 3.2 The per-confirmation wall

`drivers/wall.py`, the twelve pinned parents × **10 rounds**, paired and
interleaved, arm order reversed on odd rounds, equal walk (`past=0`, no work
cap, 1.5 mm drop), census disabled. `evidence/wall-serial.json`:

| statistic | flag-off | flag-on | paired |
|---|---:|---:|---|
| **ms per accepted confirmation** | **4.8236** | **0.8609** | **5.5745x** median, 5.178–5.961, **110/110** |
| within-arm spread (relative) | 0.149 | 0.160 | |
| slice ms (`repair + confirmation`) | 1880.7 | 826.7 | 2.2511x, 116/120 |
| process wall (s) | 4.002 | 2.814 | 1.3703x, 118/120 |

The 4.8236 ms flag-off baseline reproduces
docs/experiments/parallel-compression-schedule/ §3's 5.028 ms and
`compression-schedule` §6.1's 4.83 ms, which is the check that this is the same
quantity those rounds measured.

**Read the spread before the delta.** The within-arm relative spread is ~15% on
both arms; the between-arm ratio is 5.57x. The delta is more than thirty times
the noise, and every one of the 110 paired cells is above parity, so the shared
box is not what produced this. The `sliceMs` and process-wall rows are diluted
by construction: the slice also contains the repair, which this round does not
touch, and the process also contains the identical mode-0 preamble both arms
pay. They are reported because a reader is entitled to the denominator.

**Equal-walk integrity held on all six fields, in all 120 paired cells**:
`stepsTaken`, `confirmationsAccepted`, `rawSourceDepthMm`, `fingerprint`,
`candidateQueries`, `workUnits`, zero mismatches. That is an independent
whole-trajectory equivalence result on top of §4: 120 cells in which both
binaries walked the same path to the same layout and only the clock differed.

### 3.3 The compound: does the saved time buy depth?

`drivers/anytime.py`, mixed-61 from the **bare request** at a 10 s wall, 3 seeds
× 3 rounds, both arms running the shipping `v3=1,m34pconfirm=1` spec so the only
difference is the binary. `evidence/anytime-10s.json`:

| seed | flag-off depth | flag-on depth | gain | m34 slices, off → on |
|---|---:|---:|---:|---|
| 0 | 172.288 | **169.572** | **+2.716 mm** | 1 → 5 |
| 1 | 168.708 | **165.656** | **+3.052 mm** | 7 → 9 |
| 2 | 174.881 | **174.280** | **+0.601 mm** | 2 → 2 |
| median | 172.288 | **169.572** | **+2.716 mm** | 30 → **49** total |

Nine of nine paired cells improve and none regresses, and all 49 mode-34 slices
in the flag-on arm published.

**The honest denominator: this is three seeds, not nine cells.** At a wall budget
these runs turn out to be near-deterministic — 8 of the 9 (seed, arm) cells
reproduced their depth exactly across all three rounds, and only seed 0 flag-on
varied (169.572 twice, 168.756 once). So "9/9" is three results repeated, and
the sample that carries the claim is **3 seeds, all three improving**. It is
consistent with the mechanism and it is not a nine-fold independent
confirmation.

The mechanism is visible on seeds 0 and 1, where the slice count rises with the
depth. Seed 2 gained 0.601 mm at an unchanged slice count of 2, so its gain came
from somewhere else in the budget — an honest loose end rather than a
confirmation.

### 3.4 The validator's share of a whole from-request run

`drivers/costshare.py`, profiled 10 s runs, shares against the leaf-phase total.
`evidence/costshare.json`:

| | flag-off | flag-on |
|---|---:|---:|
| `publicationValidate` share of leaf time | **3.914%** | **0.496%** |
| `publicationValidate` ms | 1183.2 | 148.3 |
| `publicationValidate` **calls** | 236 | **330** |

The share falls about eightfold, and the call count *rises* — the run performs
more publication validations in the same ten seconds, which is the compound of
§3.3 seen from the profiler's side. These are profiled runs and their depths are
not the §3.3 depths; the share is the reading and the milliseconds are a
decomposition, not a wall claim.

---

## 4. Equivalence

### 4.1 The four pinned gates, both arms, whole-document

`drivers/gates.py` + `drivers/gatecompare.py`, rebuilt binaries, every exit
captured directly, `evidence/gates.json`:

| gate | pinned | flag-off | flag-on | document digest |
|---|---|---|---|---|
| g1 mode 20 | 206.869 / `8a7737381238fa4d` | hit | hit | `8300dc2de9d18d84` **equal** |
| g2 mode 22 | 159.09233022733062 / `fa01012af1d559ae` | hit | hit | `f55abc835c12cbf9` **equal** |
| g3 mode 22 | 159.07876040364795 / `e28fba007f8031d4` | hit | hit | `c8972af4d516f695` **equal** |
| g4 mode 22 | 164.0375677990678 / `49f094d7e59a9008` | hit | hit | `7f86c5a7a28d35fb` **equal** |

`allPassOff`, `allPassOn` and `allDigestsEqual` are all true.

The same four digests are also produced by a **pre-patch binary built from the
base commit `92a1a08`** (`evidence/gates-head-raw.json`,
`evidence/gates-head-vs-on.json`), so the parity is three-way: HEAD, flag-off and
flag-on are one document on all four gates. That is the check that the *default*
build is untouched, independently of the source-level argument that every
addition sits behind a `#[cfg]` that is off.

This is a stronger gate than the previous rounds could pass, and the difference
is worth being precise about. `pconfirm` is semantics-preserving on accepted
confirmations but charges a *refused* one a different `exactPairTests`, so its
README could only claim scalar reproduction. This filter changes no verdict on
any input and carries **no counter in the document at all**, so it has no
licence to differ anywhere — which is exactly why the census lives on stderr.
The task's allowance for "work diagnostics honestly reduced" turned out not to
be needed: nothing is reduced, because nothing was ever counted here.

### 4.2 Determinism across processes, work-budget mode

`drivers/determinism.py`, two processes per cell, `work=3,341,379`, three arms
(`serial`, `pconfirm`, `both`) × three parents × both binaries.
`evidence/determinism.json`: **`ALL_REPRODUCIBLE: true`** and
**`ALL_FLAG_EQUIVALENT: true`** on all 18 cells.

That verdict needed the instrument repaired first, and the first run of this gate
**failed on every cell, including flag-off** — which is the control saying the
instrument was at fault rather than the feature.

**This is not a new finding, and it would be dishonest to bank it as one.** It is
the parallel-compression-schedule round's repair arriving late. That round hit
precisely this, diagnosed it the same way, and fixed it in its own
`drivers/lib.py:160` by adding `repairMs`, `confirmationMs` and
`currentPoseOverlaySetupMs` — the "second repair of this list", in its own words,
and its chapter records that without the serial control arm it would have been
written up as the parallel schedule failing its gate. The protocol for this
round points at the **m34-wall-price** `gatelib.py`, which is an older copy that
predates that fix, so the older copy reproduced the older bug. This round
re-applies it, credits it, and re-measures it rather than borrowing the reason
(`drivers/leafdiff.py`, two processes of the **same** binary, `off-serial-s0`):

> of **26,989** leaves, exactly **2** differ: `confirmationMs` and `repairMs`.

`determinism.py` strips them and says all of this in the code; `gatelib.py` is
left byte-faithful to the inherited copy so the diff against the other rounds
stays readable. The transferable lesson is the one the previous chapter already
drew and this round re-learned by inheriting the wrong file: **the campaign is
carrying N copies of this list and the repairs do not propagate.**

With that done, the cross-**flag** comparison is the result of this round:

| arm | leaves compared | differing | which |
|---|---:|---:|---|
| serial, seeds 0/1/2 | 26,989 / 25,992 / 27,113 | **2** | `confirmationMs`, `repairMs` |
| pconfirm, seeds 0/1/2 | 26,989 / 25,992 / 27,113 | **2** | `confirmationMs`, `repairMs` |
| both (lanes=8), seeds 0/1/2 | 17,790 / 16,873 / 6,972 | **2 / 2 / 1** | `confirmationMs`, `repairMs` |

Flag-off and flag-on differ in **at most two leaves out of up to 27,113**, and
they are the same two that differ between two processes of the *same* binary.
One of them, `confirmationMs`, differs by more than noise — 1636.2 ms → 298.1 ms
— because it is the field the feature exists to move. Every other leaf, every
schedule step row, every placement coordinate, every fingerprint, is identical.

### 4.3 Suites

Both green, zero failures:

* `--features jagua-experimental`
* `--features jagua-experimental,compression-schedule,parallel-compression-schedule,fast-contract-validator`

The known-flaky `free_material_multi_eviction` did not fire in either run.

---

## 5. Honest caveats and what was not done

**This does not close the gap to Sparrow.** 169.572 mm median at 10 s against
Sparrow's 150.165 mm on the same box is still **19.4 mm short**. This round makes
an operator cheaper; it does not add a degree of freedom, and the accumulated
evidence in the continuous-rotation brief is explicit that the remaining
12–16 mm needs rotation in the search. A 5.6x confirmation is a multiplier on
whatever the search can already do, and on this evidence it is worth about
2.7 mm of it.

**The 10 s compound rests on three seeds.** §3.3 says why: the runs are
near-deterministic at a wall budget, so nine cells are three results repeated.
Three of three improve, which is the right direction and a thin sample. A wider
seed set is the obvious next measurement and was not run for time.

**Only mixed-61 was measured.** The task named shapes-17 and triangle-20 as
request fixtures and neither was run. The filter's value depends on layout
density — it proves pairs clear, so a sparser layout should do *better* and a
tiny piece count has fewer pairs to save — but "should" is not a measurement,
and triangle-20 in particular has a very different piece profile. No corpus
regression claim is made here beyond the four pinned gates.

**The 96% is measured at one density.** All twelve parents are mixed-61 at
171–179 mm. The skip rate at the 155 mm record line, where the layout is much
tighter, was not measured and could be materially lower; the filter degrades
gracefully to the current code, so the risk is a smaller win rather than a wrong
answer.

**The inner loop was left alone.** With 96% of pairs skipped, the ~73 survivors
still run the full `O(E₁·E₂)` nest. `measure_approach` (§1.1) shows the sound
ring- and segment-level pruning that would attack them, and against a *threshold*
rather than a running minimum it could early-exit harder. It was not built: the
confirmation is now 0.86 ms of an 826 ms slice, so the remaining headroom is
worth at most a few percent of the slice against a real increase in the amount
of hand-proved geometry. That trade may be worth revisiting if the repair ever
gets cheap.

**`cargo fmt --all` is not safe in this tree.** It reformats fourteen
pre-existing files (692 lines in `portfolio.rs` alone). Everything outside this
change was reverted and the committed diff is three files; a reviewer seeing
churn elsewhere should treat it as a mistake, not as intent.

**What the debug arm does and does not cover.** It fires on every skip in a
`debug_assert` build, so the whole test suite is a soundness run — but the suite
is small geometry. The 5.9 M-pair census in §3.1 ran in a *release* build, where
the assertion is compiled out. The dense sweep in
`a_proved_clear_pair_is_one_the_exact_loop_accepts` is the deliberate
counterweight: it walks a rotated pair through overlap, contact and separation at
five clearances and checks the proof against the exact loop at every step.

---

## 6. What ships

`fast-contract-validator`, **off by default**, as every feature in this campaign
is. Nothing in a default build compiles any of it, and §4.1 shows the default
build's four gates unchanged.

It is not armed in the coordinator by this round and needs no spec key: it has
no lever, no tuning constant a reader would want to move, and no arm-versus-arm
question — it either proves a pair clear or it does not. The single number a
reviewer should challenge is the margin in §2.2, and the two tests that would
catch it being wrong are named there.
