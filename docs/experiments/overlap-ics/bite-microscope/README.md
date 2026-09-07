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
Under p < 2 the quadratic amplification is gone (p = 1 is linear in each row, below 1 the row
term is concave; Astra review 6 corrected the earlier "p < 2 is concave": for equal weights and
a fixed total violation V spread over m rows the cost is m^(1-p) V^p, so p = 2 favours
spreading, p = 1 is indifferent, p < 1 favours concentration), and the sweep concentrates the violation on
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

## The column from committed geometry, the fork at sweep 24, the certification (review 5b Q6–Q8)

The replay-geometry instrument (commits "The replay's committed geometry" and "The pinned
fractional power"; `column-break.py`, documents under `geometry/`, log `geometry/demo-log.txt`
and `geometry/column-break-output.txt`) adds to every replay the winner's committed relocates
per iteration, the column's rows with their actual GLS weights, the core members' poses, the
extended identity gate (poses, weights and stream fingerprints, relocates and evaluation counts
against the trace: 43/43 on bite 15 at p = 2 and at `--probe=none`), `resolvedSeed`, the
`--fork=<sweep>` re-scoring and the `--certify=1` detached publication check.

**Two readings of "the column".** Review 5b asked for the column break "from committed
geometry and row identities". The instrument defines the blocking graph (pieces and the four
strip edges as vertices, rows with violation > 0 as edges) and reads the column two ways.
*Entry graph:* the bottom-to-top paths that exist in the capsule's own blocking rows. On bite
15 there is exactly one, edge(2,B)–pair(2,8)–pair(8,45)–edge(45,T), members 2, 8, 45; the
0–6–43/44 column this README described is not bottom-to-top at entry (edge(44,T) is not yet
positive) and connects at iteration 1. *Longest-lived:* the same analysis with the formation
iteration chosen by residence; at p = 2 it is the union of both chains (8 rows, members 0, 2,
6, 8, 43, 45), formed at 6, resident 31 iterations, and it breaks at 37, which is the number
review 5b registered as "reproduce 37". A break is the first permanent release of a column row
after which the original rows no longer connect bottom to top; releases that re-form before
band entry are listed separately as temporary.

**Bite 15 (margin 8, eight workers, 50 iterations maximum, one run per exponent) against the
registered targets:**

| p | column break, entry graph (deadline) | column break, longest-lived | band entry (deadline) | evaluations to band (halving target 342 990) | max column-row weight at the break | temporary releases |
|---|---|---|---|---|---|---|
| 2 | 29 (reproduce) | **37** (reproduce 37) | 43 (43) | 685 980 | 1.6e5 / 2.5e5 | 2 |
| 1 | **16** (20) | 26 | 32 (27) | 462 936 (misses) | **113** / 576 | 9 |
| 0.75 | **12** (16) | 12 | 40 (23) | 288 357 (meets) | **12.4** | 12 |
| 0.5 | **11** (12) | 11 | 49 (19) | 230 022 (meets) | **11.8** | 10 |

Every treatment meets its column-break deadline on the entry-graph reading and breaks the
column at weights three to four orders of magnitude below the control's (113 against 1.6e5 at
p = 1; 12 against 1.6e5 at p ≤ 0.75). Review 6 adds the verdict the two readings force: the
reading that reproduces the registered "37" is the longest-lived one, and under that reading
p = 1 breaks at 26 and misses its deadline of 20; one cannot take the longest-lived reading for
the control and the entry-graph reading for the treatment (under the entry-graph reading the
control breaks at 29, not 37). None meets its band-entry deadline; the halving holds at
p ≤ 0.75 and not at p = 1. In review 5b's own reading rules this is "earlier release at much
smaller actual old-row weights" without "band entry within the table", i.e. the acceptance
explanation is supported and the fast-cleanup forecast is not: at p = 1 the escape comes 13
iterations earlier and the band 11 earlier; at p = 0.75 the escape comes 17 earlier and the band
only 3 earlier, while the evaluations fall by 58 % because the iterations after the break are
cheap. The treatments also release and re-form column rows four to six times more often than
the control (the conflict-concentration hazard turned into churn).

**The fork at the end of control sweep 24** (`geom-b15-fork24.json`; identity 25/25 up to the
fork; column-row weights there 1.9e4–3.9e4, twelve blocking rows, max 31 µm). In sweep 25 the
eight workers ran 64 relocates and the fork re-scored every candidate under all four exponents
at the same poses and weights: a candidate beats the stay pose in **2** relocates under p = 2,
**36** under p = 1 (all of them finalists), **63** under p = 0.75 and **64** under p = 0.5.
For the entry column's members 2, 8 and 45 the crossover is complete: in every worker's
relocate of pieces 2, 8 and 45 (8 of 8 workers each) a finalist beats the stay under p = 1,
none under p = 2. The arithmetic is the one the README's escape-weight section predicted: the stay pose of
piece 2 scores 0.3–0.5 under `Σ w v²` (weights ~2e4 on 3–5 µm residuals) and 108–135 under
`Σ w v`, while the finalist that beats it carries a fresh overlap of raw 750–1 200 mm²
(10–30 mm of penetration, 700–1 000 mm away, at weight 1) which prices at 750–1 200 under
`v²` and 27–35 under `v`. So at these weights the quadratic price keeps the column's pieces
pinned and the linear price lets them leave. Member 0 stays under p = 1 in every worker, 6 leaves in 2 of
8, 43 and 44 in some (review 6 corrected the earlier "0 and 6 stay everywhere"). Under the deciding p = 2 the core members commit micro-moves of at
most 0.15 mm or stay.

