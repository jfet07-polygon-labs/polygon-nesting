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

---

# Part II — the promotion round

> Sol review 7 §1 listed what promotion required and refused it until then. This
> part is that list, worked. Base `09738fb`, measured on the same x86_64 box,
> **shared with another measurement agent for the whole round** — so every wall
> claim below is paired and interleaved with its within-arm spread printed
> beside it, and §12.2 says plainly which absolute numbers do not travel.
>
> Binaries, `sha256`:
>
> | arm | features | sha256 |
> |---|---|---|
> | `base-off` | pre-patch, flag off | `b39c04b9f96d753ca04457890f79b04fecfe4f20cdcff008468099ae74f8ba4b` |
> | `base-on` | pre-patch, flag on | `ab5b0b5ce9eed054bcdb2f9c856f8e01e73cc85d70a207e1b2544020b13fcc29` |
> | `off` | post-patch, flag off | `ffe3e78a8616d27b47419550b248c64edfab4a5c3eec9a8f48cb68bc8566800a` |
> | `on` | post-patch, flag on | `23335405152f73fa29280c05bee4c15966b8533a178c0c40c7f0647341c98c9c` |
> | `shadow` | release shadow corpus | `9e42b4aebfbd5deb5ff7fc541c5defa6b4163a5dfef2ab353cdd44efe7bdc70b` |
>
> Every evidence file carries the `sha256` of the binary that produced it, and
> they all name the binaries above — the gates, the determinism gate, the shadow
> corpus, the density census and both wall batteries were all re-run against the
> final artefacts after the last source edit, rather than inheriting numbers from
> a superseded build.

## 7. The numeric domain: the lemma is true, and it was true for a reason nobody had written down

Sol's first P0 is correct as stated, and §1.2's argument does not survive it:

> "L'affermazione «un minimo non finito implica assenza di segmenti» non segue
> dal solo fatto che esista una coppia di segmenti."

`the_numeric_domain_guard_fails_closed_where_the_lemma_does_not_hold` is the
witness, and it is a real one. Two three-point material sets at `x = ±1.3e308`:

* the exact loop's minimum is **not finite** — the cross-set coordinate
  difference overflows, `hypot` returns `inf`, and one segment pair's projection
  numerator is `-inf * 0 = NaN`, which `f64::min` discards rather than
  propagating — so `scan_row` **rejects** the pair;
* the unguarded certificate's `x`-projections are finite on both sides and their
  difference overflows to `+inf`, which clears every finite threshold, so the
  previous certificate would have **skipped** it.

That is a skip of a rejection: the inversion §1.2 said could not happen.

### 7.1 But it is not reachable, and the reason is `interior_sample`

Having built the counterexample, the honest next question is whether
`validate_publication` can be made to produce those sets. **It cannot**, and
tracking down why is the actual result of this section.

The grid contract bounds the *source* ring — `to_grid_mm` requires `|x| * 1000`
to be a safe integer, so `|x| <= 9.007e12 mm` — but that bound does **not**
survive the transform: `translate_x` is checked only for finiteness, and
`transform_source_ring` only rejects a non-finite *result*. `validate_sheet`
cannot be leaned on either; it runs after the transform, only over outer rings,
and against a `sheet_width_mm` that is itself only required to be finite.

The bound that does hold comes from `interior_sample`, through
`transform_placement`, which rejects any region with **no discoverable material
interior**. Discovering one requires two *distinct* `f64` y-levels among the
transformed ring points and two distinct x-intersections at some scan level. Two
distinct doubles of magnitude `M` differ by at least `M * 2^-53`, and both
differences are bounded by the region's diameter, which a rigid transform
inherits from the source ring at `2 * sqrt(2) * 9.007e12 ~= 2.55e13 mm`. So

```text
|coordinate| <= 2.55e13 * 2^53 ~= 2.29e29 mm
```

for every set `transform_placement` admits — and at `2.29e29` nothing in the
exact loop overflows, so the lemma holds.

