# Sparrow warm-started from our layouts: the separator is the whole gap

2026-09-06, on `e4069df`. Sparrow (`/var/lib/t3/tmp/sparrow-bench`, rev `14f4868f`, the x86 build
recorded in `docs/experiments/sparrow-mixed61/README.md`) accepts a solution file as a warm
start (`util/io.rs::read_spp_input` tries `ExtSPOutput` first and falls back silently to the
instance; the file must carry `solution.layout.density`, which is why the first attempt was
ignored). Reading and running Sparrow is authorised; nothing here is ported. Every Sparrow run
below is `--global-time 10 --rng-seed 0 --min-item-separation 5 --workers 8`, bench lock held,
and reports the explore-phase best at 8 s (the log's `[EXPL] finished` line). Our layouts were
converted with `tools/to-sparrow-solution.py` (rigid fit to 1e-12 mm; Sparrow's own validator
accepts every one with minimum pair distance >= 5.000). These are diagnostic runs of Sparrow,
not results of ours: the forbidden-rescue row "fixture as a seed" applies to any use of them
as a start for a scored cell.

## The runs

| Sparrow started from | width at start | explore best at 8 s | separates |
|---|---|---|---|
| its own LBF (archived run, `docs/experiments/sparrow-mixed61/log-10s-x86.txt`) | 214.027 | 150.796 | 351 |
| **our constructor** (seed-independent, `ours-constructor-182.976.sparrow-solution.json`) | 182.976 | **149.195** | 206 |
| our Legacy incumbent, seed 18 | 164.262 | 152.692 | 74 |
| our Legacy incumbent, seed 20 | 164.954 | 155.966 | 57 |
| our Legacy incumbent, seed 22 | 163.550 | 154.639 | 57 |
| our Legacy incumbent, seed 24 | 164.510 | 157.268 | 46 |
| our Legacy incumbent, seed 26 | 166.218 | 156.378 | 62 |
| our Wall10s incumbent, seed 18 | 159.256 | 155.012 | 28 |
| our Wall10s incumbent, seed 20 | 159.773 | 156.451 | 22 |
| our Wall10s incumbent, seed 24 | 159.003 | 152.919 | 40 |

Timeline from our constructor (`logs/sparrow-from-constructor.log.gz`): 89 bites in the first
second (182.976 -> 167.6), 54 in the second (-> 158.7), 25 (-> 154.8), 21 (-> 151.6), 12
(-> 149.8), then 149.2. Ours from the identical layout, Legacy profile, nine dev seeds: about
100 bites in 7.5 s of explore, 182.976 -> 163.6..167.2 (`../proxy-margin/README.md`).

## What it says

1. **The constructor's layout is a good basin.** Sparrow reaches 149.195 from it, deeper than
   from its own LBF. Our constructor is not the problem.
2. **Our bites damage the basin.** From our layouts at 164-166 mm Sparrow reaches only
   152.7-157.3; from its own layout at 165.5 mm (taken after 2 s of its own path,
   `logs/sparrow-own-2s.log.gz`) it reaches 150.8. Whatever our separator does on the way from
   183 to 165 leaves a layout that even Sparrow's separator cannot take much below 155.
3. **Our separator is the whole ten-second gap.** Same start, same split-and-close bite of
   0.1 %, same 5 mm contract, eight workers each: Sparrow does 183 -> 158.7 in two seconds;
   ours does 183 -> 165 in seven and a half. `first-bites.txt` lines the first twenty bites
   up: Sparrow's cost one pass each at width >= 175 (median 1, mean about 1.2); ours cost a
   median of 3 master iterations and a mean of 17 at the same widths, on the same initial
   layout.

So the question for the next round is not "which schedule" or "which exact gate" but why one
master iteration of ours resolves a 0.18 mm squeeze so much less completely than one Sparrow
pass, and why the layouts it leaves behind are worse. The per-iteration anatomy (workers,
sweep, sampler, coordinate descent, GLS, band) is transcribed with file:line citations in the
Astra brief 3 record; the candidate causes are the ones that brief lists.

## Reproduce

```
python3 tools/to-sparrow-solution.py <cell.json> docs/experiments/sparrow-mixed61/input.json out.json
# (then set solution.density, solution.layout.density, run_time_sec; see tools/batch.sh)
(cd /var/lib/t3/tmp/sparrow-bench && ./target/release/sparrow -i out.json -t 10 -s 0 --min-item-separation 5 --workers 8)
```
`tools/from-sparrow-solution.py` does the inverse (Sparrow solution -> our placements JSON) for
the mirror experiment, which needs a diagnostic `--start` flag in the benchmark.

## Addendum: the median bite costs the same; the tail is everything

`first-bites.txt` lines up the first twenty bites on the identical constructor layout. Bites
1-16 cost the same in both engines (Sparrow 1,4,2,2,2,2,1,5,2,3,5,1,2,4,2,2 passes; ours
3,1,2,1,1,1,2,3,2,1,6,3,2,6,6,4 master iterations). Bite 17, at 180.07 mm, is hard for both:
Sparrow spends 17 passes and its next three bites cost 13, 1, 1; ours spends 37 iterations and
its next three cost 11, 11, 14. At width >= 175 on seed 20 the medians are equal (2 and 2) and
the means are 3.5 against 32.2, max 17 against 1145. So the separator is not slower on the
ordinary bite. It is slower on the hard bite, and the way it resolves a hard bite leaves a
layout on which the following bites are hard too. That is the mechanism to find.

## Addendum 2: from the margin-8 layouts, and how far the pieces travelled

Sparrow warm-started from our **margin-8** Legacy incumbents (seeds 18/20/22/24/26, starts
164.005 / 164.005 / 157.436 / 167.102 / 163.303) reaches 152.453 / 152.454 / 151.563 / 151.042 /
151.649 (`logs/`, `tools/batch.sh proxy-margin8full legacy ...`). Against 152.7-157.3 from the
no-margin incumbents, and 150.8 from its own 165.5 layout.

Per-piece centroid displacement from the constructor layout (our placements; Sparrow's converted
back by `tools/from-sparrow-solution.py`, whose rotation/mirror columns are artefacts of the
axis swap and are not reported):

| layout | depth | displacement median / mean / max (mm) | moved > 20 mm |
|---|---|---|---|
| Sparrow, 2 s from our constructor | 160.334 | 52 / 387 / 1641 | 44/61 |
| Sparrow, 10 s from our constructor | 149.195 | 339 / 509 / 1628 | 54/61 |
| ours, devbase Legacy s20 | 164.954 | 562 / 605 / 1794 | 58/61 |
| ours, devbase Legacy s18 | 164.262 | 404 / 460 / 1550 | 52/61 |
| ours, devbase Wall10s s20 | 159.773 | 599 / 629 / 1770 | 57/61 |
| ours, margin 8 Legacy s20 | 164.005 | 45 / 351 / 1635 | 39/61 |

The no-margin path moves the median piece 400-600 mm for 18 mm of depth: the refused-proxy-zero
-> pool -> disruption cycle (22 disruptions inside one bite on seed 20; 304 disruptions over the
nine devbase cells, 68 with margin 8) scrambles the layout, and that is why those layouts are
poor basins even for Sparrow. With the margin the median piece moves 45 mm, the structure of
the constructor survives, and Sparrow takes the result to about 151.5.

From our **margin-8 Wall10s** incumbents (seeds 18/20/24, starts 160.049 / 159.063 / 160.089)
Sparrow reaches **150.122** / 151.453 / 154.580: from the seed-18 layout it goes deeper than
from its own path (150.796).

## Addendum 3: the mirror experiment (our engine started from Sparrow's layouts)

The diagnostic `--start=<placements.json>` flag (commit "Instrument: --start", cutclose only,
contract-validated, tripwire `startedFrom` in the document; `score.py` refuses such documents)
starts our trajectory from a given layout. Sparrow's own layouts, converted by
`tools/from-sparrow-solution.py`, pass both the contract validator and the Exclusive kernel.
Legacy profile, seed 20, ten seconds, no margin:

| our engine started from | start depth | depth at 10 s | explore bites | publications |
|---|---|---|---|---|
| Sparrow's layout after 2 s (`-t 2`) | 164.361 | 159.778 | 28 | 31 |
| Sparrow's final layout (`-t 10`) | 150.090 | 150.005 | 0 | 2 |

From the good basin our engine takes 28 bites of 0.1 % in the 7.5 s of explore, about 3.7 per
second; Sparrow from its own 165.5 layout takes about 95 in six seconds. From Sparrow's
150.090 layout our engine cannot complete one 0.1 % bite in ten seconds (Sparrow's own
exploration failed for the first time at 150.646, after 241 iterations). The basin does not
rescue our separator: at equal state and equal width it is four times slower in bites per
second, and at Sparrow's terminal density it does not move.
