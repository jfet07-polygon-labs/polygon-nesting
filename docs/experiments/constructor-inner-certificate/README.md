# The constructor's inner overlap certificate

The exact-confirmation census left two things on the table and said they were
the same thing. The candidate stream offers **432,710** confirmation rows on the
mode-20 gate-1 stream and accepts **2.6%** of them; **78.66%** of all
collision-polygon builds are spent on a pose the row discards two lines later;
and the separation shield could not touch any of it, because 77.3% of the
constructor's exact queries answer "they overlap" and no *outer* approximation
removes one of those.

This directory is the *inner* certificate that does, the counting build that
sized it before it existed, and the measurement that priced ordering the stream
against pruning it.

**Headline: the mode-20 gate-1 stream goes 5.85 s to 4.06 s, a paired median of
0.694 against the previous stage, and 26.28 s to 4.06 s — 6.47x — measured
against the default build in one paired interleaved A/B rather than chained.**

## The certificate

`fast-constructor-reject`, stacked on `fast-constructor-confirm`, off by default.
`search::construction_reject_certificate`.

Cover each polygon from the inside with discs; two discs that overlap prove the
two collision polygons intersect in positive area, which is the verdict the
exact tier would have returned, so the row returns `None` without the Clipper
offset and without a single pair query.

* **The fixed side is a computation, not an assumption.** The parent's collision
  polygons already exist — the row is being confirmed *against* them — so the
  discs are inscribed directly in them, by measuring the distance from a sample
  point to every ring.
* **The candidate side is the geometric content**, because its collision polygon
  is exactly what the certificate exists in order not to build. Its discs are
  inscribed in the *source* polygon, then transformed rigidly to the pose and
  inflated by the collision expansion. That inflation is sound iff
  `offset_miter(P, e) ⊇ P ⊕ disc(e)`, which the census entry recorded as
  "believable and not proved here". **It is proved now** — see the ledger
  chapter; the short version is that Clipper's *square* join is a cut tangent to
  the arc at distance exactly `e`, so both branches of the miter-limit test
  contain the round join and the containment holds at every miter limit.

Four discretisations sit between the exact statement and the code — the grid
snap of the transformed ring, the rounding of the offset distance, `math_round`
on each emitted offset vertex, and `f64` — and they sum to 0.001916 mm. The
certified radius is eroded by 0.005 mm for them, and the pair test insists on a
further 0.02 mm of penetration so the lens it proves cannot round away on the
integer grid.

## What the counting build measured

`constructor-census` now prices the certificate beside the exact answer on every
confirmation row, at cover sizes one, two, four and eight, and counts the one
observation that would falsify it: a certificate issued for a row the exact tier
then **accepted**.

Mode-20 gate-1 stream, which reproduces `independentDepthMm` 206.869 /
`8a7737381238fa4d` under the census.

| site | rows | accepted | rejected: containment / overlap | certified @1 | @2 | @4 | @8 |
|---|---:|---:|---:|---:|---:|---:|---:|
| `candidate` | 432,710 | 11,292 (2.6%) | 23,368 / 398,050 | 288,693 | 320,216 | 330,260 | 330,746 |
| `slideLadder` | 191,925 | 130,482 (68.0%) | 14,734 / 46,709 | 2,420 | 5,199 | 5,878 | 5,976 |
| `slideBisect` | 122,886 | 15,453 (12.6%) | 28,817 / 78,616 | 1,895 | 4,133 | 4,846 | 4,889 |
| deep total | 747,521 | 157,227 | 66,919 / 523,375 | 293,008 | 329,548 | 340,984 | 341,611 |

`soundnessViolationsCertificate` — a certificate issued for a row the exact tier
then **accepted** — is **0** at every cover size, on every site, over all 157,227
accepted rows.

* **The candidate stream is where it lands**: 330,260 of its 432,710 rows are
  proved at four discs — **76.32% of the rows, 82.97% of its overlap
  rejections**. The two slide sites are 3.1% and 3.9%, which is the census's own
  finding restated: a ladder rung starts from an already-valid pose, so its
  failures are shallow contact-band overlaps rather than a candidate dropped on
  top of a placed piece.