**Certification.** `--certify=1` runs the unchanged publication path
(`Engine::attempt_publication` → `publish::attempt` → `validate_placements_against_contract`)
on the band-entry state, records the checkpoint in the refused document and installs nothing.
On bite 15 the band-entry states publish at p = 1 (180.0066 mm, target 180.0146; contract and
Exclusive kernel valid, zero repair rows, one exact call), at p = 0.75 and at p = 0.5 (180.0056
mm both). The band-entry readings above are therefore certifiable depths, which review 5b's
qualification required before promoting them.

**The other two capsules.** Bite 17 (margin 8): p = 2 breaks the column (again 2–8–45) at 25
with weight 1.4e5 and enters the band at 26; p = 1 at 19 with weight 104, band 30, 212 530
evaluations; p = 0.75 at 8 with weight 6.2, band 13, 64 418; p = 0.5 at 12 with weight 11,
band 18, 71 071. Bite 17 of the margin-0 document: p = 2 forms a three-path column (members
0, 2, 6, 8, 43, 45, 48) at 4 and breaks it at 21 with weight 2.6e4, band 37; p = 1 breaks a
2–8–45 column at 17 with weight 121, band 19; p = 0.75 and p = 0.5 **never form a bottom-to-top
column** at the observed committed sweep boundaries before band entry ("column avoided",
which review 6 reads as changed trajectory formation, never as an escape or as escape time
zero) and enter the band at 30 and 45. This document's column at p = 2 forms at iteration 4,
not at entry: by review 6's Q14 the entry exposure is absent there and the formation depends
on the trajectory, so its break at 21 is a trajectory description, not a matched escape.

## The depth-triggered microscope (`--microscopetarget=<mm>[,<mm>]`, review 7 Q18)

GPT-6 Astra review 7 Q18 (`docs/astra-review-7-the-verdict.md`) locates the dominant unresolved
work of the Wall10s cell in its *last* bite, not its first hard one: the ten-second cell
publishes four explore bites and fails the cut targeting about 155.5 mm (seeds 27 and 31 publish
a fifth and fail the cut at about 150.5 mm); that unpublished bite takes 79.5 % of the
treatment's exploration evaluations, never reaches the proxy band, makes no exact attempt and
ends at the wall. The iteration trigger above cannot see it. Astra's specification: "capture the
first explore cut targeting at most 156 mm; if it publishes, also retain the next cut targeting at
most 151 mm. Record absence without substitution. Capture each arm's own entry and follow the
actual attempt to publication or its live stopping boundary."

The instrument (commit "The depth microscope"; `overlap_ics::microscope`, "the depth trigger") is
the same microscope with a second trigger in `MicroscopeConfig`, cutclose only, refused beside
`--bitemicroscope`, bit-identical off, the same `biteMicroscope` block with `trigger: {kind:
"target", targetMm, secondTargetMm}` and the same tripwire. The trigger bite is the first explore
bite whose `targetDepthMm` is at most the first threshold; it is retained whole - every separation
attempt with the full per-sweep trace, the entry capsule of every attempt (`separations[].capsule`),
the actual `SeparateStop`, the remaining wall allowance at entry and at the stop
(`wallAtEntry.leftS`, `wallAtStop.leftS`), the strikes, the rollbacks, the exact calls and the
publication outcome or the reason none was attempted (`publication`). If it publishes, the next
explore bite targeting at most the second threshold is retained the same way. If no explore bite
ever targets at most the first threshold, `exposure = {status: "absent", reason, deepestTargetMm,
lastPublishedDepthMm}` and nothing is substituted.

What the trace gains for Astra's four questions: per sweep, **every worker's** economics
(`sweeps[].workers[]`: sample evaluations, relocates, moved relocates, container commits, useful
moves, the post-sweep raw/guided/max the tournament ranked it on), the useful moves the tournament
discarded (`usefulMovesDiscarded`: a losing worker committed a move that lowered its incident
guided energy) with the losers' whole expenditure (`discardedExpenditure`); per attempt the
blocking rows at entry and at the stop (`entryBlocking`, `stopBlocking`) beside the end-of-sweep
sets. Counters per sweep and per worker, never per candidate, so a wall-long attempt of several
hundred sweeps stays a few megabytes.

The replay gains `--horizon=live`: the cap is the traced attempt's own sweep count, so the
treatment objective replayed from the control's entry (and the control from the treatment's) gets
exactly the live attempt's budget - "another 100-iteration horizon would censor much of the
observed deeper work" - and the document records `horizon: {kind, iterations}`. A p = 1 document
replays with `--guidedexponent=1` (the live knob must agree with the document, as before) and the
probe names the objective: `--probe=exponent:1` reproduces the trace, `--probe=exponent:2` is the
control objective from the treatment's entry; the reconstruction folds at the capsule's exponent
and the trajectory at the probe's.

`deep-cut.py <microscope.json>... [--replays <dir>]` prints all of it; print only, no scoring.
The demonstration documents (seeds 27 and 31, Wall10s, margin 8, both arms, `--microscopetarget=156,151`)
and their eight replays are under `/var/lib/t3/tmp/astra/deep/`; the full `deep-cut.py` reading is in
the commit message of the instrument. What the four documents say, without interpretation:

