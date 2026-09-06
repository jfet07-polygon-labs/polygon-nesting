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
