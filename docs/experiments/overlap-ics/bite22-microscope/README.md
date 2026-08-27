# The bite-22 microscope: what the 179 shelf actually is

The deterministic-30s round passed its 30-second gate and failed the
ten-second last chance at 2/9, median `179.07170 mm`, which permanently
retired the ten-second quality gate. This directory is the read of that
round's own per-cell data that asks *where the ten seconds went*, plus one new
instrument built to answer the question the existing funnel could not.

Nothing here changes a trajectory. `ics-publish-census` is a counter-only
feature, off by default; the default `overlap-ics` build is unchanged and the
four pinned gates reproduce on it.

## 1. The shelf is one bite, not a plateau

Composed arm, repetition 0, explore-bite count and published depth, from
`deterministic-30s-round/evidence/{curve3,gate10,curve30,curve60}.json`:

| seed | 3 s | 10 s | 30 s | 60 s |
| ---: | --- | --- | --- | --- |
| 0 | 21 b / 179.0314 | 46 b / 170.4418 | 120 b / 160.8923 | 138 b / 159.2463 |
| 1 | 19 b / 179.5295 | **21 b / 179.0810** | 99 b / 165.0423 | 117 b / 162.4350 |
| 2 | 21 b / 179.0796 | 85 b / 166.8448 | 125 b / 161.0390 | 134 b / 159.6170 |
| 3 | 21 b / 179.0564 | 99 b / 165.4249 | 115 b / 160.9295 | 133 b / 159.8740 |
| 4 | 21 b / 179.0812 | **21 b / 179.0812** | 115 b / 162.9424 | 132 b / 159.8850 |
| 5 | 21 b / 179.0717 | **21 b / 179.0717** | 118 b / 161.3374 | 136 b / 159.4520 |
| 6 | 21 b / 179.0543 | 74 b / 169.3496 | 108 b / 164.0009 | 120 b / 161.7600 |
| 7 | 21 b / 179.0821 | **21 b / 179.0821** | **21 b / 179.0821** | 122 b / 160.4250 |
| 8 | 21 b / 179.0821 | **21 b / 179.0821** | **21 b / 179.0821** | 80 b / 167.5630 |

**Read the 3-second column as a budget, not a stall.** Those cells used
`searchSeconds` 0.595-0.61 against a `searchBudgetSeconds` of 0.672: they ran
out of time at bite 21, they did not stop there. The claim this directory
makes rests on the 10- and 30-second columns, where seeds 7 and 8 hold
*exactly* 21 explore bites, 2 compress bites, 22 publications and the same
depth to four decimals while consuming 28.15 s and 27.10 s of search, 23 and
19 strikes, 7 and 6 disruptions. Seed 3 at 10 s, for contrast: 99 bites, **0
strikes**.

The *constructor* is one layout for every seed (`182.976 mm`, fingerprint
`a791c397...`; Round 4 Gate 0 fixed `orders=1` as bit-identical to `orders=4`),
and the first bites are cheap. Bite 22 is the first real problem, and whether a
run clears it decides most of the ten seconds.

**Four corrections both reviewers made to the first write-up of this
directory, kept here because they change what may be claimed** (`sol-review-21`
§1, `grok-review-16` §1):

1. **The 21-bite parents are not one layout.** Their nine `placementFingerprint`
   values are distinct; seed 7 (`cca8...`) and seed 8 (`32c9...`) share a depth
   and not a geometry. The prefix is *cheap*, not seed-identical, and two seeds
   already spend a disruption inside it. This is a common barrier by ordinal and
   depth on already-divergent basins, not one wall in front of one layout.
2. **Not every seed reaches 21 bites.** Seed 1 is at **19** at 3 s. It finishes
   the prefix somewhere between 3 s and 10 s and freezes at 21 there.
3. **The escape-time span is about 30.5x, not three orders of magnitude.** The
   measured work on bite 22 runs from 2.278 M evaluations (seed 3, closes it) to
   69.56 M (seed 8 at 60 s). The "under 0.1 s" in the first draft was not
   derived from any cell.
