# Coordinator v4: the schedule as a priced class, a stopping rule, and a constructor slice that competes

Coordinator v3 shipped a ranked action queue and three measured negatives, and
this stage is those three negatives, measured out.

* **The compression schedule was not in the queue.** The port measured it
  publishing in 12 of 12 matched cells against a mode-26 ladder's 10, at 6.0x
  the ladder's depth per unit of work - and left it reachable only from an
  explicit CLI mode. It is now a priced action class: on mixed-61 at 120M it
  publishes on 17 of 19 actions and returns twice compression's millimetres per
  unit of work, and on the other two requests it publishes nothing at all.
* **v3 had no stopping rule.** shapes-17 churned 281 barren crossover actions
  across nine 30-second runs for 0.0034 mm. There is now a global barren
  patience, sized from the interval v3 measured rather than fitted.
* **Diversify was gated on the priced queue emptying**, which under crossover
  churn never happens. It now competes in the queue and is auditioned after
  eight barren actions.

The measured result: on mixed-61, **9 of 9 rounds better at 10 s and at 30 s and
not one round worse at any tier**; on triangle-20, **the 3 µm regression exactly
gone at 30 s**, 9 of 9; on shapes-17, **9.9 seconds of coordinator wall
returned** for 0.38 µm. The two places it is worse are both µm-scale and both
measured: shapes-17 at 30 s, 3 of 9 rounds by 0.38 µm, which is what the
stopping rule costs; and triangle-20 at 10 s, 3 of 9 rounds by ≤2 µm, which is
what the schedule class's wall price costs (§4.5, §8).

The headline, mixed-61 from the bare request, three seeds, work budget
120,000,000, against merged-HEAD v3 as the paired reference:

| seed | coordinator v3 | **coordinator v4** | Δ |
|---:|---:|---:|---:|
| 0 | 169.14057315694365 | **163.927** | **−5.214** |
| 1 | 169.92832830680420 | **162.161** | **−7.767** |
| 2 | 172.086 | **164.004** | **−8.082** |