* Every arm's bite 5 is the trigger (target 155.48-155.49 mm from a 160.62 mm parent). The control
  (p = 2) fails it at the wall on both seeds in one attempt with no band entry and no exact call
  (seed 27: 230 iterations, 3.61 s left at entry; seed 31: 586 iterations, 5.68 s left), one or two
  strike rollbacks, and the min-raw snapshot handed back is far from the band (max residual 1280 um
  and 215 um). The treatment (p = 1) publishes it on both seeds (707 iterations with 2.11 s left;
  186 iterations with 5.94 s left; repair rows 0) and then fails bite 6 at 150.50 mm at the wall the
  same way (203 and 634 iterations, no band entry).
* The blocking set at a failed stop is mostly *new*: of the 33-51 entry rows, 0-8 persist to the
  last sweep; the rest were released and 23-79 rows were created since entry. The most persistent rows
  are present in 100-160 of the sweeps of a failed attempt.
* Seven of eight workers lose every sweep, so 87.3-87.6 % of all committed useful moves are
  discarded with 87.5 % of all evaluations (8.9-21.6 million per failed attempt).
* Replays at the live horizon: the same-objective replay of each trigger entry passes the identity
  gate on every iteration (230/230, 586/586, 707/707, 186/186) and `--certify=1` reproduces both
  treatment publications to the bit (155.4735 and 155.4747 mm); the other objective diverges at
  iteration 1. From the treatment's entry the p = 2 objective spends 2.9-3.2x the evaluations of
  the live p = 1 attempt in the same iteration count without entering the band; from the control's
  entry the p = 1 objective does not enter the band in the live count either.

## The failed fifth cut under the depth microscope (review 8, Q21-Q22)

Review 8 (`docs/astra-review-8-closing-the-exponent.md`, Q21-Q22) asked the depth microscope four
questions of the bite every Wall10s cell fails, the fifth explore cut from the parent near 160.6 mm
toward 155.5 mm, and named three readings to rank from the answer: conflict transfer or
recreation at the deeper target; useful states discarded by the tournament; insufficient remaining
work. This section is the reading. The data are 42 depth-microscope documents
(`/var/lib/t3/tmp/astra/deep/deep-<A|B>-wall10s-s<seed>.json`; Wall10s, margin 8,
`--microscopetarget=156,151`; arm A = control p = 2, arm B = treatment p = 1; the 21 consumed seeds
27-35 and the twelve 64-bit v2 seeds; diagnostic runs under the tripwire, never scored) and 100
replays of every retained attempt's entry capsule under both objectives at the live horizon with
`--certify=1` (`/var/lib/t3/tmp/astra/deep/replays/`). Fifty attempts are retained, every one a
single attempt of its bite: control 20 failed and 1 published fifth cuts (seed 10636268072709740349)
plus that seed's sixth cut; treatment 14 failed and 7 published fifth cuts (the five v2 successes
and seeds 27 and 31, which v2 never scored) plus 7 sixth cuts. All eight sixth cuts fail. On the
twelve seeds both have, the microscope's fifth-cut outcome equals the scored v2 outcome of all three
repetitions in 24 of 24 arm-seed pairs, with the worst repetition's iteration count within 1.6 % of the
microscope's at the median pair (per cell 0.44 % at the median, 14.0 % at the maximum). Four
analysts read the documents with stdlib python under `/var/lib/t3/tmp/astra/deep/analysis/`
(`persists-*.py`, `moves-*.py`, `parents-*.py`, `ends-*.py`); where two of them disagreed the
number below is recomputed by `synth-1-reconcile.py` and said so. Medians with [range] unless
stated; "handed-back state" is `stopBlocking`, the last new-minimum state the attempt restores at the
stop, which on all 42 failed attempts is the argmin-raw iteration (`restoredToIteration` equals the
last `samples[].newMinimum` on 42 of 42).

One correction to the brief before the tables: a p = 2 master iteration costs 2.20 times the
evaluations of a p = 1 iteration in these files (all-worker evaluations per iteration, bite 5,
medians 76,490 vs 34,806; same-capsule replays 1.92-2.03 at the median, range 0.97-3.54; the 180
scored cells 75,557 vs 33,253), not three to four times; the two arms burn evaluations at nearly the
same rate per second (4.85 M vs 4.32 M), so p = 1 gets about twice the iterations per second.

### 1. What persists

| group (arm, bite, outcome) | n | iterations | entry rows (pair / B / T; max um) | handed-back state at iteration | rows there (pair / B / T; max um) | entry rows there = continuous + re-formed | rows created since entry | B-T spanning component: entry / handed-back / last sweep | pieces in entry rows still in blocking rows: handed-back / last sweep |
|---|---|---|---|---|---|---|---|---|---|
| A (p = 2) bite 5 failed | 20 | 296.5 [129-582] | 46 (38 / 1 / 7; 5138) | 8.5 [5-485] | 95 [26-132] (61 / 15.5 / 17; 1262 [215-1646]) | 28.5 [1-45] = 22.5 + 7 | 64 [20-87] | 11/20 / 20/20 / 18/20 | 50 -> 43 / 34.5 |
| A bite 5 published | 1 | 479 | 48 (36 / 0 / 12; 5129) | 479 | 3 (1 / 0 / 1; 0.6) | 0 = 0 + 0 | 3 | 0/1 / 0/1 / 0/1 | 48 -> 4 / 4 |
| B (p = 1) bite 5 failed | 14 | 808.5 [566-922] | 41 (34.5 / 0 / 6.5; 5137) | 609.5 [206-890] | 44 [20-69] (28.5 / 6.5 / 8.5; 3613 [605-6383]) | 15.5 [1-26] = 0 + 15.5 | 27.5 [17-45] | 3/14 / 14/14 / 11/14 | 47 -> 29.5 / 15 |
| B bite 5 published | 7 | 416 [161-729] | 45 (38 / 0 / 6; 5135) | 416 | 1 [0-5] (0 / 0 / 0; 1.7) | 0 [0-1] = 0 + 0 | 1 [0-4] | 1/7 / 0/7 / 0/7 | 49 -> 1 / 1 |
| B bite 6 failed | 7 | 406 [126-618] | 48 (39 / 1 / 9; 4974) | 202 [2-609] | 57 [43-68] (37 / 9 / 11; 4345 [3744-5842]) | 20 [14-35] = 0 [0-13] + 14 | 33 [29-44] | 4/7 / 7/7 / 3/7 | 52 -> 37 / 16 |

