# Coordinator v3: the ranked action queue, the compression sign, and the ladder as a phase

This stage implements what the opportunity ledger and the A/B/C **measured**,
and measures the result. Three changes, no new operator:

* **the compression target's sign.** v2 asked mode 22 for `depth + 0.8` - a
  *looser* bound than the incumbent it already held - got an exact-valid answer
  and exited `noResidue`. v3 asks for `depth - COMPRESSION_RUNG_MM`, which is
  the A/B/C's control D;
* **the loop.** v2's schedule was a single pass, so the ledger's rank-0 state,
  born in the compression phase after crossover had ended, was never a parent.
  v3 is one action queue that re-enumerates after every action;
* **the ladder as a scheduled class.** The A/B/C's arm C - one short mode-26
  ladder into the global legalizer tier - is a class in the queue, priced from
  this run's own protected phase until it has priced itself.

The headline, mixed-61 from the bare request, three seeds, work budget
120,000,000:

| seed | coordinator v2 | **coordinator v3** | Δ | the A/B/C's probe (arm C, post-drain) |
|---:|---:|---:|---:|---:|
| 0 | 174.20812003998896 | **169.14057315694365** | **−5.068** | 169.251 |
| 1 | 176.05599999999998 | **169.92832830680420** | **−6.128** | 171.739 |
| 2 | 179.006 | **172.086** | **−6.920** | *(published nothing)* |

All six are `exactValid` **and** `contractValid`. **169.141 mm is a new
best-from-request layout on this request**, 5.068 mm below coordinator v2 and
0.110 mm below the number the ledger's arm C reached as a probe *after* the
schedule had finished - and v3 reaches it **in schedule**: the drain published
nothing in any of the six runs.

