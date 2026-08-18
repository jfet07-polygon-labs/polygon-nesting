# Coordinator v2: the rebudget, the anytime curve, and two requests that broke it

PR7 shipped a thin coordinator that beat the m0+coupled baseline 9 of 9 and
then wrote down what its own trace said was wrong with it: two schedule
defects, three measured verdicts on where the ten seconds went, and one
untested assumption - that the schedule was general code.

This stage acts on all of it. The headline is that **acting on the verdicts is
worth more than the coordinator was**, that **the anytime curve saturates at
about eleven seconds** on mixed-61 rather than continuing to thirty, and that
**the constructor slice's verdict is a property of the request** - it published
nothing in 43 arms on mixed-61 and it published on 6 of 12 arms on triangle-20.

Everything timed here is a per-round paired interleaved measurement with the
arm order rotating every round, because another agent benchmarks on this box
concurrently. The box was measurably quieter for the first three batteries than
for the last two, which is visible in the numbers and is called out where it
changes a claim.

## 1. The two defects PR7's ledger recorded were already fixed in the tree

Both are shipped at `8d9f7e5`, and the honest finding is that neither needed
fixing again:

* **The constructor's pose prior charged as a descent.** `ParentRole::Prior` is
  in the basin phase at `portfolio.rs`, and the mode-20 call passes it. It is
  live: in a v2 triangle-20 run, the incumbent that eight mode-20 arms read as
  their pose prior ends the run at `descents: 0`, while the basins an
  alternation quantum actually descended from carry 1, 2 and 4.
* **The frontier ordered by fairness before quality.** `distinct_frontier`
  sorts `raw_depth_mm` first and `descents` only as a tie-break, and two unit
  tests pin exactly that - one asserts a charged descent does not demote a
  better basin, the other asserts descents *do* break an exact depth tie.

There is a residual of the first defect, in the other direction, and this stage
fixes it: **the crossover's second parent was never charged a descent at all**.
Mode 23 descends from both parents; v1 charged `frontier[0]` and passed
`frontier[1]` as a pinned secondary without telling the archive. It is the same
class of bug - the fairness counter disagreeing with what happened - and it is
now charged.

Its measured effect is nil, and the reason is worth stating because it is what
makes defect 1 mostly inert too: once the frontier is ordered by quality first,
`descents` only decides exact ties in `raw_depth_mm` between structurally
distinct layouts, which did not occur on any stream measured here. The fix is
correctness of the instrument, not a schedule change, and it is reported as
such rather than as a win.

## 2. The rebudget

v1's schedule was the review's sketch in the review's order. v2 reorders it by
*measured productivity*, from PR7's own operator ledger and this stage's:

| phase | v1 | v2 | why |
|---|---|---|---|
| `m0` protected | 1st, unbudgeted | 1st, unbudgeted | unchanged; it is the answer the engine returns without a coordinator |
| `descent` m22 quanta | 3rd | **2nd** | 9 publications in 18 calls |
| `crossover` m23 | 4th, one call | **3rd, repeatable** | the largest single published gains, and v1 gave it 0.6 s |
| `compression` m22 micro-descent | 5th | **4th** | 3 publications in 9 calls |
| `basins` salted m20 | **2nd, unconditional** | **last, conditional** | 0 publications in 19 calls |
| `m31` legalizer | before the compression | **only on a residue** | 0 exact-valid results in 6 calls |
| `drain` | last | last | unchanged |

### The constructor slice is conditional, and the condition is priced in-run

The rule is `BasinTrigger::WhenDescendable`, and it is neither a seconds
threshold nor a stall test but the thing both were proxies for: **draw a basin
only when the run can still afford to descend from it.** A drawn-and-undescended
basin is exactly the 19/19 refusal PR7 measured, so the phase draws one salted
arm and spends a quantum on it in the same iteration, and it refuses to start an
iteration unless the remaining budget covers `mean(mode20) + mean(mode22)` - both
measured from this run's own operator calls, in the budget's own currency. Until
mode 20 has been priced it is charged a quantum's price, so the first draw needs
two quanta of headroom.

`basins=never|always|stall|descendable` selects the four arms; `stall` is PR7's
other candidate trigger, kept because it was one of the two the ledger named.

### And it stops when it stops paying

`basin_patience`, default 1: the phase ends after one iteration that publishes
nothing. The stopping signal is deliberately the *descendant*, never the arm's
own depth, because Pearson(immediate, descended) = -0.212 makes immediate depth
an invalid quality proxy - the ledger's own finding, and the reason the archive
exists at all.