| entry rows continuously present up to sweep k (median) | k = 1 | 2 | 3 | 5 | 10 | 20 | 50 | 100 |
|---|---|---|---|---|---|---|---|---|
| A bite 5 failed (46 entry rows) | 31.5 | 30 | 27.5 | 26.5 | 21.5 | 6.5 | 0 | 0 |
| A bite 5 published (48) | 41 | 40 | 39 | 39 | 30 | 2 | 0 | 0 |
| B bite 5 failed (41) | 18.5 | 13.5 | 11 | 6 | 1 | 0 | 0 | 0 |
| B bite 5 published (45) | 22 | 16 | 13 | 8 | 1 | 0 | 0 | 0 |
| B bite 6 failed (48) | 18 | 13 | 11 | 8 | 1 | 0 | 0 | 0 |

Sources: `persists-1-census.py` -> `persists-2-report.py`, `persists-3-dynamics.py`,
`persists-5-minima.py`, `persists-8-continuous.py` (blocking graph: 61 pieces + the strip edges
L, R, B, T as vertices, every row with residual > 0 as an edge, decoded with `rowIdScheme`).

Row identity does not persist: the entry rows are released within a few sweeps in both arms and
both outcomes (no entry row of any attempt is continuously present at sweep 100 in the control, where
six failed attempts keep one or two entry rows to sweep 51-81, or at sweep 20 in the treatment), and what is present later is re-formed, not continuous; re-formation is
the dominant row dynamic, with 3046.5 (A failed) and 6176 (B failed) organic re-formation events per
attempt and 39 of 46 (A) and 40 of 41 (B) entry rows re-formed at least once after their first
release (presence episodes counted over sweeps 1 onward; 41 and 41 if the entry state is the first episode). What persists is structure carried by pieces under new rows: a bottom-to-top spanning
blocking component is present at the handed-back state of every failed attempt (20/20, 14/14, 7/7)
and at every new-minimum state of all but A 34 and A 35 (which lack it at one and two of their 30 and
18 new minima), although present at only 11/20, 3/14 and 4/7 entries, and absent at every published
stop; the
largest component at the handed-back state has 50 pieces (A) and 31.5 (B) and shares 20 and 12
pieces with the entry's largest, and 43 of the 50 pieces in entry rows (A) and 29.5 of 47 (B) are
still in blocking rows there, against 28.5 and 15.5 of the rows. The handed-back states are not
the entry population: the control's carries 95 rows whose largest residual is about a quarter of the
5.1 mm cut (1262 um at the median, 1212-1646 um on 15 of 20), its three largest residuals forming a
bottom-piece-piece-top chain on 4 of 20 (seed 11151532166486038253: (40,B) 1227, (12,40) 1185,
(12,T) 1134 um), the treatment's 44 rows with a largest pair or boundary row of 3.3-6.4 mm on 12 of 14
(0.6 and 1.9 mm on the other two); the most persistent row of an attempt is a boundary row against the bottom or top
strip edge in 18 of 20 (A) and 12 of 14 (B) failed fifth cuts, created after entry in 17 of 20 and 14 of
14, and none is present for more than 0.85 (A) or 0.23 (B) of the attempt. What the
numbers cannot say: end-of-sweep persistence is dominated by the search's own relocations (the
winner makes at least one container relocation, of median 507 mm (A) / 540 mm (B), in 83 % (A) /
95 % (B) of sweeps at the attempt median, and 23,682 of 23,697 end-of-sweep states carry a row above
10 um, 21,295 above 10 mm, published attempts included), so the handed-back states and the
chain of new minima are the states that matter and the sweep-level decay curves are a like-for-like
comparison only between arms; the control's handed-back state is 5-10 iterations after entry in
13 of 20 attempts (recomputed, `synth-1-reconcile.py` (b); one analyst wrote 14), so its 28.5
surviving entry rows are not comparable with the treatment's 15.5 after 609 iterations; and the
documents carry no per-worker end states, so what a losing worker's blocking looked like cannot be
read. Nothing at entry separates the published from the failed attempts within an arm (the B-T span
at entry is 11/20 failed vs 0/1 published in the control, 3/14 vs 1/7 in the treatment).

### 2. Where useful moves disappear