* **Four discs is the knee.** Eight adds 486 rows to the candidate stream —
  0.15% — for four times the pair arithmetic. The armed query uses four; the
  cover is still built at eight so the census can keep pricing past it.
* **The prize barely rests on the containment lemma.** With the whole expansion
  inflation taken back off — the fallback that needs nothing but
  `offset(P, e) ⊇ P` — the candidate stream still proves **312,692** rows,
  **94.5%** of what the inflated certificate proves. The slide sites collapse to
  241 and 18, which is the same fact from the other side: their overlaps exist
  only inside the expansion band.

### Ordering, measured, and why the prune is the deliverable

The task the census set was "prune speculative rows before exact work", and the
first question is whether *ordering* the stream would do better than pruning it.
The census answers it directly. Per candidate slot it records every confirmed
row as `(signed certificate pressure, accepted)` — pressure positive is a proof
and its depth, negative is the closest approach the certificate could not close,
so ascending order is "cleanest first" — and compares two prefix lengths:

| statistic | value |
|---|---:|
| candidate slots | 2,872 |
| candidate rows confirmed | 432,710 |
| acceptances | 11,292 |
| rows the **current** order confirms to reach them | 432,710 |
| rows a lazy **proxy-ordered** confirmation would confirm | **59,414** |
| ordering's reduction factor | **7.28x** |

`prefixActual` is exactly `rows`, and that is a fact about the loop rather than a
coincidence: it breaks at the top of the iteration after the fourth finalist, so
the last candidate row it confirms is the accepting one.

So a perfect lazy proxy order would avoid **86.27%** of the candidate stream's
exact confirmations, and the sound prune avoids **76.32%** — **88.5% of the
ordering prize, with the accept/reject semantics untouched.**

That last clause is why the prune is what shipped. **A reordering of this loop
is not semantics-preserving and cannot be made so by a tie-break refinement.**
The loop is capped three ways — `CONSTRUCTION_FINALISTS_PER_SLOT` = 4,
`CONSTRUCTION_ROWS_PER_PIECE` = 320, and a per-provenance row cap — and each
accepted candidate then spends further rows on a contact walk, so *which* four
poses become finalists, and how many rows are left when they do, are both
functions of the order the rows were offered in. There is no configuration in
which the acceptance rule is order-invariant while any cap is live. The
certificate sidesteps the whole question: it removes rows whose verdict was
never in doubt, in place, leaving the order, the row charges and the finalist
set identical.

## What it is worth

Paired interleaved A/Bs of the mode-20 gate-1 stream, arms alternating order
every round, statistic the per-round paired ratio:

| sample | rounds | arm A | arm B | A median | B median | paired median | spread | rounds below 1.0 |
|---|---:|---|---|---:|---:|---:|---|---:|
| 1 | 12 | `confirm` | `+reject` | 5.849 s | 4.054 s | **0.6924** | 0.6875-0.6981 | 12/12 |
| 2 | 10 | `confirm` | `+reject` | 5.851 s | 4.066 s | **0.6956** | 0.6881-0.6984 | 10/10 |
| 3 | 8 | `confirm` | `+reject` | 5.853 s | 4.052 s | **0.6923** | 0.6875-0.6975 | 8/8 |
| chain | 8 | **default** | `+reject` | 26.278 s | 4.059 s | **0.1545** | 0.1538-0.1565 | 8/8 |
| mode 22 | 10 | `confirm` | `+reject` | 3.135 s | 3.119 s | **0.9966** | 0.9822-1.0137 | 6/10 |

Three independent samples of the mode-20 A/B agree to 0.5%; sample 3 was taken
on binaries rebuilt from the committed tree, so the headline is reproducible from
the commit rather than from the tree it was measured in. The **chain** row is
the honest multiplier taken as one measurement rather than assembled from three:
**6.47x** against the default build, where the previous stage's chained figure
was 4.48x. Mode 20 goes 26.2 s -> 6.2 s -> 5.85 s -> **4.06 s**.