All six are `exactValid` **and** `contractValid`, independently re-confirmed
through mode 27 in a separate process from a pristine base-commit binary that
contains none of this code (§6). **162.161 mm is a new best-from-request layout
on this request**, 6.967 mm below coordinator v3's 169.141 and 12.047 mm below
coordinator v2's 174.208.

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_6f601cb2-a5f-1` |
| base commit | `5d6ce0c` (coordinator v3 + compression-schedule port merged) |
| request | `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`, sha256 `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3` |
| contract | true 5.0/5.0 exact clearance; search-offset allowance **`0.002`** |
| pristine `5d6ce0c` binary (`jagua-experimental`) | sha256 `a71894d08021cdc5e4cff02d7c12b7064c3539fc2ec032000193b63173a2a335` |
| gate binary, this tree (`jagua-experimental`) | sha256 `6e633ef66b62d33835c9e155bea0438e682bde83644d390209a41c3bdad1dc6c` |
| measurement binary, this tree (`jagua-experimental,compression-schedule`) | sha256 `e2ee45122b4442f00f38333f9ef816b24880983cd61012fe3a122637cc7af895` |
| box | x86_64, 16 cores, engine pinned at 8 threads, **shared with another measurement agent throughout** |

The allowance is `0.002`, coordinator v2's, v3's and the ledger's, **not** the
four pinned gates' `0.0005`. Every depth here is therefore comparable to
174.208 / 176.056 / 179.006 and to v3's 169.141 / 169.928 / 172.086, and **is
not** comparable to the 159.079 / 164.038 record lineage.

**v3 and v4 are the same binary.** Three portfolio spec keys select the
schedule - `sched=`, `barren=`, `divq=` - so every A/B below is two processes of
one build. `v3=1,sched=0,barren=0,divq=0` is merged-HEAD v3 and reproduces the
pristine base-commit binary field for field (§5.1).

# 1 - The compression schedule as a priced class

## 1.1 The slice, and why it is nine rungs

Mode 34 is an anytime operator with two natural budgets: a work cap in its own
currency, and a step count. The port's cheap arm, `sched10-noroll`, used the
first - 10% of a measured mode-26 rung, 3,341,379 units - and published a median
**1.104 mm** for a median **869,133** of the coordinator's units, which is
**1.013 mm per million**, the best efficiency measured anywhere in this
portfolio.

A cap of 3,341,379 units is a mixed-61 number and cannot cross a request. What
*can* cross is the walk that cap bought: over its twelve cells, `sched10-noroll`
walked a median **1,568** one-micron steps. On a 174.208 mm parent,
`9 * depth * COUPLED_SEPARATOR_CONTRACTION_RATIO` is 1.5679 mm, which is
**1,568 canonical grid steps**. So the class asks for nine rungs of the
separator's own relative contraction quantum - the same derivation
`LADDER_RUNGS = 2` uses, at a different count - and no millimetre is carried:
nine rungs is 1.61 mm on a 179 mm parent, 1.80 mm on shapes-17's 200 mm one and
0.64 mm on triangle-20's 70.7 mm one.

The step count is also what makes the arm deterministic without reading a
counter. A work cap denominated in the coordinator's currency would be zero
under a wall budget, because a wall-budget run leaves the profiling counters
off; a step count is a function of the parent's depth and the canonical grid,
and of nothing else.

The rest of the schedule's configuration is the port's own measured default,
unmodified: six repair sweeps per step, a confirmation due every fourth step,
`micro_legalize` on a refused confirmation, and **`rollback_after_steps = 0`**,
which is the port's structural finding rather than a preference.

## 1.2 Pricing it honestly: charge the self-cap

The port named the problem and this stage had to answer it. The coordinator's
work meter increments `Counter::ExactPairTests` *past* the broad-phase bounds
reject, so a whole-layout confirmation that asks all 1,830 pairs reaches the
narrow phase on about 99 of them and is charged ~493 units for 4.83 ms of work.
On the schedule's arms the exact tier is **24-52% of the wall and about 4% of
the metered work**.

The consequence is visible in the port's own twelve cells, and it is not a
rounding error:

| meter | min | median | max | spread |
|---|---:|---:|---:|---:|
| the coordinator's, over `sched10-noroll` | 307,767 | 869,133 | 3,343,739 | **10.9x** |
| the schedule's own | 3,341,665 | 3,353,550 | 3,356,020 | **1.004x** |

The same arm, on the same twelve parents. A class priced on the coordinator's
meter is a class riding free on the tier it spends its wall in, *and* a class
whose price varies elevenfold between cells.

Two fixes were available:

* **extend the meter** - charge asked pairs rather than narrow-phase tests,
  process-wide. More principled, and **rejected here on blast radius**: every
  pinned work-unit number in this repository is denominated in the current
  counter, including the ledger's 32,393,757 / 31,957,935 / 27,938,867 that
  coordinator v3 §6.1 reproduces to the unit as its strongest regression
  statement. Moving the meter moves all of them and buys nothing the pricing
  decision needs.
* **charge the self-cap.** The operator carries a deterministic meter of its
  own, in the same currency by construction
  (`candidate_queries + 5 * asked pairs`), and the coordinator charges the
  larger of the two into `ClassStats`. That is what ships.

The charge is a **price, never a spend**: the budget still advances at the
meter's own rate, so this cannot make a run stop early against its own counter.
It raises the number the affordability rule and the ranking value read, and
nothing else. Under a wall budget it is not applied at all and does not need to
be - seconds are seconds, and the clock has no broad phase.

The action rows carry both numbers, so the disagreement is in the evidence
rather than in this prose. Seed 0's four schedule actions at 120M:

| action | charged (self) | metered (coordinator) | ratio | published |
|---|---:|---:|---:|---:|
| #3 | 3,311,034 | 815,114 | 4.06 | 1 |
| #12 | 2,987,470 | 1,037,695 | 2.88 | 1 |
| #16 | 3,304,538 | 413,613 | 7.99 | 1 |
| #19 | 2,846,089 | 1,394,419 | 2.04 | 1 |

It is `max`, not "the self-cap", and the difference is not cosmetic: the
coordinator's meter also counts the parent validation and depth measurement the
schedule does not charge itself for, so on seed 1's action #7 the two read the
same 1,720,710 and the meter is what was charged. The rule is "whichever meter
says this cost more", which is the same rule the class cost estimate already
uses on the prior.

## 1.3 The prior, and how right it turned out to be

`prior Δraw = 1.104 mm` (the port's measured median) against
`prior cost = 0.3806` protected-phase-0 costs (3,341,379 units against mixed-61
seed 0's own 8,778,573-unit phase 0). Prior value `2.901`, which places the
class between descent (3.74) and crossover (1.79) and above the ladder (1.29).

The two readings of that cost agree, which is the reason to trust it rather
than a coincidence to hide: the *worst* of the twelve cells' coordinator-metered
spends is 3,343,739 units, or 0.3809 phase-zeros - the same number to three
digits.

What the prior was worth, measured, on the first schedule action of every
mixed-61 run at 120M:

| seed | estimate | actual (charged) | actual/estimate |
|---:|---:|---:|---:|
| 0 | 3,341,125 | 3,311,034 | **0.991** |
| 1 | 3,664,970 | 3,539,786 | **0.966** |
| 2 | 3,410,687 | 3,455,866 | **1.013** |

Within 3.4% on all three seeds. That is a much better first estimate than any
other class in the queue gets on the same three runs - the ladder's spans
0.39-1.33, crossover's 0.92-1.35, compression's 0.84-1.08 - and it is a direct
consequence of charging a self-capped operator its own cap rather than a meter
that disagrees with it by a factor of two to eight.

# 2 - The stopping rule

## 2.1 The constant, and where it comes from

Coordinator v3 §5.2 declined to fit a constant and measured the interval one
would have to live in instead, over 1,056 actions on three requests: **at least
8**, because the mixed-61 30 s run that produced v3's headline published at
action #13 after seven barren ones and shapes-17's 10 s arm has a productive
barren run of 8; **at most 32**, because shapes-17 at 30 s churns 33 barren
actions between micron publications.

`BARREN_ACTION_PATIENCE = 16` is the **geometric** midpoint of `[8, 32]`. The
midpoint is taken geometrically because the quantity is a ratio - how many
failures before a success - whose interval endpoints are multiplicative rather
than additive. It is also exactly twice the largest productive barren run ever
measured and exactly half the churn length it has to cut.

This round re-measured the interval on its own runs
(`evidence/barren-runs.json`, 18 arms × 9 runs):

| request / arm | actions | longest productive barren run | max trailing barren |
|---|---:|---:|---:|
| shapes-17 @ 30 s, v3 | 341 | **33** | 36 |
| shapes-17 @ 30 s, **v4** | 194 | 9 | **16** |
| shapes-17 @ 10 s, v3 | 89 | 8 | 10 |
| triangle-20 @ 30 s, v3 | 268 | 3 | 27 |
| triangle-20 @ 30 s, **v4** | 242 | **8** | 13 |
| triangle-20 @ 10 s, v3 | 86 | 3 | 4 |

Two things in that table. shapes-17's 33 reproduces v3's own measurement of the
interval ceiling from a different campaign; and **no v4 arm has a productive
barren run above 9**, so a patience of 16 never cut a run that was about to
publish - except on shapes-17 at 30 s, where by construction it cut the 33 and
that is the whole point (§4.4 prices what that cost: 0.38 µm).

The triangle-20 v4 row is the audition, visible in the same statistic: its
longest productive barren run is **exactly 8**, which is the largest one measured
on that request anywhere, and it is the diversify action at
`DIVERSIFY_AUDITION_BARREN` publishing (§3.3).

## 2.2 What it does

The signal is the incumbent moving, and deliberately *not* a yield floor: v3
§5.2 suggested one, and a floor needs a millimetre to compare against, which is
exactly the kind of constant this schedule carries none of. When the counter
trips the loop returns `PhaseExitCause::Patience` **with its queue still full**,
which is a third exit cause distinct from `keysExhausted` and `affordability` -
three different findings about a run, and the report says which.

# 3 - Diversify competes, and is auditioned

## 3.1 A prior of zero is not a prior

v3 gave the class `prior Δraw = 0.0` and then scheduled it by an eligibility
rule. Zero is an absorbing value: a class ranked at zero is never chosen, so it
never earns the evidence that would displace its prior, and v3's own rule -
"the prior is worth two actions of evidence" - becomes unfalsifiable for that
one class. §4.2 of v3 measured what it costs: 3 µm on triangle-20, the one
request where the slice pays.

This round measured the number instead (`evidence/diversify-prior.json`,
coordinator v2 at `work=40,000,000`, three seeds on each of three requests,
because a prior quoted from one request is the defect being fixed):

| request | constructor arms | published | Δraw published by the class | mm per action |
|---|---:|---:|---:|---:|
| mixed-61 | 3 | 0 | 0.000 | 0.000 |
| shapes-17 | 3 | 0 | 0.000 | 0.000 |
| triangle-20 | 4 | **2** | **0.05826** | 0.01456 |
| **pooled** | **10** | 2 | 0.05826 | **0.005826** |

## 3.2 And its price, which was 12x wrong

v3 §1.3 reported that its `WhenDescendable` rule estimated an m20 ticket at
0.10 s when the ticket costs 1.18 s, and did not fix it. Putting the class in
the queue prices it like every other class - and this round measured that one
price cannot do the job, because the class's two currencies disagree by an
order of magnitude:

| request | diversify phase, work units | as phase-zeros | diversify phase, seconds | as phase-zeros |
|---|---:|---:|---:|---:|
| mixed-61 s0 | 964,363 | 0.110 | 4.08 | **1.855** |
| mixed-61 s2 | 1,189,145 | 0.133 | 4.65 | **1.979** |
| shapes-17 s0 | 301,724 | 0.260 | 1.19 | 1.266 |
| triangle-20 s0 | **7,804,768** | **1.224** | 1.61 | 1.851 |

So the class carries two priors, each the worst case of its own currency
rounded up - **1.224** phase-zeros under a work budget (triangle-20 seed 0's
1.22401) and **1.979** under a wall budget (mixed-61 seed 2's 1.97626) - and
it is the only class that does; the other five spend their time in the tiers
the work meter counts, and their two currencies agree. A test pins that
asymmetry so a sixth price cannot be added without a measurement to justify it.

Measured effect, shapes-17, first diversify action of every run:

| schedule | estimate | actual | actual/estimate |
|---|---|---|---:|
| v3 §1.3's rule | 0.10 s | 1.18 s | **11.8x under** |
| **v4's class prior** | ~2.0 s | ~1.2 s | **1.6x over** |

An overestimate is the right side to be wrong on: at a 3 s budget the queue now
*refuses* a 4 s constructor ticket on the affordability rule instead of buying
it on an eligibility clause.

## 3.3 The audition, and why 8

Ranking the class is necessary and not sufficient. A prior of 0.005826 mm
against crossover's 1.0923 mm never wins a slot at any budget this engine runs
at - and that is an honest finding, not a design failure: the measurement says
the class is worth a third of a percent of a crossover per action, pooled.

So the queue additionally promotes one untested ticket to the front of the
affordable set after `DIVERSIFY_AUDITION_BARREN = 8` consecutive barren actions.
Eight is the *floor* of the same measured interval, for the same measured
reason: a promotion at 8 is one the mixed-61 30 s stream, whose longest
productive barren run is 7, never reaches. The pair reads as one rule: **at
eight barren actions the queue buys a new basin, at sixteen it stops.**

It is still the affordability rule that decides whether the promoted ticket is
bought, and the constructor slice's own `basin_patience` still ends the class
after one barren draw. So the audition costs at most one action per eight
barren ones, and on a request where it publishes nothing it costs exactly one
action per run.

Verbatim from triangle-20 seed 0 at 30 s (`evidence/curve-triangle20.json`),
the mechanism doing exactly what it was built for:

```
#7  compression  val=1.203  70.7301 -> 70.7301   (barren 1)
#8  descent      val=0.904                        (barren 2)
#9  ladder       val=0.867                        (barren 3)
#10 crossover    val=0.676                        (barren 4)
#11 crossover    val=0.452                        (barren 5)
#12 schedule     val=0.361                        (barren 6)
#13 crossover    val=0.361                        (barren 7)
#14 crossover    val=0.301                        (barren 8)
#15 diversify    val=0.003 est=1.57 act=0.80  PUB 70.73006941379586 -> 70.72726178003285
```

Action #15 is the audition, and 70.72726178003285 is **coordinator v2's own
triangle-20 depth to the digit** - the 3 µm v3 lost, recovered. On all three
seeds and all three rounds, at 30 s, the last publication's class is
`diversify`: 9 of 9.

# 4 - The measurements

## 4.1 mixed-61, the anytime curve

Three seeds, three rounds, paired and **interleaved with the arm order rotating
every round**, one process per cell, from the bare request
(`evidence/curve-mixed61.json`, 54 runs).

### Depth

Best published raw depth over the three rounds, `best / worst` where they
differ:

| budget | seed 0 | seed 1 | seed 2 |
|---|---|---|---|
| v3 @ 3 s | 179.5869 | 179.633 | 179.006 / 179.662 |
| **v4 @ 3 s** | 179.5869 | 179.633 | **179.006** |
| v3 @ 10 s | 174.2081 / 179.5869 | 176.056 | 178.2857 / 179.006 |
| **v4 @ 10 s** | **173.5751** / 176.1078 | **171.3620** | **176.1620** |
| v3 @ 30 s | 169.1406 / 177.634 | 171.4367 | 172.086 / 172.8956 |
| **v4 @ 30 s** | **166.8080** / 167.758 | **165.3230** / 165.9716 | **168.9509** / 170.1058 |

Paired v4-minus-v3, nine rounds per tier:

| budget | median | min | max | v4 better | v4 worse | equal |
|---|---:|---:|---:|---:|---:|---:|
| 3 s | 0.000 | −0.656 | 0.000 | 1 | **0** | 8 |
| **10 s** | **−2.844** | **−4.694** | −0.633 | **9** | **0** | 0 |
| **30 s** | **−3.945** | **−9.876** | −1.383 | **9** | **0** | 0 |

**v4 is strictly better in 9 of 9 rounds at 10 s and at 30 s, and never worse in
any round at any tier - including the 3 s tier where v3 was worse than v2 in 2
of 9.** The 3 s column moves once, on seed 2, by 0.656 mm, and is otherwise
identical: at one affordable action the schedule class is priced out and the
queue makes the same first move it always made.

**The 10 s tier is the one the port's data predicted would move, and it moved
past 174.208 on every seed**: 173.575, 171.362, 176.162 against v3's 174.208,
176.056, 178.286. The prediction was that a class costing a third of a
protected phase and publishing 1.1 mm becomes affordable at a budget where the
ladder never is, and that is what the traces show. Pooled over the nine 10 s
rounds: the ladder makes **0 actions in either arm** - it is priced out at that
budget in both, exactly as v3 §5.3 said it should be - and the schedule class
makes **9 actions and publishes on all 9**, for 14.386 mm. Crossover collapses
from 17 actions to 1, because the two cheap classes take the incumbent past the
depths crossover used to find.

### Wall, and who overran

Coordinator wall - the coordinator's own clock, which is the clock its budget is
quoted in - median over nine rounds, and the worst single round:

| budget | v3 median | v3 max | v4 median | v4 max |
|---|---:|---:|---:|---:|
| 3 s | 2.79 s | 2.91 s | 2.77 s | 2.95 s |
| 10 s | 9.27 s | 9.92 s | 9.50 s | **10.19 s** |
| 30 s | 28.50 s | 29.52 s | 28.83 s | **32.23 s** |

**v4 overran its own budget in 2 of its 27 mixed-61 runs and v3 overran in
none, and that is a regression against v3's headline claim.** Both are
identified. `v4at10-s2-r0` went 1.9% over on a crossover estimated at 1.21 s
that cost 1.86 s - a class v4 did not touch. `v4at30-s1-r0` went 7.4% over on
its last action, a schedule slice estimated at 1.95 s that cost **5.12 s**, 2.6x
its price: the class's wall prior is the weakest number in this stage (§8), and
this is what it costs when the ratchet has not yet seen a slow slice. For scale,
coordinator v2 overran at 3 s by 41% and at 10 s by 6% on this same request.

### In schedule, not at the end

The final publication's class, over nine rounds per arm: at 30 s it is
`compression` in 6 and **`schedule` in 3**, so the new class is not only feeding
compression - it closes a third of the runs itself. At 10 s it is `compression`
in 9 of 9 for v4, against v3's 7 compression and 2 crossover.

## 4.2 The work budget

Work-budget mode is deterministic and load-independent, so one run per cell is
the whole measurement (`evidence/work-mixed61.json`).

| budget | seed | v3 | **v4** | Δ | v4 spent | of budget | v4 exit |
|---|---:|---:|---:|---:|---:|---:|---|
| 40M | 0 | 170.63217550422073 | **169.891** | −0.741 | 39,309,265 | 98.3% | affordability |
| 40M | 1 | 176.05599999999998 | **171.3619986855876** | −4.694 | 38,518,915 | 96.3% | affordability |
| 40M | 2 | 172.89557339904468 | **165.779** | −7.117 | 37,102,965 | 92.8% | affordability |
| 120M | 0 | 169.14057315694365 | **163.927** | −5.214 | 113,570,496 | 94.6% | affordability |
| 120M | 1 | 169.92832830680420 | **162.161** | −7.767 | 115,804,466 | 96.5% | affordability |
| 120M | 2 | 172.086 | **164.004** | −8.082 | 111,276,458 | 92.7% | affordability |

**Six of six, at both budgets.** Every v4 depth above is `dualGateValid`, and
every v3 depth above reproduces coordinator v3's own published table digit for
digit **and unit for unit** - 37,575,714 / 39,177,529 / 38,960,559 at 40M and
113,968,463 / 117,233,295 / 115,712,290 at 120M - from a binary that contains
all of v4.

## 4.3 What each class cost and produced

Pooled over the three seeds at 120M (`evidence/class-economics-120M.json`).
Work is the coordinator's meter, which is what makes the schedule row an
*understatement* of its own cost and the reason §1.2 charges it differently:

| class | actions | published | work units | per action | Δraw | **Δraw / M eval** | Δraw / action |
|---|---:|---:|---:|---:|---:|---:|---:|
| **schedule** | 19 | **17** | 34,932,849 | 1.839M | **20.292** | **0.5809** | 1.068 |
| compression | 35 | 23 | 86,540,157 | 2.473M | 25.752 | 0.2976 | 0.736 |
| ladder | 4 | 5 | 90,781,550 | **22.695M** | 4.805 | 0.0529 | **1.201** |
| crossover | 12 | **0** | 81,809,329 | 6.817M | 0.000 | 0.000 | 0.000 |
| descent | 7 | **0** | 19,218,167 | 2.745M | 0.000 | 0.000 | 0.000 |

**The schedule publishes on 17 of 19 actions and is the best class per unit of
work in the queue by a factor of two**, and it is the cheapest per action of the
three that pay. Compression stays the workhorse by volume. The two classes that
published nothing on this stream at this budget are the two v3's own §3.1
table already flagged - descent published 0 of 10 there too - and crossover has
gone from 3 publications in 22 actions to 0 in 12, because the schedule and
compression now take the incumbent past the depths crossover used to find.

## 4.4 shapes-17: the patience rule, and what it is worth

Three seeds, three rounds, paired interleaved (`evidence/curve-shapes17.json`,
36 runs).

| budget | v3 depth | v4 depth | paired Δ | v3 coordinator wall | **v4 coordinator wall** |
|---|---:|---:|---:|---:|---:|
| 10 s | 200.349 | 200.349 | −0.00038 median 0.000 (1 of 9 better, 0 worse) | 9.52 s | 9.41 s |
| 30 s | 200.349 | 200.349 | median **0.000** (0 better, **3 worse** by 0.00038) | 28.98 s | **19.06 s** |

**The wall drops by 9.9 seconds at the 30 s tier - 34% - and the quality change
is 0.38 µm in three of nine rounds.** The mechanism is exactly the one v3 named:
v3 makes 341 actions across nine runs and 272 of them are crossovers worth 12 µm
each; v4 makes 194, of which 108 are crossovers, and **exits `patience` in 9 of
9 runs** where v3 exits `affordability` in 9 of 9.

The 0.38 µm is the honest price and it is charged here rather than rounded away:
the patience rule cuts shapes-17's 33-action productive barren run by
construction, and 12 µm is what that run was worth.

**It does not reach coordinator v2's 2.57 s, and this round says why rather than
claiming it did.** Two thirds of the remaining wall is *not* the barren tail.
On seed 0 at 30 s the first publication is action **#9**, so nine actions
precede any barren counter at all, and the patience then pays for sixteen more:
26 actions is the floor for any patience of 16 on this stream, whatever the
budget. v2 terminates at 2.57 s because each of its *phases* reaches a fixpoint
and the phase sequence then ends; v4 stops because the incumbent stopped moving,
which is a weaker condition and buys a strictly more general schedule. Cutting
the other two thirds needs the *first* publication to arrive sooner, not the
last barren run to be cut shorter, and that is a different experiment.

## 4.5 triangle-20: the 3 µm regression, at 30 s

Three seeds, three rounds, paired interleaved
(`evidence/curve-triangle20.json`, 36 runs).

| budget | v2 (reference) | v3 depth | **v4 depth** | paired v4−v3 |
|---|---:|---:|---:|---:|
| 10 s | 70.72726178003285 | 70.73007 / 70.73005 / 70.72882 | the same on seeds 0 and 1 | median 0.000, **3 of 9 worse** by ≤0.002 |
| 30 s | 70.72726178003285 | 70.73007 / 70.73005 / 70.72882 | **70.72726178003285** ×3 | **−0.002788 median, 9 of 9 better** |

**At 30 s the regression is gone, exactly and on every seed and round**: all
three seeds reach 70.72726178003285, which is coordinator v2's own number to the
digit, and the last publication's class is `diversify` in 9 of 9 runs. The
diversify class takes 13 actions across the nine runs and publishes on 9 of
them.

**At 10 s it is not gone**, and the cause is measured. The audition needs eight
consecutive barren actions and triangle-20 at 10 s never gets there: its whole
run is ten actions and its longest barren run is 4. v4 at 10 s is worse than v3
in 3 of 9 rounds, all on seed 2, by at most 0.002 mm, because the schedule class
spends 9 actions there for **0 publications** at 5.1x its wall estimate (§8).
The measured floor of the patience interval is 8 and this round declines to fit
a smaller constant to one request's ten-action runs; the honest next move is to
price the schedule class per request, not to shorten the audition.

# 5 - Regression

## 5.1 The default path, and the reference arm

The coordinator's own v3 path reproduces the pristine `5d6ce0c` binary as
**whole documents**, not merely on the depth
(`evidence/reproduce-v3.json`, mixed-61, three seeds, `work=40,000,000`,
`v3=1,sched=0,barren=0,divq=0` against the pristine binary's `v3=1`):

| seed | fields compared | differing | raw depth | work units |
|---:|---:|---:|---|---|
| 0 | 3,405 | **9** | 170.63217550422073 = 170.63217550422073 | 37,575,714 = 37,575,714 |
| 1 | 2,770 | **9** | 176.05599999999998 = 176.05599999999998 | 39,177,529 = 39,177,529 |
| 2 | 3,483 | **11** | 172.89557339904468 = 172.89557339904468 | 38,960,559 = 38,960,559 |

Every one of the 29 differing fields is `meteredCost`, a **field this round
adds** that the pristine binary does not emit at all. No behavioural field
differs, and the work-unit spend is identical to the unit, which is the strong
form: every affordability decision, every ranking value and every action the
queue took is the same.

mixed-61 never reaches merged-HEAD v3's empty-queue fallback, so the same
comparison was run on shapes-17, which does
(`evidence/reproduce-v3-shapes17.json`, `work=40,000,000`):

| seed | fields compared | differing | of which `meteredCost` | raw depth | work units |
|---:|---:|---:|---:|---|---|
| 0 | 3,047 | 66 | 63 | 200.349 = 200.349 | 38,499,095 = 38,499,095 |
| 1 | 3,047 | 66 | 63 | 200.349 = 200.349 | 38,929,805 = 38,929,805 |
| 2 | 3,021 | 65 | 62 | 200.349 = 200.349 | 38,610,809 = 38,610,809 |

The nine remaining fields are three per seed, all on the one diversify action
the fallback draws, and all three are **reporting**: its `estimatedCost`, its
`value`, and the class's `firstEstimatedCost`. On the fallback path the
affordability check reads `mean_operator_cost` and the ranking never sees a
Diversify candidate, so neither number is consulted by a decision - which the
work-unit spend, identical to the unit, is the check on rather than the claim.

They are worth reading, because they are §3.2's fix in one line: the old rule
priced that ticket at **126,985** work units and the class prior prices it at
**1,420,745**.

## 5.2 The four pinned gates

Three binaries - the pristine `5d6ce0c` default-feature build (`base`), this
tree's default-feature build (`after`), and this tree's `compression-schedule`
build (`final`):

| gate | pinned | fingerprint | `base` | `after` | `final` |
|---|---:|---|---|---|---|
| g1 mode 20 `independentDepthMm` | 206.869 | `8a7737381238fa4d` | hit | hit | hit |
| g2 mode 22 raw | 159.09233022733062 | `fa01012af1d559ae` | hit | hit | hit |
| g3 mode 22 raw | 159.07876040364795 | `e28fba007f8031d4` | hit | hit | hit |
| g4 mode 22 raw | 164.0375677990678 | `49f094d7e59a9008` | hit | hit | hit |

Whole-document comparison against `base`, wall-clock and build-identity fields
removed (`evidence/gates-docdiff-after.json`,
`evidence/gates-docdiff-final.json`):

| comparison | fields compared (g1/g2/g3/g4) | differences |
|---|---|---|
| `after` vs `base` | 3,261 / 3,242 / 3,242 / 3,242 | **0** |
| `final` vs `base` | 3,261 / 3,242 / 3,242 / 3,242 | **0** |

All four are `exactValid` and `contractValid` on all three binaries. The gates
never enter the coordinator - they are pinned-parent positional replays - so
they are a *check*; §5.1 is the argument.

## 5.3 Determinism

Both schedules, three seeds, `work=40,000,000`, two processes each, compared
field by field with wall-clock and build-identity fields removed
(`evidence/determinism.json`):

| arm | seed | raw depth | work units | fields compared | **differing** |
|---|---:|---:|---:|---:|---:|
| v4 | 0 | 169.891 | 39,309,265 | 3,557 | **0** |
| v4 | 1 | 171.3619986855876 | 38,518,915 | 2,846 | **0** |
| v4 | 2 | 165.779 | 37,102,965 | 3,635 | **0** |
| v3 | 0 | 170.63217550422073 | 37,575,714 | 3,405 | **0** |
| v3 | 1 | 176.05599999999998 | 39,177,529 | 2,770 | **0** |
| v3 | 2 | 172.89557339904468 | 38,960,559 | 3,483 | **0** |

The work-unit spend is identical to the unit across processes. The self-cap
charge does not break this and could not: it is a function of the schedule's own
step and confirmation counts, which are functions of the counters.

## 5.4 The ablation: which key bought which millimetres

Three changes landed together, so the headline is a joint number and the
attribution is measured rather than apportioned. One key at a time, mixed-61,
`work=120,000,000`, three seeds, one run per cell because the mode is
deterministic (`evidence/ablation-mixed61.json`):

| arm | keys | seed 0 | seed 1 | seed 2 | median Δ vs v3 |
|---|---|---:|---:|---:|---:|
| v3 | `sched=0,barren=0,divq=0` | 169.14057315694365 | 169.92832830680420 | 172.086 | — |
| **sched** | `sched=1,barren=0,divq=0` | **163.927** | **162.161** | **164.004** | **−7.767** |
| barren | `sched=0,divq=0` | 169.14057315694365 | 169.92832830680420 | 172.086 | 0.000 |
| divq | `sched=0,barren=0` | 169.14057315694365 | 169.92832830680420 | 172.086 | 0.000 |
| **v4** | all three | **163.927** | **162.161** | **164.004** | **−7.767** |

**On this request at this budget every millimetre is the schedule class, and
the other two keys are exactly inert** - `barren` and `divq` reproduce the
reference arm's depth, its iteration count (25 / 21 / 23) and its work spend
(113,968,463 / 117,233,295 / 115,712,290) to the unit.

That is the designed behaviour rather than a null result: on mixed-61 no barren
run reaches 16, so the patience never trips, and none reaches 8, so the audition
never fires. The two keys are for the requests where they *do* fire, and §4.4
and §4.5 measure them there. Reading the three tables together: **the schedule
class buys the depth, the patience buys the wall, and the audition buys the 3
µm.**

## 5.5 Suite

`cargo test --release`, both feature sets, full logs at `evidence/suite.log` and
`evidence/suite-schedule.log`:

| features | passed | failed | ignored | exit | merged HEAD |
|---|---:|---:|---:|---:|---|
| `jagua-experimental` | **1,250** | **0** | 2 | 0 | 1,244 (v3's own count) |
| `jagua-experimental,compression-schedule` | **1,262** | **0** | 2 | 0 | 1,256 (1,244 + the port's 12) |

Six new tests, and each pins a number this stage argues from rather than a
behaviour it hopes for: that no class prior is an absorbing zero; that exactly
one class is priced twice and which one; that a scheduled slice is nine rungs
and reproduces the port's own 1,568-step walk; that both patience constants sit
inside `[8, 32]` and that 16 is its geometric midpoint; that the three keys
default on inside a v3 that defaults off; and that the queue's two diversify
construction sites name one action. No rerun was needed - the known-flaky
`free_material_multi_eviction` case passed first time in both runs.

# 6 - Independent confirmation

Every one of the twelve work-budget layouts - six v4 and six v3 - was written
out as a pinned-parent fixture and replayed through **mode 27**, the
micro-legalization probe, the one mode meant to be pointed at states that may
not validate, in a separate process from the **pristine `5d6ce0c` binary**,
which carries no mode-34 class and does not know the three spec keys
(`evidence/confirmations.json`):

| budget | seed | arm | exactValid | contractValid | rawSourceDepthMm | fingerprint unchanged | violating pairs before | pieces moved |
|---|---:|---|---|---|---:|---|---:|---:|
| 40M | 0 | **v4** | true | true | **169.891** | yes | 0 | 0 |
| 40M | 1 | **v4** | true | true | **171.3619986855876** | yes | 0 | 0 |
| 40M | 2 | **v4** | true | true | **165.779** | yes | 0 | 0 |
| 120M | 0 | **v4** | true | true | **163.927** | yes | 0 | 0 |
| 120M | 1 | **v4** | true | true | **162.161** | yes | 0 | 0 |
| 120M | 2 | **v4** | true | true | **164.004** | yes | 0 | 0 |
| 40M/120M | 0,1,2 | v3 | true | true | 170.632 / 176.056 / 172.896 / 169.141 / 169.928 / 172.086 | yes | 0 | 0 |

Zero repair applied, zero violating pairs, fingerprint unchanged, raw depth to
the digit, twelve of twelve: a different build, a different code path and a
different process agree that these are legal layouts at those depths under the
request's own 5.0/5.0 contract. `exactValid` and `contractValid` are separate
measured fields here, not the coordinator's own `dualGateValid` composite.

**162.161 mm is therefore a confirmed publication**, not a coordinator's own
opinion of one.

# 7 - The 2.5-degree warm-start snap: examined, and left alone

The port measured the entry transform costing a median **+0.448 mm** on every
171-179 mm parent entering the relaxed lane, and named it the largest single
thing between this band and the next. This round opened it, and is not touching
it. The reason is not caution, it is that **there is no flag to put it behind.**

`initialize_complete_state` (`general_relaxed.rs:15370`) maps a warm start's
rotations through `canonical_angle` (`general_relaxed.rs:17114`), which rounds
onto `SURROGATE_ANGLE_STEP_DEG = 2.5`. The snap is not a normalisation applied
to a representation that could hold something finer - **it is the
representation**. The structured surrogate catalog this lane scores against is
built by `SurrogateCatalogMode::StructuredGrid`
(`general_relaxed.rs:15293`), which enumerates
`(0..angle_count).map(|i| i as f64 * SURROGATE_ANGLE_STEP_DEG)` and nothing
else. A state carrying an off-grid rotation is a state the proxy tier has no
surrogate for.

So the flagged change the brief asks for does not exist at this seam. Removing
the snap would have to switch the catalog to `SurrogateCatalogMode::CurrentAssignment`
- the mode that *does* carry a warm start's own angles, and the mode the
`DirectionalPenetration` pressure model already selects
(`general_relaxed.rs:5455`). That is a different proxy tier, with a different
candidate stream and a different cost model, and every number the port measured
- the 82% proxy-infeasible frontier, the 4.83 ms confirmation, the 24-52% exact
share, the 1.013 mm per million - is a measurement of the structured tier. A
flag whose off-arm and on-arm run different pressure models is not an ablation
of the snap; it is two engines.

The honest statement is therefore narrower than "it is risky": **the 0.448 mm is
not attributable to `canonical_angle` alone, and cannot be measured against a
control until the structured catalog can represent the poses it would keep.**
The experiment that would settle it is a `CurrentAssignment`-catalog mode-34 arm
matched against the `StructuredGrid` one on the twelve pinned parents, which is
a campaign and not a stretch goal. It is named here with the three line
references so the next round starts from them.

# 8 - Honest limits

* **The headline is one request and three seeds.** shapes-17 and triangle-20
  say the schedule *runs* generally and that the stopping rule and the audition
  do what they were built for; they say nothing about whether the 5-8 mm is
  general. On both of them the schedule class publishes **nothing at all**: 0 of
  29 actions on shapes-17 and 0 of 37 on triangle-20, pooled over both budget
  tiers. Its 1.104 mm prior is a mixed-61 number and does not transfer, exactly
  as crossover's 1.0923 mm does not transfer to triangle-20.
* **The schedule class's price transfers in work units and not in seconds, and
  that is the weakest number in this stage.** First-action actual/estimate is
  **0.97-1.01** on mixed-61 at a work budget - as good as any prior in this queue
  has ever been - and a median **2.54-2.59 on mixed-61 under a wall budget**,
  2.94-3.07 on shapes-17 and **5.1 on triangle-20**. The self-cap is the right
  price in the currency it is denominated in and about a third of the right price
  on the clock, and the coordinator has no measured conversion between the two
  for this operator. The ratchet corrects it after one action, and one action is
  1.5-2.5 s of a 10 s budget. That is the mechanism behind triangle-20's 10 s
  rounds (§4.5) and behind the 30 s overrun below, and it is the first thing to
  price better.
* **v4 overran its own coordinator budget in 2 of 27 mixed-61 runs**, where v3
  overran in 0 of 27 (§4.1). One is a crossover this stage did not touch; the
  other is a schedule slice that cost 2.6x its wall estimate on its first
  appearance in that run. The affordability rule is only as good as the price
  it is given, and this class's wall price is the weakest one here.
* **The stopping rule does not recover v2's shapes-17 wall** (§4.4): 28.98 s to
  19.06 s, not to 2.57 s, and the remainder is the pre-first-publication prefix
  rather than the barren tail.
* **triangle-20 at 10 s is still 3 µm short of v2** and gains a new ≤2 µm
  regression on one seed. The 30 s tier is exact.
* **`DIVERSIFY_AUDITION_BARREN` is not a spec key**, so the claim that 8 is the
  right threshold is supported by the trace and by the interval's measured floor
  but is not ablated. Making it a key is a one-line change and the next round
  should do it before arguing about the value.
* **The two work readings still disagree, and the fix here is local.** Charging
  the self-cap prices *one class* honestly; it does not make the coordinator's
  meter correct. A schedule action is charged ~3.3M and metered ~0.4-1.4M, and
  the run's budget still advances on the meter - so at a work budget v4 gets
  slightly more schedule than its own accounting says it paid for. Reported
  rather than smoothed: `meteredCost` is in every action row.
* **`0.002`, not `0.0005`.** Not comparable to the record lineage.
* **Wall against work.** Every quality number is at a work budget or is a paired
  interleaved wall comparison over 9 rounds, and the box was shared with another
  measurement agent for the whole campaign.

# 9 - Files

* `drivers/runlib.py` - the pinned CLI tail, the salt sets and the `0.002`
  allowance, a diffable copy of coordinator v3's with one addition: the binary
  is overridable from the environment, so one driver runs the gate build, the
  schedule build and the pristine base-commit build.
* `drivers/lib.py`, `drivers/gates.py` - the four pinned gates, byte-identical
  to `constructor-inner-certificate`'s with `ROOT` and the output directory
  repointed.
* `drivers/battery.py` - the paired interleaved wall battery. An arm spec now
  carries extra portfolio keys, which is how one binary runs both schedules.
* `drivers/workquality.py` - v3 against v4 at identical work budgets.
* `drivers/ablation.py` - the three keys one at a time, at one work budget.
* `drivers/diversifyprior.py` - what one constructor arm is measured to be
  worth, on every request the coordinator has been measured on. This is where
  `ActionClass::Diversify`'s prior comes from.
* `drivers/reproduce.py` - the reference arm against the pristine base-commit
  binary, as whole documents, through the coordinator rather than around it.
* `drivers/determinism.py`, `drivers/recheck.py` - two processes, whole
  documents.
* `drivers/docdiff.py` - two gate runs, whole documents.
* `drivers/confirm.py` - every published layout replayed through mode 27 from
  the pristine binary.
* `drivers/summarize.py`, `drivers/classeconomics.py`, `drivers/barrengaps.py` -
  the tables above.
* `drivers/smoke.py` - one v3 and one v4 run with the action trace printed.
* `evidence/*.json`, `evidence/suite*.log` - every table above as measured.

Reproduce:

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental                          # gate binary
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,compression-schedule     # measurement binary

export V4_BIN=<measurement-binary>

python3 drivers/diversifyprior.py evidence/diversify-prior.json <gate-binary> \
    mixed-61,shapes-17,triangle-20 0,1,2 40000000

python3 drivers/battery.py curve-mixed61 3 mixed-61 0,1,2 \
    'v3at3:wall:3000:1:sched=0,barren=0,divq=0'  'v4at3:wall:3000:1' \
    'v3at10:wall:10000:1:sched=0,barren=0,divq=0' 'v4at10:wall:10000:1' \
    'v3at30:wall:30000:1:sched=0,barren=0,divq=0' 'v4at30:wall:30000:1'
python3 drivers/battery.py curve-shapes17   3 shapes-17   0,1,2 \
    'v3at10:wall:10000:1:sched=0,barren=0,divq=0' 'v4at10:wall:10000:1' \
    'v3at30:wall:30000:1:sched=0,barren=0,divq=0' 'v4at30:wall:30000:1'
python3 drivers/battery.py curve-triangle20 3 triangle-20 0,1,2 \
    'v3at10:wall:10000:1:sched=0,barren=0,divq=0' 'v4at10:wall:10000:1' \
    'v3at30:wall:30000:1:sched=0,barren=0,divq=0' 'v4at30:wall:30000:1'
python3 drivers/summarize.py  <battery.json> <summary.json>
python3 drivers/barrengaps.py <battery.json> ... --out evidence/barren-runs.json

python3 drivers/workquality.py   work-mixed61 mixed-61 0,1,2 40000000,120000000
python3 drivers/classeconomics.py <workquality.json> 120000000 <out.json> v4
python3 drivers/ablation.py      ablation-mixed61 mixed-61 0,1,2 120000000
python3 drivers/determinism.py   determinism mixed-61 0,1,2 40000000

python3 drivers/reproduce.py evidence/reproduce-v3.json <pristine> <measurement> \
    mixed-61 0,1,2 40000000
python3 drivers/gates.py base  <pristine-binary>    /var/lib/t3/tmp/v4/gates/base
python3 drivers/gates.py after <gate-binary>        /var/lib/t3/tmp/v4/gates/after
python3 drivers/gates.py final <measurement-binary> /var/lib/t3/tmp/v4/gates/final
python3 drivers/docdiff.py /var/lib/t3/tmp/v4/gates base after
python3 drivers/docdiff.py /var/lib/t3/tmp/v4/gates base final
python3 drivers/confirm.py <workquality.json> /var/lib/t3/tmp/v4/confirm \
    <pristine-binary> evidence/confirmations.json
```

The three keys default **on inside v3**, and `v3` itself defaults **off**, so a
default build is coordinator v2 to the digit and the four gates are a check on a
path nothing in this stage can reach. `v3=1,sched=0,barren=0,divq=0` is
merged-HEAD v3.

# 10 - What the next round should do first

1. **Make `DIVERSIFY_AUDITION_BARREN` a spec key and ablate it.** 8 is the
   measured floor of the patience interval and the trace shows it working, but
   it is the one constant here with no A/B behind it.
2. **Price the schedule class per request, not per portfolio.** Its first-action
   estimate is within 3.4% on mixed-61 and 3-5x low on the other two, which is
   the same prior-transfer failure crossover has on triangle-20. The ratchet
   fixes it after one action, and at a 10 s budget one action is 20% of the run.
3. **Shorten the prefix, not the tail.** shapes-17's remaining 19 s is nine
   actions before the first publication plus sixteen of patience. A patience
   rule cannot touch the first nine.
4. **The 2.5-degree snap, with the catalog** (§7). `CurrentAssignment` against
   `StructuredGrid` on the twelve pinned parents, as a campaign.