**So the previous round's claim was true and its proof was not.** The step it was
missing lives three functions away, in the one function whose declared job is
"this piece has no interior", and it had not been written down anywhere. A
publication authority should not rest on an unstated consequence of a helper that
exists for another purpose, which is the whole of Sol's point.

### 7.2 What is coded

`CLEARANCE_SLAB_MAX_COORDINATE_MM = 2^112 ~= 5.19e33`, checked per material set
in `ClearanceSlabs::of`, which returns `None` — no certificate, exact loop, **fail
closed** — for any set with a coordinate outside it, a non-finite projection, or
an out-of-domain interior sample. The value is chosen to sit

* **above** the `2.29e29` structural ceiling by `2.3e4`, so it can never refuse a
  layout the contract admits and the shipped skip rate is untouched (§10 measures
  that: zero refusals in 1.7 M pairs, and the 5.93 M-pair census unchanged); and
* **below** the `2^497 ~= 4.09e149` horizon at which `orient2d`'s splitter could
  overflow, by 385 binades — so on the guarded domain it proves finiteness *on
  its own*, without borrowing §7.1's argument at all.

`the_domain_guard_admits_everything_the_contract_can_build` recomputes the
structural ceiling from its two inputs and fails if either bound ever crosses the
guard, so §7.1's derivation is executable rather than prose.

Within the domain, no product, sum, square or division in
`point_segment_distance` reaches infinity; no `inf` means no `inf - inf`,
`0 * inf` or `inf / inf`, hence **no `NaN`**; so `point_segment_distance` always
returns a finite non-negative number and `minimum_boundary_distance` is
non-finite only when it saw no segment pair — which the empty-set `None` already
forecloses. The domain guard is also what keeps `orient2d` **exactly** signed,
and that exactness is load-bearing: it is why `rings_properly_cross`,
`segments_touch_or_cross` and `classify_point_in_ring` are exact predicates
rather than approximations, which the overlap half of every skip depends on.

## 8. The margin is a derivation now, and the hot loop did not change

Sol's second P0:

> "«A handful of ulps» per proiezione punto-segmento non è un bound."

Agreed. It is replaced by two separate things, deliberately kept apart.

**A certified geometric bound, carrying no epsilon.** `ClearanceSlabs::of` now
rounds each diagonal projection *outward* — `next_down` for the lower bracket,
`next_up` for the upper — so `[min, max]` is a guaranteed superset of the true
projection interval. `next_down(gap)` is then a rigorous lower bound on the true
gap, unconditionally.

**A derived error bound for the exact loop's own arithmetic**, which is what the
margin is for, because the skip is a claim about the *computed* distance. With
`u = 2^-53` and `C` the pair's largest coordinate magnitude:

| source | bound |
|---|---|
| `fl(p * dx)` against `p * (E.x - S.x)` | `2.1 * C * u` |
| `closest_x` against the true segment point `Q` | `5.3 * C * u` |
| `P.x - closest_x`, per component | `7.4 * C * u` |
| the difference vector against `P - Q` | `10.5 * C * u` |
| `hypot`, `2u` relative on a result `<= 3C` | `6 * C * u` |
| **`computed >= true_distance - `** | **`16.5 * C * u`** |

The clamped parameter lies in `[0, 1]`, so `Q = S + p * (E - S)` is a real point
*on* the segment and `true_distance <= |P - Q|` — that is the step that makes the
chain work. The degenerate `length_squared == 0` branch measures to an endpoint
and is looser only safely, at `9 * C * u`.

The **overlap** half needs its own bound and gets a much smaller one: with
`orient2d` exact on the guarded domain, the only inexact input to
`material_sets_overlap` is the rounded edge midpoint in
`has_material_sample_inside`, within `1.5 * C * u` of a true point of its own
set. (The interior sample needs no slack at all: it is a stored point the exact
winding rule certified as inside its own polygon.)

