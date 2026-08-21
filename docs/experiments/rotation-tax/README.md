# The continuous-rotation tax, decomposed: the named cost was the smallest one, and the real one is a Clipper offset per rung

> The task: make the proven rotation mechanism affordable on the wall, then
> measure the compound. Base commit `a2fd148` (rotation merged, fast validator
> merged). x86_64, 16 cores, box shared with other measurement agents for the
> whole round, so every wall claim below is a **paired interleaved** difference
> with the within-arm spread printed beside it.

## The result in one table

| | base `a2fd148` | committed | |
|---|---:|---:|---|
| off-grid resolutions paying **two** ordered-map descents | 88.2 M | **23.2 M** | 73.7% removed |
| `ensure_state_surrogates` wall, three isolated slices | 428.2 ms | **39.1 ms** | **10.9x** |
| equal-work slice wall, three pinned parents × 10 rounds | 690 / 554 / 2428 ms | **667 / 536 / 2295** | **1.0373x**, **30/30** paired |
| equal-work process wall | 4.147 / 3.233 / 6.781 s | **4.028 / 3.152 / 6.556** | **1.0306x**, **30/30** paired |
| equal-work trajectory equality | — | — | **0 mismatches / 54 cells** |
| mode-34 slice wall, from the bare request at 10 s, armed | 1.978 s | **1.824 s** | **1.0796x**, 12/12 paired |
| four pinned gates, three binaries | 4/4 | 4/4 | **whole-document digests identical** |
| the indexed probe queue the task called certain | — | **reverted** | 0.38% slower, 22/24 (§2.3) |
| **mixed-61 at a 10 s wall, `crot=1` vs `crot=0`** | — | — | **+6.735 mm worse** (§4) |

The fixes work, they are answer-preserving to the fingerprint, and **they are
not enough**. §1 is why, and it is not the decomposition
docs/experiments/continuous-rotation/ §6 predicted.

---

## 0. The correction this round exists to make

docs/experiments/continuous-rotation/ §3 measured a 2.2x per-slice wall on an
armed mode-34 slice, attributed 0.32 s/slice of it to surrogate builds, and
concluded:

> **Only a sixth of that is the surrogate builds.** [...] The other five sixths
> is the resolution tax of §1.2.

That conclusion is **wrong, and wrong for an instructive reason**. The
coordinator arms the operator on modes **22 and 34** — `portfolio.rs:2123`,
`self.settings.continuous_rotation && matches!(mode, 22 | 34)` — but
`rotation_surrogate_builds` is only ever *reported* on a mode-34 schedule
slice. Every build a mode-22 lane paid for was invisible to the instrument.

Counted process-wide for the first time here (`profiling::rotation_tax`), one
armed ten-second mixed-61 run at seed 0 performs **1,129,375 surrogate builds**
costing **6,407 ms**. The mode-34 slices of **that same run** report **169,772**
builds costing **703 ms** — **15.0%** of the builds and **11.0%** of the build
wall.

The instrument is not in doubt, because it reproduces the previous round's
reading of the part it could see. `docs/experiments/continuous-rotation/` §3.1's
one-cell probe — mixed-61, seed 0, ten seconds, rungs in the fine pass only —
records **169,772** surrogate builds. This round's armed run of the same cell
reports **169,772** in its mode-34 slices, to the unit, and 1,129,375 in the
process. The previous round measured the right thing and was handed a
denominator that was 15% of the run.

With the whole number in view the decomposition inverts: the builds are not a
sixth of the tax, they are **the** tax, and the resolution tax that §6 named is
real but second-order. §1 has the numbers and §5 has what follows from them.

---

## 1. The decomposition

Two instruments, because one could not do it.

**`rotation-tax-census`** (`crates/polygon-nesting-core/src/profiling.rs`,
`pub mod rotation_tax`) adds process-wide per-thread counters to the armed
lane's own sites: every pose resolution tagged by which map answered, every
step of the two named deque scans, every call into the operator's entry points,
and — timed directly — the surrogate build and its four stages. Off by default
and **never compiled into a binary that carries a wall claim**; the §3 and §4
batteries run on a build without it.

> The first version of this instrument used one process-global `[AtomicU64]`
> and `fetch_add`. Eight fan-out lanes turned the 160 M-call resolution counter
> into 160 M contended read-modify-writes on one cache line, and the armed
> coordinator never reached a mode-34 slice at all — the instrument destroyed
> the phase structure it was there to describe. The committed version gives each
> thread its own block, exactly as `profiling::ThreadProfile` does. This is
> written down because the failure looked like a finding.

**The isolated slice** (`drivers/taxprobe.py`): one pinned parent, one serial
mode-34 slice, a fixed work cap, both arms of the operator on one binary. A
from-request run is the wrong place to take a decomposition — its arms do not
run the same number of slices, so their totals are not comparable — and a
replay at a fixed work cap has both arms answering the same number of proxy
questions by construction.

### 1.1 Where an armed run's extra work is

One armed ten-second mixed-61 run, seed 0, census build,
`evidence/decompose-fromrequest.json`. The unarmed arm of the same cell is in
the right column, and its zeros are the point: none of this exists without the
operator.