Measured, thirty-second budget, mixed-61, three seeds, three rounds, paired and
interleaved:

| arm | mode-20 arms drawn | published | median process wall | published depth |
|---|---:|---:|---:|---|
| `basins=never` | 0 | - | 10.20 s | 174.208 / 176.056 / 179.006 |
| `patience=1` (default) | 9 | **0** | 12.57 s | 174.208 / 176.056 / 179.006 |
| `patience=8` | 72 | **0** | 23.91 s | 174.208 / 176.056 / 179.006 |

**All 27 rounds published identical depths.** Patience 8 spends 13.7 s of a
thirty-second budget on 72 exact-valid constructor arms and 72 descents from
them, and changes nothing; patience 1 caps that at 2.4 s. That is the cost of
keeping the mechanism, and §4 is why it is kept.

### Mode 31 is demoted to the trigger Sol described

v1 asked m31 to legalize a *clean* m22 fixpoint one drop-ladder rung below its
own depth: 6 calls, 0 exact-valid, every one "global legalization did not reach
a feasible fixpoint". v2 inverts the order - compress first, then hand m31 the
residue **if and only if the compressing descent returned a complete layout the
exact validator refuses**.

Measured: **mode 31 was called zero times** in 9 runs at ten seconds, 9 at
thirty, 9 on shapes-17 and 9 on triangle-20. The m22 micro-descent came back
exact-valid every time, so there was never a residue. The v1 arm in the same
paired battery made 9 m31 calls and got 9 refusals. The demotion is a measured
no-op on quality and it removes a call that has never once succeeded.

### Two more schedule changes the measurement forced

**"May I start?" became "can I finish?".** v1 checked only the deadline, so a
2.7 s crossover could be launched 0.1 s before its own deadline and overrun the
phase after it. v2 requires `remaining >= mean cost of this operator`, measured
in-run; an operator with no measurement yet degrades to v1's check, because
refusing an unpriced operator would mean never pricing it.

**Phase deadlines became fractions of what phase 0 left**, not of the whole
budget. This is a generality fix and §4 is where it was found, but it is listed
here because it changes the ten-second schedule too, by about 4% of each
deadline.

## 3. The number, and the anytime curve

### Ten seconds, five arms, mixed-61

Three seeds, three rounds, paired and interleaved (`battery-ten-second-arms.json`):

| seed | base | v1 review schedule | v1 focused | **v2** | v2, `basins=never` |
|---:|---:|---:|---:|---:|---:|
| 0 | 181.589 | 179.587 | 179.587 | **174.208** (3/3) | 174.208 (2/3) |
| 1 | 179.690 | 179.633 | 176.056 | **176.056** | 176.056 |
| 2 | 179.662 | 179.006 | 179.006 | **179.006** | 179.006 |

* v2 against the bare engine: **median −3.634 mm, min −7.381 mm, 9 of 9 rounds
  strictly better.** v1's focused arm on the same rounds: −2.002 mm.
* v2 against the v1 champion: **0 of 9 worse, 3 of 9 better**, all three on seed
  0 and all three by 5.379 mm.
* **174.208 mm is a new best-from-request layout at ten seconds**, 1.848 mm
  below PR7's 176.056.

Where it came from, from the seed-0 operator ledger of one paired round:

```
v1 focused  descent m22  1.97s -> 179.587  PUBLISHED
            crossover    3.60s -> 179.639  (one call, pair 0-1)
            m31          5.56s   invalid
            compression  5.65s -> duplicate            final 179.587, ends 6.5s
v2          descent m22  1.96s -> 179.587  PUBLISHED
            crossover#1  3.58s -> 179.639
            crossover#2  5.52s -> 176.309  PUBLISHED   <- the whole gain
            compression  7.71s -> 174.208  PUBLISHED   final 174.208, ends 8.6s
```

**The second crossover attempt is the change.** v1 made one crossover per run
and stopped; the review's own schedule made *one in nine runs*, because the
constructor slice ahead of it had spent its deadline.

### The curve

Best published depth against wall budget, from the bare request, three seeds,
three rounds each, quality-trace armed with counters off
(`battery-final-mixed61.json`, `battery-thirty-second-triggers.json`):

