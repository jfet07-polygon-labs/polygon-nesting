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

## The bite and the separation budget are one parameter in two halves

`EXPLORE_SHRINK_STEP = 0.001` is Sparrow `config.rs`'s `shrink_step`, the third
Table 1 constant this campaign inherited and the one the shrink-step round left
alone because Sparrow's value held the best median. That round swept it at
`cap = 50` on an engine running 138 master iterations per second, and read
`0.0020` as the first setting that clearly loses:

> 41 bites instead of 87, and each one **a bigger shock than a 50-iteration
> separation can absorb**.

That sentence is the whole result. It is not a statement about the bite; it is a
statement about the *pair*. Ten seconds, nine seeds, two repetitions,
`cap = 100`, eighteen cells per arm:

| constant step | median | mean | best | worst | explore bites |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 0.001 (frozen) | 164.974 | 164.455 | 160.802 | | 106.1 |
| 0.004 | 162.389 | 162.471 | 160.601 | 164.157 | 30.2 |
| 0.008 | 160.590 | 161.648 | 159.490 | 165.799 | 16.2 |
| **0.016** | **160.564** | **160.586** | 157.438 | **163.016** | 8.9 |
| 0.024 | 161.411 | 161.007 | 157.710 | 161.982 | 6.1 |
| 0.032 | 159.802 | 160.431 | 159.015 | 165.813 | 4.9 |
| 0.048 | 164.601 | 163.237 | **157.030** | 165.502 | 3.2 |
| 0.064 | 160.003 | 161.967 | 159.129 | 170.078 | 2.8 |

**A hundred small bites became nine large ones, and the depth fell by four
millimetres.** Every bite costs a separation whether it is 0.1 % or 1.6 % of the
width, and a fixed wall buys a roughly fixed number of separations - so the
depth reached is `start * (1 - step)^n`, and the only thing a smaller step buys
is a smaller exponent base for the same `n`. The limit is that a larger bite is
harder to separate; the optimum is where the extra retries eat the extra ground.

Then move the other half. Step fixed at `0.016`:

| cap | median | mean | best | worst | under 159 mm | explore bites | retries |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 50 | 163.137 | 162.663 | 159.496 | 165.662 | 0/18 | 8.0 | 19.1 |
| 100 | 160.551 | 160.581 | 157.411 | 163.114 | 2/18 | 8.9 | 10.5 |
| **200** | 160.015 | **159.498** | **155.613** | 163.241 | 6/18 | 9.3 | 6.1 |
| **400** | **158.175** | 159.777 | 157.783 | 163.357 | **10/18** | 9.3 | 5.6 |
| none | 160.546 | 160.535 | 157.750 | 163.386 | 6/18 | 9.0 | 5.6 |

**`cap = 50` - the value the wall-cap round measured as the optimum, and it
was - is now the worst arm on the board**, at 19 retries per run against 6. It
was the optimum for a 0.1 % bite. At 1.6 % a separation cut off at 50 iterations
has not finished the job and the retry does it again from a pool entry.

`155.613 mm` in a bare ten-second request. The campaign's previous best anywhere,
at any budget, was `159.079` - a fixed-work gate.
