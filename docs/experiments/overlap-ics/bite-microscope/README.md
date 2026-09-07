# The bite microscope: the hard bite is a pinned column, not a step floor

2026-09-06/07, commit "The bite microscope" (`--bitemicroscope=1`, cutclose only, diagnostic,
bit-identical off, trajectory-identical on: same `finalPoseDigest`, depth, publications and
per-bite iteration sequence with the flag on and off on a 40-bite fixed-work run). The trigger
is fixed prospectively: the first explore bite whose master-iteration count reaches 34; that
bite and the next three are retained, everything else discarded at its end, emitted after the
timed region. Two cells, seed 20, Legacy, eight workers, cap 50, one with `--proxymargin=8`
(GPT-6 Astra's prescription, review 4 Q3) and one without. Documents (gzipped) and the
`analyse-trace.py` readings are beside this file. `score.py` refuses documents carrying
`biteMicroscope` or `startedFrom`.

## The trigger bite, margin 8: bite 15, 180.195 -> 180.015, 43 iterations, published

End-of-sweep maximum violation, in micrometres, iteration by iteration:

```
88.8 72.1 52.3 43.5 30.8 26.5 25.9 41.3 41.4 41.4 41.4 28.1 37.4 15.9 20.4 14.8 58.6 86.1 10.1 23.7
26.6  7.8 19.4 31.0 34.0  9.6  9.0 14.3  6.1  6.1 95.2 28.9 10.7 50.9 10.7 10.7 1815.2 1134.0 582.6
385.5 88.9 17.1 0.0
```

Under 50 um by iteration 4, under 20 by 14, in the 4 um band only at 43. The rows that block
at the end of each sweep are the same rows for most of the bite:

| row | present (of 43) | residual median / last (um) |
|---|---|---|
| edge(piece 0, bottom) | 36 | 3.7 / 5.2 |
| pair(0, 6) | 35 | 5.1 / 3.6 |
| pair(6, 44) | 34 | 4.3 / 4.3 |
| pair(6, 43) | 34 | 5.7 / 4.8 |
| edge(piece 44, top) | 32 | 6.1 / 3.1 |
| edge(piece 43, top) | 31 | 5.6 / 5.6 |
| pair(8, 45) | 29 | 8.2 / 1.0 |
| edge(piece 2, bottom) | 28 | 6.1 / 0.6 |

That is one object: a column 0 - 6 - 43/44 standing from the sheet's bottom edge to the strip
top, with 2 - 8 - 45 beside it. The bite shortened the strip by 0.18 mm and the column has no
slack: every member is pinned between its neighbour above and below. The fine coordinate
descent exits are consistent with that and not with a step floor:

- 339 committed relocates in the bite; 282 (83 %) exit the fine CD with a nonzero incident
  violation, median 7.8 um; every exit is by `limits`.
- **108 of the 282 are unmoved**: the relocate ran the 75 samples, the coarse and the fine
  descents, and committed the entry pose, because every +- candidate was strictly worse. Those
  are strict local minima of the weighted incident objective at a median 5.4 um of violation,
  and they belong to pieces 0 (22 times), 6 (17), 43 (16), 44 (15), 45 (10): the column.
- 174 moved and were stopped by the limits (median exit 14 um, final translation step 37 um
  against a 60 um limit). A continuation at 1 um resolution can act on these; it cannot act
  on a pinned piece.
- Changed rows by the other endpoint's status in the sweep: later 201, visited 176, absent 37,
  boundary 78. Conflict births (a zero row made positive): visited 28, absent 37, later 23,
  boundary 11. The conflict does move to pieces whose turn has passed or never came.

The bite resolves at iteration 36, when the maximum violation jumps to 1.8 mm: after 36 weight
updates the column's rows carry weights of order 1.2^36, and a member's weighted objective
finally prefers a distant sample with a real overlap to its pinned place. Seven iterations
later the state is proxy-zero and publishes. Thirty iterations of the bite were the column
waiting for the weights.

## Without the margin: bite 17, 180.070 -> 179.890, 37 iterations

The same column (edge(0, B), pair(0, 6), pair(6, 43), edge(43, T), edge(2, B), pair(2, 8),
pair(8, 45)), residuals 25-30 um for 18-20 iterations, then the same jump (2.1 mm at iteration
19), then 18 iterations to converge. On the identical constructor layout Sparrow resolved this
bite in 17 passes (`../sparrow-warm-start/first-bites.txt`).

## The three bites after

Margin 8: 4, 26 and 16 iterations; the 26-iteration bite blocks on edge(piece 44, top),
pair(6, 44) and related rows again: the column is still there. Margin 0: 11, 11, 14.

## What this says about the ranked causes

- The step floor (rank 1) is real for the 174 moved exits but it is not what holds the hard
  bite: the persistent blockers are pinned pieces, where no single-piece move of any step
  size helps. A CD continuation will tidy the moved exits and cannot break the column.
- Conflict transfer (rank 2) is present (176 + 37 changed rows with an unavailable endpoint)
  and is the shape of the column: each member's move worsens a row with a member already
  visited. A revisit queue would keep trading the same micrometres around the column.
- What breaks the column is a rearrangement: one member leaving, which the GLS weights
  produce after thirty iterations. Sparrow's weights follow the same rule and it escapes in
  seventeen passes on the same layout; why is the question for review 5.

## Addendum: how the column broke, and what it cost in weight

The winner's relocates at iterations 35-38 of bite 15 (margin 8), from the trace:

- iterations 35-36: pieces 47, 6, 40, 0, 44, 43 all `stayPut`, unmoved or moved by 0.05-0.09 mm;
  residuals 4.3-10.7 um; the column's pieces carry guided costs of 4-8 on raw violations of
  4e-5 to 8e-5 mm^2, i.e. effective weights of 1e5 on their rows.
- iteration 37: piece 43 (top of the column) commits a **container** sample 165.9 mm away,
  rotated 120 degrees, opening 1.77-1.82 mm overlaps with pieces 40, 42 and 60 (fresh rows,
  weight 1): raw 6.1e-5 -> 9.58, guided 10.7 -> 9.58. The weighted cost of a 5.6 um residual on
  a row that has been blocking for 36 updates had just exceeded the cost of nearly two
  millimetres of new overlap elsewhere. In the same sweep piece 6 (focused, 7.2 mm, 200
  degrees) and piece 0 (2.4 mm) clear their rows: the column is gone.
- iterations 38-43: pieces 40, 42, 60, 47 absorb the new overlaps in 1-2 mm moves; proxy-zero
  at 43.

Effective weight (guided / raw) on the pinned pieces, iterations 25-37:

```
effective weight (guidedBefore / rawBefore) of the pinned column pieces, iterations 25-37:
  it 25 max    34.0 um  p44:1.47e+02 p43:1.22e+03 p0:2.48e+03 p6:1.81e+03
  it 26 max     9.6 um  p43:1.13e+03 p0:3.22e+03 p6:2.35e+03 p44:1.97e+02
  it 27 max     9.0 um  p44:6.95e+02 p43:1.96e+03 p6:3.16e+03 p0:4.94e+03
  it 28 max    14.3 um  p44:2.82e+03 p0:7.69e+03 p43:3.49e+03 p6:4.93e+03
  it 29 max     6.1 um  p43:5.50e+03 p6:7.34e+03 p44:7.69e+02 p0:1.10e+04
  it 30 max     6.1 um  p6:1.13e+04 p44:7.21e+03 p43:1.10e+04 p0:1.90e+04
  it 31 max    95.2 um  p43:2.16e+04 p6:2.55e+04 p44:1.43e+04 p0:3.28e+04
  it 32 max    28.9 um  p44:1.78e+04 p0:4.05e+04 p43:2.31e+04 p6:2.28e+04
  it 33 max    10.7 um  p43:3.13e+04 p44:1.18e+04 p0:5.81e+03 p6:1.57e+04
  it 34 max    50.9 um  p44:2.26e+04 p6:4.63e+04 p0:2.34e+04 p43:5.36e+04
  it 35 max    10.7 um  p6:2.04e+03 p0:1.04e+05 p44:2.92e+04 p43:6.87e+04
  it 36 max    10.7 um  p43:1.09e+05 p6:1.10e+05 p0:1.61e+05 p44:4.32e+04
  it 37 max  1815.2 um  p43:1.75e+05 p44:7.74e+04 p6:1.22e+05 p0:2.02e+05
```

So the escape from a pinned column is a weight race: the guided objective is `w * v^2`, a
micrometre residual is 1e-5 mm^2, and a member leaves only when its rows' weights make that
residual cost more than a millimetre-scale fresh overlap, which takes weights of order 1e5,
about 36 updates at the 1.4-1.6x per update that `1.2 + 0.8 v / v_max` gives a row that is
not the current maximum. If Sparrow's quantifier is closer to linear in the penetration, the
same escape needs weights of order 3e2, about 15 updates: its 17 passes on this bite. That is
a hypothesis with a direct test in the replay harness: `--probe=linear` (guided = w * v, same
weights, same sampler) from the bite-15 capsule, and the prediction is that the column breaks
in 15-20 iterations instead of 36. It is also a landscape change, not a constant change, and
it would go to a prospective spec on its own.

The reading record already has Sparrow's functional form (`docs/grok-review-12-reading-sparrow.md`
§1, line 85): its per-pair loss is `sqrt(overlap_area_proxy(poles) + eps^2) * shape_penalty`,
Algorithm 4 of arXiv:2509.13329, with the pole overlap area of Algorithm 3. The overlap area
of two circles at penetration `d` scales as `d^1.5` for small `d`, so Sparrow's loss scales as
about `d^0.75`: sub-linear in the penetration. Ours is `v^2`. For a 5 um residual against a
1.8 mm fresh overlap the ratio of the two costs is `(0.005/1.8)^2 = 8e-6` for us and
`(0.005/1.8)^0.75 = 0.012` for Sparrow: the weight needed to make a pinned member leave is
about 1e5 for us and about 1e2 for Sparrow, and at 1.4-1.6x per update that is 30-36 updates
against 8-12. Sparrow's 17 passes on this bite fit. The pole proxy itself is not licensed
(forbidden-rescue list), and is not needed: the exponent on our own signed-gap violation is
our design decision (`guided = w v^2` was Grok review 12's recommendation, line 188 there, for
"one guided path"), and the replay probe `guided = w v^p` for `p` in {1, 0.75, 0.5} from the
bite-15 capsule tests it directly, with the prediction that the column breaks in 10-20
iterations at `p <= 1`.