`32 * u = 3.553e-15` dominates both. The shipped
`CLEARANCE_SLAB_RELATIVE_MARGIN` is `1e-12`, **281x** that — so the derivation
changes no threshold, which is why §10's census is bit-identical. And the margin
is computed as `max(shipped, derived)`, so the proof is **structural rather than
tested**: the constant can be raised freely and cannot be lowered under the
derivation even by someone who never read it.
`the_shipped_margin_dominates_the_proven_error_bound` is the second line of
defence.

### 8.1 The hot loop is byte-identical, on purpose

The first implementation put the outward rounding on the gap, which cost two
`next_down` calls per direction per pair — about three times the filter's
arithmetic. It was replaced, because `next_down` is monotonic and
`next_down(g) >= t` is *exactly* `g >= next_up(t)`: the rounding moves to the
threshold, where it is paid `O(directions)` per `validate_publication` call
instead of `O(pairs * directions)`.

`ClearanceSlabs::gap` and `ClearanceBroadPhase::provably_clear` are therefore
unchanged from the binary §3.2 timed — two subtractions and a `max` per
direction, four comparisons per pair. **The 5.57x is a result about a specific
instruction sequence and this round did not touch it**, which is what lets §3.2
stand without being re-run.

### 8.2 The chain, end to end

`gap >= threshold[i]` where `threshold[i] = next_up(next_up(fl(T * |d_i|_ub)))`
gives `next_down(gap) >= T`, and `next_down(gap) <= true gap`, so
`true gap >= T = (clearance + margin) * |d_i|`. Since
`|b - a| * |d| >= (b - a) . d >= gap` for any `a`, `b` in the two sets, the true
Euclidean distance is `>= clearance + margin`, and the distance bound above then
gives `computed >= clearance`. That is the claim, as an implication.

## 9. The release shadow corpus

Sol's third P0 named the hole exactly: the `debug_assert` is compiled out of
release, so the 5.9 M-pair census in §3.1 ran with **no checking behind it**, and
`the_broad_phase_changes_no_verdict` compared one path against enumerated
expectations rather than against another implementation.

Two pieces of machinery close it, both in `--release`:

* **`validate_publication_exact_reference`** runs the same validator with the
  broad phase *disarmed* — every slab `None`, every threshold infinite — so one
  binary holds both implementations and they can be compared at runtime. It costs
  the armed path nothing: the disarmed state is a value, not a branch.
* **`contract_validator_shadow_audit`** re-runs **both** bypassed tests for every
  certified pair and returns every disagreement, with explicit branches rather
  than `debug_assert`.

`examples/contract_validator_shadow.rs` drives both over a randomized corpus:
convex and non-convex pieces, rings **with holes**, **multi-region** sets,
slivers down to 0.001 mm thickness, tiny 0.02 mm pieces, mirrored and rotated
placements, four displacement regimes, six clearances spanning `0.0` to `40 mm`
including the contractual `0.0005`, a deterministic near-threshold sweep, a
contractual-extreme sweep at magnitudes from `1e-3` to `1e12 mm`, and 3-to-8
piece layouts so the scan row's *ordering* is exercised.

`drivers/shadow.py`, five seeds, `evidence/shadow-corpus.json`:

| | |
|---|---:|
| layouts | **1,051,980** |
| pairs offered | **1,695,677** |
| pairs **certified** | **1,002,726** (59.13%) |
| layouts the validator **rejected** | **470,524** |
| layouts accepted | 581,456 |
| tightest certified pair, above clearance | **2.138e-9 mm** |
| domain-guard refusals | 0 |
| **verdict mismatches** (whole `Result`, message included) | **0** |
| **per-pair audit mismatches** | **0** |

**Read the three coverage rows before the two zeros.** A corpus that certifies
nothing has tested nothing, and 1.0 M certificates is not that. A corpus of only
legal geometry makes the verdict comparison vacuous, and 470 k rejections is not
that. And a corpus whose tightest certificate sits millimetres clear has not
probed the margin — this one's tightest sits **2.1 nanometres** above the
clearance, which is inside the margin's own order of magnitude. The randomized
regime could not do that (a random rotation randomises the achieved gap, and its
tightest was 7.8e-2 mm), so the near-threshold sweep is deterministic and
axis-aligned, stepping the separation through `clearance + k * margin` for
`k = -10..10` on both an axis and a diagonal. `k < 0` is a violation the exact
loop must reject; none was certified.