And the saturation is gone as a budget statement, not only as a depth. The
ledger's three v2 runs stopped at **23-27% of a 120M budget** with every phase
out of keys. v3's three stop at **94.9 - 97.7%**, and all three stop on
`affordability` - the remaining budget no longer covers the cheapest action the
queue can name - rather than on `keysExhausted`.

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_8687e703-3d9-1` |
| branch | `wf-coordinator-v3` |
| base commit | `fccda7f` (opportunity ledger + A/B/C + lane stage 2 merged) |
| request | `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`, sha256 `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3` |
| contract | true 5.0/5.0 exact clearance; search-offset allowance **`0.002`** |
| measurement binary (`jagua-experimental`) | sha256 `1bac4c509a47ec99d75d05b1af5d9783879b7d7c7c9611504abaf8b6b2b29476` |
| pristine `fccda7f` binary (`jagua-experimental`) | sha256 `d17533dc47e0686e9ea3809e1fd0b29ed8817f4c88cf644211a6df2aa7e3d7c5` |
| box | x86_64, 16 cores, engine pinned at 8 threads, **shared with another measurement agent** |

The allowance is `0.002`, coordinator v2's and the ledger's, **not** the four
pinned gates' `0.0005`. Every depth here is therefore comparable to
174.208 / 176.056 / 179.006 and to the ledger's 169.251 / 171.739, and **is
not** comparable to the 159.079 / 164.038 record lineage.

**v2 and v3 are the same binary.** The schedule is selected by one portfolio
spec key, `v3=0|1`, so every A/B below is two processes of one build rather
than two builds compared across days. `v3=0` is the default and is v2 to the
digit.

# 1 - What v3 is

## 1.1 The queue

Every iteration: enumerate, rank, spend the best affordable action, repeat.
The enumeration is bounded **by construction**, not by truncation - the ledger's
point is that the action space is 4,318 wide, not that a schedule should walk
it. Over the top-3 distinct frontier one enumeration offers at most **21**
actions:

| class | offered over | per iteration |
|---|---|---:|
| compression | the best distinct state (`descent_states`, 1) | 1 |
| descent | the same | 1 |
| ladder | rank 0 only | 1 |
| crossover | 6 ordered pairs × (the constant cut + 2 derived cuts) | 18 |

The ledger's top-3 frontier alone carries 360 ordered, cut-derived actions; a
v3 enumeration offers 21 of them and the loop re-enumerates after every action,
so a pair that keeps paying keeps being offered its next cut, outward from
`0.5` in the ledger's own canonical order.

Keys are built from the two parents **in the order they are handed to the
operator**, plus the cut's bit pattern - never from ranks. That is the ledger's
own pinned bug and it is pinned here too
(`a_v3_crossover_key_is_parent_and_cut_ordered_never_rank_ordered`).

## 1.2 The ranking, and how it is quoted

`value(class) = expected Δraw per action / expected cost per action`, where the
cost is quoted as a multiple of **the protected phase-0 pipeline this run just
paid for**, in the budget's own currency. That choice is what makes one prior
price a 61-piece request and a 17-piece one, and a wall budget and a work
budget, from one table:

| class | prior Δraw per action | prior cost, in phase-0 costs | prior value | source |
|---|---:|---:|---:|---|
| compression | 2.101 mm | 0.2176 | 9.66 | ledger §5, seed 0: 2.101 mm in 1 call, 1.91M units |
| descent | 1.001 mm | 0.2678 | 3.74 | ledger §5, seed 0: 2.002 mm in 2 calls, 4.70M units |
| crossover | 1.0923 mm | 0.6092 | 1.79 | ledger §5, seed 0: 3.277 mm in 3 calls, 16.04M units |
| ladder | 3.0914 mm | 2.3923 | 1.29 | A/B/C arm C: 4.957/4.317/0 mm; cost = the **largest** of 14.8/21.0/5.7M |
| diversify | 0 | 0.1094 | 0 | ledger §4: 0 descendant publications from any archived m20 basin |

The prior ordering reproduces the ledger's Δraw/M-evaluation ordering to better
than 10% - the ledger's 1.1017 : 0.4264 : 0.2043 is 5.39 : 2.09 : 1, and these
priors are 5.39 : 2.09 : 1 - which is pinned by a test
(`the_class_priors_reproduce_the_ledgers_measured_order`). It has to be quoted
this way rather than in millimetres per million evaluations because **under a
wall budget the evaluation counters are off**, and a queue that could not rank
under a wall budget would be a queue that could not rank in production.

The prior is worth two actions of evidence and this run's own publications
displace it, which is the "let publications re-rank" half of the rule. On the
seed-0 30 s trace below, compression enters at 9.655 and is down to 2.375 by
its eleventh action; crossover enters at 1.793, decays to 0.307 over six barren
actions, and then publishes 1.736 mm on the seventh.

## 1.3 Pricing mode 26 honestly

The ledger's mode-20 finding was that the work budget prices a constructor arm
at 260-335 units against 3.1 seconds of clock - four orders of magnitude - and
that v2's affordability rule gave an *unpriced* operator a free pass. v3 has no
free-pass clause: every class has a price before its first action, and the
estimate is `max(prior, this run's worst observed action of the class)` - never
the mean, and never lowered by one lucky sample.

What that prior was worth, measured, on every ladder action v3 made at a 120M
work budget:

| seed | action | estimate | actual | actual/estimate | published |
|---:|---|---:|---:|---:|---:|
| 0 | #4 | 21,000,980 | 10,309,810 | 0.49 | 1 |
| 0 | #10 | 21,000,980 | 7,312,203 | 0.35 | 0 |
| 1 | #11 | 23,036,540 | **28,091,870** | **1.22** | 1 |
| 1 | #15 | 28,091,870 | 16,435,810 | 0.59 | 2 |
| 2 | #4 | 21,438,218 | 8,399,788 | 0.39 | 0 |
| 2 | #15 | 21,438,218 | **24,502,643** | **1.14** | 1 |

The prior is right to within a factor of 2.9 in both directions and it is
conservative in four of six. It is *not* mode 20's pricing and it is not
transferred from it: it is the A/B/C's own measured arm-C spend expressed
against that run's own protected phase.

The contrast is worth stating because this round measured it directly. The one
class v3 still prices with v2's `WhenDescendable` rule is diversify, and on
shapes-17 that rule estimates an m20 ticket at **0.10 s** and the ticket costs
**1.18 s** - a ratio of **11.7 - 12.0** on four sampled actions. The four
orders of magnitude are a work-budget artefact; **12x is what the mispricing is
worth on the clock**, and it is still there because this round did not touch it.

## 1.4 The ladder is derived, not carried

A scheduled ladder is `LADDER_RUNGS = 2` rungs of the separator's own relative
contraction quantum: the drop is `2 × depth × COUPLED_SEPARATOR_CONTRACTION_RATIO`,
which is exactly two rungs of `ladder_compression_bounds` at any parent depth
(pinned by `a_scheduled_ladder_is_two_rungs_of_the_engines_own_quantum`). On a
174 mm parent that is 0.348 mm, the same two-rung shape the A/B/C probed with
its 0.3 mm drop; on triangle-20's 70.7 mm parent it is 0.141 mm. No millimetre
is carried across requests.

# 2 - The anytime curve, mixed-61

Three seeds, three rounds, paired and **interleaved with the arm order rotating
every round**, one process per cell, from the bare request
(`evidence/curve-mixed61.json`, 54 runs).

## 2.1 Depth

Best published raw depth, `best / worst` over the three rounds:

| budget | seed 0 | seed 1 | seed 2 |
|---|---|---|---|
| v2 @ 3 s | 179.5869 | 176.7528 / 179.633 | 179.006 |
| **v3 @ 3 s** | 179.5869 | **179.633** | 179.006 |
| v2 @ 10 s | 174.2081 | 176.056 | 179.006 |
| **v3 @ 10 s** | 174.2081 | 176.056 | **176.7132 / 178.2857** |
| v2 @ 30 s | 174.2081 | 176.056 | 179.006 |
| **v3 @ 30 s** | **169.1406** | **169.9283 / 171.4367** | **172.086** |

Paired v3-minus-v2, nine rounds per tier:

| budget | median | min | max | v3 better | v3 worse | equal |
|---|---:|---:|---:|---:|---:|---:|
| 3 s | 0.000 | 0.000 | **+2.880** | 0 | **2** | 7 |
| 10 s | 0.000 | **−2.293** | 0.000 | 3 | 0 | 6 |
| **30 s** | **−5.068** | **−6.920** | −4.619 | **9** | **0** | 0 |

**At 30 s v3 is strictly better in 9 of 9 rounds and never worse anywhere at
10 s or 30 s.** The 30 s column is the one the A/B/C predicted would move and it
moved by more than the probe did: 169.141 against 169.251 on seed 0, 169.928
against 171.739 on seed 1, and 172.086 on seed 2 where arm C published nothing
at all.

The 3 s regression is real and it is on seed 1 only, in 2 of 3 rounds. §5 says
what it is.

## 2.2 Wall, and who overran

Coordinator wall, median over nine rounds - the coordinator's own clock, which
is the clock its budget is quoted in:

| budget | v2 | v3 |
|---|---:|---:|
| 3 s | **4.23 s** | 2.71 s |
| 10 s | **10.57 s** | 9.24 s |
| 30 s | 14.16 s | 28.77 s |

**v2 overran its own budget at 3 s (by 41%) and at 10 s (by 6%); v3 overran
neither, in any of its 27 mixed-61 runs.** That is the affordability rule doing
what the ledger asked for: v2 asks "may I start?", v3 asks "can I pay for the
worst version of this I have seen?" and declines when it cannot. At 30 s the
direction reverses and it is the point: v2 saturates at 14.2 s with half its
budget unspent, and v3 spends 28.8 s of 30.

## 2.3 The seed-0 30 s trace

One paired round, verbatim from `evidence/curve-mixed61.json`. `val` is the
class's ranking value at the moment it was chosen; `est`/`act` are seconds.

```
#0  compression val=9.655 est=0.41 act=0.83  PUB 181.5890 -> 179.5869  m22 compress rank0 181.5890 -> 181.1890
#1  compression val=4.693 est=0.83 act=0.77      179.5869 -> 179.5869
#2  descent     val=3.738 est=0.51 act=0.80      179.5869 -> 179.5869
#3  crossover   val=1.793 est=1.15 act=1.92      179.5869 -> 179.5869  m23 rank0->rank1 cut=0.5 constant
#4  ladder      val=1.292 est=4.52 act=2.63  PUB 179.5869 -> 179.5101  m26 ladder 2 rungs
#5  compression val=3.520 est=0.83 act=0.65  PUB 179.5101 -> 179.5061
#6  compression val=2.818 ...                    (barren)
#7  descent     val=1.583 ...                    (barren)
#8  ladder      val=0.872 ...                    (barren)
#9  crossover   val=0.715 ... cut=0.5 constant   (barren)
#10 crossover   val=0.536 ... cut=0.487153832 derived band=42.116 mm   (barren)
#11 crossover   val=0.429 ... cut=0.514273886 derived band=64.733 mm   (barren)
#12 crossover   val=0.358 ... cut=0.471779904 derived band=18.455 mm   (barren)
#13 crossover   val=0.307 est=1.92 act=2.23  PUB 179.5061 -> 177.7698  cut=0.539114160 derived band=33.134 mm
#14 compression val=2.348 est=0.83 act=0.61  PUB 177.7698 -> 177.6340
#15 compression val=2.057 est=0.83 act=0.86  PUB 177.6340 -> 172.3770
#16 compression val=3.179 est=0.86 act=0.65  PUB 172.3770 -> 171.7384
#17 compression val=2.981 est=0.86 act=0.81  PUB 171.7384 -> 170.8920
#18 compression val=2.868 est=0.86 act=0.98  PUB 170.8920 -> 169.6118
#19 compression val=2.508 est=0.98 act=0.94  PUB 169.6118 -> 169.1406
#20 compression val=2.375 est=0.98 act=0.90      169.1406 -> 169.1406
#21 descent     val=1.187 est=0.80 act=0.91      169.1406 -> 169.1406
```

Three things in that trace are the whole stage.

**#13 is an action v2 could not name.** A derived interface-band cut at
`0.539114160`, in a 33.134 mm band, one pose apart from the constant `0.5` that
had already failed twice at #3 and #9. It published 1.736 mm and it is what
unlocked everything after it.

**#14 - #19 are the loop.** Six consecutive compression actions on six
successive incumbents, 177.770 -> 169.141, each one the schedule's own most
efficient operator pointed at a state that did not exist when the previous
action started. v2 has exactly one compression call per run and it is at the
wrong sign.

**#6 - #12 are seven barren actions in a row**, and the eighth published. Any
patience rule shorter than 8 would have destroyed this result. §5.2 measures
that number rather than guessing it.

# 3 - The work budget: quality, in-schedule, and the class economics

Work-budget mode is deterministic and load-independent, so one run per cell is
the whole measurement (`evidence/work-mixed61.json`).

| budget | seed | v2 | v3 | Δ | v3 spent | of budget | v3 exit |
|---|---:|---:|---:|---:|---:|---:|---|
| 40M | 0 | 174.20812003998896 | **170.63217550422073** | −3.576 | 37,575,714 | 93.9% | affordability |
| 40M | 1 | 176.05599999999998 | 176.05599999999998 | 0.000 | 39,177,529 | 97.9% | affordability |
| 40M | 2 | 179.006 | **172.89557339904468** | −6.110 | 38,960,559 | 97.4% | affordability |
| 120M | 0 | 174.20812003998896 | **169.14057315694365** | −5.068 | 113,968,463 | 95.0% | affordability |
| 120M | 1 | 176.05599999999998 | **169.92832830680420** | −6.128 | 117,233,295 | 97.7% | affordability |
| 120M | 2 | 179.006 | **172.086** | −6.920 | 115,712,290 | 96.4% | affordability |

v2 spends 27.9 - 32.4M whatever the budget is. Every v3 depth above is
`dualGateValid`.

**In schedule, not as a post-drain probe.** The drain phase published **0** in
all six v3 runs; the final publication's phase is `compression` in all six. The
A/B/C's 169.251 was one probe action taken *after* the schedule and the drain
had both finished; v3's 169.141 is action #19 of 25 with six more actions after
it.

## 3.1 What each class actually cost and produced

Pooled over the three seeds at 120M (`evidence/class-economics-120M.json`):

| class | actions | published | work units | per action | Δraw | **Δraw / M eval** | Δraw / action |
|---|---:|---:|---:|---:|---:|---:|---:|
| compression | 31 | **21** | 73,781,552 | 2.380M | **18.159** | **0.2461** | 0.586 |
| ladder | 6 | 5 | 95,052,124 | **15.842M** | 6.291 | 0.0662 | **1.048** |
| crossover | 22 | 3 | 124,660,637 | 5.666M | 5.337 | 0.0428 | 0.243 |
| descent | 10 | **0** | 26,050,367 | 2.605M | **0.000** | 0.0000 | 0.000 |

Per seed the three classes that published sum exactly to the run's whole gain
over its own mode-0 result: seed 0 `10.635 + 1.736 + 0.077 = 12.448` on
`181.589 -> 169.141`; seed 1 `1.395 + 2.880 + 5.486 = 9.762` on
`179.690 -> 169.928`; seed 2 `6.128 + 0.720 + 0.728 = 7.576` on
`179.662 -> 172.086`.

**The measured order is not the prior order, and the queue found that out
during the run.** Compression's prior was right and it is the schedule's
workhorse. Descent's prior (the second highest) was **wrong on this stream** -
ten actions, zero publications, 26.1M units - and the shrinkage is what stops
it after three or four per run rather than after ten. The ladder's prior was
too pessimistic: it is the *worst* class per evaluation and the *best* class
per action, and on seed 1 it produced 5.486 mm of a 9.762 mm run - more than
compression and crossover together.

**The ladder is 2 of 3, exactly as arm C was.** It carried seed 1 (5.486 mm)
and paid a little on seed 2 (0.728 mm) and almost nothing on seed 0 (0.077 mm).
That is the same 2-of-3 shape the A/B/C measured, on a different set of parents,
which is a modest independent confirmation that the mechanism is real and
seed-dependent rather than a seed-0 artefact.

## 3.2 The ledger's own "next action", executed

The ledger named, for each seed, the next untried derived crossover action in
its canonical order. For seed 2 that was `forward rank0 -> rank1`, cut fraction
**0.495566704**, band gap **0.578 mm**, 28 pieces from A and 33 from B, and the
A/B/C's arm A executed it as a probe and published **178.2857218718321**.

v3's seed-2 run reaches it as action **#6** of 23, in schedule, with the same
cut fraction and the same band gap to nine digits, and publishes the same
178.2857218718321 - and then spends eleven more actions taking it to 172.086.

# 4 - Generality: shapes-17 and triangle-20

Same code path, same derived budgets, 10 s and 30 s, three seeds, three rounds,
paired interleaved (`evidence/curve-shapes17.json`,
`evidence/curve-triangle20.json`, 72 runs).

## 4.1 shapes-17: no quality change, and a wall-time regression

| budget | v2 depth | v3 depth | paired Δ | v2 coordinator wall | **v3 coordinator wall** |
|---|---:|---:|---:|---:|---:|
| 10 s | 200.34937729570953 | 200.349 | −0.00038 (6 of 9 better, 0 worse) | 3.08 s | **9.60 s** |
| 30 s | 200.34937729570953 | 200.349 | −0.00038 (9 of 9) | 3.06 s | **28.90 s** |

The depth change is a rounding-scale gain from a crossover and is not a result.
**The wall is a regression and it is reported as one:** v2 terminates in 3.1 s
whether the budget is 3 s or 30 s, because every phase reaches its fixpoint;
v3 spends the whole budget. At 30 s it makes **281 crossover actions across nine
runs and gains 0.0034 mm** for them.

The mechanism is exact and is in the evidence: each crossover on this request
publishes a rounding-scale improvement, that publication is a new archive
member, the new member changes the frontier, and the frontier's ordered pairs
regenerate. The queue is not stuck - it genuinely has new keys - but the keys
are worth 12 µm each.

## 4.2 triangle-20: a 3 µm regression, and the constructor slice never runs

| budget | v2 depth | v3 depth | paired Δ | v2 wall | v3 wall |
|---|---:|---:|---:|---:|---:|
| 10 s | 70.72726178003285 | 70.73007 / 70.73005 / 70.72882 | **+0.00279** (0 of 9 better, **9 of 9 worse**) | 6.56 s | 9.39 s |
| 30 s | 70.72726178003285 | the same three depths | **+0.00279** | 6.49 s | 28.82 s |

**This is the one place v3 is worse on quality, and the cause is identified.**
Coordinator v2's own generality measurement found that on triangle-20 the
constructor slice published on **half its arms** (6 of 12) - the one request
where the m20 feeder pays. v3 makes the diversify class eligible only when the
priced queue is empty ("un ticket m20 quando non rimangono coppie
complementari", which is the review's own rule), and on triangle-20 the priced
queue **never empties**: crossover regenerates pairs at 217 actions per nine
runs. So v3 never draws a ticket at all, and it loses the 3 µm those tickets
were worth.

Note also that v3 at 30 s produces the identical depths to v3 at 10 s on all
three seeds: the extra twenty seconds buy 217 crossover actions and 0.0139 mm.

# 5 - Measured negatives, stated as such

## 5.1 The 3 s tier on mixed-61

v3 is worse in 2 of 9 rounds, all on seed 1, by 2.880 mm. At a 3 s budget the
protected mode-0 phase is ~0.75 of the whole, the queue can afford exactly one
action, and it spends it on compression (prior value 9.66) where v2 spends it
on a descent quantum. On seed 1 v2's `depth + 0.8` ask published 176.753 and
v3's `depth − 0.4` ask published 179.633.

**The compression prior is confounded with position in the schedule.** The
ledger measured compression at 1.10 mm/M in v2's compression *phase*, which ran
last, on a state two publications deep. The descent row was measured first, on
mode 0's own output. v3's ranking treats them as position-independent, and the
3 s tier is where that assumption is load-bearing and wrong. The honest reading
of the 3 s column is that at one action the *first* action's target matters and
the loose ask wins; from the second action on, the tight ask wins by 5-7 mm.
Note also that v2's 3 s arm takes 4.23 s of coordinator wall to produce its
better number, against v3's 2.71 s.

## 5.2 No stopping rule, and the number one would need

v3 has no global patience: it runs until the budget or a true all-actions
fixpoint, and §4 shows two requests where the fixpoint never arrives. The
obvious fix is a global barren-action patience, and this round measured the
statistic it would have to be sized from rather than guessing it
(`evidence/barren-runs.json`, longest barren run that was *followed by* a
publication):

| request / arm | actions | longest productive barren run | max trailing barren |
|---|---:|---:|---:|
| mixed-61 @ 10 s | 58 | 4 | 2 |
| **mixed-61 @ 30 s** | 170 | **7** | 7 |
| triangle-20 @ 10 s | 93 | 3 | 5 |
| triangle-20 @ 30 s | 295 | 3 | 28 |
| shapes-17 @ 10 s | 90 | 8 | 10 |
| **shapes-17 @ 30 s** | 350 | **33** | 35 |

So any patience must be **at least 8** or it destroys this stage's headline
result (the seed-0 30 s run's publication at #13 came after seven barren
actions), and shapes-17's churn needs one **at most 32**. The interval `[8, 32]`
is measured; the constant inside it is not, and this round declines to fit one
to nine runs on one request. It is the first thing the next round should size
properly - and, on shapes-17, the barren runs it would be cutting are between
publications worth 12 µm each, so a yield floor may be the better instrument
than a count.

## 5.3 The rest

* **The diversify class is scheduled by an eligibility rule, not by its rank**,
  and its rank would be meaningless: its work-unit price is four orders of
  magnitude wrong (the ledger) and its wall price is 12x wrong (§1.3). The rule
  costs triangle-20 3 µm (§4.2). Nothing here fixes m20's pricing.
* **Descent's prior is wrong on mixed-61** - 10 actions, 0 publications - and
  the shrinkage only limits the damage to 26.1M units of a 120M budget rather
  than preventing it.
* **The ladder is priced within a factor of 2.9 in both directions** and
  underprices in 2 of 6 (§1.3). At a 120M budget that costs nothing because the
  loop exits on affordability with headroom; at a wall budget it is why the
  ladder does not run at all below ~10 s, which is the correct behaviour and not
  a tuning.
* **One request, three seeds, for the headline.** shapes-17 and triangle-20 say
  the schedule *runs* generally; they say nothing about whether the 5-7 mm is
  general.
* **`0.002`, not `0.0005`.** Not comparable to the record lineage.
* **Wall against work.** Every quality number above is at a work budget or is a
  paired interleaved wall comparison over 9 rounds. The class `seconds` columns
  are process wall inside the coordinator's own clock, never thread sums, and no
  unpaired wall claim is made.

# 6 - Regression

## 6.1 The four pinned gates reproduce the pristine binary as whole documents

Both binaries built from this worktree - the pristine one from a detached
checkout of `fccda7f`, the measurement one from `wf-coordinator-v3` - and both
run through the same four gates:

| gate | pinned | fingerprint | fields compared | **differences** |
|---|---:|---|---:|---:|
| g1 mode 20 `independentDepthMm` | 206.869 | `8a7737381238fa4d` | 3,261 | **0** |
| g2 mode 22 raw | 159.09233022733062 | `fa01012af1d559ae` | 3,242 | **0** |
| g3 mode 22 raw | 159.07876040364795 | `e28fba007f8031d4` | 3,242 | **0** |
| g4 mode 22 raw | 164.0375677990678 | `49f094d7e59a9008` | 3,242 | **0** |

All four are `exactValid` and `contractValid` on both binaries. The comparison
is the whole document with wall-clock and build-identity fields removed;
`drivers/docdiff.py` lists exactly which, and the only fields it had to remove
beyond the clock were `engineCommit` and `engineWorktreeDirty`.

The gates never enter the coordinator at all - they are pinned-parent
positional replays - so this is a *check* rather than the argument. The argument
is that `run_portfolio` branches on `coordinator_v3` and the default is `false`.

**The coordinator's own default path reproduces the ledger to the unit.** The
`v3=0` arm of §3 is the same schedule the opportunity ledger measured at
`work=120,000,000` on the same three seeds, and it spends **32,393,757 /
31,957,935 / 27,938,867** work units for **174.20812003998896 /
176.05599999999998 / 179.006** - the ledger's Part 1 table, digit for digit and
unit for unit, from a binary that contains the whole v3 queue. That is a
stronger statement than the four gates make, because it is the *coordinator*
path rather than a path that never enters it.

## 6.2 Determinism: two processes, whole documents, one work budget

Both schedules, three seeds, `work=40,000,000`, two processes each, compared
field by field with wall-clock and build-identity fields removed
(`evidence/determinism.json`):

| arm | seed | raw depth | work units | fields compared | **differing** |
|---|---:|---:|---:|---:|---:|
| v2 | 0 | 174.20812003998896 | 32,393,757 | 3,255 | **0** |
| v2 | 1 | 176.05599999999998 | 31,957,935 | 2,648 | **0** |
| v2 | 2 | 179.006 | 27,938,867 | 3,233 | **0** |
| **v3** | 0 | 170.63217550422073 | 37,575,714 | 3,396 | **0** |
| **v3** | 1 | 176.05599999999998 | 39,177,529 | 2,761 | **0** |
| **v3** | 2 | 172.89557339904468 | 38,960,559 | 3,472 | **0** |

The work-unit spend is identical to the unit across processes, which is the
strong form: the queue's affordability decisions, its ranking values and its
action order are functions of the counters and of nothing else.

## 6.3 Suite

`cargo test --release --features jagua-experimental`: **1,244 passed, 0 failed,
2 ignored**, exit 0, including six new coordinator-v3 unit tests. Full log at
`evidence/suite.log`. No rerun was needed; the known-flaky
`free_material_multi_eviction` case passed first time.

# 7 - Files

* `drivers/runlib.py` - the pinned CLI tail, the salt sets and the work anchors,
  a diffable copy of the ledger's.
* `drivers/lib.py`, `drivers/gates.py` - the four pinned gates, `ROOT`
  repointed at this worktree.
* `drivers/battery.py` - the paired interleaved wall battery.
* `drivers/workquality.py` - v2 against v3 at identical work budgets.
* `drivers/determinism.py`, `drivers/recheck.py` - two processes, whole
  documents.
* `drivers/docdiff.py` - two gate runs, whole documents.
* `drivers/summarize.py`, `drivers/classeconomics.py`, `drivers/barrengaps.py` -
  the tables above.
* `drivers/smoke.py` - one v2 and one v3 run with the action trace printed.
* `evidence/*.json`, `evidence/suite.log` - every table above as measured.

Reproduce:

```
cargo build --release --example general_request_benchmark --features jagua-experimental

python3 drivers/battery.py curve-mixed61 3 mixed-61 0,1,2 \
    v2at3:wall:3000:0 v3at3:wall:3000:1 v2at10:wall:10000:0 v3at10:wall:10000:1 \
    v2at30:wall:30000:0 v3at30:wall:30000:1
python3 drivers/battery.py curve-shapes17   3 shapes-17   0,1,2 \
    v2at10:wall:10000:0 v3at10:wall:10000:1 v2at30:wall:30000:0 v3at30:wall:30000:1
python3 drivers/battery.py curve-triangle20 3 triangle-20 0,1,2 \
    v2at10:wall:10000:0 v3at10:wall:10000:1 v2at30:wall:30000:0 v3at30:wall:30000:1
python3 drivers/summarize.py     <battery.json> <summary.json>
python3 drivers/barrengaps.py    <battery.json> ...

python3 drivers/workquality.py   work-mixed61 mixed-61 0,1,2 40000000,120000000
python3 drivers/classeconomics.py <workquality.json> 120000000 <out.json>
python3 drivers/determinism.py   determinism mixed-61 0,1,2 40000000

python3 drivers/gates.py pristine <pristine-binary> /var/lib/t3/tmp/v3/gates/pristine
python3 drivers/gates.py v3final  <v3-binary>       /var/lib/t3/tmp/v3/gates/v3final
python3 drivers/docdiff.py /var/lib/t3/tmp/v3/gates pristine v3final
```

The schedule is armed by one portfolio spec key, `v3=1`; absent or `v3=0`,
every existing invocation is byte-identical to coordinator v2's.