Correction from the pinned source (`src/quantify/overlap_proxy.rs` and `src/quantify/mod.rs` at
`14f4868f`, read on GitHub; nothing ported): per pole pair `pd = r1 + r2 - d`, contribution
`pd_decay * min(r1, r2)` with `pd_decay = pd` for `pd >= eps` and `eps^2 / (-pd + 2 eps)`
below it (positive even without overlap: a smooth tail beyond contact), summed and multiplied
by pi; then `loss = sqrt(overlap + eps^2) * penalty` with `penalty` the geometric mean of the
two convex-hull square-root areas. So for a colliding pair Sparrow's loss is about
`sqrt(pd)`, not `pd^0.75`: a 5 um residual against a 1.8 mm fresh overlap costs
`(0.005/1.8)^0.5 = 0.053` of it for Sparrow and `8e-6` of it for us. The escape weight is
about 20 for Sparrow, about 1e5 for us. The replay probe should therefore include `p = 0.5`,
and the prediction for the bite-15 capsule is a break within a handful of iterations at that
exponent. Note what else the exponent touches: only the guided ranking among colliding
candidates and the tournament's winner selection; raw Phi, the band, the strike meter's
minimum and the weight growth rule `v / v_max` stay on the violation itself.

## The two detached probes (commit "The replay harness", `--cell=replay`)