| | armed | unarmed |
|---|---:|---:|
| **surrogate builds** | **1,129,375** | 0 |
| **wall inside `build_oriented_surrogate`** | **6,407 ms** | 0 |
| pose resolutions, catalogue-answered | 85,240,396 | 157,789,789 |
| pose resolutions, overflow-answered | 53,687,385 | **0** |
| `ensure_rotation_surrogate` calls | 2,116,504 | 0 |
| `prepare_continuous_candidate` calls, armed | 1,429,252 | 0 |
| deque scan calls | 1,140,368 | 0 |
| **pair-NFP cache lookups** | **0** | **0** |

The build count is not a noisy quantity: a second census binary, built to split
the build's stages apart, ran the same cell and performed **1,129,375** builds
again, to the unit.

> **This table is measured on a census binary that already carries §2's
> fixes**, because the process-wide build counter and the fixes were written in
> the same pass. That is only honest to quote as "an armed run's work" because
> §3.1 shows the fixes change no volume in it: over three isolated slices,
> before and after, `resolveCatalogHit`, `rotationSurrogateBuilds`,
> `rotationSurrogateEvictions` and the rung counts are equal to the unit. The
> two rows that *are* post-fix quantities are **deque scan calls** — §2.2
> removed 47% of them — and `rotationSurrogateHits`, which §2.2 redefines. The
> pre-fix figures for both are in §1.3.

Two of these settle questions the task asked to be settled by measurement
rather than by reading:

* **The pair-NFP cache is not on this path at all.** The brief's hypothesis was
  that `pair_nfp_cache`'s keys carry both operands' angle keys, so continuous
  angles might blow it. They cannot: the cache is `DirectionalPenetration`'s,
  `continuous_rotation_lane` refuses that pressure model, and the counter is
  **zero in both arms**. It is instrumented anyway so that this is a
  measurement.
* **`rotationBuildsRefused` is zero everywhere**, on every cell of every
  instrument in this round, so the residency guard of the previous round's §3.2
  still never binds.

### 1.2 What a build actually costs

`build_oriented_surrogate`, timed stage by stage over the same 1,129,375 builds
(`evidence/decompose-buildstages.json`):

| stage | µs/build | share |
|---|---:|---:|
| `source.transformed(θ, mirrored)` | 0.51 | 8.8% |
| **`.offset(expansion_mm)` — the Clipper offset** | **4.71** | **81.4%** |
| `triangulate_ring` | 0.16 | 2.7% |
| poles + cell axes + `CellIndex` | 0.41 | 7.0% |
| **total** | **5.78** | |

The µs/build column is a **census-build** rate and is inflated: the same
quantity on the measurement binary, from §4's own battery, is **4.19 µs**
(4,863 ms over 1,160,616 mode-34 builds at ten seconds), and the previous
round's per-iteration figures imply about 7.4 µs (5.4 µs per iteration at 0.73
builds per iteration). The *shares* are what the stage split is for, and a
uniform slowdown does not move them. Four fifths of a build is one
`ClipperOffset::execute_poly_tree` on a shape whose triangulation is under three
cells — that is the reading, on any of the three rates.

What was never missing was the rate. The previous round measured it correctly.
What was missing was the **count**.

This is the number the round turns on, so it is worth stating plainly: **the
continuous-rotation operator's price is one polygon offset per proposed rung.**
`prepare_continuous_candidate` was entered armed **1,429,252** times and
**1,129,375** of those — **79.0%** — landed on an angle this lane had never
seen, each one a fresh offset.

There is no cache design that fixes a 79% compulsory-miss rate. A per-piece
continuous angle space has no reuse across rungs *by construction*: the rung is
`dtheta = dx / r` scaled by a factor the descent contracts on every rejection,
so consecutive iterations propose angles that differ, and an angle once
rejected is never proposed again. The 48-entry window is not too small; the
misses are compulsory. The previous round's §4 said as much about triangle-20's
52.5% hit rate — "a per-piece continuous angle space has no reuse across pieces
by construction" — without noticing that the same sentence applies to *every*
request once the mode-22 builds are counted.

### 1.3 The resolution tax, priced

The isolated slice, three parents, armed, before any fix
(`evidence/taxprobe-before.json`):

| | count |
|---|---:|
| pose resolutions, total | 175,114,298 |
| ... answered by the catalogue, one descent | 86,880,167 |
| ... **missing the catalogue, then answered by the overflow map** | **88,234,131** |
| deque scan calls / entries compared | 3,822,609 / **212,524,182** |
| `ensure_state_surrogates` calls / wall | 50,866 / **428.2 ms** |

So **50.4% of an armed lane's resolutions paid two ordered-map descents**,
one of them a full-depth failed search of the catalogue. That is the tax
§1.2 of the previous round named, measured. The deque scan it named in §6 is
real too — 212.5 M comparisons, 55.6 per call, because the queue is trimmed
only at the top of `search_piece` and drifts above its 48 capacity in between —
but it is the **smallest** of the four components, not the one to fix first.

---

## 2. The fixes

Three were built. **Two ship**, both under the existing `continuous-rotation`
flag and both **answer-preserving**: §3 shows 30 paired equal-work cells with
zero mismatches on fingerprint, depth, candidate queries and exact pair tests,
and §6 shows the four pinned gates reproducing as whole documents. The third —
the one the task called certain — was measured and reverted, and §2.3 is that
measurement.

### 2.1 A remembered route, for the resolution tax