| budget | seed 0 | seed 1 | seed 2 | vs bare engine |
|---|---:|---:|---:|---|
| bare engine (≈2.0–2.3 s, no coordinator) | 181.589 | 179.690 | 179.662 | — |
| **3 s** | 179.587 | 179.633 | 179.006 | −0.656 mm median, 9/9 |
| **10 s** | 174.208 *(1/3)*, else 179.587 | 176.056 | 179.006 | −2.002 mm median, 9/9 |
| **30 s** | **174.208** (3/3) | 176.056 | 179.006 | −3.634 mm median, 9/9 † |

† The 30 s column's depths are from `battery-thirty-second-triggers.json` (the
shipping build, three arms, 27 rounds, all identical). Its paired delta against
the bare engine is quoted from `battery-anytime-curve.json`, where a 30 s arm and
a baseline arm were interleaved in the same rounds and published the same three
depths.

Time to depth, seed 0, thirty-second arm, median over three rounds, from the
quality-frontier trace:

| ≤185 | ≤182 | ≤180 | ≤179 | ≤178 | ≤177 | ≤175 | ≤174.5 |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 0.68 s | 0.97 s | 3.00 s | 7.67 s | 8.36 s | 8.36 s | 10.59 s | 11.22 s |

**The curve saturates at about eleven seconds.** The thirty-second arm's median
process wall is 12.57 s: every phase reaches a joint fixpoint and the schedule
ends with more than half its budget unspent. Seed 1 flattens at 176.056 by
5.6 s and seed 2 never crosses 179.

The one thing thirty seconds buys over ten is *reliability*, not depth. Seed 0's
174.208 needs a second crossover to fit inside the budget; on the quieter box of
the first two batteries it fit 6 times in 6 at ten seconds, and on the busier box
of the last one it fit once in three. Pooled across all three ten-second
batteries on mixed-61: **27 paired rounds against the v1 champion, 7 strictly
better, 0 worse, 20 identical.** (Those 27 rounds span three builds of v2 -
before the deadline rescaling, before the patience rule, and the shipping one -
so the pooled count is a floor on the claim rather than one experiment.)

### Against Sparrow

Sparrow's pins on this request are 157.971 at 3 s and 150.165 at 10 s. Ours are
179.6 and 174.2. **We are 21.6 mm behind at three seconds and 24.0 mm behind at
ten**, and the shape of our curve says the gap is not a scheduling gap: our
operators reach a joint fixpoint at eleven seconds, so more budget spent the
same way does not close it. The review said orchestration alone cannot reach
160, and this is the first curve that says so in our own numbers.

## 4. Generality: two requests, two bugs, and one verdict that flipped

Run from the bare request on `shapes-17/2000x2700-compact` and
`triangle-20/2000x2700-compact`, with the budgets derived identically - the same
3 s / 10 s / 30 s wall budgets, the same spec, the same code path.

### Bug 1: the constructor clamp assumed mixed-61's packing ratio

`CONSTRUCTOR_CLAMP_MULTIPLE_OF_AREA_LOWER_BOUND = 2.0` is dimensionless, and the
module header claimed on that basis that every length here is derived from the
request. It is - but a *dimensionless* constant can still be a fact about one
request. Twice the area lower bound is above the reachable depth only if the
request packs at better than 50% of its own bound:

| request | area lower bound | phase-0 constructed depth | ratio | 2× bound |
|---|---:|---:|---:|---|
| mixed-61 | 130.399 mm | 183.079 mm | 1.40× | 260.797 mm — above |
| shapes-17 | 96.310 mm | 200.903 mm | **2.09×** | 192.620 mm — **below** |
| triangle-20 | 32.123 mm | 73.720 mm | **2.29×** | 64.246 mm — **below** |

On both other requests **every constructor arm failed**, with "skyline
construction produced no publishable layout within the target depth", because
the clamp was below every layout that exists. Eight arms per run, 2.04 s of a
3.88 s shapes-17 run, buying a guaranteed refusal.

The fix is `constructor_clamp_mm`: the larger of the area-bound multiple and a
depth this request is *known* to admit a complete layout at, which is the one the
coordinator's own phase-0 constructor just built. mixed-61's clamp is unchanged
to the digit - `max(260.797, 183.079)` - and both other requests are rescued.
After it, 12 of 12 triangle-20 arms and 9 of 9 shapes-17 arms are exact-valid.

Second, smaller: the phase now **stops at the first arm that produces no complete
layout**, because consecutive slots differ only by a salt of one part in ten
thousand, so the next arm would be refused for the same reason at the same
price.