The replay rebuilds the state from a capsule and re-runs the separation with the same eight
worker streams; with `--probe=none` it reproduces the traced sweeps bit for bit (43/43 on bite
15, 26/26 on bite 17, 37/37 on the margin-0 bite 17: raw, max and winner equal at every
iteration). The probes diverge at iteration 1 by construction. Documents gzipped beside this
file (`replay-*.json.gz`). Iterations and evaluations are to band entry, all workers charged,
continuation evaluations included:

| capsule | control | `cdfinish` (1 um continuation, 64 pairs) | `revisit` (bounded queue) |
|---|---|---|---|
| margin 8, bite 15 | 43 it / 685 980 ev | 30 it / 524 855 ev (81 388 continuation) | 35 it / 674 855 ev (114 queued in winners) |
| margin 8, bite 17 | 26 it / 262 987 ev | 20 it / 245 520 ev (enters the band with the five rows still ~1 um positive) | 15 it / 209 704 ev |
| margin 0, bite 17 | 37 it / 500 681 ev | 33 it / 626 469 ev (+25 % evaluations) | 32 it / 446 862 ev |

The control's five most persistent rows on bite 15 clear at iterations 37/37/37/37/38; under
`cdfinish` at 28/28/27/28/28; under `revisit` at 33/32/32/33/34. So: the revisit queue helps
on every captured bite at less work; the continuation helps in iterations everywhere and
costs more evaluations without the margin; neither breaks the pinned column early. The
column still waits for its weights, and the exponent probe is next.

