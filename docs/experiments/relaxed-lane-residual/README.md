# The relaxed lane's residual

The constructor chain ended with the residual pointing somewhere else: at 4.06 s
the mode-20 leaf was `moveSweep` 5,222 ms and `scorePlacement` 4,753 ms, and the
note said "the relaxed lane, not the constructor". This directory measures that
lane, sizes what is removable from it without changing a search, and ships the
part of it that is.

**Headline, and it is a negative one as much as a positive one: the ~2x that
Sol's ~2 s slice needs is not in this lane as a semantics-preserving change.**
42% of the candidate scorer is the pair-geometry floor that is already proven to
be at its floor; the upper-bound cutoff the roadmap hoped was unexploited is
already taken by **82% of scans**; and the largest lookup lever in the lane —
**103 million ordered-catalogue descents** on a 10 s coordinator run — is worth
**2.5-3.0%**, which is what shipped. The levers that would be worth 2x all
change which candidates get visited, and are class (B).

## What the profiled table actually says

One correction first, because every number below depends on it. **Phase
milliseconds are summed across the eight lane threads.** `moveSweep` at 5,222 ms
inside a 4,060 ms stream is not a paradox and not an error; it is eight lanes'
time added together. A phase total is therefore never a wall-clock claim, and
the only wall-clock claims in this document are paired interleaved A/Bs.

Three streams, one census build
(`search-profiling,relaxed-lane-census`, which costs about 9% of a mode-20
stream), thread-summed milliseconds:

| phase | m20 g1 | calls | m22 g2 | calls | coordinator 10 s | calls |
|---|---:|---:|---:|---:|---:|---:|
| `moveSweep` *(enclosing)* | 5,691.6 | 4,089 | 16,429.8 | 8,145 | 30,262.2 | 29,414 |
| `scorePlacement` *(enclosing)* | 5,221.8 | 4,089,768 | 15,186.6 | 10,898,458 | 27,529.7 | 20,645,490 |
| `scoreScan` *(enclosing)* | 3,116.6 | 3,600,763 | 8,818.4 | 9,523,974 | 14,502.2 | 16,765,870 |
| `pairCollide` | 1,262.0 | 15,562,760 | 3,580.5 | 41,122,236 | 6,019.3 | 70,468,035 |
| `pairPressure` | 940.9 | 6,363,087 | 2,823.6 | 17,139,907 | 4,448.2 | 28,614,530 |
| `scoreProbe` | 530.1 | 3,600,763 | 1,458.9 | 9,523,974 | 2,501.4 | 16,765,870 |
| `boundaryPenalty` | 342.0 | 4,450,975 | 906.3 | 11,637,224 | 2,002.0 | 23,362,267 |
| `scoreFinalize` | 74.2 | 3,600,763 | 197.5 | 9,523,974 | 343.3 | 16,765,870 |
| leaf total | 8,182.4 | | 14,955.6 | | 30,052.4 | |

`scoreProbe`, `scoreScan` and `scoreFinalize` are this stage's addition: they
split the generic scorer into the broad-phase probe, the neighbour scan and the
row sort, so the enclosing `scorePlacement` can be reconciled against leaves
instead of being a single opaque 5-second block.

**The coordinator is the case that matters and it is the most extreme:**
`scorePlacement` is **91.6%** of leaf time on a 10 s run from the bare request,
which published 179.052 mm dual-gate-valid. `collisionPolygonBuilds` there is
2,913 — the constructor is a rounding error at a budget, exactly as the task
predicted. Mode 22 is the same story: 2,920 builds, `scorePlacement` 15,186.6 ms.

## The scan's structure, exactly

The counters are the durable half of the census: exact call structure, one
relaxed add on a thread-local block, no sampling. The remarkable finding is how
**stream-invariant** the scan's shape is.

| statistic | m20 g1 | m22 g2 | coordinator 10 s |
|---|---:|---:|---:|
| candidate queries | 4,089,768 | 10,898,458 | 20,645,490 |
| …through the **generic** scorer | 3,600,763 | 9,523,974 | 16,765,870 |
| …through the dynamic-hazard scorer | 489,005 | 1,374,484 | 3,879,620 |
| ordered-catalogue descents | **22,617,886** | **59,877,384** | **102,974,975** |
| neighbours returned per scan | 7.37 | 7.45 | 7.17 |
| neighbours **visited** per scan | 4.28 | 4.29 | 4.14 |
| returned but never visited | 41.9% | 42.4% | 42.2% |
| scans stopping on the upper bound | **81.7%** | **83.7%** | **82.1%** |
| collision rows per scan | 1.77 | 1.80 | 1.71 |
| visited neighbours that collide | 41.3% | 42.0% | 41.2% |

