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