## The matched comparison: Sparrow from the exact parents of the two traced hard bites

Astra's review 5 objected that the first-bite comparison in `../sparrow-warm-start/first-bites.txt`
is not matched after bite 1. It can be: the capsule at bite entry holds the post-cut poses and
the cut mask, and `split_and_close` only adds `delta` to the moved pieces' `ty`, so the parent is
recovered exactly (both parents pass Sparrow's validator: minimum pair distance 5.0052 and
5.0012 mm, boundary 5.000). Sparrow warm-started from each parent (`-t 10 -s 0
--min-item-separation 5 --workers 8`; solution files and logs beside this file) first
re-legalises the parent on its own proxy (simplified polygons: it reads our contract-valid
layout as a 12.5 K / 10.9 K loss and clears it in 9 / 6 passes), then cuts at `W/2`, the same
rule as ours, and separates:

| parent | our bite (master iterations) | Sparrow's matched bite (passes, entry loss) | Sparrow's next bites (passes) |
|---|---|---|---|
| bite-15 parent, 180.195 (margin 8 trace) | 43 | **8** (6.31 K) | 2, 3, 3, 1, 8, 13, 2, 3, 1, 1 |
| bite-17 parent, 180.070 (margin 0 trace) | 37 | **7** (4.73 K) | 1, 4, 5, 4, 45, 6, 2, 1, 2, 2 |

