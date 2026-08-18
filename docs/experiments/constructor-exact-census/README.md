# The constructor's exact confirmation, counted

After the bit-grid redesign took mode 20 from 26.2 s to 6.2 s, its leaf is
`exactOverlapTest` 33.1% plus `collisionPolygonBuild` 20.1% — the exact
confirmation *inside* construction, 1,266,102 overlap-test spans and 750,434
collision builds on the gate-1 stream. Sol's portfolio wants the constructor at
about 2 s.

This directory is the counting build that measured what is inside those two
numbers, and the prefilter it sized.

## The instrument

`constructor-census`, a cargo feature on `polygon-nesting-core`, off by default
and empty when off. Build:

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,constructor-census,search-profiling
```

It attributes every confirmation row and every exact pair question to its call
path, and on every pair that reaches Clipper it evaluates three **sound**
separation tests beside the exact answer:

| test | what it is |
|---|---|
| `aabb` | the axis-aligned box reject already in the code |
| `slabs` | the box plus the two diagonals — a four-direction DOP |
| `hull` | separating-axis over both convex hulls |

All three run on the integer Clipper path the exact query is executed on, in
exact arithmetic, so each answers "provably separated" or "no information". The
census reports `soundnessViolations*` — a pair the exact query calls
*overlapping* that a test called separated — and both are **0**.

A counting build runs a convex hull per observed pair. Its clock is meaningless;
only its counts are quotable.

## What it found

Mode-20 gate-1 stream (parent `ex5-seed-native.json`, target 320.000), which
reproduces `independentDepthMm` 206.869 / `8a7737381238fa4d` under the census.

### The exact pair question

| quantity | count | share |
|---|---:|---:|
| pair questions offered | 22,080,053 | |
| ... rejected by the existing box test | 20,789,701 | 94.16% |
| ... reaching Clipper | 1,290,352 | 5.84% |
| ... of those, **genuinely overlapping** | 997,826 | **77.33%** |
| ... of those, clean | 292,526 | 22.67% |
| clean and separated by `slabs` | 136,671 | 46.72% of clean |
| clean and separated by `hull` | 245,715 | **83.998% of clean** |
| soundness violations, either test | **0** | |

**The ceiling is 22.67%.** Three quarters of the constructor's exact queries
return "they overlap", and no conservative prefilter can ever remove one of
those: proving an overlap needs an inner certificate, not an outer one. The
hull tier reaches 19.04% of all Clipper queries, which is 84.0% of everything
that is reachable at all.

The census reconciles with the profiler exactly, which is worth stating because
the two count different things. `exactOverlapTest` opens **one span per
confirmation row** in the deep constructor and **one span per narrow-phase
query** in `general_fast`, so its 1,266,102 spans are 680,602 deep rows that
passed containment, plus 584,478 `general_fast` queries, plus 1,022 elsewhere —
1,266,102 to the digit. Those spans contain 1,290,352 Clipper pair queries and
13.1M box rejects.

The mean query is **13.69 combined vertices**, and the phase costs 3,309.6 ms
over those 1,290,352 queries — 2.6 µs of phase time each. That is not geometry
cost on a fourteen-vertex problem; it is Clipper's per-call setup.

### The collision-polygon build

| quantity | count | share |
|---|---:|---:|
| builds | 750,434 | |
| ... inside a deep confirmation row | 747,521 | 99.61% |
| ... in `general_fast`'s short-side-first constructor | 2,913 | 0.39% |
| **builds whose pose the row then rejected** | **590,299** | **78.66%** |
| rows rejected by sheet containment | 66,919 | 8.95% of rows |
| rows rejected by an exact overlap | 523,375 | 70.02% of rows |
| rows accepted | 157,227 | 21.03% of rows |

Four builds in five are spent on a pose that is discarded two lines later. This
stage does **not** address that; see "What is left".

### Where the queries come from

| site | rows | accepted | pairs offered | reaching Clipper | overlapping |
|---|---:|---:|---:|---:|---:|
| `candidate` (station / shelf / anchor-local / orientation) | 432,710 | 11,292 | 6,243,603 | 450,840 | 398,050 |
| `slideLadder` (contact drop ladder) | 191,925 | 130,482 | 4,918,267 | 140,790 | 46,709 |
| `slideBisect` (slide refinement) | 122,886 | 15,453 | 1,958,553 | 113,222 | 78,616 |
| `shortSideFirst` (`general_fast`, mode 0) | — | — | 8,927,135 | 584,478 | 474,449 |
| other | — | — | 32,495 | 1,022 | 2 |

The three constructor sites are not one population. The candidate stream is
**speculative** — 2.6% of its rows survive, and 88.3% of the pairs it takes to
Clipper are overlaps. The slide ladder is the opposite: 68.0% of its rows
survive, because every rung starts from an already-valid pose. The bisection
sits between them by construction, since it is a search between one valid and
one invalid offset.

That asymmetry is why "reject the candidate stream earlier" and "make the exact
query cheaper" are different projects with different prizes, and why only the
second one is sound today.

## The prefilter this sized

`fast-constructor-confirm`, stacked on `fast-constructor-profile`, off by
default. `search::construction_confirm_shield`. Slabs then hull, both on the
integer path, both proofs. The parent's certificates are derived once per beam
slot and the row's once per row, into reused buffers.

Paired interleaved A/Bs of the mode-20 gate-1 stream against
`fast-constructor-profile` alone, arms alternating order every round, statistic
the per-round paired ratio:

| sample | rounds | flag off | flag on | paired median | spread | rounds below 1.0 |
|---|---:|---:|---:|---:|---|---:|
| 1 | 14 | 6.231 s | 5.858 s | **0.9396** | 0.9245-1.0340 | 13/14 |
| 2 | 10 | 6.264 s | 5.848 s | **0.9367** | 0.6793-1.1404 | 9/10 |

Sample 2's spread is wide because the box was under a load average of 11 from
another agent's benchmarking for part of it; the two medians agree to 0.3%
anyway, which is what a paired interleaved design is for. A third sample taken
against an earlier build of the same change, before its per-row allocations were
removed, read 0.9494 over 10 rounds.

Profiled decomposition, one run each (`search-profiling` costs about 4.5%, so
this is a decomposition and not a wall-clock claim):

| phase | flag off | flag on | calls |
|---|---:|---:|---:|
| `exactOverlapTest` | 3,309.6 ms (32.64%) | 2,926.2 ms (29.94%) | 1,266,102 both |
| `collisionPolygonBuild` | 2,016.5 ms | 2,015.5 ms | 750,434 both |
| `pairCollide` | 1,285.7 ms | 1,285.4 ms | 15,562,760 both |
| leaf total | 10,141.0 ms | 9,774.8 ms | |

Every other phase is unchanged to the digit; the whole delta is inside the
phase the prefilter is in.

**The honest multiplier is 4.48x** against Sol's ~13x: 4.21x from the bit-grid
redesign times 1.064 from this. Mode 20 goes 26.2 s → 6.2 s → 5.86 s, and a
two-second slice still needs 2.9x that this stage does not have. The census
says where it is not: not in the exact pair question, three quarters of whose
calls are load-bearing.

## Equivalence evidence

* **Flag off**, all four regression gates reproduce the pristine `0cf1163`
  binary as **whole documents** — 3,271 and 3,252 compared leaf fields per gate,
  6 differing, all of them the executable hash and the five wall-clock fields.
* **Flag on**, the gate-1 document is identical to the
  `fast-constructor-profile` arm in every field but one: `clipperInputVertices`
  falls 39,043,027 → 37,012,470, which is the work the prefilter removed being
  honestly *not* charged. Gates 2, 3 and 4 are identical in every field.
* **Two flag-on runs** of gates 1 and 2 are identical field for field apart from
  the clock.
* **A debug build with the `debug_assert` live** — every skipped pair handed to
  Clipper anyway and required to return zero area — reproduces all four gates.
  The assertion never fired.
* A unit test issues certificates over a 6,561-placement grid of an L against a
  bar and requires that none is ever issued for an overlapping pair.

## The quality gate

Descendant depth under a fixed downstream work budget, four salts. The salt is
the **target depth**, not the relaxed-seed argument: `construction_seed` derives
from the anchor, the seed domain and the target, so seeds are replicas and
targets are samples (the caveat the previous round recorded). Each endpoint is
pinned and given the identical short mode-22 descent by the **default** binary
at two relaxed seeds, so only the parent differs.

| salt | endpoint, both arms | descended s0 | descended s1 | paired delta |
|---|---:|---:|---:|---:|
| 320.000 | 206.869 | 173.047 | 170.648 | 0.0 |
| 321.500 | 206.666 | 171.463 | 171.256 | 0.0 |
| 323.000 | 199.801 | 179.003 | 175.089 | 0.0 |
| 324.500 | 214.042 | 179.003 | 179.654 | 0.0 |

Four different endpoints, so the salts are samples. All eight paired deltas are
**exactly 0.0**, at identical endpoint *and* descendant fingerprints — which is
the stronger statement the prefilter's soundness predicts: it does not change
what the constructor decides, only how much work it does deciding it.

## What is left

* **The wasted build is the bigger prize and it is not sound yet.** 78.66% of
  collision-polygon builds are discarded, and `collisionPolygonBuild` is 20% of
  leaf. Removing one needs a certificate in the *opposite* direction — a proof
  that a pose **does** overlap — and the natural one is an inner circle cover of
  the unexpanded source, transformed rigidly per pose: two inscribed circles at
  centre distance below `r1 + r2 + 2 * expansion` prove the expanded polygons
  meet. That is cheap and general, and it rests on `offset_miter(P, e)`
  containing `P + disc(e)`, which is believable for Clipper's miter join but is
  not proved here. It should not ship on "believable".
* **The remaining `exactOverlapTest` cost is Clipper's per-call setup**, not
  geometry: 14 vertices per query and 2.6 µs to answer it. A pair query that
  reused one engine and one scratch path set across calls is the next
  measurement, and it is a Clipper-binding change rather than a search change.
* **`general_fast`'s short-side-first constructor is untouched deliberately.**
  It carries 584,478 of the 1,290,352 Clipper queries and 15.9% of them are
  hull-separable, but it is the protected legacy path and it runs on eight
  threads in about 0.65 s, so its share of the *wall* is small and its share of
  the risk is not.

## Reproducing

```
python3 drivers/gates.py <label> <binary>              # four pinned gates
python3 drivers/census.py <census-binary> g1           # the counting build
python3 drivers/diffall.py a <binA> b <binB>           # whole-document diffs
python3 drivers/ab.py 14 profile <binA> confirm <binB> # paired interleaved A/B
python3 drivers/profile.py profile <binA> confirm <binB>
python3 drivers/qualitygate.py profile <binA> confirm <binB> <defaultBinary>
python3 drivers/summarize.py
```

`drivers/lib.py` carries the pinned positional CLI tail and the four gates;
point `ROOT` at your worktree.

## Artifacts

| path | what |
|---|---|
| `evidence.json` | every number above, as measured |
| `drivers/` | the runners |