`SurrogateRoute` rides in `AngleKeyCache`'s existing per-piece slot, beside the
rotation key it already memoises, and records which of the two maps answered
last time. `resolve_surrogate_routed` tries the overflow map first when the
slot says `Overflow`.

Soundness is a property of the data rather than of the hint: **the two maps'
key sets are disjoint by construction.** The overflow map has exactly one
insert site, `ensure_rotation_surrogate`, guarded by
`catalog.orientations.contains_key(&key)` returning false; the catalogue is an
immutable `Arc` for the whole lane. No key is ever in both, so probe order
cannot change the answer. The hint is *verified* anyway — a miss falls through
to the original catalogue-then-overflow order — because the slot is keyed on
`(input_index, rotation bits)` and a `SurrogateKey` also carries `mirrored`.

The slot forgets the route in the same store that re-derives the key on a bit
mismatch, so there is no separate invalidation path to get wrong. An unarmed
lane's overflow map is empty, so no slot can ever leave `Unknown`/`Catalog` and
the resolution performed is today's, instruction for instruction.

Wired into the two hot sites: `scan_fixed_neighbors`'s per-neighbour resolution
and both bodies of `score_placement`'s candidate resolution.

**Measured:** 65,033,076 of the 88,234,131 off-grid resolutions — **73.7%** —
now answer on the hint with one descent. The residue is the resolutions that
reach `resolve_surrogate` from sites this round did not route (`local_shape_bounds`
through `oriented`, `score_state`, `ensure_oriented`); §5 says why they were
left.

### 2.2 A per-piece pose slot, for `ensure_state_surrogates`

`ensure_state_surrogates` walks every placement on **every whole-state entry
point** — `move_sweep`, `tight_strip_depth`, `refresh_boundary_rows`,
`score_state` — and 50,866 of those walks in three isolated slices cost 428 ms,
almost all of it re-establishing an invariant that already held.