Two of the task's four hypotheses die here, and it is worth saying so plainly:

* **"Upper-bound cutoffs unexploited" is false.** Between 81.7% and 83.7% of
  every scan already stops early on the caller's bound. There is no unexploited
  cutoff to take; the cutoff is why only 4.2 of 7.3 returned neighbours are ever
  looked at.
* **"Rescans of unmoved pieces" is not where the calls are.** The scan is
  re-entered because the *candidate* moved, not because the neighbours did, and
  the neighbour set is small (7.3) and already bounded by a 16x16 broad-phase
  grid. The 42% of returned neighbours that are never visited are dropped by the
  cutoff, not by a missing filter.

## Where the milliseconds inside `scorePlacement` go

Attributing the enclosing phases to their leaves, thread-summed:

| item | m20 g1 | m22 g2 | coordinator |
|---|---:|---:|---:|
| `pairCollide` inside the scan | 1,250.1 | 3,555.0 | 5,931.7 |
| `pairPressure` inside the scan | 940.7 | 2,823.0 | 4,446.9 |
| **scan residual** | **925.8** | **2,440.3** | **4,123.6** |
| …per visited neighbour | **60.1 ns** | **59.8 ns** | **59.4 ns** |
| `scoreProbe` | 530.1 | 1,458.9 | 2,501.4 |
| …per generic scan | 147.2 ns | 153.2 ns | 149.2 ns |
| `boundaryPenalty`, generic share | 276.7 | 741.7 | 1,436.7 |
| `scoreFinalize` | 74.2 | 197.5 | 343.3 |
| **floor share of `scorePlacement`** | **42.0%** | **42.0%** | **37.7%** |

The two per-unit costs reproduce to within 1.2% across three streams with
different piece counts, different operators and a 5x range of call counts, which
is the strongest evidence here that the census is measuring the loop and not the
noise.

**42% of the scorer is `pairCollide` + `pairPressure`**, which the corpus has
already proved is at its floor: the pole-loop early-out is impossible and a
correctly-rounded `hypot` is 87% of the loop. That 42% is not addressable. What
is left is a 60 ns-per-neighbour scan residual and a 149 ns-per-scan probe, and
neither is one thing — they are a catalogue descent, a rotation-key derivation,
a weights lookup, a bin walk, a small sort and a `Vec` push each.

## What shipped

Two stacked flags, both off by default, both **bit-identical as whole
documents**.

### `relaxed-scan-shape-reuse`

The scorer descended the ordered catalogue for the candidate **twice**: once
through `oriented()` to get the bounds the broad-phase probe needs, and again
inside the scan for the shape itself — the same key, the same entry. The probe
now runs from the shape the scan already resolved. The neighbour loop moved into
`scan_fixed_neighbors` so that both arms run byte-identical physics and can only
differ in how many times the candidate's key is looked up.

It removes one descent per generic scan — 3,600,763 on m20 g1, 9,523,974 on
m22 g2, 16,765,870 on the coordinator — **and** one uncached `derive_rotation_key`,
which runs `rem_euclid` and a rounding step.

### `relaxed-cached-pose-bounds`

Stacked on the first, in the same style `fast-constructor-reject` is stacked on
`fast-constructor-confirm`, so the two can be priced separately. The lane's
per-pose bounds lookup — `boundary_penalty` is nearly all of its 4.45M / 11.64M /
23.36M calls — also re-derived the rotation key with `rem_euclid` on every call,
while the candidate scan has had an `AngleKeyCache` memo for it all along. It now
asks the memo. Same key, same catalogue entry, same bounds, same
missing-orientation error.

Together the two remove **8.05M of 22.6M descents on m20 (36%), 21.2M of 59.9M
on m22 (35%), and 40.1M of 103M on the coordinator (39%)**.

## What it is worth

Paired interleaved A/Bs, arms alternating order every round, statistic the
per-round paired ratio. Arm A is
`jagua-experimental,fast-constructor-profile,fast-constructor-confirm,fast-constructor-reject`
— the 4.06 s stream the constructor stage left behind.