| arm, class | n | sweeps (median) | contested % | winner = min raw % | winner = min max % | some loser lower raw % | some loser lower max % | useful / moved, all workers % | discarded expenditure % | container commits per sweep, all workers / winner (retained share %) | best winner max / best loser max over the attempt, mm | sweeps with a loser < 50 um while the winner is not |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A bite 5 published | 1 | 479 | 100.0 | 38.8 | 24.2 | 61.2 | 75.6 | 98.0 | 87.52 | 4.17 / 0.76 (18.2) | 0.0006 / 0.0380 | 3 |
| A bite 5 failed | 20 | 296 | 100.0 | 32.6 | 23.6 | 67.4 | 76.1 | 99.2 | 87.47 | 7.82 / 1.38 (17.7) | 1.2431 / 1.2061 | 0 |
| B bite 5 published | 7 | 416 | 100.0 | 32.2 | 25.3 | 67.8 | 73.9 | 97.3 | 87.48 | 10.11 / 1.44 (14.3) | 0.0017 / 0.0026 | 9 |
| B bite 5 failed | 14 | 808 | 100.0 | 32.9 | 24.5 | 67.1 | 75.4 | 97.4 | 87.47 | 12.51 / 1.77 (14.1) | 3.5193 / 3.4355 | 0 |
| B bite 6 failed | 7 | 406 | 100.0 | 34.0 | 25.4 | 66.0 | 74.4 | 98.5 | 87.53 | 14.14 / 1.97 (13.9) | 4.3446 / 4.1740 | 0 |

Sources: `moves-economics.py`, `moves-aggregate.py`, `moves-disagreement.py`, `moves-losers.py`
(band = 0.004 mm; chance rates under guided-only ranking of 8 workers: winner = min raw 12.5 %,
some loser lower raw 87.5 %; per-class values are medians over attempts, the container and
expenditure columns pooled over sweeps).

The tournament is exercised on all 23,697 sweeps and by construction discards seven eighths of
all committed work in every attempt, published or failed, in both arms: the retained worker's
evaluation share is 12.44-12.60 % and the discarded expenditure 87.40-87.56 % on every attempt
(87.47-87.53 % pooled per class),
and "useful" is operationally "moved" (95.1-99.95 % of committed moves qualify), so
`usefulMovesDiscarded` counts the losers' moves and carries no information about their value. The
guided objective disagrees with raw and with the max residual on about two thirds of sweeps (the
winner is the min-raw worker on about a third, the min-max worker on about a quarter; when it is
not, the min-raw worker's raw is 20.6 % (A) / 27.2 % (B) below the winner's at a guided excess of
16.2 % / 11.1 %, pooled over the failed fifth cuts), at the same rates in published and failed attempts, in both arms and in every
quartile of the attempt, so the tournament's behaviour does not distinguish the attempts that reach
the band from those that do not. No discarded worker ever ended a sweep near the band in any of the
42 failed attempts (0 sweeps with any worker's max below 50 um, also at 20 and 4 um); the best state
a loser reached is within about 5 % of the best state the tournament retained (ratio 0.955 A,
0.961 B), and the only sweeps on which a loser was inside the band while the winner was not (2 at
4 um, 12 at 50 um) occur in attempts that published anyway. Container commits, the long-range
moves, are more frequent under p = 1 (12.51 vs 7.82 per sweep, B above A on 18 of 21 seeds) and the
control's tournament is more selective for them (retained share 17.7 % vs 14.1 %, A above B on 21
of 21); their supply is flat across the quartiles of the failed fifth cuts (7.5-8.0 and 12.1-13.2 per
sweep), rises over the treatment's sixth cuts (11.8 to 15.7), and falls only in the published ones as
the state nears the band. What the counters cannot establish, as review 8 warned:
that selection discards states useful for subsequent feasibility. A loser's lower one-sweep raw or
max is an endpoint, the losers' poses are not in the documents, and no controlled continuation from
a discarded state exists in this archive; the readable data contains no candidate "discarded
promising release", which does not refute the hypothesis.

### 3. What releases accomplish, the parents near 160.6 mm, and the origin x objective cross

| bite-5 entry, by arm and outcome (q1 / median / q3) | n | parent published at s | allowance left s | pieces moved by the cut | entry rows | pair / edge rows | pieces in rows | residual median mm | rows >= 0.9 shrink | components | largest component (pieces) | B-T spanning at entry | iterations | min raw mm^2 | strikes > 0 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A published | 1 | 1.63 | 5.87 | 37 | 48 | 36 / 12 | 48 | 2.77 | 11 | 6 | 33 | 0/1 | 479 | 0.00 | 0 |
| A failed | 20 | 2.01 / 2.68 / 3.68 | 3.82 / 4.82 / 5.49 | 35.75 / 36.5 / 37 | 43.75 / 46 / 50.5 | 38 / 8 | 49 / 50 / 51 | 2.58 / 2.80 / 3.24 | 9 / 11 / 14 | 5 / 7 / 10 | 20 / 24 / 33.5 | 11/20 | 228 / 296.5 / 397.75 | 6.60 / 23.32 / 34.97 | 1 |
| B published | 7 | 0.80 / 0.84 / 1.16 | 6.34 / 6.66 / 6.70 | 36 / 36 / 37 | 40.5 / 45 / 46 | 38 / 6 | 45 / 49 / 50 | 2.94 / 3.04 / 3.23 | 9.5 / 10 / 13.5 | 7 / 7 / 8 | 19 / 24 / 27.5 | 1/7 | 176 / 416 / 614 | 0 / 0 / 0 | 0 |
| B failed | 14 | 0.76 / 1.03 / 1.21 | 6.28 / 6.47 / 6.74 | 35.25 / 36 / 36.75 | 38.5 / 41 / 44.25 | 34.5 / 7.5 | 46 / 47 / 49.75 | 2.86 / 3.01 / 3.36 | 8.5 / 11.5 / 12.75 | 7 / 7.5 / 9 | 14.25 / 21.5 / 26.75 | 3/14 | 750.25 / 808.5 / 846.5 | 30.47 / 50.32 / 81.22 | 5 |