The comparison is on the whole `Result` **including the error message**, so a
filter that changed *which* pair failed first would be caught, not only one that
changed the verdict.

## 10. The coverage §5 admitted it did not have

### 10.1 The skip rate is not a property of one density

§5's caveat was explicit: "the skip rate at the 155 mm record line... was not
measured and could be materially lower". `drivers/censusdensity.py` walks the
campaign's own pinned layouts — the same 61 pieces on the same sheet — from
179.6 mm down to the 155.264 mm record line. `evidence/census-density.json`:

| layout | depth | pairs | skip rate |
|---|---:|---:|---:|
| record line, 4 parents | 155.264–164.038 mm | 111,630 | **96.47%** |
| the §3.1 band, 12 parents | 171.614–179.620 mm | 5,934,690 | **96.02%** |

**The prediction is refuted, in the favourable direction.** Packing the same
pieces 16 mm deeper does not cost the filter anything measurable; the record line
skips *slightly more*. The mechanism is visible once stated: with 61 pieces on a
2000x2700 sheet most pairs are across the sheet, and depth compression moves
pieces closer only locally.

The other three request fixtures, at their own pinned parents:

| fixture | pieces | depth | skip rate |
|---|---:|---:|---:|
| shapes-17 | 17 | 200.349 mm | 95.90% |
| triangle-20 | 20 | 70.727 mm | **97.19%** |
| small-8 | 8 | 70.252 mm | 96.43% |

Every layout the campaign has, at every density it has, lands in **95.5%–97.2%**.

### 10.2 The 5.93 M-pair census is bit-identical to the previous round's

The band row above is not merely close to §3.1, it is the same numbers:
**3,243 calls, 5,934,690 pairs, 5,698,534 proved clear, `skipRate`
0.9602075255826337**, and all twelve per-parent rates agree to the printed
precision.

That is the strongest equivalence statement in this document. The domain guard,
the outward-rounded slab bounds and the doubly-bumped threshold changed **not one
of 5,934,690 pair-level certificate decisions**. §11's digests say the documents
match; this says the *certificates* match.

### 10.3 The per-confirmation wall on the other fixtures, including where there is none

`drivers/wallfixtures.py`, paired and interleaved, 10 rounds, arm order reversed
on odd rounds, equal walk (`past=0`, no work cap), census disabled, from pinned
parents built by `drivers/buildparents.py` and committed under
`evidence/parents/`. `evidence/wall-fixtures.json`:

| fixture | confirmations/run | flag-off | flag-on | paired |
|---|---:|---:|---:|---|
| **triangle-20** | 59 | **0.2090 ms** | **0.1062 ms** | **1.9733x**, 10/10 above parity, 1.632–2.379 |

Within-arm relative spread 0.219 (off) and 0.213 (on); equal walk held on all six
fields in all ten rounds. An earlier ten-round pass on the pre-`rustfmt` binaries
put the same cell at 1.9521x (0.2091 → 0.1071 ms), which is the reproducibility
this fixture offers and is well inside the spread.

**1.95x, not 5.57x, and that is the expected direction.** A 20-piece layout
offers 190 pairs where mixed-61 offers 1,830, so there is an order of magnitude
less to save even at a higher skip rate — the filter's value scales with the
pair count it removes, not with the fraction.

**On shapes-17, small-8 and the 155.264 mm record parent there is no
per-confirmation wall to measure, because the validator is never called.** All
three replay with `confirmationsAttempted = 0` and
`confirmationsSkippedInfeasible` equal to `stepsTaken - 3`: the proxy tier calls
every reduced layout infeasible, so the exact validator is never reached.
`drivers/dropprobe.py` swept the shapes-17 drop over `0.05, 0.1, 0.2, 0.4,
0.8 mm` (`evidence/drop-probe-shapes17.json`) and found **zero confirmations at
every one**, so this is a property of the fixture's mode-34 schedule and not of
the drop I picked. It is consistent with the campaign's own record that mode 34
publishes 0/9 on shapes-17 and triangle-20 in both arms.