| sample | rounds | arm B | A median | B median | paired median | range | below 1.0 |
|---|---:|---|---:|---:|---:|---|---:|
| m20 g1 | 12 | `+shape-reuse` | 4.0740 s | 4.0634 s | 0.9976 | 0.9888-1.0052 | 7/12 |
| m22 g2 | 10 | `+shape-reuse` | 3.1360 s | 3.0706 s | **0.9754** | 0.9611-1.0031 | 9/10 |
| m20 g1 | 12 | `+both` | 4.0653 s | 4.0310 s | **0.9917** | 0.9836-1.0023 | 11/12 |
| m22 g2 | 12 | `+both` | 3.1089 s | 3.0170 s | **0.9700** | 0.9473-0.9823 | 12/12 |
| m22 g2, rebuilt from the commit | 10 | `+both` | 3.1063 s | 3.0055 s | **0.9677** | 0.9468-0.9849 | 10/10 |
| coordinator `work=20000000` | 10 | `+both` | 5.0953 s | 4.9686 s | **0.9750** | 0.9620-0.9909 | 10/10 |

* **m22 must not regress, and it does not — it is the biggest winner**, because
  the record-replay stream barely touches the constructor and is therefore
  almost pure relaxed lane. This is the mirror image of the constructor stage,
  where m22 was the row that came back at parity.
* **m20 g1 with `shape-reuse` alone is a real parity result** and is reported as
  one: 0.9976 with the range straddling 1.0 and only 7 of 12 rounds below it.
  Stacking `cached-pose-bounds` is what lifts m20 clear of the noise, to 0.9917
  with 11 of 12 rounds below parity.
* **The coordinator row is the load-bearing one.** It is budgeted in *work
  units*, not seconds, so both arms run the identical scheduled search and the
  wall clock is the comparison. All ten rounds are below parity, and the
  incumbent is identical in both arms — depth 180.64489329491147, fingerprint
  `b5672b945f325b4d…`, one publication at 9,064,287 work units — with the
  publication's `seconds` timestamp the only field that moves.
* **The fourth row was taken on binaries rebuilt from the committed tree**, so
  the headline reproduces from the commit and not only from the tree it was
  measured in. It agrees with the third row to 0.24%, on a box whose one-minute
  load average was 1.92 rather than 0.88 — which is what the paired design is
  for.

## Equivalence evidence

* **The refactor is free.** The default build, after the neighbour loop moved
  into `scan_fixed_neighbors`, reproduces the pristine `b522373` binary as whole
  documents on all four gates: 3,271 and 3,252 fields compared, **6 differing —
  the executable hash and the five wall-clock quartile fields**.
* **Both flags are bit-identical**, which is stronger than the inner
  certificate managed: `default` vs `+both` and `fcp` vs `fcp+both` differ in
  those same 6 fields and nothing else, on **all four** gates. No work
  diagnostic moves at all, because a catalogue descent is not a counted work
  unit.
* All four pinned values reproduce on every arm: 206.869 / `8a7737381238fa4d`,
  and 159.09233022733062 / `fa01012af1d559ae`, 159.07876040364795 /
  `e28fba007f8031d4`, 164.0375677990678 / `49f094d7e59a9008`, every arm
  `exactValid` and `contractValid`.
* **The release suite is green at 1,238 tests** on `jagua-experimental`, on
  `jagua-experimental,relaxed-scan-shape-reuse,relaxed-cached-pose-bounds`, and
  on `jagua-experimental,search-profiling,relaxed-lane-census`.
* One behaviour difference is stated rather than glossed: on the *error* path
  where a fixed piece has no canonical orientation, the flag-on arm leaves the
  broad-phase scratch buffer populated where the default arm left it empty. That
  path returns `Err` and ends the lane, so nothing reads the buffer again; the
  default arm preserves the original behaviour exactly.

## What is left, sized

* **Allocation is the next lever and it is bigger than this one.** On m22 g2 —
  the relaxed-lane-dominated stream, 2,920 collision-polygon builds — the stream
  makes **50,455,080 allocations for 8.41 GB of gross demand**, which is **5.30
  allocations per candidate scan**. The scorer builds a fresh
  `Vec<(usize, usize, f64)>` per call for 1.80 rows on average, grows it one
  power of two at a time, and then that vector is cloned again by
  `search_piece`'s `current_score.clone()` and `best_score.clone()`. (The m20
  figure, 72,224,132 allocations at 20.06 per scan, is not comparable: the
  constructor is still live there and Clipper dominates it.) A pooled row buffer
  that the tracker swaps rather than copies is the obvious shape — the lane
  already does exactly this for `collision_merge_scratch` — but it is a
  structural change to what `MovedRowDelta` owns, and it wants its own stage.