Same parent (up to Sparrow's own re-legalisation of it), same cut, same 0.1 %: 8 passes
against 43 and 7 against 37. Sparrow also shows a 45-pass bite six bites later on the margin-0
parent, so the column is hard for it too, five times less so. From these parents it reaches
151.404 and 152.668 at 8 s.

## The exponent probe (commit "The exponent probe", `--probe=exponent:<p>`)

Guided = sum w v^p in the candidate ranking and the tournament's winner selection only; raw
Phi, the band, the strike minimum and the weight growth untouched; p = 2 reproduces the traced
sweeps bit for bit (43/43, 26/26, 37/37). Evaluations are all workers, to band entry:

| capsule | p = 2 (control) | p = 1 | p = 0.75 | p = 0.5 |
|---|---|---|---|---|
| margin 8, bite 15 | 43 it / 685 980 | 32 it / 462 936 | 40 it / **288 357** | 49 it / 230 022 |
| margin 8, bite 17 | 26 it / 262 987 | 30 it / 212 530 | **13 it / 64 418** | 18 it / 71 071 |
| margin 0, bite 17 | 37 it / 500 681 | **19 it / 213 938** | 30 it / 175 440 | 45 it / 174 352 |

Astra's threshold for a probe to lead (review 5 Q4) was certification within half the control's
evaluations: p = 0.75 and p = 0.5 meet it on all three capsules, p = 1 on one. The
continuation and revisit probes met it on none.

What the exponent changes is the character of the descent, not only the speed of the escape.
Under p = 2 the state trickles: 8-11 blocking rows per iteration at 5-40 um, 16 000
evaluations per iteration, and the pinned column waits thirty iterations for its weights.
Under p < 2 the objective is concave in each row, so the sweep concentrates the violation on
one or two rows instead of spreading it: 1-3 blocking rows per iteration at 150-200 um, 4 000
to 7 000 evaluations per iteration (fewer colliding pieces, so fewer relocates), a plateau, then
a multi-millimetre jump that breaks the state, then convergence. The jump comes earlier at
p <= 1 on two of the three capsules (bite 17: iteration 12 at p = 0.75 against never at
p = 2; margin-0 bite 17: 14 at p = 1 against 19), later on bite 15 in iterations but at a third
of the evaluations. In the live engine evaluations are the wall (about 16 000 per master
iteration at 4.7 M per second, 3.4 ms of a 3.7 ms iteration), so the evaluation column is the
one that predicts time.

The prediction written before the probe ("the column breaks in 10-20 iterations at p <= 1,
in a handful at 0.5") is confirmed on bite 17 and refuted on bite 15 in iterations; in
evaluations it is exceeded everywhere. The exponent is one parameter fitted on three capsules
of one seed, which is why the live knob is `--guidedexponent=<p>` with p fixed prospectively
and screened on seeds 18-26 at more than one value before any prospective specification names
one.

The ordinary bites under the same probe (the three retained after each trigger; evaluations to
band, all workers; `replay-easy-*.json.gz`):

| capsule | p = 2 | p = 1 | p = 0.75 | p = 0.5 |
|---|---|---|---|---|
| margin 8, bite 16 (easy) | 4 it / 60 262 | 5 / 59 229 | 4 / 45 204 | 3 / 38 368 |
| margin 8, bite 18 | 16 / 184 129 | 19 / 164 488 | 22 / 115 518 | 15 / 74 032 |
| margin 0, bite 18 | 11 / 137 747 | 7 / 70 588 | 4 / 33 810 | 9 / 50 481 |
| margin 0, bite 19 | 10 / 113 209 | 5 / 67 686 | 5 / 61 707 | 5 / 38 669 |
| margin 0, bite 20 | 14 / 146 453 | 6 / 67 978 | 7 / 54 254 | 14 / 66 627 |

The easy bites stay easy or get cheaper; no capsule of the eight costs more evaluations at
p = 0.75 than at p = 2, and only bite 16 at p = 1 costs one iteration more.

## Scoring the exponent probe against the targets Astra registered before seeing it

Review 5b (`docs/astra-review-5b-the-exponent.md`, Q6) was written after the continuation and
revisit probes and before the exponent results, and registered, for the bite-15 capsule
(margin 8, eight workers, 50 sweeps maximum): column-break deadlines 20 / 16 / 12 and first
band-entry deadlines 27 / 23 / 19 for p = 1 / 0.75 / 0.5, plus the separate halving target of
at most 342 990 evaluations and 21 sweeps. Against those:

| p | band entry (deadline) | evaluations (target 342 990) | verdict on the registered forecast |
|---|---|---|---|
| 1 | 32 (27) | 462 936 | misses both |
| 0.75 | 40 (23) | 288 357 | misses the iteration deadline, meets the evaluation target |
| 0.5 | 49 (19) | 230 022 | misses the iteration deadline, meets the evaluation target |

On bite 17 (margin 8) p = 0.75 enters the band at 13 with 64 418 evaluations and p = 0.5 at 18
with 71 071; on the margin-0 bite 17, p = 1 enters at 19 with 213 938 (its deadline there,
scaled, would be about 23). So the fast-escape forecast, the weight race resolving in a
handful of updates, is **refuted on the trigger bite and supported on the two others**; the
efficiency result holds on all three at p <= 0.75 and on one at p = 1. What the traces show
in place of an early escape on bite 15 is Astra's own "conflict concentration" hazard turned
into the benefit: under a sublinear objective the sweep keeps one or two rows at 150-200 um
instead of eight to eleven at 5-40 um, fewer pieces collide, each iteration costs a quarter
of the evaluations, and the state still needs a jump to finish. That is "faster band entry
through a different contact arrangement", which review 5b says "supports a different causal
account". The column-break definition from committed geometry, the sweep-24 diagnostic fork
and the extended replay identity that review 5b asks for are not yet built; the live knob is
being built first, per its Q9: `--guidedexponent`, control 2 against treatment 1 only, in the
108-cell screen with margin 8 and the profile caps in both arms.