| parent origin x continuation objective, bite 5, band entry within the live horizon (21 entries per arm) | continuation p = 1 | continuation p = 2 |
|---|---|---|
| parent made by arm A (p = 2) | 1/21 (replay; 0.20-1.03x the live evaluations, median 0.52; seed 28 at iteration 274 of 487, 6.69 M evaluations against the live p = 2 attempt's 33.41 M without band) | 1/21 (live) |
| parent made by arm B (p = 1) | 7/21 (live; evaluations to band 4.4, 4.6, 5.5, 14.6, 15.1, 17.0, 23.4 M) | 5/21 (replay; 0.74-3.19x the live evaluations, median 1.89 on the failed and 2.45 on the published capsules; three live successes at 11.1-26.1 M and two live failures, seed 29 at 662 of 918 iterations / 37.3 M and seed 6185102792240140886 at 424 of 723 / 27.3 M; two more failures end at the horizon with max 0.0118 and 0.0128 mm at 43.9 M and 39.3 M) |

Budget-matched readings: B entries under p = 2 enter the band within 33.4 M evaluations (the
largest live control budget) on 4/21 against the control's live 1/21; A entries under p = 1 with
5.4-25.2 M evaluations gave 1, where the treatment entries' evaluation-to-band distribution would
predict 3.38, and the four A entries with at least 14.6 M gave 0. Sources: `parents-entry.py`,
`parents-crossarm.py`, `parents-pieces.py`, `parents-replays.py`, `parents-extra.py`,
`parents-v2.py`; releases from `persists-5-minima.py`, `persists-10-rollbacks.py`,
`synth-1-reconcile.py` (e); the budget multipliers are the full ranges from `synth-2-budget.py`
(two analysts had quoted 1.25-2.88x and 1.3-3.2x for the treatment-made capsules, and 0.41-0.63x for
the control-made ones, from subsets or quartiles).

What the cut creates is the same in every cell: about 36 pieces on one side of the half-depth
split are translated by the full 5.14 mm shrink, producing 33-63 positive rows of which almost all
pair rows cross the split (1510 of 1560 over the 42 entries) plus top-edge rows on moved pieces, and
the moved pieces land on the largest pieces, which stay put (eight pieces, among the twelve largest
in the fixture by bounding box (piece 14, a star, ranks 15th by polygon area), are in every one of
the 42 entries and are moved by the cut in at most 10 of 42),
while the specific rows do not recur (763 distinct, none in more than 21 of 42). Within each arm
nothing measured at entry separates the continuations that publish from those that fail (parent
publication time, allowance, rows, residuals, components, largest component and B-T span all
overlap; in the 180 scored cells the treatment's success seeds rank 7-18 of 30 by parent time and
39 of 75 failures have both an earlier parent and more allowance than the latest success), and the
outcome is visible only afterwards. The same seed's two parents are unrelated layouts (bite-1
parent fingerprints equal on 21/21 seeds, placement fingerprints on 0/21; at bite 4 no piece has an
identical pose on any seed; same-seed entry-row Jaccard 0.05 equals cross-seed 0.05-0.06), so
"seed" carries no inherited geometry into the fifth cut and parent origin is arm-made. The cross
therefore separates continuation objective from parent, not the objective's upstream contribution
to the parent: under both objectives the treatment-made parents enter the band more often (7/21
and 5/21 against 1/21 and 1/21), the continuation objective alone does not separate (7/21 vs 5/21
on B parents with about twice the evaluations, 1/21 each on A parents), and four treatment
successes reach the band only under p = 1, three under both, two treatment failures and one
control failure only under the other objective: an interaction pattern on a sample too small to
rank, confounded by the horizon asymmetry (the other objective gets 0.20-1.03x the live evaluations
from A entries and 0.74-3.19x from B entries). The control's exceptional seed is a reverse example:
its A parent (1.63 s, 5.87 s left, 48 rows, no B-T chain, all edge rows on T) publishes at 479
iterations under p = 2 and does not approach the band under p = 1 (min raw 105.6 at 0.63x the
evaluations), while its B parent (1.47 s, 6.03 s left, 47 rows, a B-T-spanning component of 35
pieces) fails under both, even with 2.36x the live evaluations under p = 2 (best max 1.207 mm).
What the releases accomplish is read from the chain of new minima: the control's chain stops at
iteration 8.5 [5-485] (8 new minima per attempt) and each of its 18 rollbacks restores the argmin-raw state of its time
only for the next sweep to stand at 13.85 mm against 1.27 mm restored (above 2x on 14 of 18); the treatment's
chain continues slowly to iteration 609.5 [206-890] (7.5 new minima), its 45 rollbacks in the failed
fifth cuts (recomputed; one analyst's 56 pooled the sixth cuts) restore 4.13 mm and the next sweep
stands at 4.19 mm (above 2x on 1 of 45, above 5 mm on 14 of 45); the sixth cuts, the parents near
155.5 mm, all fail far from the band (best max violation 0.93-4.81 mm, three of seven treatment
sixth cuts with their best raw at iteration 2-3), so there is no successful sixth-cut comparison,
as review 8 anticipated. What the numbers cannot say: the documents carry poses but no polygon
geometry, so near-contact structure before the cut and the translation and rotation freedom around
the spanning components were not computed; the piece-size ranks assume fixture order equals engine
index; and with 1 control and 7 treatment successes the associations are exploratory.

### 4. Why the attempt ends

| quantity, bite 5 (21 attempts per arm) | A (p = 2) | B (p = 1) |
|---|---|---|
| stop reasons | 20 deadline / 1 published | 14 deadline / 7 published |
| strikes per attempt | 0 x20, 1 x1 | 0 x16, 1 x3, 2 x2 |
| rollbacks per attempt | 0 x4, 1 x15, 2 x2 | 0 x3, 1 x2, 2 x2, 3 x10, 4 x4 |
| iterations min / med / max | 129 / 310 / 582 | 161 / 744 / 922 |
| evaluations, all workers, min / med / max | 10.57 M / 23.50 M / 33.41 M | 4.39 M / 26.81 M / 30.12 M |
| evaluations per iteration, all workers, med | 76,490 | 34,806 |
| iterations per second, med | 62.5 | 126.1 |
| wall left at entry, s, min / med / max | 2.067 / 4.980 / 6.702 | 4.730 / 6.472 / 6.929 |
| wall left at stop, deadline stops | -0.018 .. -0.001 | -0.009 .. -0.000 |
| deadline stops: best raw mm^2, min / med / max | 0.195 / 23.3 / 52.7 | 0.557 / 50.3 / 106.0 |
| deadline stops: best max violation mm, min / med / max | 0.207 / 1.243 / 1.509 | 0.605 / 3.519 / 4.398 |
| deadline stops: fraction of the attempt at which the best raw was reached, min / med / max | 0.02 / 0.04 / 1.00 | 0.27 / 0.78 / 1.00 |
| deadline stops: iterations / evaluations spent after the best | 3,789 of 6,284 (60.3 %) / 284.7 M of 445.7 M (63.9 %) | 3,147 of 11,160 (28.2 %) / 111.2 M of 388.3 M (28.6 %) |
| deadline stops: last raw / best raw, min / med / max | 1.5 / 22.7 / 1274 | 1.0 / 61.4 / 2096 |

| classification of the 34 deadline stops (exclusive; best raw = the handed-back state) | A | B | seeds |
|---|---|---|---|
| best raw in the last 10 % of iterations | 2 | 4 | A 14782797682586776746 (2.096 mm^2 at 408 of 415), 8755853977552987277 (0.742 at 485 of 487); B 10636268072709740349 (10.663 at 811 of 899), 12153648923418518200 (0.557 at 769 of 823), 2586638353860241226 (19.474 at 890 of 922), 33 (28.044 at 744 of 744) |
| best raw in [50 %, 90 %) | 4 | 6 | A 31 (0.195 at 386 of 582), 5671471283886933426 (8.101 at 244 of 310), 35 (0.794 at 301 of 347), 34 (0.822 at 352 of 392); B 6185102792240140886, 28, 35, 30, 32, 14782797682586776746 (37.7-106.0 mm^2) |
| best raw before the middle (plateau) | 14 | 4 | A: 13 seeds with the best at iteration 5-10 (19.0-52.7 mm^2) and 28 (11.489 at 217 of 487); B 8755853977552987277 (at 208 of 770), 7797222088981460957 (206 of 566), 29 (407 of 918), 34 (407 of 851) |
| crossed raw < 1 mm^2 with less work left than the slowest success needed from there (133 iterations / 4.65 M) | 3 | 1 | A 34 (44 it / 2.09 M left), 35 (48 / 1.58 M), 8755853977552987277 (3 / 0.12 M); B 12153648923418518200 (54 / 1.66 M); A 31 crossed with 413 / 12.2 M left and stalled |

Sources: `ends-attempts.py`, `ends-summary.py`, `ends-threshold.py`, `ends-trajectory.py`,
`ends-replays.py`, `ends-v2.py`; `synth-1-reconcile.py` (b), (c), (d); `synth-2-budget.py`.

Every failed attempt ends at the phase deadline with the allowance consumed to within 18 ms, none
on strikes (the explore limit is 3; the maximum seen is 2), with no band entry and no exact call,
and what it hands back is its best-raw state, which is also its last new minimum (42 of 42). The
control reaches that state early: 13 of its 20 deadline stops have their best raw at iteration 5-10
(19.0-52.7 mm^2, max violation 1.21-1.51 mm) and never beat it, so 60 % of its failed iterations and
64 % of its failed evaluations are spent after the returned state was already in hand; the treatment
plateaus at 10.7-106 mm^2 (max violation 1.9-4.4 mm) in 13 of 14, reached late (0.78 of the attempt
at the median) in small steps after each rollback, with only 12153648923418518200 below 1 mm^2. No
trajectory is monotone: in all 42 attempts the winner's raw falls for the first 4-10 iterations and
then, at iteration 5-11, jumps above twice the running minimum on a sweep where the winner's
largest relocation is 1.2 m at the median (1.23 m A, 1.20 m B; below 25 mm in 3 of 42) instead of the
7-9 mm median of the sweeps before it, while another worker still held a
state below twice the running minimum in 39 of 42 attempts; the documents do not say why the
guided objective preferred that state. "Insufficient remaining work" is supported strictly for two
attempts (A 8755853977552987277 and A 14782797682586776746, last new minimum 2 and 7 iterations
before the stop at 0.742 and 2.096 mm^2), undecidable for three (A 34, A 35, B 12153648923418518200,
which crossed raw < 1 with 44-54 iterations left, more than the median success needed from there
and less than the slowest), contradicted for A 31 (raw 0.195 at iteration 386, then 196 iterations
without a new minimum), and not supported for the other 28, none of which got below 8 mm^2. The
replays show that more p = 2 work than any live control attempt receives does reach the band on
some capsules: p = 2 on the treatment's fifth-cut capsules (27-61 M evaluations on the 14 failed capsules, 1.02-2.36x
the live evaluations; 11-49 M on the 7 published) enters on 5 of 21 including two live failures and ends within 0.013 mm of the band on two more; p = 1 on the control's
capsules (5.4-25.2 M) enters on 1 of 21. What cannot be read: the winner-selection rule (argmin
guided, 23,697 of 23,697) and the rollback rule (restore to argmin raw, 83 of 83; the rollback falls 200 iterations after
the last new minimum or rollback on 78 of 83, 196-199 on the other five) are
inferred from the data and not from the engine; the guided objective is evaluated under weights that
change every iteration and is not a cross-iteration progress measure; and the degradation of the
winner's trajectory after its best (last raw above 2x best in 32 of 34) is in the abandoned
trajectory, since the cell discards the failed explore state and compresses from the previous
parent (review 8, Q21).