* **The remaining 60-65% of catalogue descents are the fixed-side ones**:
  15.4M / 40.8M / 69.4M, one per visited neighbour. Removing them needs a stable
  handle into the catalogue that survives across calls, which the current
  `BTreeMap<SurrogateKey, OrientedSurrogate>` cannot give — the honest design is
  a slab (`BTreeMap<SurrogateKey, u32>` plus `Vec<OrientedSurrogate>`) with a
  per-piece slot memo. **Do not size it by scaling this stage's result**: what
  shipped removed a `rem_euclid` *and* a descent per call, whereas the
  fixed-side loop already reads its rotation key from the memo, so its per-unit
  prize is strictly smaller than 4.3x what shipped.
* **The `weights` lookup is a second `BTreeMap` descent** on every colliding
  row — 6.36M / 17.1M / 28.6M of them — keyed on `(usize, usize)`. Same shape of
  fix, same caveat.
* **The 42% of returned neighbours that are never visited are a class (B)
  lever.** They are dropped by the upper-bound cutoff, so the prize is in
  *ordering* the scan cheapest-first rather than by piece index — and the scan's
  iteration order determines which rows land before the cutoff fires, hence
  `pruned`, hence `MovedRows`, hence what the tracker installs. This is exactly
  the situation the constructor census met with its finalist loop, and the same
  verdict applies: not semantics-preserving, and no tie-break refinement makes
  it so. It needs matched-arm quality evidence, designed below.

### The experiment the class (B) lever needs — as designed

*Run in stage 2 below, and the verdict is **reject**.*

Endpoint: descendant depth under a fixed **work** budget, never wall clock, and
never the immediate depth. Arms: `scan-order-index` (today) against
`scan-order-proxy` (neighbours sorted by a cheap separation proxy before the
scan). Matched on: the same pinned parent, the same relaxed seed, the same
`work=` budget through the coordinator so both arms are handed identical
schedules. Replication: four target salts x two relaxed seeds, as the
constructor stage's quality gate did, because `construction_seed` derives from
the target and so targets are samples while seeds are replicas. Falsifier: the
proxy order must not change `exactValid`/`contractValid` on any published
candidate; the paired delta per salt is the statistic, and a win has to survive
all eight cells rather than being read off the mean.

---

# Stage 2 — the class (B) lever run, and the allocation lever sized

Both levers designed above were built behind their own default-off flags and
measured at parent `57ad992`. **One is adopted and one is rejected, and the
rejected one is the one that removes more work.**

| lever | class | flag | verdict |
|---|---|---|---|
| scan ordering, cheapest-first | (B) | `relaxed-scan-order-proxy` | **reject** — 15% fewer neighbour visits, one 8.968 mm quality regression in 16 cells, no compensating win, and no measured speed win |
| candidate row-buffer reuse | (A) | `relaxed-row-buffer-reuse` | **keep, as a default-off flag** — bit-identical on all four gates, 15.0% of a mode-22 stream's allocations removed, and **wall-clock parity**: it buys 1.0-1.5% by arithmetic and this box cannot resolve that |

Neither lever is proposed for the default build. The class (A) one is
bit-identical and therefore free to turn on whenever a stream is allocator-bound
enough for it to show; the class (B) one should stay off, and the ledger below
records why so the next person does not re-run it.

## The class (B) scan-ordering experiment

`relaxed-scan-order-proxy` sorts the broad phase's neighbours by the squared
distance between the two placements' translation origins before the scan runs,
instead of taking them in the ascending piece-index order
`PieceQueryScratch::query_into`'s `sort_unstable` leaves them in. Near
neighbours are asked first, so the caller's upper bound is crossed on fewer of
them. The keys are built once per neighbour into a lane scratch and the order is
a strict total order — `total_cmp` on the key, then the piece index — so the
unstable sort is still deterministic.

The proxy is deliberately the cheapest separation statistic the loop can reach.
Anything better — a bounds gap, an extent overlap — needs the *fixed* operand's
oriented shape, and resolving that for all 7.3 returned neighbours in order to
skip 3.1 of them is the cost the lever exists to avoid. The origin is not the
centroid, so this orders on the pose and not on the geometry.

### It removes exactly the work it was predicted to remove