The honest reading is that **on these fixtures the feature is worth exactly
zero** — not because the filter fails but because the operator it accelerates
never reaches its expensive step. That is coverage, and it is the kind that a
promotion case has to carry rather than omit.

## 11. Equivalence: four binaries, one document

`drivers/gates4.py`, the four pinned gates against four binaries, rebuilt before
gating, exits captured directly. `evidence/gates-four-way.json`:

| gate | pinned | digest, all four arms |
|---|---|---|
| g1 mode 20 | 206.869 / `8a7737381238fa4d` | `6cf4fc905fa1c22b` |
| g2 mode 22 | 159.09233022733062 / `fa01012af1d559ae` | `8d63b305db6369bf` |
| g3 mode 22 | 159.07876040364795 / `e28fba007f8031d4` | `973c18319ca02746` |
| g4 mode 22 | 164.0375677990678 / `49f094d7e59a9008` | `7103c9a9a05be686` |

`ALL_PASS`, `ALL_DIGESTS_EQUAL` and the three named comparisons are all true:

* **`DEFAULT_BUILD_UNTOUCHED`** (`base-off == off`) — this round edited only
  code behind the `#[cfg]`, and the default build's four gates prove it.
* **`CERTIFICATE_UNCHANGED`** (`base-on == on`) — the question the previous
  round's three-way could not ask, because this round *edited the flag-on path*.
  The guard and the outward rounding moved nothing.
* **`FLAG_EQUIVALENT`** (`off == on`) — the property the feature is held to.

Determinism, work-budget mode, two processes per cell, three arms x three
parents x both binaries (`evidence/determinism-round2.json`):
**`ALL_REPRODUCIBLE: true`** and **`ALL_FLAG_EQUIVALENT: true`** on all cells.

Both suites green, exits captured directly:
`--features jagua-experimental`, and
`--features jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,fast-contract-validator`.
The debug arm's `debug_assert` is live in both, so both are soundness runs.

## 12. The factorial: `pconfirm` is not a tax, and what it is worth depends on the box

Sol §1 and Grok §3(c) both raised the same hypothesis: after the filter, a serial
confirmation costs 0.86 ms, so `pconfirm`'s job-pool dispatch might now cost more
than the parallelism it buys. Neither previous round could answer it —
`parallel-compression-schedule` priced `pconfirm` against a 4.82 ms confirmation,
and §3.3 ran `m34pconfirm=1` in *both* arms.

`drivers/factorial.py`, mixed-61 from the bare request at a 10 s wall, 3 seeds x
3 rounds, all four cells paired and interleaved with cell order rotated by round.
`evidence/factorial-10s.json`:

| cell | median depth | per accepted confirmation |
|---|---:|---:|
| fcv off, pconfirm 0 | 173.575 mm | **4.6116 ms** |
| fcv **on**, pconfirm 0 | **170.453 mm** | **0.7952 ms** |
| fcv off, pconfirm **1** | 172.288 mm | **0.9870 ms** |
| fcv **on**, pconfirm **1** | **168.756 mm** | **0.2774 ms** |

**The tax hypothesis is refuted at the microbenchmark.** `pconfirm` still buys
**2.87x** on top of the filter (0.7952 → 0.2774 ms): the dispatch overhead has
not overtaken the work, and the fear that it might is answered "no".

Paired per (seed, round):

| contrast | median | wins/ties/losses |
|---|---:|---|
| baseline → fcv alone | **+3.122 mm** | **9 / 0 / 0** |
| baseline → pconfirm alone | +1.882 mm | 9 / 0 / 0 |
| baseline → both | **+4.819 mm** | **9 / 0 / 0** |
| fcv alone → both | **+1.527 mm** | 5 / 3 / 1 |
| pconfirm alone → both | +3.052 mm | 7 / 2 / 0 |

