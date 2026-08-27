# Where the engine's time actually goes

The wall iteration cap works because it buys restarts. That raises a sharper
question: **why does the engine need a cap at all, when Sparrow's explore loop
has the same shape and no such bound?** Their `exploration_phase` is
`while !term.kill()` and their `separate` is
`while n_strikes < strike_limit && !term.kill()` - no iteration bound anywhere.
If the structure is the same and the result is not, the difference is how many
iterations fit in ten seconds.

So: measure the engine's throughput, and then measure what one iteration is
spent on.

## Throughput

Nine ten-second cells at `cap = 50, ratio = 0.95`, from their own counters:

- **160.1 master iterations per second**
- **2.26 M sample evaluations per second**
- **14,119 sample evaluations per master iteration**
- **~2.7 us per sample evaluation** (`sweepTotal / evaluations`)

## The phase profile

Three ten-second cells built with `ics-profile` (`evidence/prof-*.json`):

| phase | share of barrier-to-barrier | ns per master iteration |
| --- | ---: | ---: |
| **worker sweep, critical path** | **95.6 %** | 7,676,268 |
| prep + dispatch | 3.3 % | 265,832 |
| exact authority and repair | 0.7 % | 52,773 |
| merge and GLS | 0.2 % | 19,046 |
| snapshot, residual, band fold | 0.2 % | 15,168 |

`sweepTotal` over `barrierToBarrier` is 610 %, so the eight workers deliver
**6.1x** of parallel speedup - 76 % efficiency. Everything that is not the sweep
sums to under 5 %. **The engine is the sweep.**

## What one candidate evaluation is made of

From the same cells' work vector, per sample evaluation:

| | per evaluation |
| --- | ---: |
| pose transforms | 1.00 |
| broad-phase box tests | **60.3** - one for every other piece |
| of which rejected | **93.1 %** |
| surviving pairs | 4.15 |
| convex cell-gap queries | 9.1 |

And timed directly, clock outside the loop, 300,000 rounds per figure
(`--cell=evalcost`, `evidence/ev2-*.json`):

| piece | transform | **O(n) scan floor** | pair geometry | fold | total |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 32 ns | **477 ns** | 146 ns | 86 ns | 741 ns |
| 5 | 29 ns | **473 ns** | 186 ns | 88 ns | 776 ns |
| 20 | 33 ns | 355 ns | **1327 ns** | 75 ns | 1789 ns |
| 40 | 19 ns | **491 ns** | -29 ns | 79 ns | 561 ns |

The scan floor is measured by moving the piece a hundred metres outside the
sheet so that every box test rejects and no pair survives. What remains is the
`O(n)` loop itself.

**Three readings, and the first two refute the obvious guesses:**

1. **The transform is not the cost.** Re-transforming the whole decomposed
   polygon for every candidate costs **19-33 ns**, three per cent of an
   evaluation. The pieces are small and the arithmetic is cheap. The
   "transform-then-query instead of query-driven transform" theory is wrong
   here.
2. **The broad phase is not saving what it appears to save.** It rejects
   93.1 % of pairs, but the rejection *is* the cost: `rebuild_piece_rows` walks
   all `n-1` other pieces on every candidate, and that walk alone is
   **355-491 ns**, which on three of the four pieces is **64-87 % of the whole
   evaluation**.
3. **Crowded pieces are a different regime.** Piece 20 spends 1,327 ns in pair
   geometry - the cell-by-cell gap queries of the pairs that do survive - and
   there the `O(n)` scan is a third of the cost rather than most of it.

`incident_totals` is `O(n)` for the same reason: it folds all `n-1` pair rows of
the piece, at 75-88 ns.

## The lever this identifies

The two `O(n)` parts - the scan in `rebuild_piece_rows` and the fold in
`incident_totals` - are together about **560 ns of a typical 740 ns
evaluation, 75 %**. A maintained near-set, so that a candidate touches its
~8 real neighbours instead of all 60, would take them to roughly 80 ns.

That is a **~2.5x speedup on a typical evaluation**, with no change to any
result: the pairs it skips are exactly the ones the box test already zeroes.
And 2.5x more iterations at ten seconds is the currency this campaign has just
finished proving buys depth - the cap works by converting the same seconds into
more restarts, and this converts the same seconds into more of everything.

It is not implemented here. This directory measures and stops.

## Reproducing

```
cargo build -p polygon-nesting-core --release \
    --features overlap-ics --example overlap_ics_benchmark
target/release/examples/overlap_ics_benchmark --cell=evalcost \
    --request=tests/fixtures/mixed-61/mixed61-request-exact-clearance.json \
    --edge=5 --pair=5 --orders=1 --seed=0 --rounds=300000 --piece=20
```

---

# The near set: implemented, bit-identical, and much smaller than the bench promised

`IcsState` now carries `near: Vec<Vec<u32>>` - for each piece, the others whose
pair row with it is non-zero, ascending. The invariant is maintained at the only
two sites that write a violation, `rebuild_all` and `rebuild_piece_rows`; every
other writer touches `weight` alone. `rebuild_piece_rows` zeroes only the rows it
owned and writes only the rows that survive, and `incident_totals` folds
`near[piece]` instead of walking all `n-1`.

**It is exact, not approximately exact.** Both functions already skipped zero
rows, so visiting the same non-zero rows in the same ascending order gives the
same sum bit for bit. Verified against the frozen pre-change binary on three
fixed-work cells, whole documents with the wall stripped:

| seed | frozen | near set | document digest |
| ---: | ---: | ---: | --- |
| 0 | 178.849978 | 178.849978 | **identical** |
| 3 | 178.829998 | 178.829998 | **identical** |
| 7 | 179.036238 | 179.036238 | **identical** |

839 `overlap-ics` unit tests and 1,104 workspace tests pass.

## And the honest number

On the bench it looks excellent - 1.63x, 1.81x, 1.48x on pieces 0, 5 and 40, and
1.08x on the crowded piece 20. In the real ten-second search it is **1.044x**:

| | master iterations / s | median | best | under-bar |
| --- | ---: | ---: | ---: | --- |
| frozen | 136.7 | 167.687 | 163.440 | 6, 6, 6 |
| near set | **142.7** | 167.687 | 163.362 | 6, 6, 6 |

Three repetitions, nine seeds, twenty-seven cells per arm. The depth does not
move at all, which is what a 4 % throughput change should do.

**The bench was measuring the wrong regime, and the gap between 1.7x and 1.04x
is the finding.** It ran at the constructor's depth, where pieces are sparse and
the `O(n)` walk really is most of an evaluation. The real search spends its time
*inside a bite*, where the layout is overlapping and four or more pairs survive
per candidate - piece 20's regime, where the near set buys 1.08x because the
cell-by-cell gap queries of the surviving pairs dominate everything else.

So the earlier claim in this file - that the `O(n)` parts are 75 % of an
evaluation and a near set is worth about 2.5x - is **wrong for the regime that
matters**. It is true at constructor density and false under the search. The
correction stands where the claim did.

The near set stays: it is exact, it costs nothing, and it removes a real `O(n)`
term. But the lever is confirmed to be `convex_cell_gap` on the surviving pairs,
and that is where the next work goes.