Whole-document diffs of the four gates, default arm against the flag, work
counters only:

| counter | m20 g1 default | g1 proxy | ratio | m22 g2-g4 default | g2-g4 proxy | ratio |
|---|---:|---:|---:|---:|---:|---:|
| `pieceBroadPhaseProbes` | 15,562,760 | 13,255,792 | **0.8518** | 19,264,010 | 16,313,072 | **0.8468** |
| `satTests` | 21,238,746 | 20,633,791 | 0.9715 | 29,834,907 | 28,743,810 | 0.9634 |
| `cellIndexProbes` | 19,883,361 | 19,734,179 | 0.9925 | 28,178,865 | 27,855,653 | 0.9885 |
| `surrogateEvaluations` | unchanged | | 1.0000 | 4,503,802 | 4,481,973 | 0.9952 |
| `acceptedMoves` | unchanged | | 1.0000 | 18,794 | 18,730 | 0.9966 |

**One neighbour visit in seven disappears.** That is the largest single work
reduction this directory has measured, and it is why the lever was worth
building rather than arguing about.

The trajectory changes, exactly as predicted: on m20 g1 nine document fields
move (the executable hash, the five wall-clock quartiles and three probe
counters), and on each mode-22 gate sixteen do, including `acceptedMoves` and
the epoch's evaluation counts.

### The four pinned gates survive it, which is a fact about the gates

Every one of the four pinned values reproduces **under the flag**: 206.869 /
`8a7737381238fa4d`, 159.09233022733062 / `fa01012af1d559ae`,
159.07876040364795 / `e28fba007f8031d4`, 164.0375677990678 /
`49f094d7e59a9008`, every arm `exactValid` and `contractValid`. The gate-1
depth *list* is identical element for element.

This is worth stating plainly because it is easy to misread as evidence for the
lever. It is not. It is evidence that the four pinned replays are strong
attractors: the counters prove the search took a different path and arrived at
the same place. A regression gate that cannot be moved by a lever that deletes
15% of the neighbour visits is not measuring the lever.

### The quality gate, and the one cell that moved

Sixteen cells: four mode-20 target salts x four relaxed seeds, the designed
eight (seeds 0 and 1) plus a widening to seeds 2 and 3 taken *after* the
designed eight came back with a mover. Both arms descend the **same** pinned
parent — the parents are produced once by the default binary, so the only thing
that differs across arms is the scan's visit order — on the identical pinned
mode-22 schedule, statistic `rawSourceDepthMm`, lower better.

| salt | parent | seed | index order | proxy order | delta |
|---|---:|---|---:|---:|---:|
| 320.000 | 206.869 | 0 | 173.04696 | 173.04696 | 0 |
| 320.000 | 206.869 | **1** | **170.648** | **179.616** | **+8.968** |
| 320.000 | 206.869 | 2 | 179.003 | 179.003 | 0 |
| 320.000 | 206.869 | 3 | 179.003 | 179.003 | 0 |
| 321.500 | 206.666 | 0-3 | 171.463 / 171.256 / 181.59868 / 180.207 | identical | 0 |
| 323.000 | 199.801 | 0-3 | 179.003 / 175.08864 / 179.62678 / 179.633 | identical | 0 |
| 324.500 | 214.042 | 0-3 | 179.003 / 179.654 / 179.52913 / 179.609 | identical | 0 |

**Fifteen cells are bit-identical — same raw depth, same placement fingerprint —
and the sixteenth loses 8.968 mm.** Zero cells improve. Every cell in both arms
is `exactValid` and `contractValid`, so the falsifier is not what sinks the
lever; the ledger does. The designed eight-cell gate on its own reads the same
way: seven ties, one 8.968 mm loss, no wins.

The honest reading of "fifteen ties" is not "the lever is harmless". It is that
the mode-22 descent from these parents is itself a strong attractor, so this
endpoint has very little resolving power, and the *only* resolution it produced
was negative. A lever with no upside and a demonstrated 8.968 mm downside on the
one cell where the trajectory actually diverged does not clear "quality neutral
or better".

### The coordinator at an identical work budget

The design's other matched arm: `work=20000000,cells=13:15:17:19` through the
coordinator, five relaxed seeds, `PortfolioBudget::Work` so both arms are handed
the same scheduled search.

