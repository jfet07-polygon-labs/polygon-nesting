# The cut: what the caller never looks at, the primitive never computes

The evaluation-cost work ended by naming its own next lever:

> the lever is confirmed to be `convex_cell_gap` on the surviving pairs, and
> that is where the next work goes.

The axis cache took the first half of that - the outward normals and each
cell's own projection onto them, which a candidate re-derives about nine times
per evaluation. This is the second half, and it is not a cache. It is the
observation that **`measure_pair` asks for a number it usually throws away.**

## What the counters said

Two counters were added to `WorkVector` for this round and are kept, because
nothing else in the vector can distinguish the two branches of the SAT:

- `satSeparatedCalls` - the query ended disjoint, so it paid the
  `O(|a| * |b|)` `closest_feature` segment scan rather than stopping inside the
  axis loop;
- `satDiscardedCalls` - the query returned a gap **at or above the pair
  clearance**, which cannot beat any `worst >= 0`, so the exact answer was
  computed and discarded.

One bare ten-second mixed-61 request at `cap = 50, ratio = 0.95`, seed 0:

| | count | share of queries |
| --- | ---: | ---: |
| `convexCellGapQueries` | 267,570,008 | 100 % |
| ended on the separated branch | 155,900,535 | **58.3 %** |
| gap at or above the clearance | 102,189,692 | **38.2 %** |
| separated **and** wanted | 53,710,843 | 20.1 % |

**Two queries in five are pure waste, and they are the expensive branch.**

## The cut

`measure_pair` keeps the worst violation over the cell pairs and installs a
contact only when `clearance - gap > worst`. So a gap at or above
`clearance - worst` is discarded whatever it is. That number is the **cut**, it
tightens with every violation the scan finds, and it is now handed down to all
three places that can act on it:

1. **the cell-pair box proof** - `box_gap` is a lower bound on the distance, so
   a box gap at or above the cut proves the pair is irrelevant;
2. **the SAT axis loop** - every unit axis carries the gap between the two
   projected intervals, which is also a lower bound on the distance. When one
   clears the cut the query returns `None` and the segment scan never runs;
3. **the segment scan itself** - `hypot(dx, dy) >= max(dx, dy)`, so the larger
   box-gap leg of a segment pair is a lower bound on that pair's distance, at
   the price of two subtractions against a `segment_distance` that costs
   divisions and a `hypot`.

### Two corrections the first implementation needed

**The separating axis may no longer end the loop.** The old code broke on the
first axis that proved disjointness. A later axis can carry a *stronger* bound,
and the stronger bound is what pays for the segment scan, so the loop now runs
on. `touch_axis` still records the **first** separating axis, which is the only
thing `finish_gap` reads from that loop, so the surviving answers are unchanged.

**`box_gap` is clamped at zero.** It proves a distance and says *nothing* about
penetration. Once an overlapping cell pair pushes `worst` past the clearance the
cut goes negative, and comparing a clamped-at-zero box gap against a negative
cut prunes every remaining pair - including a deeper overlap that should have
won. This was caught by the bit-identity check, not by review: the first
implementation diverged on all three fixed-work cells. The box proof now only
reads a positive cut. The SAT's own bound needs no such guard, because on the
separated branch the distance is non-negative and therefore already above a
negative cut.

## Why the bound is a proof and not an estimate

`PRUNE_SLACK_MM = 1e-9`. Every bound pruned on is an `f64` computed from
micrometre-quantised coordinates, carrying a handful of roundings on values no
larger than the sheet, so its absolute error is under `1e-12 mm`. Requiring the
bound to clear the cut by a nanometre is a genuine proof with three orders of
magnitude to spare, and it costs nothing: it only declines to prune a pair whose
gap sits within a nanometre of the cut, and the contract cannot distinguish
those from the cut itself.

## Bit-identity

Three fixed-work cells (`--budget=40000`, seeds 0, 3 and 7, eight workers),
whole documents compared with the timings and the four work counters that
*measure* the pruning stripped:

| seed | depth | final pose digest | SAT queries |
| --- | ---: | --- | ---: |
| 0 | 181.517305 | same | 719,417 -> 715,855 |
| 3 | 181.517305 | same | 916,682 -> 911,622 |
| 7 | 181.517305 | same | 865,033 -> 861,441 |

Everything outside the work counters is byte-identical.