The mode-22 row is a real zero and is reported because it could have been a
regression: the record-replay stream barely exercises the constructor, so the
certificate's own arithmetic is charged there with almost nothing to remove, and
it comes back at parity with the sign changing across the sample.

Profiled decomposition, one run each (`search-profiling` costs about 4.5%, so
this is a decomposition and not a wall-clock claim):

| phase | flag off | flag on | calls off | calls on |
|---|---:|---:|---:|---:|
| `exactOverlapTest` | 2,948.8 ms | 1,791.5 ms | 1,266,102 | 925,309 |
| `collisionPolygonBuild` | 2,024.3 ms | 1,062.9 ms | 750,434 | **409,450** |
| `vacancyExactRows` | 3,562.4 ms | 1,514.4 ms | 747,521 | **747,521** |
| `vacancyProposals` | 4,091.6 ms | 2,306.0 ms | 2,872 | 2,872 |
| `pairCollide` | 1,253.7 ms | 1,264.7 ms | 15,562,760 | 15,562,760 |
| `pairPressure` | 935.5 ms | 940.4 ms | 6,363,087 | 6,363,087 |
| `moveSweep` | 5,229.7 ms | 5,222.5 ms | 4,089 | 4,089 |
| `scorePlacement` | 4,758.5 ms | 4,752.7 ms | 4,089,768 | 4,089,768 |
| leaf total | 9,712.8 ms | 7,565.1 ms | | |

Two numbers in that table are the whole story. `collisionPolygonBuild` falls by
**340,984 calls — exactly the census's `rowsCertified4` total, to the digit** —
and `vacancyExactRows` keeps all 747,521 of its calls, because the row is still
charged, still counted against the finalist-row budget, and still asked. Only
the work behind the answer is gone.

## Equivalence evidence

* **Flag off**, all four regression gates reproduce the pristine `8d9f7e5`
  binary as **whole documents**: 3,271 and 3,252 compared leaf fields per gate,
  six differing, all of them the executable hash and the five wall-clock fields.
  Pinned values reproduce: 206.869 / `8a7737381238fa4d`, and the three mode-22
  records at 159.09233022733062, 159.07876040364795 and 164.0375677990678 at
  `fa01012af1d559ae`, `e28fba007f8031d4`, `49f094d7e59a9008`, every arm
  `exactValid` and `contractValid`.
* **Flag on**, gates 2, 3 and 4 are identical to the `fast-constructor-confirm`
  arm in **every** field. Gate 1 differs in five, and all five are the removed
  work honestly not charged:

  | field | confirm | +reject |
  |---|---:|---:|
  | `experimentalCollisionBuilds` | 786,724 | 445,740 |
  | `transformedCollisionVertices` | 5,221,858 | 3,313,762 |
  | `experimentalPairVisits` | 13,120,423 | 8,189,813 |
  | `clipperInputVertices` | 37,012,470 | 32,823,802 |
  | `clipperOutputVertices` | 2,572,148 | 654,362 |

* **Determinism**: two runs of the flag-on binary are identical field for field
  on all four gates apart from the five wall-clock fields.
* **A debug build with the `debug_assert` live** — every certified row builds
  its collision polygon anyway and is required to find a positive exact
  intersection area against some active piece — reproduces all four gates, and
  the assertion never fired.
* **Two unit tests** issue certificates over dense placement grids of a rotated
  and of a mirrored non-convex piece against an L, and require that no
  certificate is ever issued for a pair whose exact collision polygons are
  disjoint.
* **The counting build's own falsification counter**, `soundnessViolationsCertificate`,
  is **0** over 157,227 accepted rows.
* **The release suite** is green at 1,234 tests on `jagua-experimental` and 1,236
  with the reject flag on, which is the same suite plus the two tests above. One
  caveat is inherited rather than introduced: `cargo test --features
  fast-constructor-profile` does not compile at the base commit `8d9f7e5` either,
  because a `construction_void_grid` test there calls `derived_cell_mm` with
  three arguments of four.