| seed | index order | proxy order | delta | published fingerprints |
|---|---:|---:|---:|---|
| 0 | 179.5868589805926 | 179.5868589805926 | 0 | identical (`02f5afe48f`) |
| 1 | 176.75281805880576 | 176.75281805880576 | 0 | identical (`000f27d387`, `7d7c09b204`) |
| 2 | 179.006 | 179.006 | 0 | identical (`c429b1a245`) |
| 3 | 179.08499999999998 | 179.08499999999998 | 0 | identical (`92409c9d0f`, `d5dd900685`) |
| 5 | 180.64489329491147 | 180.64489329491147 | 0 | identical (`b5672b945f`) |

All five incumbents are `dualGateValid` in both arms. The publication *work
ordinals* do move — 10,793,256 against 10,793,345 on seed 0, 11,968,680 against
11,967,536 on seed 3 — so the arms did diverge and then reconverged. Quality at
identical work: **exactly neutral, five out of five.**

That leaves the whole case for the lever resting on speed, and there is no speed
to show for it.

## What the levers cost in wall clock

Paired interleaved A/Bs, arms alternating order every round, statistic the
per-round paired ratio. **The box was shared with another agent's compile and
test load throughout this stage** — one-minute load average moved between 1.7
and 15 — so every timing row here carries more spread than the stage-1 table it
sits beside, and the per-round rows are in `evidence-stage2.json` so a reader
can see the bimodality rather than take a median on trust.

Two campaigns were taken, and **both are reported**, because they disagree and
the disagreement is the finding. The first ran while another agent's
`cargo build` and `cargo test` were on the box; the second ran in a quieter
window. Nothing was discarded.

| campaign | sample | rounds | arm A | arm B | A median | B median | paired median | range | below 1.0 |
|---|---|---:|---|---|---:|---:|---:|---|---:|
| quiet | m20 g1 | 14 | `fcp` | `fcp+row` | 4.1005 s | 4.0997 s | **1.0014** | 0.9735-1.0107 | 5/14 |
| quiet | m22 g2 | 16 | `fcp` | `fcp+row` | 3.1764 s | 3.1723 s | **1.0004** | 0.9793-1.0309 | 8/16 |
| quiet | coordinator `work=20000000` | 16 | default | `+row` | 5.1180 s | 5.1095 s | **0.9976** | 0.9274-1.0759 | 10/16 |
| quiet | coordinator `work=20000000` | 14 | default | `+ord` | 5.2845 s | 5.3097 s | **0.9867** | 0.9512-1.0677 | 8/14 |
| loaded | m20 g1 | 12 | `fcp` | `fcp+row` | 4.0996 s | 4.1345 s | 1.0081 | 0.9961-1.0563 | 4/12 |
| loaded | m22 g2 | 12 | `fcp` | `fcp+row` | 3.6183 s | 4.0872 s | 1.0867 | 0.9122-**1.9107** | 3/12 |
| loaded | coordinator `work=20000000` | 10 | default | `+row` | 6.9969 s | 5.7337 s | 0.8289 | **0.6065**-1.0070 | 8/10 |
| loaded | coordinator `work=20000000` | 10 | default | `+ord` | 5.1731 s | 5.2291 s | 1.0100 | 0.7867-1.0257 | 1/10 |

* **The row-buffer lever is at parity on the wall clock on all three streams**,
  and that is the result, not a failure to measure. 1.0014, 1.0004, 0.9976,
  with 5/14, 8/16 and 10/16 rounds below parity — three coin flips. The arithmetic
  agrees: 7.57M allocation/free pairs at a tcache-resident 20-30 ns each is
  0.15-0.23 s of *thread-summed* time, and the m22 g2 stream runs about 4.8
  lane-seconds per wall second, so the whole prize is 30-50 ms on a 3.17 s
  stream — **1.0-1.5%, which is inside this box's per-round spread.** The lever
  removes real allocator traffic; it does not, on this hardware, buy time that
  can be told from noise.
* **The scan-order lever has no reproducible speed win either**, and the two
  campaigns disagree in sign: 0.9867 in the quiet one, 1.0100 in the loaded one,
  both with ranges straddling parity. One neighbour visit in seven disappears
  and the wall clock does not move, which says the per-neighbour cost it saves
  is close to the per-scan cost of the keying pass and the sort that save it.
* **The loaded campaign is left in as a warning.** A single round there produced
  a paired ratio of 1.911 and another of 0.607 — a 3.1x span on two builds that
  differ by one `Vec::new()`. Any conclusion read off a median without the range
  and the round count is not a measurement on this box.