**The query count is the wrong place to look for the win** - it moves by 0.5 %,
because a pruned query still *happened*. The win is inside the query, and its
own counters say so: on seed 0's fixed-work cell the axis bound pruned 285,030
of the 287,595 discardable queries (**99.1 % capture**), `closest_feature` ran
144,813 times instead of 429,843 (**3.0x fewer**), and of 1,917,374 candidate
segment pairs only 533,842 were evaluated (**72 % pruned**) - about **11x fewer
`segment_distance` calls**.

## The measurement that was worthless, and why it is recorded

The first speed reading was a fixed-work A/B: three seeds at `--budget=40000`,
old 2.013 s, new 1.968 s. **1 %.** It was very nearly enough to abandon the
change.

It was measuring nothing. A `--budget=1` run of the same cell takes **0.661 s**:
the entire 0.67 s per seed is process startup, request parsing and the
constructor. Raising the budget a hundredfold to `--budget=4000000` produced the
*same* 2.0 s, which is the tell - `proposal_budget` does not scale the work of
the `cutclose` cell at all.

This is the third time this campaign a bench has answered a question nobody
asked. The near set was measured in the wrong regime; the T-row grid box was
measured against a quantised source; this was measured against a startup. **A
bench that does not move when the work moves is not a bench.**

## The measurement that was worth something

Nine seeds, three repetitions, twenty-seven bare ten-second requests per arm,
`cap = 50, ratio = 0.95` (`evidence/ab-10s-3reps.json`):

| | master iterations per rep | median of 27 | best of 27 | cells under 165 mm |
| --- | ---: | ---: | ---: | ---: |
| `c42ed22`, axis cache | 15,123 | 166.781 | 162.690 | 3 / 27 |
| **plus contact pruning** | **19,557** | **166.173** | **161.441** | **12 / 27** |

**1.293x the iterations, and the depth moved on all nine seeds** - not a median
that improved while some seeds got worse, but nine of nine, paired, mean
**-1.718 mm**:

| seed | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| best of 3, delta mm | -1.250 | -1.942 | -0.728 | -4.504 | -2.745 | -1.739 | -0.684 | -1.654 | -0.221 |

## Then the broad phase, for the same reason

The pruning made the segment scan cheap enough that the next line up the profile
became visible: `box_gap` ends in a `libm::hypot`, and `pair_is_near` calls it
**1.73 billion times in ten seconds**, rejecting 93 %.

None of those callers wants the distance. They want a *comparison*. For
non-negative legs `hypot(dx, dy) >= max(dx, dy)` exactly, and `hypot` is
correctly rounded, so **a single leg at or above the threshold settles the
question with no square root at all** - and on a strip layout the rejects are
overwhelmingly far in one axis. `hypot(0, dy)` is exactly `dy`, so a zero leg
settles it too. Only the corner annulus, both legs strictly inside the
threshold, still pays. `box_gap_below` is that predicate; it is the same
predicate, not an approximation of one.

The same profile line carried a second cost that was nothing to do with
geometry. `measure_pair` is one large body, far too large to inline, so each of
those 1.6 billion rejects paid a call, a prologue and an epilogue in order to do
four subtractions. It is now an `#[inline]` shell holding the counters and the
box proof, delegating to `measure_pair_near` only for the 7 % that survive.

Three seeds, ten seconds, master iterations:

| seed | pruning only | plus `box_gap_below` and the split |
| --- | ---: | ---: |
| 0 | 1,821 | 1,946 |
| 3 | 1,903 | 2,046 |
| 7 | 2,141 | 2,695 |
| **total** | **5,865** | **6,687 (1.140x)** |

Bit-identical on all three fixed-work cells, whole documents, work counters
included - this pair changes no arithmetic at all, only how much of it runs.

## Not done, and why

**A cached candidate list for the row rebuild.** Only `piece` moves during a
coordinate descent, so the set of others its box can reach is stable across the
whole walk, and a guard box makes the cache exact: a piece excluded because the
*guard* clears the clearance also clears it from any pose inside the guard. But
29 % of the engine's evaluations are container and focused *samples*, which jump
across the strip and miss the guard every time, and each miss costs a full
`n`-wide rebuild on top. The arithmetic lands near 1.8x on a broad phase that,
once the `hypot` is gone, is a handful of operations per probe. It was not
worth the invalidation surface.