So on this battery both levers are worth having and they compose: fcv is the
larger of the two, and adding `pconfirm` on top of it is worth a further
1.5 mm.

### 12.1 That conclusion is load-dependent, and the retraction is the finding

**This battery was run twice, and the first run said something else.** An earlier
pass with the same protocol and behaviourally identical binaries
(`evidence/factorial-10s-loaded-box.json`, taken while the shared box was busier)
produced:

| cell | loaded box | quieter box |
|---|---:|---:|
| fcv off, pconfirm 0 | 173.575 mm | 173.575 mm |
| fcv **on**, pconfirm 0 | **170.453 mm** | **170.453 mm** |
| fcv off, pconfirm **1** | 172.288 mm | 172.288 mm |
| fcv **on**, pconfirm **1** | 171.111 mm | **168.756 mm** |

and therefore a *different* verdict on the last contrast: `fcv alone → both` was
**+0.000 mm, 4/3/2** there and is **+1.527 mm, 5/3/1** here. **I wrote the first
result up as "`pconfirm` buys nothing in depth" and recommended shipping it
disarmed. That was wrong and this supersedes it.**

Read the table again, because it explains itself: **the two `pconfirm=0` cells
are identical across the two batteries, to the millimetre, on every seed.** The
serial arms do not care what else the machine is doing. The `fcv on,
pconfirm = 1` cell moved 2.4 mm between them. So the real finding is not "how
much is `pconfirm` worth" but:

> **`pconfirm`'s value is a function of the cores actually available, and
> `pconfirm=0`'s is not.** On a machine with spare capacity the parallel
> confirmation converts it into depth; on a contended one it degrades toward the
> serial arm, and the serial arm is a constant.

That is a sharper and more useful statement than either battery alone, and it is
only visible because the first battery was kept rather than discarded when the
second disagreed with it.

### 12.2 `pconfirm` costs cross-round reproducibility, in both batteries

Per (cell, seed) across the three rounds, over **both** batteries — 24
seed-cells:

* every `pconfirm=0` seed-cell reproduced its depth **exactly**, 12 of 12;
* four `pconfirm=1` seed-cells varied — 172.288/170.453 and 171.111/169.512 in
  the loaded battery, 174.280/174.881 and 171.111/168.756 in this one.

**All four varying cells are `pconfirm=1` and none is `pconfirm=0`.** At a wall
budget the parallel confirmation's scheduling jitter converts into how many
actions fit in ten seconds, and therefore into depth. This does not contradict
§4.2's determinism gate, which is a *work*-budget gate and passes on all 18
cells with both binaries; it is a statement about wall-budget runs specifically,
and it is the price of the 1.5 mm above.

### 12.3 What these absolute numbers are, and are not

`fcv on, pconfirm 1` is the configuration `rotation-tax` §4.2 reports at
**168.484 mm**, on the same base commit, request, pinned CLI tail and a
byte-identical spec. This battery puts it at **168.756 mm** — 0.27 mm apart,
which is as close as two wall-budget batteries on a shared box can be expected
to land, and it is the reconciliation the loaded battery's 171.111 mm did not
offer.

**Absolute depths here still should not be quoted against another round's**: at a
wall budget, depth is a function of available CPU, and §12.1 is the demonstration
rather than the caveat. The contrasts are the claim, and only within one battery,
because all four cells of a battery are run paired and interleaved inside one
window with cell order rotated by round.

## 13. The promotion case

The recommendation, and **no default is flipped by this round** — the
recommendation is the deliverable.

### 13.1 What is now proven