* **The searches themselves are identical where they should be.** On the row
  arm the coordinator publishes the same incumbent in all 32 runs of the quiet
  campaign — depth 180.64489329491147, fingerprint `b5672b945f32…`, one
  publication at 9,064,287 work units — with only the publication's `seconds`
  timestamp moving, which is why `outcomesIdenticalWithinArm` reads false in the
  raw driver output and means nothing.

## The allocation lever, class (A)

`relaxed-row-buffer-reuse` recycles the candidate scorer's collision-row buffer.
The scorer wrote `Vec::new()` per call and pushed about 1.80 rows into it; the
refinement loop then retired **two** such buffers per iteration — the loser of
the paired probe, and the incumbent it displaced — straight back to the
allocator. The flag hands both to a four-slot per-lane pool that the next two
`score_placement` calls take from.

It is bit-identical by construction: the scan clears the buffer and pushes the
same values in the same order, the terminal `sort_by_key` runs on the same
slice, and nothing in the lane reads a `Vec`'s capacity. The default build calls
the same two helpers, which compile to `Vec::new()` and a `drop` — exactly what
the code did before — so the field and the `pop` do not exist in it at all.

### Allocations removed, as measured

`profiling-allocator` builds, `POLYGON_NESTING_PROFILE=1`, gross demand (no
`dealloc` subtraction):

| stream | arm | allocations | bytes | per candidate scan |
|---|---|---:|---:|---:|
| m22 g2 | default | 50,455,078 | 8,406,034,810 | 4.630 |
| m22 g2 | `+row-buffer-reuse` | 42,881,288 | 7,671,679,301 | 3.934 |
| m22 g2 | **removed** | **7,573,790 (15.01%)** | **734,355,509 (8.74%)** | **0.695** |
| m20 g1 | default | 115,375,028 | 24,341,428,590 | 28.211 |
| m20 g1 | `+row-buffer-reuse` | 112,544,207 | 24,065,306,857 | 27.519 |
| m20 g1 | **removed** | **2,830,821 (2.45%)** | **276,121,733 (1.13%)** | **0.692** |

The two streams remove **0.695 and 0.692 allocations per candidate scan** — a
0.4% agreement across a 2.7x range of scan counts, two different operators and
two completely different allocator mixes. That is the strongest evidence here
that what was removed is exactly the refinement loop's two retired buffers and
nothing else: the loop runs one iteration per two scans, so two buffers per
iteration is 1.0 per scan *of the scans the loop makes*, and the sample and
probe loops make the rest.

The m22 baseline reproduces the census figure this directory published from a
different build: 50,455,078 against 50,455,080, a difference of two allocations
in fifty million. The m20 g1 figure here is **115.4M, not the 72.2M** quoted
above, because that one was taken on the `fast-constructor-*` stream and this
one on the plain `jagua-experimental` build; the constructor's Clipper traffic
is a different quantity in the two, which is exactly why the per-scan rate and
not the total is the comparable statistic.

### One correction to what this directory said before

The sizing note above credited part of the per-scan allocation to `search_piece`
cloning the row vector twice. **Both of those clone sites are unreachable on
every stream measured here.** `current_score.clone()` is inside
`if self.uses_directional_pressure()`, which the default backend never
satisfies, and `best_score.clone()` is inside `if ENABLE_NFP_AXIS_MINIMIZER`,
which is a `const false` in this file. The per-scan allocation the lever
actually removes is the scorer's own buffer, retired by the refinement loop.

### Reuse sites left alone, and why

* **`report_diverse_sample`'s discarded samples.** It drops a delta on its early
  return and can drop a second on `truncate`, and it is a free function that
  owns the sample list, so recycling means giving it a way to hand one or two
  buffers back. That is bit-identical and buildable — the reason it is not here
  is share: the sample loops are ten scans per `search_piece` against the
  refinement budget's `refinement_rounds * 10 * starts.len()`, and the measured
  0.69 per-scan removal is already the whole of what the refinement loop retires.
* **`minimize_candidate_axes`'s scorer call.** Dead on every measured stream for
  the same two reasons as the clones above.
* **The tracker's own row list.** Already reuses `collision_merge_scratch`,
  swapped rather than copied, since before this directory existed.
* **The remaining 97.5% of m20 g1's allocations.** Not the lane. That stream
  builds collision polygons through Clipper, and the census already said the
  constructor dominates its allocator traffic; a lane lever cannot reach it.