### What this supports

Ranked strictly by what the tables show. First, conflict transfer or recreation: review 8's
criterion was that retained moves release the original blockers, new neighbouring blockers
repeatedly replace them, the overall obstruction persists, and discarded alternatives show no
durable advantage. The first three are shown: every entry row is released (0 continuous at sweep 100 in the control, at sweep 20 in the treatment), 39 of 46 and 40 of 41 entry rows re-form, a
bottom-to-top spanning component is present at the handed-back state of every failed attempt (and
at every new minimum of all but two) and absent at every published stop, the largest component keeps 43 of 50 (A) and 29.5 of 47 (B) of
the entry's pieces under new rows, and the handed-back states carry a largest residual of about a
quarter of the cut (control, 1.26 mm at the median; a bottom-piece-piece-top chain of three such rows
on 4 of 20) or a largest row of 3.3-6.4 mm on 12 of 14 (treatment). The fourth is
shown only at the endpoint: no discarded worker was ever within 50 um of the band in a failed
attempt and the best loser state is within 5 % of the best retained one; "durable" is not testable
without a continuation. Second, insufficient remaining work: supported for 2 of 34 deadline stops,
undecidable for 3, contradicted for 1 and not supported for 28, so it explains the margin, not the
class; the separate finding that p = 2 with 0.74-3.19x the live evaluations enters the band from 5 of
21 treatment-made capsules is a statement about more work under the other objective from those
parents and is kept distinct from equal-work comparisons, as review 8 required. Third, selection
discards useful states: neither supported nor refuted. The counters cannot establish it by
construction (seven eighths discarded in every attempt, "useful" equals "moved", the raw-versus-guided
disagreement is identical in published and failed attempts), and the readable data contains no
candidate discarded promising release; it remains open only because the losers' complete states are
not in the documents. If it is to be closed, the next step is the decisive diagnostic review 8 named:
capture every worker's complete state at a selection boundary, branch from that boundary retaining
either the actual winner or a specified alternative (the min-raw or min-max loser), and continue
under the same policy and budget with complete states, never combining moves from different
workers. The natural boundary this reading points at is the jump sweep at iteration 5-11, where the
control's chain of minima stops and another worker still held a state below twice the running
minimum in 39 of 42 attempts. That capture is not in today's microscope documents; it would be a
new instrument option, not a re-reading of these files.