4. **Clearing bite 22 is necessary, not sufficient.** 168.484/182.976 at 0.1 %
   is about **83** successful explore bites. Seeds 0 and 6 won the 22-lottery at
   10 s - 46 and 74 bites - and still finished at 170.44 and 169.35. If all five
   frozen seeds escaped exactly as seed 0 did, the 10 s 5/9 clause would still
   fail. The 30 s pass is "seven of nine won the lottery inside 30 s"; that
   sentence may not be imported back onto 10 s.

## 2. Where the iterations go

`bite-records-seed7.json` and `bite-records-seed3.json`, wall mode, 10 s,
`--orders=1 --workers=8 --edge=5 --pair=5`, reproduce the round exactly: seed 7
finishes at `179.0821` on 21 explore bites, seed 3 at `165.44` on 98.

Seed 7's bites 1-21 cost **1 to 18 master iterations each** and every one
publishes with `minRawPhi` exactly `0.0`. Bite 22 costs **1,103 iterations, 2
strikes, `minRawPhi` `8.06e-05`, and does not publish**. Compress bite 24
costs 260 more, enters the proxy band **114 times**, and calls the exact
authority **zero times**.

Totals: seed 3 spends **1,263 master iterations over 110 bites**. Seed 7
spends **1,440 over 24** - and **1,363 of those 1,440, 95 %, go to the two
bites that never publish**. In the round's own 10-second cell for seed 7, bite
22 records `masterIterations: 7262` with `exactCheckpointCalls: 0`.

## 3. Which gate refuses - the new instrument

`attempt_publication` has three early returns before the exact authority is
ever called: the pose-digest guard, `proxy_depth > target_depth_mm`, and the
improvement gate. The funnel counts band entries and exact calls but cannot
say which of the three consumed the difference. `ics-publish-census` counts
them at the band entry, using the same three predicates, reading only.

All nine seeds, 10 s, wall mode (`g-seed*.json`, field `publishCensus`):

| seed | depth | band entries | above target | called | **above-target refusals that beat the incumbent** | best gain thrown away |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 169.7166 | 1100 | 766 | 184 | **766** | 0.179902 mm |
| 1 | 179.0810 | 40 | 11 | 28 | **11** | 0.177370 mm |
| 2 | 164.8791 | 1331 | 1038 | 274 | **1034** | 0.179310 mm |
| 3 | 165.3374 | 386 | 157 | 217 | **157** | 0.177384 mm |
| 4 | 179.0812 | 212 | 181 | 31 | **181** | 0.180290 mm |
| 5 | 175.0016 | 692 | 628 | 64 | **628** | 0.085847 mm |
| 6 | 169.1409 | 973 | 618 | 235 | **618** | 0.176035 mm |
| 7 | 179.0821 | 67 | 45 | 22 | **45** | 0.175196 mm |
| 8 | 179.0821 | 462 | 435 | 27 | **435** | 0.179719 mm |

The pose-digest guard fires 0-3 times in a whole run; the improvement gate
never fires. **`proxy_depth > target_depth_mm` is the whole difference**, and
the excess is between `1.5 um` and `4.0 um` - bounded above by the 4 um band,
because the depth overhang *is* a boundary violation and the band admitted it.

## 4. What that means

The explore step is 0.1 % of 179 mm, `0.179 mm`. The refused layouts are
**0.175-0.180 mm better than the incumbent**: a full bite of real, measured
progress. They are discarded for missing the bite's own self-set target by one
and a half micrometres, and the engine then spends thousands of iterations
failing to recover those micrometres. When it gives up, `mod.rs`'s
`None => break` ends the entire explore phase and the run is over at 179.

Whether a descent lands a micrometre under the target or a micrometre over it
is one of the coin flips that decide a ten-second run - and per correction 4
above, not the only one. That is part of why the same binary, from the same
constructor, produces 160.89 on seed 0 and 179.08 on seed 7.

