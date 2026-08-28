# The one place the engine measured difficulty and threw the answer away

The coordinate descent has adapted its own step since the first round: multiply
by `1.1` when a step improves, by `0.5` when it does not. It is Sparrow's
`CD_STEP_SUCCESS` and `CD_STEP_FAIL`, and it is the reason the descent works
without anyone tuning a step size per instance.

One level up, the explore loop takes a bite of **exactly** `EXPLORE_SHRINK_STEP`
every single time. It has the same signal available - a bite that published on
its first separation was absorbed without effort, a bite that needed three pool
restarts was not - and it discards it. Every bite is the same fraction of the
width, from the first one at 181 mm with 48 mm of certified headroom ahead of
it, to the last one at 165 mm where every micrometre is contested.

That asymmetry is the experiment.

## The rule

`homotopy::adapt_explore_step`, and it is four lines:

```text
first separation published  ->  step = min(step * 1.2, ceiling)
needed a retry              ->  step = max(step * 0.5, base)
```

`ceiling = 0` restores the constant exactly, and is the default, so a run that
does not name `--adaptivestep` takes the path it always took - verified
bit-identical on the three fixed-work cells, poses and depth unchanged.

The steeper fall than rise is deliberate and is the descent's own asymmetry: an
over-large bite is paid for immediately and repeatedly, in retries; an
under-large one only in ground not taken.

**Where it settles is a fixed point, not a setting.** Growing by `1.2` and
falling by `0.5` balances when `1.2^p * 0.5^(1-p) = 1`, that is `p = 0.79`: the
step walks to whatever size lets about four bites in five succeed on the first
separation. Nobody chose that number and it is not in the code.

## It works, and then the control kills it

Nine seeds, two repetitions, bare ten-second requests, `cap = 100,
ratio = 0.95`, eighteen cells per arm. `0.000` is the frozen constant.

| ceiling | median | mean | best | under 165 mm | under 163 mm | explore bites |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| **0.000** (frozen) | 164.974 | 164.455 | 160.802 | 11/18 | 2/18 | 106.1 |
| 0.002 | 164.003 | 163.514 | 161.517 | **18/18** | 4/18 | 65.1 |
| 0.004 | 163.660 | 163.217 | 160.084 | 14/18 | 8/18 | 41.5 |
| 0.008 | 160.957 | 161.154 | 159.027 | **18/18** | 16/18 | 27.7 |
| 0.016 | 160.804 | 161.126 | 159.006 | | | 23.1 |
| **0.032** | **160.663** | **160.917** | **159.004** | | | 22.7 |
| 0.064 | 160.682 | 160.934 | 159.012 | | | 22.7 |

**-3.5 mm of mean at ten seconds**, and it looked like the mechanism was real:
the curve saturates from `0.016` upward, which is exactly what a self-limiting
rule should do - the ceiling stops mattering because the halving finds the size
that works before the ceiling ever binds.

Then the control. Same cap, same wall, same seeds, a **constant** step:

| step | median | mean | best | worst | under 163 mm | explore bites |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0.001 (frozen) | 164.974 | 164.455 | 160.802 | | 2/18 | 106.1 |
| 0.004 | 162.389 | 162.471 | 160.601 | 164.157 | 12/18 | 30.2 |
| 0.008 | 160.590 | 161.648 | 159.490 | 165.799 | 12/18 | 16.2 |
| **0.016** | **160.543** | **160.564** | **157.357** | **162.848** | **18/18** | 8.9 |
| adaptive, ceiling 0.032 | 160.663 | 160.917 | 159.004 | 164.278 | 14/18 | 22.7 |

**A constant `0.016` beats the adaptive rule on every column** - mean, best,
worst, and count under the bar - and it beats it by more than the adaptive rule
beat the frozen constant on the tails. `157.357` against `159.004`; a worst case
of `162.848` against `164.278`.

So the hypothesis is dead, and the interesting part is *why* it looked alive.
The engine was not throwing away a difficulty signal. **The step was sixteen
times too small,** and any change that made the first bites bigger - including
one that grows a step until it hits a ceiling - was going to look like a
mechanism. The adaptive rule's own saturation, which read as self-limitation,
was the step running to the ceiling and staying there: `max step == ceiling` in
every one of the eighteen cells of every arm.

The knob stays in the tree, defaulted off and bit-identical when unnamed,
because it is the control that a future adaptive proposal has to beat. What
ships is the number.

## Why the earlier sweep read the opposite

The shrink-step round measured `0.0020` as the first setting that clearly loses
- "41 bites instead of 87, and each one a bigger shock than a 50-iteration
separation can absorb" - and that reading was correct **for the engine and the
cap it was taken on**. It held `cap = 50` fixed, on an engine running 138 master
iterations per second.

The sentence explains its own reversal: *a bigger shock than a 50-iteration
separation can absorb*. The bite size and the separation budget are one
parameter in two halves. At `cap = 100` on an engine 1.47x faster, a separation
can absorb sixteen times the shock, and the same measurement inverts.