### Bug 2: phase deadlines were fractions of the whole budget

On this box mode 0 costs about two seconds. At a three-second budget that is 0.67
of the whole, so every phase whose absolute fraction was below 0.67 was skipped
and the first one above it ran: the most productive operator in the schedule
dropped, a crossover run in its place on an archive nothing had descended in,
and a 3.9 s process against a 3.0 s budget. Deadlines are now
`f0 + (1 − f0) × share`, and the 3 s arm runs a descent quantum and finishes in
2.8 s.

This is also what makes the schedule the *same* schedule across requests: mode 0
is 20% of a ten-second budget on 61 pieces and 9% on 17.

### shapes-17: the coordinator is exactly the baseline

| budget | seed 0 | seed 1 | seed 2 | process |
|---|---:|---:|---:|---:|
| bare engine | 200.349 | 200.349 | 200.349 | 0.94 s |
| 3 s / 10 s / 30 s | 200.349 | 200.349 | 200.349 | 2.56 s / 2.57 s / 2.57 s |

Nine rounds at each budget, **zero publications by any operator**. Mode 0's
result is already a joint fixpoint: the descent quantum returns a `Duplicate`,
the compression quantum returns a `Duplicate`, the whole coupled separator
collapses to one layout, so `distinct_frontier(2)` has one member and the
crossover phase never runs at all.

The schedule's behaviour here is the correct one and worth stating: it
terminates in 2.57 s **whether the budget is 3 s or 30 s**, because every phase
either reaches its fixpoint or is refused by the patience rule. It does not burn
a budget it cannot use.

### triangle-20: the curve is real, and the constructor slice pays

| budget | seed 0 | seed 1 | seed 2 | paired vs bare engine |
|---|---:|---:|---:|---|
| bare engine (0.95 s) | 70.931 | 70.904 | 70.901 | — |
| 3 s | 70.771 | 70.747 | 70.742 / 70.901 / 70.901 | −0.157 mm median, 7/9 |
| 10 s | 70.727 | 70.727 *(2/3)* | 70.727–70.743 | **−0.177 mm median, 9/9** |
| 30 s | **70.727** (3/3) | **70.727** (3/3) | **70.727** (3/3) | **−0.177 mm median, 9/9** |

Every operator earns its place on this request, and the ranking is different
from mixed-61's:

| operator | calls | published |
|---|---:|---:|
| `crossover/mode23` | 23 | **10** |
| `descent/mode22` | 16 | 9 |
| `compression/mode22` | 7 | **7** |
| `diversify/mode20` | 12 | **6** |
| `diversify/mode22` | 12 | 0 |

**The constructor slice published on half its arms here.** On mixed-61 it has
published on **0 of 207 arms in this stage** and 0 of 19 in PR7. That is the
single most important generality result here: PR7's "the constructor slice does
not pay" is a true statement about mixed-61 and a false one about triangle-20,
which is why v2 makes the slice *conditional* rather than deleting it, and why
the condition is priced from the run rather than from a fixture.

One PR7 caveat is *not* discharged. The archive's eviction rule still never
fires in the shipping configuration: triangle-20's archive peaks at 11 of 16
across all 27 rounds, with zero evictions and zero
`RefusedArchiveFullAllDistinct`. It did fire on a pre-patience probe of the same
request, which drew eight arms instead of one and filled the archive to 16 - so
the rule is reachable on this request, but nothing that ships here exercises it,
and it remains covered by unit tests only.

## 5. Determinism

The work-budget mode adds one new branch point - the affordability guard - and
that guard reads a *measured operator cost*, which is exactly the kind of thing
that quietly turns a reproducible schedule into a clock-dependent one. It does
not, because the cost is quoted in the budget's own currency: work units under a
work budget, seconds only under a wall budget, pinned by a unit test.

| gate | budget | request | result |
|---|---|---|---|
| in-process `runs=2` | 40M | mixed-61 | pass, 176.056 |
| two processes, whole documents | 40M | mixed-61 | **0 differences**, 176.056, 32,327,123 units |
| in-process `runs=2` | 20M (binding) | mixed-61 | pass, 176.753 |
| two processes, whole documents | 20M (binding) | mixed-61 | **0 differences**, 176.753, 16,794,870 units |
| two processes, whole documents | 20M | triangle-20 | **0 differences**, 70.747, 23,342,900 units |

