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

### The experiment the class (B) lever needs

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
| `evidence.json` | every number above, as measured |
| `drivers/` | the runners |