`RotationCache::ensured[i]` records the `(rotation bits, mirrored)` last
*pinned* for piece `i`, written in `ensure_and_protect_pose` beside the pin it
describes, so the other pinning caller (`move_sweep`'s accepted replacement)
keeps it in step for free. A piece whose pose has not moved is skipped.

The skip is sound because **a pin is never evicted**: `pin_counts` holds a
pinned key out of the eviction queue for exactly as long as it is a pin, so
"piece `i` still sits at the pose we pinned" implies "that pose still
resolves", which is the whole of what the loop had to establish.

That argument is only as good as the coupling between the slot and the pin, so
the coupling is made local rather than left as a fact about the call graph:
`pin_rotation_pose` **clears** the slot whenever it moves a pin, and
`ensure_and_protect_pose` re-establishes it immediately afterwards. Today
`ensure_and_protect_pose` is the only production caller of `pin_rotation_pose`,
so the clear never fires on a hot path; it exists so that a future second
caller cannot silently license a skip over an evictable pose.
`a_state_ensure_skips_a_piece_that_has_not_moved` drives that case directly.

**Measured:** 428.2 ms → **39.1 ms**, 10.9x, with `rotationSurrogateBuilds`,
`rotationSurrogateEvictions` and every trajectory quantity unchanged.

> One diagnostic counter does change, and it would be dishonest to leave it
> unsaid: `rotationSurrogateHits` falls from 810,186 to 113,917 on the isolated
> slices, because a skipped ensure no longer counts the cache hit it would have
> taken. The counter now reports *cache hits the operator needed*, not *cache
> hits including ones taken to re-confirm an unmoved pin*. Nothing reads it but
> a reader.

### 2.3 The indexed probe queue the previous round asked for, built and reverted

This is the one fix the task called **certain**, and it is the one that is not
in the committed tree. It was built, measured, and taken back out.

docs/experiments/continuous-rotation/ §6:

> `touch_rotation_probe` and `acquire_rotation_key` linear-scan a 48-entry
> deque, and at ~1.2 M cache hits per armed ten-second run that is tens of
> millions of comparisons — bounded, but not free, and **removable with an
> index**.

The count is right and larger than the estimate: **212,524,182** entry
comparisons over three isolated mode-34 slices, 55.6 per call, because the queue
is trimmed only at the top of `search_piece` and drifts above its 48 capacity in
between. The index was built exactly as described — two ordered maps and a
monotone stamp, `order` the queue front-to-back and `stamp` its inverse, every
operation `O(log n)`, and the deque's eviction order reproduced element for
element — and it brought the comparisons down to **2,011,914**, one per call.

**It was 0.38% slower.** `drivers/run-probequeue-ab.sh` builds the indexed
variant off the committed tree and `drivers/ablate.py` pairs them at equal work,
armed, three parents × 8 rounds = 24 cells, `evidence/probequeue-ab.json`:

| statistic | deque (committed) | indexed | |
|---|---:|---:|---|
| slice ms, median | **665.9** | 668.7 | deque ahead on **22 of 24** |
| paired slice ratio | | | **0.9962** (deque / indexed) |
| paired process ratio | | | **0.9887**, deque ahead on 21 of 24 |
| equality mismatches | | | **0 of 24** |

At a window of 48 the index loses to the scan, and it is not mysterious: 55
contiguous 24-byte tuples are 21 cache lines the prefetcher walks in order,
against two ordered-map descents that chase pointers. "Removable with an index"
was true and "worth removing" was not, and the only way to tell those apart was
to build both.

**What is kept is the counter, not the code.** `probeScanCalls` /
`probeScanSteps` stay in the census, because the number they produced is what
retires the item: the scan is the smallest of §1's four components, and it is
now measured rather than estimated. If anyone raises
`ROTATION_CACHE_PROBE_CAPACITY` to attack the miss rate of §1.2, the scan grows
linearly with it and this trade flips — the driver that rebuilds the indexed
variant is committed so that the flip is one command away.

### 2.4 The two documented mid-round defects

Both were already repaired at the base commit and **nothing remained**:

* the **residency guard** (previous round §3.2) is
  `rotation_residency_available`, checked in `ensure_rotation_surrogate` and
  again in `prepare_continuous_candidate` before anything is proposed, over
  resident cells rather than the catalogue's cumulative counter.
  `rotationBuildsRefused` is **0** in every cell of every instrument this round
  ran, so it still never binds;
* the **mirror second probe on a no-rotation piece** (§3.3) is conditioned on
  `allow_rotation && allow_mirror`, pinned by
  `a_piece_forbidden_to_rotate_is_never_offered_the_mirror_companion`.

Both tests are in the suite runs of §6.4.

---

## 3. What the fixes are worth, at equal work

`drivers/ablate.py`: one pinned parent, one serial mode-34 slice, a fixed work
cap (the anatomy's design slice, 3,341,379 units), the operator **armed on both
sides**, and the only difference is the binary. Paired and interleaved with the
binary order reversed on odd rounds. Three parents × 10 rounds = 30 paired
cells. `evidence/ablate.json`.

| statistic | base `a2fd148` | committed | paired |
|---|---:|---:|---|
| slice ms (`repair + confirmation`), seed 0 | 689.5 | 667.3 | |
| ... seed 1 | 553.9 | 535.6 | |
| ... seed 2 | 2427.6 | 2295.4 | |
| **paired slice speedup** | | | **1.0373x**, **30/30**, [1.0065, 1.0627] |
| paired process speedup | 4.147 / 3.233 / 6.781 s | 4.028 / 3.152 / 6.556 s | **1.0306x**, **30/30**, [1.0124, 1.0496] |
| within-arm relative spread | 0.018 / 0.016 / 0.016 | 0.022 / 0.014 / 0.012 | |

**Read the spread before the delta.** The within-arm spread is 1.2–2.2% and the
slice delta is 3.7%, so the *unpaired* medians would not carry this. The paired
count does: 30 of 30 cells, every one above parity, on both statistics.

> An earlier version of this table, on the binary that still carried §2.3's
> index, read 1.0328x on the slice and 1.0186x on the process. The index was
> costing 0.45 and 1.2 percentage points of it, which is the same verdict
> §2.3's direct isolation reached from the other side.

### 3.1 Equal-work quality: not "unchanged", *identical*

The task asked for a quality spot-check on 3 of the 12 pinned parents at equal
work, because the previous round measured +0.005 mm median there and the fixes
must move wall and not quality. The fixes do better than that, and the stronger
claim is the one worth making: over all **30 paired cells**, `fingerprint`,
`rawSourceDepthMm`, `processCandidateQueries`, `processExactPairTests`,
`exactValid` and `contractValid` are compared cell by cell and there are
**zero mismatches**. The two binaries walked the same trajectory to the same
layout; only the clock differed.

The check is not vacuous — the fields it compares are populated. Round 0, seed
0, both arms: fingerprint `c81f12964bc31339…`, `rawSourceDepthMm`
173.52416349775987, `processCandidateQueries` 7,839,877, `processExactPairTests`
618,872, `exactValid` and `contractValid` true.

The census agrees from the other side. Over the three isolated slices, before
and after: `resolveCatalogHit` 86,880,167 both, `rotationSurrogateBuilds`
117,242 both, `rotationSurrogateEvictions` 116,928 both,
`rotationRungsProposed` 81,358 both, `mirrorTogglesProposed` 80,868 both. The
same search, resolved differently.

And §2.3's own 24 cells add 24 more zero-mismatch pairs on a third binary, so
the equality claim rests on 54 paired cells across three builds.

### 3.2 And on the wall, from the bare request

`drivers/binab.py`, mixed-61, 10 s wall, **`crot=1` on both sides**, 3 seeds ×
4 rounds interleaved, `evidence/binab-10s.json`:

| | base `a2fd148` | fixed | paired |
|---|---:|---:|---|
| **mode-34 wall per slice** | 1.9775 s | **1.8243 s** | **1.0796x** median, 12/12 |
| mode-22 wall per call | 0.9163 s | 0.9042 s | 1.0103x median |
| mode-34 slices per run | 2 | 2 | 0 |
| published depth | 175.219 | 175.219 | **+0.000 mm** |

An 8% cheaper armed slice, and **it buys nothing**: the slice count does not
move, so the depth does not move. That is the shape of what follows.

> This table was measured before §2.3's index was reverted, so its "fixed"
> column is the A+B+C binary rather than the committed one. It is left as
> measured rather than re-run, and the direction of the correction is known:
> the index cost 0.38% of slice wall (§2.3), so the committed binary is if
> anything slightly *better* than 1.0796x here. Nothing in this section's
> reading depends on which side of 8% the number falls.

---

## 4. The compound battery

`drivers/run-battery.sh` → `drivers/battery.py`, anytime **WALL** from the bare
request, both arms with `fast-contract-validator` compiled in (it has no spec
key) and `m34lanes=1,m34pconfirm=1`, `m34wall` and `m34bit` at their v3
defaults. Three requests × three seeds × three rounds × 3/10/30 s × two arms =
162 runs. The statistic is the per-round **paired** difference in published
depth, `crot` minus `base`, so **a negative number is the operator winning**.
`evidence/curves-summary.json`, `curve-*.json`.

### 4.1 The verdict

| request | 3 s | 10 s | 30 s |
|---|---|---|---|
| **mixed-61** | +0.000 mm, 3 of 9 better | **+6.735 mm worse, 3 of 9** | **+5.736 mm worse, 0 of 9** |
| shapes-17 | +0.000 mm, 0 of 9 | +0.000 mm, 0 of 9 | +0.000 mm, 0 of 9 |
| triangle-20 | +0.000 mm, 0 of 9 | +0.002 mm, 0 of 9 | +0.000 mm, 0 of 9 |

The round's success criterion was "mixed-61 10 s better paired with rotation
armed". It is **worse by 6.735 mm**, and worse by *more* than the round that
preceded the tax fixes measured (+3.721 mm). That is not a regression in the
operator; it is the base arm improving faster than the armed arm, and §4.2 is
why.

### 4.2 The mechanism: the fast validator helped the arm that was not paying for rotation

Aggregated over the nine paired rounds per budget, mixed-61:

| | 10 s base | 10 s crot | 30 s base | 30 s crot |
|---|---:|---:|---:|---:|
| mode-34 slices run | **40** | **15** | **62** | **34** |
| ... published | 40 | 12 | 56 | 25 |
| mode-34 wall, total | 31.38 s | 24.99 s | 69.90 s | 53.39 s |
| **wall per slice** | **0.784 s** | **1.666 s** | **1.127 s** | **1.570 s** |
| surrogate builds, m34 only | 0 | 1,160,616 | 0 | 2,477,398 |
| surrogate build ms, m34 only | 0 | 4,863 | 0 | 11,243 |
| published depth, median | **168.484** | 175.219 | **165.262** | 170.458 |

The armed slice costs **2.12x** the unarmed one at ten seconds — *after* the tax
fixes. §3.2 priced the fixes on exactly this quantity, paired on one binary
pair under one spec: **1.0796x**, so the ratio before them was about 2.29x.
(The previous round's 2.23x is not the right comparator: it was measured on a
binary without `fast-contract-validator`, whose unarmed slice was 0.87 s where
this one is 0.784 s.) Either way, 2.12x still buys 15 slices where the base arm
gets 40.

**The fast contract validator is what changed the question, and it changed it
against rotation.** Its 5.57x confirmation lifted the *base* arm from the
previous round's 172.288 mm to **168.484 mm** at ten seconds — 3.8 mm. The armed
arm went from 175.219 mm to **175.219 mm**: the same three decimals both rounds
report, which is as far as the comparison can be pushed across two binaries. A cheaper
confirmation multiplies whatever the search can already do, and the armed
lane's bottleneck is not confirmation: it is 1.13 M polygon offsets in the proxy
tier, which no confirmation speedup touches. The task's hypothesis — that the
new budget structure (49 slices instead of 30) might make rotation's per-slice
tax affordable — is answered, and the answer is the opposite: **more slices per
second makes the arm that fits more slices in win by more.**

### 4.3 Against Sparrow

Same box, same request, same 5.0 mm pair clearance
(`docs/experiments/sparrow-mixed61/` x86_64 addendum: 157.971 mm @ 3 s,
150.165 mm @ 10 s, exact-valid at 61/61):

| budget | Sparrow | base median | behind | crot median | behind |
|---|---:|---:|---:|---:|---:|
| 3 s | 157.971 | 179.587 | **21.6 mm** | 179.633 | **21.7 mm** |
| **10 s** | **150.165** | **168.484** | **18.3 mm** | 175.219 | **25.1 mm** |

The fast validator closed the ten-second gap from 22.1 mm to **18.3 mm**. The
rotation operator reopens it to **25.1 mm**. On the budget the binding user
priority names, arming the operator moves the engine 6.7 mm *away* from
Sparrow.

### 4.4 Two readings that are not the verdict

**The rungs still work.** At ten seconds the armed arm ran 903,045
rotation/mirror iterations, **8.9%** of them improving the incumbent, and
**56.4%** of all the proxy loss the refinement removed was removed by a rotation
or mirror move against 43.6% by the four translation axes. **68.9%** of the
moves the sweep committed changed the pose. Every one of those numbers
reproduces the previous round's (8.3%, 56.0%, 67.1%). The mechanism is not what
failed.

**The cache hit rate the previous round reported was not the operator's.** It
recorded 89.4%; this round records **54.0%** at ten seconds on mixed-61,
**72.1%** on shapes-17 (against 88.3%) and **40.3%** on triangle-20 (against
52.5%) — and the difference is §2.2, not a regression. The old number counted
the cache hits `ensure_state_surrogates` took to re-confirm poses that had not
moved, which are hits nothing needed. With those skipped, what is left is the
operator's own probe reuse, and on mixed-61 it is barely better than a coin
flip. That is the same story §1.2 tells from the other end: at a 79% compulsory
miss rate there is no cache to win. The previous round's §4 read the 52.5% on
triangle-20 as that request breaking the cache; the honest reading is that the
window was never holding much on any request, and triangle-20 was simply the
one where the re-confirmation hits were too few to hide it.

### 4.5 The corpus, and an accidental cross-validation

shapes-17 saturates at 200.349 mm in both arms at every budget — 0.000 mm on
27 of 27 cells, with a within-arm spread of exactly zero — and its armed
mode-34 slice publishes on **0 of 9** runs in both arms, as it did in the
previous round. Both arms run **9** mode-34 slices at ten seconds and the same
9 at thirty (the coordinator exits early on a saturated request), and the armed
arm pays **1.370 s** per slice against **1.085 s**, 355,404 surrogate builds,
for a depth delta of exactly zero. Bought and thrown away, as before.

triangle-20 is the same story with one extra digit: **+0.002 mm** at ten
seconds — 70.730 against 70.732 on two of three seeds, in every round — and
0.000 mm at three and thirty. Its armed mode-34 slice publishes on 0 of 9 runs
in both arms too, so the operator has nothing it can change about the
incumbent and pays 1,336,518 rotation/mirror iterations to find that out.

What makes shapes-17 worth a paragraph anyway is that its armed arm reproduces
the previous round's attribution **to the unit**, on a different binary five
commits later:

| 10 s, armed | shapes-17, prev | shapes-17, now | triangle-20, prev | triangle-20, now |
|---|---:|---:|---:|---:|
| rotation + mirror iterations | 436,998 | **436,998** | 1,336,518 | **1,336,518** |
| improved the incumbent | 0.1% | 0.1% | 0.7% | 0.7% |
| share of proxy loss bought by rotation | 93.3% | 93.3% | 83.0% | 83.0% |
| accepted moves that changed the pose | 22.2% | 22.2% | 3.7% | 3.7% |
| build wall per rotation iteration | 9.2 µs | 9.2 µs | 6.3 µs | 6.3 µs |
| **cache hit rate** | **88.3%** | **72.1%** | **52.5%** | **40.3%** |

Every row is identical except the last, on **both** requests, and the last is
§2.2 by construction: the hits that vanished are the ones
`ensure_state_surrogates` was taking to re-confirm poses that had not moved.
The agreement everywhere else is the strongest available evidence that this
round's binary is the previous round's binary plus a routing change — and that
the cache-hit number the previous round published was measuring something other
than the operator's probe reuse.

### 4.6 Caveats

* **One 30 s mixed-61 cell has a confound, and it favours the base arm.** On
  seed 2 the coordinator overran its 30 s budget to **41.24 / 41.30 / 41.34 s**
  in the base arm in all three rounds, against 29.0 s in the armed arm; no other
  cell in the battery overran by more than two seconds. `battery.py` records
  `overrunSeconds` per row. The cell is kept because dropping an inconvenient
  cell is worse than reporting it, and because the 30 s verdict does not turn on
  it: seed 1, which does not overrun, is **+10.448 mm in all three rounds**, and
  the overrunning cell is the *least* bad one for the operator (+0.969 mm).
* **Nine cells is three results repeated**, at a wall budget, exactly as
  docs/experiments/fast-contract-validator/ §3.3 warned. At ten seconds, eight
  of the nine (seed, arm) cells reproduced their depth to every digit across all
  three rounds — only seed 0's base arm varied (168.484 twice, 169.588 once).
  The sample that carries the mixed-61 verdict is **three seeds**, not nine.
  All three seeds agree at 30 s; at 10 s **seed 2 is a genuine 1.139 mm win for
  the operator**, in all three of its rounds, which is where "3 of 9 better"
  comes from and is worth more than the count makes it look.
* **Step 5 of the task did not trigger.** It was conditional on the compound
  being positive at ten seconds. It is negative, so no extra 30 s battery and no
  deep-parent equal-work probe were run.

---

## 5. Why this is where the round stops, and what the next lever is

The arithmetic of §1 is what closes it. An armed ten-second run spends
**6,407 ms inside `build_oriented_surrogate`**, four fifths of it in one Clipper
offset, on 1.13 M rungs of which 79% land on an angle no cache has seen. The
fixes in §2 attack everything *around* that number and remove 3.7% of an
equal-work slice and about 8% of an armed from-request one. Nothing in the
resolution path, the eviction queue or the state-ensure walk can remove a
compulsory offset per rung.

> **6,407 ms is CPU summed over every lane thread, not 6.4 s of a 10 s wall**,
> and it was measured on the census build, which is slower than the measurement
> build. The measurement build's own counter agrees on the part it can see: 537
> ms of mode-34 build time per run at ten seconds (`evidence/binab-10s.json`),
> against the census build's 703 ms for the same slices. Neither caveat touches
> the correction, because the correction is a **count** ratio — 169,772 mode-34
> builds of 1,129,375 in the process — and a count carries no clock at all.

**The next lever is named and it is not a tax fix.** A miter-join offset is
rotation-equivariant in exact arithmetic: offsetting a rotated polygon and
rotating an offset polygon are the same set, because a miter join is built from
the two incident edge normals and both rotate with the ring. So the operator
could offset each piece
**once**, at zero degrees and mirrored, and derive every rung's surrogate by
transforming the already-offset ring — replacing 4.71 µs of Clipper with a ring
transform. The remaining three stages total 1.08 µs, and the transform would run
on the offset ring rather than the source one (an offset adds vertices), so the
honest estimate is **roughly four times cheaper per rung**, not exactly 5.4x.

It is deliberately **not** done here, and the reason is the one this round was
given: the two orders round differently on Clipper's integer grid, so the
surrogate's geometry changes, so proxy scores change, so trajectories change.
That is not a wall fix with a quality spot-check; it is a new operator geometry
that needs its own matched-arm quality battery, and shipping it inside a round
whose licence is "answer-preserving" would be exactly the kind of silent change
this file is organised against. The measurement that makes it worth doing is
here: **81.4% of the operator's dominant cost is one function call away.**

Three smaller items, for the same reason and with the same honesty:

* the **routing residue**. 23.2 M of 88.2 M off-grid resolutions still pay both
  descents, because they reach `resolve_surrogate` from `local_shape_bounds`
  through `oriented`, from `score_state`, and from `ensure_oriented`. Routing
  `local_shape_bounds` means making its default body read `AngleKeyCache`
  instead of `derive_rotation_key`, which is what the separately-priced
  `relaxed-cached-pose-bounds` feature exists to do; folding half of that
  feature into this round would have made the gate result ambiguous about which
  change produced it. Left, named, with the count attached.
* the **catalogue's failed descent** could be made `O(1)` for every call site at
  once with an exact membership filter on `SurrogateCatalog`, rather than site
  by site with a hint. That is the better structure and it touches the default
  build's catalogue, so it wants its own round.
* the **48-entry probe window** is the one tuning knob left that could attack
  the 79% miss rate directly, and it is the one place §2.3's reverted index
  would earn its keep, because the deque's scan grows linearly with the window
  and the index's descent does not. §1.2 says why the expected return is small —
  the misses are compulsory, not capacity — but "small" is an argument and a
  battery is a measurement, and this round did not run one.

---

## 6. Gates, suites, determinism

**The four pinned gates, on rebuilt binaries, exits captured directly.**
`drivers/gates.py`, `evidence/gates-*.json`.

| binary | g1 206.869 / `8a7737381238fa4d` | g2 159.09233022733062 | g3 159.07876040364795 | g4 164.0375677990678 |
|---|---|---|---|---|
| `base` (a2fd148, `jagua-experimental`) | hit | hit | hit | hit |
| `commit-gate` (committed tree, `jagua-experimental`) | hit | hit | hit | hit |
| `commit-meas` (committed tree, full feature set, flags off) | hit | hit | hit | hit |

All four raw depths and fingerprints reproduce, and — the stronger check — the
**whole-document digest** is identical across all three binaries on all four
gates: `cb9caac2a1667635`, `20a6d7eace3011aa`, `cc4f738b84907bfc`,
`3f13a6e7ec0a3717`. Not the same pinned scalars: the same document, field for
field, with only the elapsed-derived statistics and the build-identity fields
stripped. That is the property that matters here, because `AngleKeyCache` and
`resolve_surrogate` are on the hot path of **every** mode in a default build,
not only of the armed lane.

> The gates' wall times are **not** a wall claim and are not quoted. The base
> binary's gate run shared the box with a compilation; the two committed
> binaries' runs did not. §6.1 is the controlled version of that question.

### 6.1 The flag-off path, which every mode uses

The gates prove the default build's *answers* did not move. They do not prove
its *clock* did not, and this round put a branch and a byte into
`AngleKeyCache` — the memo on the hot path of every mode, not only of the armed
lane. `drivers/binab.py` with **`crot=0`** on both sides, mixed-61, 10 s wall,
3 seeds × 3 rounds, base binary against the committed one:

| | base `a2fd148` | committed |
|---|---:|---:|
| published depth, median | 168.4836 | **168.4836** |
| paired depth delta, median | | **+0.000 mm** (1 better, 1 worse, 7 equal) |
| mode-34 slices per run, median | 2 | 2 |
| mode-34 wall per slice | 0.7221 s | 0.7240 s (**0.997x**) |
| mode-22 wall per call | 0.6742 s | 0.6791 s (**1.005x**) |

**Read this one with its instrument's limits stated.** A from-request run's
mode-34 slice population varies enormously between seeds — the within-arm
spread on per-slice wall is **74%**, against a between-arm ratio of 0.3% — so
this cannot resolve anything smaller than a few percent, and it is not being
asked to. What it establishes is the absence of a *visible* regression on the
path every mode uses: the median depth is the same to four decimals, the
paired depth delta is zero, and the two directional ratios (0.997x on the
slice, 1.005x on mode 22) straddle parity. The strong statement about the
flag-off path is §6's gates, which are exact.

### 6.2 Flag-off document reproduction against the base commit

`drivers/reproduce.py`, whole documents at a 40 M **work** budget through the
coordinator — work rather than wall so both sides are deterministic and
load-independent — three requests × three seeds, both sides on the plain default
spec so neither names a key:

**9 of 9 identical**, `allEqual: true`, `evidence/reproduce.json`. Fields that
two processes cannot agree on by construction — every timing, the executable
hash, the worktree fields — are stripped from both sides and listed in the
output, so what remains is a diff of the search. With `crot` unset, the
committed binary **is** the base binary.

### 6.3 Determinism across two processes, armed

The hard gate for anything armed. `drivers/determinism.py`,
`crot=1,m34lanes=1,m34pconfirm=1`, 40 M work budget, three requests × three
seeds, two processes per cell, whole documents. It matters more this round than
last: §2.1's route is a piece of *per-lane mutable state* that the resolution
path now reads, and §2.2's slot decides whether work happens at all. Both are
deterministic functions of the lane's own sequence of calls — nothing here
reads a clock or a thread id — but that is an argument, and this is the
measurement.

**9 of 9 equal**, `allEqual: true`, `evidence/determinism-crot.json`. The armed
mixed-61 cells publish 175.21893730689536 / 172.3309999959951 / … in both
processes, to the last bit.

### 6.4 Suites

`drivers/run-suites.sh`, both suites, **exit status read from `$?` on the line
after the command** rather than through a pipe — `cargo test … | tee log`
reports `tee`'s status, which is how a red suite gets written up as green.

| suite | targets | result | exit |
|---|---:|---|---:|
| `--features jagua-experimental` | 55 | **1,262 passed, 0 failed** | **0** |
| `--features jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,fast-contract-validator` | 55 | **1,300 passed, 0 failed** | **0** |

The 38-test difference is the schedule features' own tests plus this round's.
Both new tests appear in the second suite's log and pass, as do the two that pin
§2.4's defects.

**Suite 1 needed its rerun, and the first run is reported rather than
discarded.** Its first attempt exited **101** on
`search::layout_scorer::tests::free_material_multi_eviction_shrinks_retained_container_capacity`
— `assertion failed: cache.entries.capacity() < entries_capacity_before` — which
is the flake the campaign protocol names by name and instructs to rerun once.
The rerun is the row above. Nothing this round touches is anywhere near
`layout_scorer`'s container capacities, and the same suite's 1,300-test superset
passed that test on its first attempt in the same session.

### 6.5 The operator's regression tests

Three added by this round, all in
`crates/polygon-nesting-core/src/search/general_relaxed.rs`:

Two added by this round — one per shipped fix; §2.3's went out with the code it
covered.

| test | what it pins |
|---|---|
| `a_remembered_route_never_changes_which_surrogate_resolves` | §2.1's soundness, against both ways a hint can be wrong: an `Overflow` hint on a pose the catalogue holds, and an `Overflow` hint on a pose that has been evicted. Asserts pointer identity with the unrouted resolution, not equality |
| `a_state_ensure_skips_a_piece_that_has_not_moved` | §2.2: the skip happens, the skipped pose still resolves, a moved piece is ensured again, and unpinning behind the ensure's back withdraws the licence |

The previous round's eight are unchanged and still pass, including the two that
pin the defects of §2.4.

---

## 7. Files

* `drivers/taxprobe.py` — §1's decomposition on an isolated mode-34 slice.
* `drivers/decompose.py` — §1's decomposition on a from-request run.
* `drivers/ablate.py` — §3's equal-work paired ablation and its equality check.
* `drivers/binab.py` — §3.2's armed two-binary wall comparison.
* `drivers/phaseshare.py` — the leaf-phase difference that first showed the
  extra time was inside `move_sweep` and outside `score_placement`.
* `drivers/run-probequeue-ab.sh` — §2.3's isolation. It rebuilds the **indexed**
  variant from the committed (deque) tree by patching four call sites in place,
  builds it, and restores the file in a trap; every anchor is fatal on a miss,
  so a silent no-op cannot produce a 1.00x ratio that looks like a null result.
* `drivers/run-suites.sh` — §6.4's two suites, exits read from `$?`.
* `drivers/battery.py`, `summarize.py`, `workgate.py`, `gates.py`,
  `gatelib.py`, `reproduce.py`, `determinism.py`, `runlib.py`, `docdiff.py`,
  `smoke.py`, `offgrid.py` — from
  `docs/experiments/continuous-rotation/drivers/`, `ROOT` repointed at this
  worktree and otherwise byte-faithful.
* `drivers/run-battery.sh`, `drivers/run-verify.sh`, `drivers/collect.sh` —
  §4's battery, §6's equivalence run, and the evidence copy, exactly as run.
* `drivers/probequeue-index.patch` — the indexed variant of §2.3, as a patch
  against the committed tree.
* `evidence/*.json`, and the suite logs. `ablate.json` is §3's table;
  `ablate-with-index.json` is the same battery on the A+B+C binary, kept
  because §2.3's verdict is half-visible in the difference between them.

Reproduce:

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental                                     # gate binary
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,compression-schedule,\
parallel-compression-schedule,continuous-rotation,fast-contract-validator
cargo build --release --example general_request_benchmark \
    --features ...,rotation-tax-census                                # §1 only

D=docs/experiments/rotation-tax/drivers
P=docs/experiments/parallel-compression-schedule/evidence/parents.json

TAX_CENSUS_BIN=<census-binary> python3 $D/taxprobe.py <outdir> \
    <census-binary> $P 0.3 0.002 0,1,2
TAX_CENSUS_BIN=<census-binary> python3 $D/decompose.py decomp mixed-61 0 \
    wall 10000 'm34lanes=1,m34pconfirm=1'
python3 $D/ablate.py <outdir> <base-binary> <new-binary> $P 10 0,1,2
python3 $D/binab.py binab-10s 4 mixed-61 0,1,2 wall 10000 \
    'm34lanes=1,m34pconfirm=1,crot=1' <base-binary> <new-binary>
V4_BIN=<new-binary> bash $D/run-battery.sh
python3 $D/gates.py <label> <binary> <outdir>
```