The 20M mixed-61 run is genuinely binding and takes a *different* schedule -
one descent call instead of two, one crossover instead of three, compression,
diversify and drain all refused - and both processes take that different
schedule identically.

One honest limit, visible in the triangle-20 row: it spent 23.3M against a 20M
budget. The affordability guard cannot refuse an operator it has never priced,
so the *first* call of each operator can overrun, and on triangle-20 the first
crossover cost 9.45M units. A work budget is a bound on what may be *started*,
not on what is spent. PR7's other limit stands unchanged: the deep operators'
Clipper counters are behind `search-profiling`, so a work budget under-prices
constructor arms against relaxed ones.

## 6. Protected legacy

All four pinned regression gates reproduce the pristine `8d9f7e5` binary as
**whole documents** - every counter, every restart row, every diagnostic field,
with wall-clock and build-identity fields removed:

| gate | value | fingerprint | fields compared | differences |
|---|---:|---|---:|---:|
| mode 20 `independentDepthMm` | 206.869 | `8a7737381238fa4d` | 3,261 | **0** |
| mode 22 raw | 159.09233022733062 | `fa01012af1d559ae` | 3,242 | **0** |
| mode 22 raw | 159.07876040364795 | `e28fba007f8031d4` | 3,242 | **0** |
| mode 22 raw | 164.0375677990678 | `49f094d7e59a9008` | 3,242 | **0** |

Every gate is `exactValid` and `contractValid`, and re-running each gate in a
second process on the worktree binary reproduces it field for field, so the
gates are stable across processes as well as equal to the base.

Full release suite green: **1,238 passed, 0 failed, 2 ignored**, including six
new portfolio unit tests.

One observation that belongs in the record because it is not this stage's doing:
**`cargo build --release` with the literal default feature set (`default = []`)
does not compile at `8d9f7e5`**, and has not for at least this commit -
`CoupledSeparatorArm::label` and `LaneSearch::uses_dynamic_pressure` are
`#[cfg(feature = "jagua-experimental")]` while their call sites in
`general_relaxed.rs` are not. That file is byte-identical to the base commit
here. The gate binary in this stage is therefore
`--features jagua-experimental` with no measurement features, which is what the
ledger's "default-features binary" has meant in practice.

## Reproducing

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,fast-constructor-profile,quality-trace   # v2
cargo build --release --example general_request_benchmark \
    --features jagua-experimental                                          # gates

python3 drivers/battery.py final 3 mixed-61 0,1,2 \
    'base:v2:' 'v1focus:v1:wall=10000,slots=0,states=1,cycles=1,epochs=4' \
    'v2at3:v2:wall=3000,cells={cells}' 'v2at10:v2:wall=10000,cells={cells}'
python3 drivers/battery.py thirty 3 mixed-61 0,1,2 \
    'v2at30:v2:wall=30000,cells={cells}' \
    'v2at30patient:v2:wall=30000,patience=8,cells={cells}' \
    'v2at30never:v2:wall=30000,basins=never,cells={cells}'
python3 drivers/battery.py shapes17   3 shapes-17   0,1,2 'base:v2:' 'v2at3:v2:wall=3000,cells={cells}' 'v2at10:v2:wall=10000,cells={cells}' 'v2at30:v2:wall=30000,cells={cells}'
python3 drivers/battery.py triangle20 3 triangle-20 0,1,2 'base:v2:' 'v2at3:v2:wall=3000,cells={cells}' 'v2at10:v2:wall=10000,cells={cells}' 'v2at30:v2:wall=30000,cells={cells}'
python3 drivers/determinism.py 20000000 1
python3 drivers/gates.py worktree <gate-binary> --twice
python3 drivers/collect.py
```

`drivers/lib.py` carries the pinned CLI tail and the two binary paths; point
`ROOT` at your worktree. The coordinator is armed by the trailing positional
argument 48; absent or empty, every existing invocation is byte-identical.

## What this does not measure

* **Three requests.** shapes-17 and triangle-20 are 17 and 20 pieces on the same
  sheet; nothing here says anything about a fourth.
* **One box, two noise regimes.** The seed-0 ten-second result depends on a
  crossover fitting inside the budget, and that is a property of the box.
* **The 3 s point is nearly all mode 0.** At three seconds the protected phase
  is 0.67 of the budget on mixed-61, so the 3 s column is one descent quantum
  more than the baseline and not an independent schedule.
* **Nothing about 160 or 150.** The curve saturates 14 mm above our own record
  and 24 mm above Sparrow's ten-second pin.