| Sol review 7 §1 requirement | status |
|---|---|
| numeric-domain guard, fail-closed, derived from real maxima | **done** — `2^112`, derived §7.1, coded §7.2, witness test |
| margin replaced by outward rounding or a domain-proven bound | **done** — both; §8, and structural via `max` |
| release shadow corpus, explicit assertion, verdict + message identity | **done** — §9, 1.70 M pairs, 1.00 M certified, zero mismatch |
| perf coverage: shapes-17, triangle-20, small-N, 155 mm density | **done** — §10, including two honest "no measurement exists" results |
| fcv x pconfirm factorial, with per-confirmation microbenchmark | **done** — §12 |

Plus, unasked but load-bearing: **5,934,690 pair-level decisions bit-identical**
to the pre-patch certificate (§10.2), and a four-binary whole-document gate that
separates "the default build did not move" from "the certificate did not move"
(§11).

### 13.2 What default-on would require

1. **A spec key, or a decision not to have one.** The feature still has no lever
   and no arm-versus-arm question, so it needs no key to *tune*; but promotion to
   default-on means the exact loop becomes unreachable in a shipping build, and a
   way to disarm it in the field is worth more than its absence.
   `validate_publication_exact_reference` already exists and is the natural
   implementation.
2. **A second box.** Every wall number in both parts of this document comes from
   one shared x86_64 machine. Nothing here is a claim about another target.
3. **The seed set.** §5's caveat stands unchanged: 3 seeds, and §12's four-cell
   battery is still 3 seeds. The contrasts are 9/0/0 on the two that matter, but
   nine cells are three results repeated (§3.3 says so of itself and it is still
   true here).
4. **A decision on `pconfirm`, which §12 makes for the first time — and which
   depends on the deployment, not only on the engine.** The factorial says both
   levers are worth having and compose (`+4.819 mm` over the double-off
   baseline, 9/0/0), so the shipping combination is
   **`fast-contract-validator` armed *and* `m34pconfirm=1`**, which is what the
   previous round already shipped and this round confirms rather than changes.

   The qualification is §12.1's: **`pconfirm`'s 1.5 mm is contingent on spare
   cores.** On a contended machine it decays to parity with the serial arm,
   while the serial arm's depth is a constant across both batteries on every
   seed. A deployment that cannot promise the cores should expect `+3.1 mm` from
   the filter alone and not the `+4.8 mm`; and if cross-round reproducibility at
   a wall budget matters more than the last millimetre, `m34pconfirm=0` is the
   configuration that has it (§12.2).

### 13.3 What remains open

* **The gap to Sparrow is untouched.** Part I's §5 says 19.4 mm; this part's best
  configuration lands at **168.756 mm** median against Sparrow's 150.165 mm, so
  the gap is **18.6 mm**. This round makes an operator sound and better covered;
  it adds no degree of freedom, and it does not close a millimetre of that gap
  that Part I and the rotation-tax round had not already claimed — the 168.756 mm
  reproduces the 168.484 mm already on the books, it does not improve on it.
* **The inner `O(E1*E2)` nest** on the ~73 survivors is still untouched, for the
  reason Part I gives.
* **Three of the four fixtures have no per-confirmation measurement** — shapes-17,
  small-8 and the 155.264 mm record parent — because their mode-34 schedule never
  reaches a confirmation (§10.3). Whether that is worth fixing is a question about
  the schedule, not about the validator, but it does mean the wall evidence for
  this feature rests on mixed-61 and triangle-20 alone.

## 14. Corrections to Part I

Two statements in Part I are wrong or under-argued, and this part supersedes
them rather than quietly leaving them:

* **§2.2's finiteness bullet** ("a skip therefore requires both sets to have at
  least one point, hence one ring, hence one segment pair, hence a finite
  minimum") is not valid as an argument. The conclusion is true on contractual
  input, but only via §7.1's `interior_sample` bound, which §2.2 does not make.
  The guarded version is §7.2.
* **§2.2's margin paragraph** ("a handful of ulps") is not a bound. §8 is.

One statement stands and is worth re-affirming because a reader might expect this
round to have overturned it: **§2.3's containment argument is correct**, and both
reviewers independently confirmed it. §9's corpus adds the holes and multi-region
fixtures Grok asked for, and found nothing.