Commands (all stdlib python3, read-only on the data; cd `/var/lib/t3/tmp/astra/deep/analysis` first):
`python3 persists-1-census.py && python3 persists-2-report.py && python3 persists-3-dynamics.py &&
python3 persists-5-minima.py && python3 persists-8-continuous.py` (section 1);
`python3 moves-economics.py && python3 moves-aggregate.py && python3 moves-disagreement.py &&
python3 moves-losers.py` (section 2); `python3 parents-entry.py && python3 parents-crossarm.py &&
python3 parents-pieces.py && python3 parents-replays.py && python3 parents-extra.py && python3
parents-v2.py` (section 3); `python3 ends-attempts.py && python3 ends-summary.py && python3
ends-threshold.py && python3 ends-trajectory.py && python3 ends-replays.py && python3 ends-v2.py`
(section 4); `python3 synth-1-reconcile.py && python3 synth-2-budget.py` (the reconciliations named above);
`python3 verify-0-preamble.py && python3 verify-1-persists.py && python3 verify-1b-persists-text.py &&
python3 verify-1c-reformation.py && python3 verify-2-moves.py && python3 verify-2b-moves-ties.py &&
python3 verify-2c-gap-quartiles.py && python3 verify-3-parents.py && python3 verify-3b-rollbacks.py &&
python3 verify-3c-misc.py && python3 verify-4-ends.py && python3 verify-4b-jump-pieces.py` (the
independent recomputation of every table and text number above, from the documents only).
