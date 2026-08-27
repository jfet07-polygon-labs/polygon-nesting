# What a 1.47x engine does to the constants that were tuned on the slow one

`cap = 50` and `ratio = 0.95` were swept on an engine running 138 master
iterations per second. The near set, the axis cache and the contact pruning have
since made it about **200**. The wall-cap round's own closing paragraph said the
honest form of the cap is budget-derived rather than constant, "precisely
because the defect it repairs is a constant that did not scale". A 1.47x change
in how much fits in ten seconds is exactly the perturbation that says whether
`50` was a property of the search or a property of the clock.

Everything here is `--cell=cutclose`, mixed-61 exact-clearance, `--orders=1
--edge=5 --pair=5`, bare wall requests, one fresh process per cell, machine
otherwise idle.

## Workers: eight, and the machine says why

The profile had the eight workers delivering 6.1x - 76 % efficiency - which
reads like headroom on a sixteen-core machine. Nine seeds, two repetitions, ten
seconds, `cap = 50`:

| workers | median | mean | best | under 165 mm | total master iterations |
| ---: | ---: | ---: | ---: | ---: | ---: |
| **8** | 165.240 | **164.582** | **161.087** | 9/18 | **42,409** |
| 12 | 165.785 | 166.399 | 164.002 | 5/18 | 40,524 |
| 16 | 164.584 | 165.118 | 162.359 | 11/18 | 31,749 |

**More workers buys fewer master iterations, not more.** The host is an Intel
Core Ultra 7 270K Plus: sixteen physical cores, no hyperthreading, and
*heterogeneous* - performance cores and efficiency cores. A master iteration is
a barrier, so its cost is the **slowest** worker's, and a worker placed on an
efficiency core sets the pace for all of them. Eight workers is not a compromise
with the operating system; it is the count that fits on the fast cores.

This closes the question rather than opening it: the 76 % is not idle silicon
waiting to be claimed.

## Thirty seconds: the cap's optimum moved with the clock

Nine seeds, two repetitions, bare thirty-second requests
(`evidence/wall-30s.json`). The wall-cap round's own thirty-second reading, on
the engine as it was then, was **median 164.000, best 161.017, 9/9**.

| arm | median of 18 | mean | best | cells under 162 | cells under 160 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `c42ed22`, cap 50 | 161.926 | 162.755 | 159.704 | 9/18 | 1/18 |
| contact pruning, cap 50 | 160.491 | 161.096 | **158.703** | 12/18 | 6/18 |
| **contact pruning, cap 100** | **159.819** | **160.877** | 158.921 | 11/18 | **10/18** |

Both arms are 9 of 9 on every repetition; the bar stopped discriminating some
time ago and the median is what to read.

**`cap = 100` beats `cap = 50` at thirty seconds, and that is the point.** At
ten seconds on the slow engine the sweep found a clean optimum at 50 and called
150 flat-to-worse. Give the same search half again as many iterations per
second and three times the wall, and the optimum moves up. The cap is not a
property of the instance; it is a property of *how many separations the budget
can pay for*, which is exactly what the wall-cap round predicted when it wrote
that the honest form of the knob is budget-derived rather than constant.

Per seed, best of the two repetitions:

| seed | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `c42ed22` cap 50 | 160.815 | 166.089 | 161.223 | 160.622 | 165.530 | 164.001 | 164.016 | 161.825 | 159.704 |
| pruned cap 50 | 160.379 | 162.624 | 160.423 | 159.002 | 164.001 | **159.265** | 164.001 | 161.287 | **158.703** |
| pruned cap 100 | 159.002 | **159.745** | 160.972 | **158.921** | **162.481** | 162.866 | 164.002 | **159.319** | 159.591 |