The success path already adopts the **achieved** depth rather than the
targeted one (`depth_mm = publication.raw_source_depth_mm`), so the machinery
for a partial bite exists; only the pre-gate refuses to reach it.

This directory states the measurement and stops there. Whether the target
pre-gate should be relaxed, and under what pre-committed gate, is a quorum
question and a new specification's business - the ten-second gate was retired
under a rule that only a genuinely new mechanism may reopen.

Both reviewers also refuse the first draft's suggestion that `mod.rs:2002`'s
`None => break` is the defect, on the same isomorphism: the analogue of
Sparrow's `while !term.kill()` is our **per-width inner loop**, which pools,
restores, disrupts and re-separates at the same `W`, with
`attempts_per_bite = 0` meaning unlimited under the Calibrated pacer. The outer
`None` is reached only after the deadline or phase exhaustion. The real and
deliberate difference is that Sparrow advances on proxy loss zero while this
member advances only on a dual-valid publication. Line 2002 is not why five
seeds sit at 21 bites.

## Reproducing

```
cargo build -p polygon-nesting-core --release \
    --features overlap-ics,ics-publish-census --example overlap_ics_benchmark
target/release/examples/overlap_ics_benchmark --cell=cutclose \
    --request=tests/fixtures/mixed-61/mixed61-request-exact-clearance.json \
    --edge=5 --pair=5 --mode=wall --wall=10.0 --orders=1 --workers=8 \
    --arm=control --seed=7
```

The committed cells are that command's document with the pose, publication and
fingerprint arrays dropped and the per-bite `profile`/`strikeMeter` blocks
removed; every counter this README cites is retained verbatim.

## 5. The frozen five at two clocks, and which mechanism the numbers pick

`evidence/frozen-five/b-s{1,4,5,7,8}-w{10,30}.json`, same command with
`--wall=10` and `--wall=30`. This was measured after the first cross-exchange
went out, to answer the reviewers' split: one proposed repairing the **exit**
(the terminal depth lip) and the other improving the **entry** (a sequential
pass before the first `separate()` of a bite).

| seed | 10 s depth | bites | band entries | above target | 30 s depth | bites | band entries | above target |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 179.0810 | 21 | 34 | 4 | 165.0150 | 99 | 1885 | 1654 |
| 4 | 179.0812 | 21 | 197 | 171 | 164.0011 | 109 | 1639 | 1373 |
| 5 | 168.3083 | 37 | 381 | 92 | 162.5926 | 117 | 918 | 462 |
| **7** | **179.0821** | **21** | 254 | 232 | **179.0821** | **21** | **5235** | **5212** |
| **8** | **179.0821** | **21** | 282 | 255 | **179.0096** | **21** | 3060 | 3026 |

Seed 7 at 30 s reaches the publication band **5,235 times** and is refused on
the depth lip **5,212 of them - 99.6 %** - and still holds 21 explore bites.
Seed 8: 3,060 and 3,026, 98.9 %. They are not entry-limited at that clock;
they are entirely lip-limited, with thousands of shots and nothing to show.

Seeds 1, 4 and 5 escape by 30 s on their own. That is the reading that picks
the discriminating clock: at a 30 s leftover budget the control already closes
three of the five, so a "3 of 5" conversion clause would be vacuous there and
must be measured on the 10 s leftover instead - which is where the two arms
actually differ.

**Honesty about variance.** Wall mode is not the round's deterministic
calibrated pacer, and single wall runs move: seed 5 printed `175.0016 mm` at
41 bites in the section-3 sweep and `168.3083 mm` at 37 bites here. The
structural facts this directory rests on - thousands of band entries, ~99 %
refused on the lip, `exploreBites` frozen at 21 - are far outside that
variance; individual decimals of a single wall cell are not evidence and are
not used as any clause.