## The quality gate

Descendant depth under a fixed downstream work budget, four salts, two relaxed
seeds each. The salt is the target depth, per the caveat two rounds ago:
`construction_seed` derives from the anchor, the seed domain and the target, so
seeds are replicas and targets are samples. Every endpoint is pinned and given
the identical short mode-22 descent by the **default** binary.

| salt | endpoint, both arms | descended s0 | descended s1 | paired delta |
|---|---:|---:|---:|---:|
| 320.000 | 206.869 | 173.04696209930026 | 170.648 | 0.0 |
| 321.500 | 206.666 | 171.463 | 171.25599999999997 | 0.0 |
| 323.000 | 199.801 | 179.003 | 175.08863905832428 | 0.0 |
| 324.500 | 214.042 | 179.003 | 179.654 | 0.0 |

All eight paired deltas are exactly **0.0**, at identical endpoint *and*
descendant fingerprints. As with the separation shield, a zero here is the
predicted result rather than a lucky one — a sound certificate cannot move a
search, because it removes rows whose verdict was never in doubt. The gate earns
its place by being the thing that would catch a wrong soundness argument.

## What is left, sized

* **The containment rejection is the next 66,919 rows.** The census counts
  66,919 deep rows rejected by `fits_rect` rather than by an overlap — 8.95% of
  rows, every one of them a wasted build, and 16.3% of the 409,450 builds this
  stage leaves behind. The same lemma covers it: the collision polygon contains
  the transformed source inflated by `e`, so a transformed source vertex more
  than `e` outside the sheet rectangle proves the row fails containment. It is
  not implemented here, deliberately, because it is a second mechanism with a
  second slack budget and this stage's value is that it ships one.
* **The candidate stream's last 17%.** 67,304 of its 398,050 overlap rejections
  are grazes the disc cover cannot close, and the ordering measurement says a
  perfect proxy would reach about 10 points more of the stream than the sound
  prune does. Closing that needs a better inner cover, not a different idea:
  discs are a poor cover for a long thin piece, and an inner *convex
  decomposition* would be the honest next tier.
* **The slide sites remain almost untouched** at 3.1% and 3.9% of rows, and the
  uninflated column says why: their overlaps live inside the expansion band, so
  they are exactly the population an inner certificate is worst at. They are
  also the population the separation shield is best at.
* **The residual is no longer the constructor.** At 4.06 s the mode-20 leaf is
  `moveSweep` 5,222.5 ms and `scorePlacement` 4,752.7 ms — the relaxed lane, not
  the constructor. Sol's ~2 s slice is now about 2.0x away and the next
  measurement belongs somewhere else.

## Reproducing

```
python3 drivers/gates.py <label> <binary>                    # four pinned gates
python3 drivers/census.py <census-binary> g1                 # the counting build
python3 drivers/diffall.py a <binA> b <binB>                 # whole-document diffs
python3 drivers/ab.py 12 profile <binA> reject <binB> g1     # paired interleaved A/B
python3 drivers/profile.py profile <binA> reject <binB>
python3 drivers/qualitygate.py profile <binA> reject <binB> <defaultBinary>
python3 drivers/summarize.py
```

`drivers/lib.py` carries the pinned positional CLI tail and the four gates;
point `ROOT` at your worktree. Binaries:

```
cargo build --release --example general_request_benchmark --features \
    jagua-experimental,fast-constructor-profile,fast-constructor-confirm            # arm A
cargo build --release --example general_request_benchmark --features \
    jagua-experimental,fast-constructor-profile,fast-constructor-confirm,fast-constructor-reject
cargo build --release --example general_request_benchmark --features \
    jagua-experimental,constructor-census,search-profiling                          # counting build
```

## Artifacts

| path | what |
|---|---|
| `evidence.json` | every number above, as measured |
| `drivers/` | the runners |