**No site was found that could not be made bit-identical.** The pooled-buffer
shape the sizing note worried about — "a structural change to what
`MovedRowDelta` owns" — turned out not to be needed: the delta still owns a
plain `Vec`, and only the two points where the refinement loop *drops* one had
to change.

## Stage 2 equivalence evidence

* **The default build is unchanged.** The edited tree's default binary
  reproduces the pristine `57ad992` binary as whole documents on all four
  gates: 3,271 and 3,252 fields compared, **6 differing — the executable hash
  and the five wall-clock quartile fields**.
* **`relaxed-row-buffer-reuse` is bit-identical**, on all four gates, in those
  same 6 fields and nothing else. No work diagnostic moves.
* **`relaxed-scan-order-proxy` is not**, and says so: 9 fields on g1 and 16 on
  each mode-22 gate, all four pinned values nevertheless reproducing.
* **The stacked build** (`+row-buffer-reuse,+scan-order-proxy`) also reproduces
  all four pinned values, `exactValid` and `contractValid`. The stacked *timing*
  measurement the task reserved for an adopt verdict was not run, because the
  scan-order verdict is reject.
* **The release suite is green at 1,238 tests** on `jagua-experimental`, on
  `jagua-experimental,relaxed-row-buffer-reuse`, and on
  `jagua-experimental,relaxed-scan-order-proxy`.

## Reproducing

```
python3 drivers/gates.py <label> <binary>                       # four pinned gates
python3 drivers/diffall.py a <binA> b <binB>                    # whole-document diffs
python3 drivers/decompose.py g1 census <binCensus>              # leaf-phase table
python3 drivers/coordinator.py 10000 census <binCensus>         # bare-request coordinator
python3 drivers/ab.py 12 a <binA> b <binB> g1                   # paired interleaved A/B
python3 drivers/coordab.py 10 20000000 a <binA> b <binB>        # work-budgeted A/B
python3 drivers/summarize.py
```

Stage 2 adds four:

```
python3 drivers/scanorder_quality.py <parentBin> idx <binA> proxy <binB>
RELAXB_SEEDS=0,1,2,3 python3 drivers/scanorder_quality.py ...   # the widening
python3 drivers/coordquality.py 20000000 0,1,2,3,5 idx <binA> proxy <binB>
python3 drivers/allocs.py g2 base <allocBin> row <allocRowBin>
python3 drivers/collect_stage2.py docs/experiments/relaxed-lane-residual/evidence-stage2.json
```

Stage-2 binaries:

```
cargo build --release --example general_request_benchmark --features \
    jagua-experimental,relaxed-row-buffer-reuse
cargo build --release --example general_request_benchmark --features \
    jagua-experimental,relaxed-scan-order-proxy
cargo build --release --example general_request_benchmark --features \
    jagua-experimental,profiling-allocator                                # alloc counts
cargo build --release --example general_request_benchmark --features \
    jagua-experimental,profiling-allocator,relaxed-row-buffer-reuse
```

`allocs.py` needs `POLYGON_NESTING_PROFILE=1`, which it sets itself; the
allocator build is slower than the arm it describes, so its numbers are counts
and never a wall-clock claim.

`drivers/lib.py` carries the pinned positional CLI tail and the four gates;
point `ROOT` at your worktree. Binaries:

```
cargo build --release --example general_request_benchmark --features \
    jagua-experimental                                                   # gates
cargo build --release --example general_request_benchmark --features \
    jagua-experimental,relaxed-scan-shape-reuse,relaxed-cached-pose-bounds
cargo build --release --example general_request_benchmark --features \
    jagua-experimental,fast-constructor-profile,fast-constructor-confirm,\
fast-constructor-reject                                                  # A/B arm A
cargo build --release --example general_request_benchmark --features \
    jagua-experimental,fast-constructor-profile,fast-constructor-confirm,\
fast-constructor-reject,search-profiling,relaxed-lane-census             # census
```

Note that `fast-constructor-confirm` and `fast-constructor-reject` are stacked
on `fast-constructor-profile`: without it the mode-20 gate-1 stream is 24.2 s,
not 4.06 s, even though the certificate is active and the build count falls to
409,450.

## Artifacts

| path | what |
|---|---|
| `evidence.json` | every stage-1 number above, as measured |
| `evidence-stage2.json` | every stage-2 number: gates, whole-document diffs, the 16 quality cells, the coordinator work-budget cells, the allocation counts, and both timing campaigns with their per-round rows |
| `drivers/` | the runners |
