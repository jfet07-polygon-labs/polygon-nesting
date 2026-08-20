# Pricing the m34 slice: one gate re-priced, one bit, and three mechanisms that did not survive their own measurement

> Grok review 1 §2b items **1**, **3** and **4**, measured on HEAD `65f6fc9`
> (coordinator v5, with the self-metered debit merged), x86_64, 16 cores. The
> box was shared with one other measurement agent for the whole campaign, so
> every wall claim below is a paired interleaved difference over nine rounds
> with the within-arm spread printed beside it.

## What was asked, and what happened

| Grok §2b | asked for | verdict |
|---|---|---|
| **1** wall-price the first m34 action | a wall prior from p95/worst-of-request, ratchet after, **keep a floor** | **ships, on the affordability gate only.** Pricing the *ranking* at the same worst case was built first and measured to cost a median **0.649 mm** over nine paired thirty-second rounds on mixed-61 (§1.1) |
| **3** feasible entry without the overlay | `global_legalize` the parent at its own depth, `parentProxyFeasible=true` at entry, else skip | **measured negative, twice.** The repair reaches feasibility on **0 of 9** slices, and the entry is infeasible on the request where the class publishes 9 of 9 exactly as it is on the two where it never has, so a skip on that test refuses every slice on every request. A second discriminator - the entry's own depth loss - fires nowhere (§2) |
| **4** one-bit per-request prior | prior zero / cost infinity for m34 after N=0 publications on a request, rare audition | **ships**, as a within-run bit; the engine has no cross-run store and §4 is explicit that this is a deviation |
| — | *not asked for; the measurement asked for it, and then took it back* | **the probe**: buy the first slice on a prior, *continue* it on evidence. It works, and it is **off**: the wall it returns buys no depth, and at thirty seconds it abandons slices that publish (§3) |

**What ships:** `m34wall=1` (the affordability gate reads the worst-case wall
price) and `m34bit=1` (a class that has published nothing here comes off the
queue), both on by default inside v3, which is itself off by default.

**What does not:** `m34entry`, `m34skip`, `m34drop`, `m34probe`, all off, all
kept in the tree with their evidence, because "we measured it and it did not
work" is a stronger statement when the thing measured is still there and can be
switched on by a reader who disagrees.

---

## 1. The re-baseline: what the first slice costs on HEAD

Coordinator v4 §8 called the schedule class's wall price "the weakest number in
this stage" and quoted 2.54-2.59x on mixed-61, 2.94-3.07 on shapes-17 and 5.1 on
triangle-20 against the work-denominated prior. Those are v4-era numbers and
this round does not reuse them. Re-measured on HEAD over **18 cells** - three
requests, three seeds, ten- and thirty-second budgets, one process per cell
(`drivers/firstslice.py`, `evidence/base-firstslice.json`):

| request | actual/estimate | first slice, in phase-zeros |
|---|---|---|
| mixed-61 | 2.601 - 3.013 | 0.990 - **1.147** |
| shapes-17 | 2.990 - 4.255 | 1.138 - **1.619** |
| triangle-20 | 5.581 - 5.879 | 2.124 - **2.238** |

The v4 ratios reproduce and are, if anything, slightly worse on HEAD. The right
column is the one a prior needs: **2.2375** phase-zeros is the worst of the
eighteen, and that is what `SCHEDULE_WALL_PRIOR_PHASE_ZEROS` is. Every other
class in this table is priced by its own worst case - the ladder by the largest
of three arm-C spends, the diversify class by the worst of three requests in
each of its two currencies - and this is that rule applied to the class v4 said
to apply it to next.

The same table already contains the round's first problem. A worst-case wall
prior is **2.2x too high on mixed-61**, where this class publishes on nine of
nine at ten seconds and buys the run its whole advantage over v3. Quoting the
queue's ranking value against it drops the class from 2.901 to `1.104 / 2.2375
= 0.493` - below the ladder (1.292) and below crossover (1.793) - and a class
that never wins a rank never earns the evidence that would displace the prior.
That is coordinator v4 §3.1's "a prior of zero is not a prior, it is a deletion"
arriving from the cost side.

### 1.1 The first cut priced both rules at the worst case, and this is what that cost

The obvious implementation gives the class one wall price and lets the
affordability rule and the ranking value both read it. That was built, and run
as a full nine-round paired battery on mixed-61 before anything else was
measured (`evidence/curve-mixed61-priorfloor.json`, arm
`m34wall=1,m34probe=3,m34bit=1` against HEAD):

| tier | paired median | rounds better / worse | m34 slices per run, head -> new | overruns, head -> new |
|---|---:|---|---|---|
| 3 s | 0.000 mm | 0 / 0, 9 equal | 0.00 -> 0.00 | 2 of 9 -> 3 of 9 |
| **10 s** | **0.000 mm** | **2 / 0**, 7 equal | 1.00 -> 1.00 | 3 of 9 -> 4 of 9 |
| **30 s** | **+0.649 mm worse** | 0 / **8**, 1 equal | 2.89 -> **1.00** | **2 of 9 -> 0 of 9** |

At ten seconds it is harmless and twice a gain (one round by 6.546 mm). At
thirty seconds it costs a median 0.649 mm and eight rounds of nine, and the
mechanism is visible in the slice count: 2.89 slices per run become 1.00. Those
later slices are not speculative - in the same battery they publish on **23 of
26**. The cause is not the affordability gate; it is the *rank*. A class quoted
at `1.104 / 2.2375 = 0.493` sits below the ladder (1.292) and below crossover
(1.793) and simply never wins another rank.

**Retraction, of this round's own first design:** "the schedule class should be
priced at its worst-case wall" is right for one of the two rules and wrong for
the other, and the arm that did both is the one measured in the table above. It
is kept in the evidence directory rather than deleted.

### 1.2 What ships: the affordability gate, for the first slice only

A worst-case wall price is the right answer to "can I afford to finish this?"
and the wrong answer to "is this worth buying?" The two questions have different
failure modes - an overrun and a mis-purchase - and different statistics - the
tail and the centre. And a cross-request worst case is the right answer only
until this run has a same-request measurement of its own. So, for the schedule
class under a wall budget:

* **Ranking** reads what coordinator v4 read, always:
  `max(0.3806 * phase_zero, this run's worst slice)`.
  `Coordinator::class_rank_cost_estimate` is v4's `class_cost_estimate` for this
  class, unchanged.
* **Affordability** reads `2.2375 * phase_zero` while the class has bought
  nothing in this run, and **this run's own worst slice** thereafter - Grok's
  "p95/worst of the same request, **ratchet after**", with the portfolio worst
  case standing in until the request has spoken.

That leaves the prior doing exactly the job item 1 named - **the first action has
no ratchet** - and nothing else. §5.2 measures both of the wider readings that
were tried first and what each cost.

The floor Grok asked for ("at least one slice if eligible") falls out without a
special case: the class keeps the rank v4 gave it, so it is still chosen where it
was chosen before, and the budget refuses it only when the worst case does not
fit in what is left. Measured, that refusal happens on 18 of 18 three-second
runs on shapes-17 and triangle-20 and on 0 of 54 mixed-61 runs at any tier.

---

## 2. Item 3, measured: the entry repair does not reach feasibility, and entry feasibility is not the discriminator

The port's §7.1 is the fact this item is built on: every 171-179 mm coordinator
parent arrives proxy-infeasible at 26-38 colliding pairs, because
`initialize_complete_state` snaps continuous rotations onto the surrogate's
2.5-degree grid. Grok's proposal was to run the existing translation-only
`global_legalize` on the parent before step 0, and to skip the slice if the
entry was still infeasible.

Both halves were built (`CompressionScheduleSettings::legalize_entry`,
`skip_infeasible_entry`) and both were measured, nine cells at ten seconds,
`drivers/entrycensus.py`, `evidence/probe-sweep.json` (the
`off-with-entry-legalization` arm).

### 2.1 The first cut was wrong and the round says so

The first implementation bounded the program at the **parent's** depth. The
lane's parent is the *snapped* state and it is deeper than the parent it came
from, so that bound asks a translation-only legalizer to compress as well as
separate. It came back with no layout at all on **9 of 9** slices. The bound is
now the entry's own measured depth. This is recorded rather than quietly fixed
because it is the difference between "the instrument does not work" and "we
pointed it at the wrong number".

### 2.2 With the bound corrected, it still does not close the books

| request | parent proxy pairs -> after repair | entry feasible after | repair accepted | repair ms |
|---|---|---|---|---|
| mixed-61 | 28 -> 12, 43 -> 43, 34 -> 34 | **0 of 3** | 1 of 3 | 4.7 - 11.4 |
| shapes-17 | 5 -> 5 (x3) | **0 of 3** | 0 of 3 | 7.8 |
| triangle-20 | 0 -> 0 (x3) | **0 of 3** | 1 of 3 | 0.4 - 14.2 |

The program's own reasons, from its diagnostics: on mixed-61 it closes the
boundary rows and leaves 2-5 violating pairs; on shapes-17 it closes all four
violating pairs and leaves **one** boundary piece; on triangle-20 it has **zero**
violating pairs to begin with and 20 boundary pieces, of which it clears all 20
on one seed and none on the other two. Nine cells, nine different failures of
the same shape: the exact tier and the proxy tier disagree about this parent by
construction, and closing one does not close the other.

**A skip on `parentProxyFeasible` would therefore refuse every slice on every
request**, including mixed-61's 9 of 9. It is not shippable and it is off.

### 2.3 The entry is not one phenomenon, and that is the useful finding

The three requests are not infeasible for the same reason, and the slice does
not spend its wall in the same place:

| request | parent pairs | parent boundary violations | confirmation ms | repair ms | slice s |
|---|---:|---:|---:|---:|---:|
| mixed-61 | 28 - 43 | 4 - 8 | **1120 - 1446** (50-76%) | 393 - 1060 (21-47%) | 1.90 - 2.25 |
| shapes-17 | 5 | 1 | 158 - 169 (11-15%) | **817 - 1222** (76-82%) | 1.08 - 1.49 |
| triangle-20 | **0** | 7 - 11 | 2 - 15 (0.1-0.8%) | **1792 - 2949** (98-100%) | 1.82 - 2.95 |

Grok's "a fraction of every slice is regrid, not compression" is **right for
triangle-20 and wrong for mixed-61**: triangle-20's slice is 98-100% repair
sweeps against a frontier that never becomes publishable, and mixed-61's is
majority exact-tier confirmations, which is the tier it is being paid to spend
its wall in. Two of the three requests have *no* meaningful collision-pair
problem at entry at all - triangle-20 has **zero** colliding pairs and is
infeasible on boundary violations alone.

### 2.4 The second discriminator also fails

If feasibility does not separate the requests, perhaps arithmetic does: the lane
publishes only a layout whose *source* depth beats the parent's, and it may walk
only `requested_drop_mm` of clamp to get there. A slice that arrives more than
its own drop above the parent cannot publish however well it goes. That test is
one depth measurement, it carries no request in it, and it is built
(`skip_unpublishable_entry`).

Measured, it fires nowhere, and on one request it cannot even be evaluated:

| request | entry depth loss | requested drop | unpublishable? |
|---|---|---:|---|
| mixed-61 | 0.148 mm on seed 0; **not measurable** on seeds 1 and 2 | 1.611 - 1.617 mm | no |
| shapes-17 | 0.327 mm (all three seeds) | 1.803 mm | no |
| triangle-20 | 0.143 / 0.379 / 0.380 mm | 0.636 - 0.637 mm | no |

Every slice whose entry depth can be measured *could* publish arithmetically -
the loss is 8% to 60% of the drop - and two of the three requests never do. The
entry-loss median the port measured at +0.448 mm is reproduced here at
0.143-0.380 mm on the coordinator's own parents.

The "not measurable" cells are a finding of their own and they are the second
reason this rule is off: `coupled_independent_source_depth` is a measurement of
a *valid* layout, and on two of three mixed-61 seeds the snapped entry is not
one, so the test has no number to compare. A gate that cannot evaluate its own
predicate on the request that matters most is not a gate. The rule is kept, off,
because it is cheap and correct where it can be evaluated; it bought nothing
here and is not claimed to.

**Retraction owed to this round's own first hypothesis:** "the slice is
unpublishable because the snap costs more than the drop" is false wherever it
can be tested. It was a good hypothesis and the data says no.

---

## 3. The probe: the only mechanism that can charge the *first* slice, built, measured, and switched off

A prior can decide *whether* to buy the first slice; it cannot make the first
slice cheaper. §2 says nothing observable before step 0 separates the request
where this class pays from the two where it does not - but §2.3 says the
difference is plain from the slice's own steps. So the first slice's price is
charged as an **anytime budget** instead:

> spend `steps_planned / n` steps; if nothing reached beats the parent yet,
> abandon the slice.

This is the coordinator's own audition rule one level down. It works, it is
cheap, its constant is ablatable - and it is off by default, for two measured
reasons that are not the ablation.

### 3.1 The denominator has a measured floor at ten seconds

Nine cells, three requests, three seeds, ten seconds, every other mechanism off
(`evidence/probe-sweep.json`):

| denominator | mixed-61 slices publishing | shapes-17 first slice | triangle-20 first slice |
|---|---:|---:|---:|
| off (HEAD, `evidence/base-firstslice.json`) | **3 of 3** | 1.07 - 1.46 s | 1.64 - 1.92 s |
| 2 | **3 of 3** | 0.50 - 0.74 s | 0.13 - 0.38 s |
| **3** | **3 of 3** | **0.38 - 0.50 s** | **0.017 - 0.020 s** |
| 4 | **2 of 3** | 0.31 - 0.38 s | 0.012 - 0.015 s |

**At four the mechanism breaks mixed-61.** Seed 2's slice is abandoned at step
402 having spent 0.60 s and published nothing; its run finishes at 178.286 mm
against 176.162 mm with the probe at three - a **2.1 mm** regression on one seed
in one round. At three, all three seeds keep their publication and their depth,
and the sterile slices this is aimed at cost **2 to 100 times less wall**.

Three is where the entry loss puts the floor *at ten seconds*: a lane arrives
0.148-0.380 mm above its parent against a drop of 0.636-1.803 mm, so a probe has
to outlast that before it can ask the lane for evidence the lane could not yet
have produced.

### 3.2 It makes the class *cheaper* and therefore more *frequent*

The same sweep, slice counts over three runs:

| request | slices, probe off | slices, probe at 3 |
|---|---:|---:|
| mixed-61 | 3 | 3 |
| shapes-17 | 3 | 6 |
| triangle-20 | 3 | 10 |

A slice that costs 18 ms stays affordable and its `cost_max` ratchet no longer
prices the class out, so the queue buys it again and again. The sterile bit (§4)
is what stops that, and the two would have shipped together.

### 3.3 First reason it is off: the wall it returns is unspendable

The sweep's own published depths, at ten seconds, over all four arms:

| request | every probe arm | HEAD's own ten-second numbers |
|---|---|---|
| shapes-17 | 200.349 / 200.349 / 200.349 | the same |
| triangle-20 | 70.73007 / 70.73005 / 70.72882 | the same, and coordinator v4's |

One to two seconds of a ten-second budget come back on the two requests where a
sterile slice exists to cut, and **nothing in the queue can spend them**. On
shapes-17 at ten seconds nothing publishes at all in any arm; on triangle-20 the
depth is identical to the digit. The mechanism returns wall it cannot convert.

### 3.4 Second reason: the floor moves with the parent, and at thirty seconds it moves past three

The entry loss is not a constant of the request, it is a property of the parent,
and deeper parents arrive further above themselves. On mixed-61 seed 0 at thirty
seconds:

| slice | parent | entry loss | drop | loss as % of walk | probe at 3 | outcome |
|---|---:|---:|---:|---:|---:|---|
| #3 | 179.587 | 0.158 mm | 1.616 mm | 10% | 538 of 1616 | publishes, 1.41 mm |
| #17 | 168.940 | **0.453 mm** | 1.520 mm | **30%** | 506 of 1520 | **abandoned** at 1.30 s, 10 confirmations |

The unabridged #17 takes 2.61 s and publishes 1.03 mm; the run that abandoned it
finishes at 168.940 against 166.808 - **2.132 mm worse** on that round. The
probe at three is above the *first* slice's handicap and below the second's.

So the shape is right and the budget is wrong: a step count cannot know how far
the lane has to walk before it is allowed to have evidence. The key stays, off,
with the sweep and the counter-example, so the next attempt starts from an
entry-loss-relative probe rather than from scratch. §5's third arm measures what
arming it costs across all three requests and all three tiers.

---

## 4. Item 4: the one bit, and what it actually is

The class published **0 of 29** actions on shapes-17 and **0 of 37** on
triangle-20 pooled over both budget tiers (coordinator v4 §8), against 9 of 9 at
ten seconds on mixed-61. Grok asks for prior zero / cost infinity for m34 *on
that request*, with a rare audition.

Implemented as: once the class has spent `SCHEDULE_STERILE_ACTIONS = 1` action
on this request and published nothing, its candidates are withheld from the
queue; after `SCHEDULE_AUDITION_BARREN = 16` further barren actions it is
offered once, and once only, per run.

Three deliberate choices:

* **It filters the candidate list, not the prior.** A prior of zero is a
  deletion the class can never argue with; a candidate withheld is a candidate
  the audition hands back. The class keeps its prior, its stats and its ratchet
  throughout.
* **The audition is at 16, not at the diversify class's 8.** That class is being
  promoted *into* a queue that outranks it; this one is being let back into a
  queue it was removed from, which is a weaker claim and gets the more
  conservative number. On the measured streams it fires at most once and usually
  never.
* **It is a within-run bit, not a per-request memory, and that is a deviation
  from what was asked.** The engine is one process per request with no
  persistent store, so "after N=0 publications on a request" can only mean "after
  N=0 publications *in this run*". A rule that remembered across runs would be a
  different artefact - a cache with an invalidation problem - and this round did
  not build one. Where the review says the prior should be zero *there*, this
  says the queue stops offering it *here*, and **the first slice on each request
  is still bought at full price**. That first slice is what §3's probe was for,
  and §3 is why the probe is off - so on shapes-17 and triangle-20 this round
  returns the second and later sterile slices and not the first one.

---

## 5. The anytime curves

Three seeds, three rounds, paired and **interleaved with the arm order rotating
every round**, one process per cell, from the bare request, at 3 / 10 / 30 s on
all three requests. The baseline arm is HEAD `65f6fc9` reached from the same
binary by `m34wall=0,m34entry=0,m34skip=0,m34drop=0,m34probe=0,m34bit=0` - one
binary, two arms, which is the only kind of paired comparison worth anything on
a box shared with another measurement agent.

Two batteries were run to completion and both are in the evidence directory,
because the second exists to correct the first:

* `evidence/curve-*-priorheld.json` - three arms, 243 runs. The wall prior held
  over **every** slice, plus the probe as a third arm. This is the ablation.
* `evidence/curve-*.json` - two arms, 162 runs. The shipping arm: the wall
  prior on the **first** slice only, sterile bit on, probe off.

### 5.1 The shipping arm against HEAD

Depth first, because it is the only thing the coordinator is paid for. Paired
per round, 27 rounds per request, `evidence/curves-summary.json`:

| request | tier | paired median | min / max | better / worse / equal |
|---|---|---:|---|---|
| mixed-61 | 3 s | 0.00000 | 0 / 0 | 0 / 0 / **9** |
| mixed-61 | **10 s** | **0.00000** | 0 / 0 | 0 / **0** / **9** |
| mixed-61 | 30 s | 0.00000 | −0.082 / 0 | **1** / **0** / 8 |
| shapes-17 | 3 s | 0.00000 | 0 / 0 | 0 / 0 / **9** |
| shapes-17 | 10 s | 0.00000 | 0 / 0 | 0 / 0 / **9** |
| shapes-17 | 30 s | 0.00000 | 0 / 0 | 0 / 0 / **9** |
| triangle-20 | 3 s | 0.00000 | −0.004 / 0 | **3** / **0** / 6 |
| triangle-20 | 10 s | 0.00000 | 0 / 0 | 0 / 0 / **9** |
| triangle-20 | 30 s | 0.00000 | 0 / 0 | 0 / 0 / **9** |

**Nine cells, 81 paired rounds, and the arm is never worse in any of them.** It
is better in four: one mixed-61 round at 30 s by 0.082 mm and three triangle-20
rounds at 3 s by 0.004 mm. Every seed's published depth at every tier is
identical to HEAD's own, to the digit - mixed-61 at ten seconds is 172.288 /
171.362 / 176.162 in both arms and in all three rounds, and the m34 class makes
one action and publishes on it in **9 of 9** runs in both arms. Grok's
mixed-61 success criterion - "keeps 9/9-or-better" - is met by keeping the 9/9
itself rather than a statistic about it.

Then the wall, which is what was bought:

| request | tier | m34 slices (publishing), HEAD -> arm | m34 s per run | coordinator wall, median | overruns |
|---|---|---|---|---|---|
| mixed-61 | 3 s | 0 (0) -> 0 (0) | 0.00 -> 0.00 | 2.68 -> 2.65 s | 0/9 -> 0/9 |
| mixed-61 | 10 s | 9 (9) -> 9 (9) | 2.06 -> 2.06 | 9.96 -> 10.06 s | 3/9 -> 5/9 |
| mixed-61 | 30 s | 18 (15) -> 20 (17) | 5.08 -> 5.66 | 29.28 -> 29.35 s | 3/9 -> 3/9 |
| **shapes-17** | **3 s** | **9 (0) -> 0 (0)** | **1.21 -> 0.00** | **2.96 -> 1.85 s** | **3/9 -> 0/9** |
| shapes-17 | 10 s | 9 (0) -> 9 (0) | 1.21 -> 1.21 | 9.48 -> 9.53 s | 0/9 -> 0/9 |
| **shapes-17** | **30 s** | **18 (0) -> 9 (0)** | **2.27 -> 1.21** | 18.73 -> 18.13 s | 0/9 -> 0/9 |
| **triangle-20** | **3 s** | **9 (0) -> 0 (0)** | **1.79 -> 0.00** | **3.70 -> 3.10 s** | **9/9 -> 6/9** |
| triangle-20 | 10 s | 9 (0) -> 9 (0) | 1.79 -> 1.79 | 9.37 -> 9.39 s | 0/9 -> 0/9 |
| **triangle-20** | **30 s** | **36 (0) -> 9 (0)** | **7.53 -> 1.78** | **29.03 -> 28.61 s** | 0/9 -> 0/9 |

Three readings, in the order they matter.

**The three-second tier.** HEAD offers this class a slice with about a second of
its budget left, prices it at 0.35 phase-zeros, and pays 1.11 s (shapes-17) or
1.82 s (triangle-20) for it. It publishes nothing, on either request, in 18 of
18 runs. Priced at its own worst case the slice does not fit and is refused:
shapes-17's coordinator wall falls **2.96 s -> 1.85 s (-37%)** with its overruns
going **3 of 9 -> 0 of 9**, and triangle-20's falls **3.70 s -> 3.10 s (-16%)**
with its overruns going **9 of 9 -> 6 of 9**. The published depth is identical
in 18 of 18 rounds, and three triangle-20 rounds are 4 µm better.

**The thirty-second tier on the two requests where the class is sterile.**
triangle-20's HEAD arm takes **36 m34 slices across nine runs - four per run,
7.53 s per run, a quarter of the budget - and publishes on none of them.** The
sterile bit reduces that to one slice per run, 1.78 s, and the depth is
identical in 9 of 9 rounds. shapes-17 halves the same way, 18 slices to 9. This
is the item-4 claim, measured: 5.75 s and 1.06 s per run returned for no depth.

**mixed-61 is untouched, which is the point.** The class keeps its slice count,
its publications and its depth at all three tiers; at thirty seconds it takes
*more* slices than HEAD in this battery (20 against 18, 17 publishing against
15) because the first-slice price never binds where the budget is large.

**What the wall price did *not* buy: mixed-61's overruns.** 3 of 9 at 10 s and
3 of 9 at 30 s in both arms, worst 32.07 s (HEAD) against 31.99 s (arm). This is
an honest miss against Grok's "and overruns drop": the overruns on this request
are on *later* actions - a ladder rung and a second or third schedule slice,
both priced by the run's own ratchet - and a first-slice prior cannot reach
them. Coordinator v4 §4.1's 5.12 s slice is of that kind too. The overruns this
round *does* remove are the three-second ones on the other two requests, where
the offending slice is the first.

### 5.2 The ablation: holding the prior over later slices, and arming the probe

From `evidence/curves-priorheld-summary.json`. The `bnew` arm here is the wall
prior held over every slice; `cprobe` is that arm plus `m34probe=3`.

| request | tier | `bnew` - HEAD | `cprobe` - HEAD | HEAD's own within-arm spread |
|---|---|---|---|---|
| mixed-61 | 3 s | 0.000, 9 equal | 0.000, 9 equal | 0.000 |
| mixed-61 | 10 s | 0.000, 9 equal | 0.000, 9 equal | 0.000 |
| **mixed-61** | **30 s** | **+0.137 median**, 0/6/3, max +3.952 | **+0.082 median**, 0/6/3, max +3.952 | **1.820** |
| shapes-17 | all | 0.000, 9 equal | 0.000, 9 equal | 0.000 |
| triangle-20 | 3 s | 0.000, 1 better | 0.000, 1 better | 0.154 |
| triangle-20 | 10 s | 0.000, 1 better 1 worse | same | 0.041 |
| triangle-20 | 30 s | 0.000, 1 better 1 worse | 0.000, 2 better 1 worse | 0.003 |

The 30 s mixed-61 row is the reason §1.2's rule is "the first slice only". Read
per round it is not a median at all: seed 1 is identical in 3 of 3, seed 2 is
+0.137 in 3 of 3, and **seed 0 is +2.132 / +3.952 / +3.952**. The trace says
exactly where it goes (`aheadat30-s0-r0` action #17 against `bnewat30-s0-r0`
#17): the queue ranks a schedule slice first, the affordability gate prices it
at `max(2.2375 * 1.979, 1.924) = 4.428 s`, less than that is left, and the slice
is refused. HEAD buys it, it costs 2.606 s, it publishes 1.03 mm, and HEAD's run
finishes at 29.17 s - **inside** its own budget. A worst-case price refused a
slice that fit.

It is worth reading against the noise: HEAD's own within-arm spread at this cell
is **1.820 mm**, so a +0.137 mm median is inside the baseline's round-to-round
variation and the +3.952 mm rounds are not. Both are reported.

Two things this arm bought that the shipping arm buys too, and one it bought
that the shipping arm gives up:

* The three-second wall return and the thirty-second slice count are the same in
  both arms - the wall prior binds the *first* slice on shapes-17 and
  triangle-20 either way, and the sterile bit cuts the later ones either way. In
  this battery shapes-17 goes 3.09 s -> 1.90 s at 3 s with overruns 7 of 9 -> 1
  of 9, triangle-20 3.70 s -> 2.75 s with overruns 8 of 9 -> 3 of 9, and
  triangle-20's 30 s m34 spend goes 6.39 s -> 1.88 s per run.
* It also took mixed-61's own 30 s overruns from **2 of 9 to 0 of 9** by
  refusing later slices. The shipping arm does not: it leaves later slices
  priced by the ratchet, which is what HEAD does, and mixed-61's overruns stay
  where HEAD has them (§5.1, last paragraph). That is the price of the 0.137 mm
  and it is paid deliberately.

And the probe, as the third arm: it halves the m34 wall again wherever a sterile
slice survives (shapes-17 10 s and 30 s, 1.21 -> 0.45 s per run; triangle-20
10 s and 30 s, 1.44 -> 0.01 and 1.88 -> 0.02) with **no depth change anywhere**,
and it is off for the reasons in §3.3 and §3.4: it returns wall nothing can
spend, and its step-count budget does not know how far a deeper parent has to
walk before it is allowed to have evidence.

---

## 6. Regression, determinism and suites

### 6.1 The four pinned gates, on three binaries

`drivers/gates.py` with `drivers/gatelib.py`, the **repaired** `doc_digest` from
`docs/experiments/se2-rigidity/drivers/lib.py` with `ROOT` repointed at this
worktree - the one whose VOLATILE list drops the five elapsed-derived summary
statistics and `engineWorktreeStatus`, so a digest match means something.

| binary | features | g1 206.869 | g2 159.09233022733062 | g3 159.07876040364795 | g4 164.0375677990678 |
|---|---|---|---|---|---|
| base commit `65f6fc9` | jagua+compression | hit | hit | hit | hit |
| this branch, gate build | jagua only | hit | hit | hit | hit |
| this branch, measurement build | jagua+compression | hit | hit | hit | hit |

All four fingerprints reproduce (`8a7737381238fa4d`, `fa01012af1d559ae09c`,
`e28fba007f8031d49f`, `49f094d7e59a9008`), and the **whole-document digests are
identical across all three binaries** on every gate:
`31e0daf0b537b259` / `a449f71267615de7` / `df2a7aa132357eec` /
`9c950d0f37512260`. `evidence/gates-*.json`.

That the gate build (no `compression-schedule`) is byte-identical to the
measurement build on these documents is the statement that matters here: mode 34
does not exist in it, so nothing this round touches can reach the pinned path.

### 6.2 Flag-off reproduces the base commit, whole documents

`drivers/reproduce.py`, nine cells at `work=40,000,000`, the base-commit binary
against this branch's binary with
`m34wall=0,m34entry=0,m34skip=0,m34drop=0,m34probe=0,m34bit=0`:
**9 of 9 equal**, whole documents, with only the elapsed-derived fields (every
key ending in `Seconds` or `Ms`, plus `occupancyOverTime` and the bare `seconds`
on action and publication rows), the build-identity fields and this round's own
new `scheduleSlice` block removed from *both* sides. Depths reproduce to the
digit: 169.891 / 171.3619986855876 / 170.155 on mixed-61,
70.7711336311948 / 70.74684851410467 / 70.74164651120441 on triangle-20.
`evidence/reproduce.json`.

### 6.3 Work-budget determinism, two processes

`drivers/determinism.py`, the **shipping** arm at `work=40,000,000`, two
processes per cell, whole documents: **9 of 9 equal** on all three requests.
The three mechanisms - a queue filter, a re-priced gate and a step-counted probe
inside the operator - are all functions of the archive and of counters, not of
the clock. `evidence/determinism.json`.

### 6.4 Both suites

* `cargo test --release --features jagua-experimental` - **EXIT=0**, 1,260
  passed, 0 failed (`evidence/suite-jagua.log`).
* `cargo test --release --features jagua-experimental,compression-schedule` -
  **EXIT=0**, 1,282 passed, 0 failed (`evidence/suite-compression.log`).

Every exit code in this round was captured with `cmd > log 2>&1; echo EXIT=$?`
and never through a pipe.

Four unit tests were added and one was rewritten
(`only_the_constructor_slice_is_priced_twice` becomes
`only_the_two_measured_classes_are_priced_twice`, because the compression
schedule is now the second class with two currencies).

---

## 7. Honest limits

* **The 10 s tier does not move, on any request.** Every published depth is
  identical to HEAD's in 27 of 27 rounds per request. This round returns wall at
  3 s and at 30 s and buys no millimetre anywhere. Against the binding priority -
  quality at ten seconds from a bare request - **the honest headline is zero**,
  and the reason is in §5.1: at ten seconds mixed-61's slice publishes and is
  worth keeping, and shapes-17's and triangle-20's are sterile but affordable,
  so nothing this round prices changes what gets bought.
* **The wall returned at three seconds is not converted into depth either.** On
  shapes-17 the run finishes at 1.85 s instead of 2.96 s with the same layout;
  the coordinator has no actions left it can afford, so the seconds are simply
  not spent. The claim is "the budget is now honoured", not "the budget is now
  better used".
* **mixed-61's own overruns are untouched** (§5.1, last paragraph). Grok's
  criterion said overruns should drop; they drop on two requests at one tier and
  not on the third at any tier, because a first-slice prior cannot price a
  third-slice overrun.
* **The wall prior is a portfolio constant, not a per-request one.** Grok asked
  for "p95/worst of the *same* request". The coordinator has no same-request
  sample before the first slice, so 2.2375 is the worst of three other requests
  and the same-request sample takes over the moment there is one. On mixed-61,
  where the true multiple is 1.02, the prior is 2.2x too high for exactly one
  action per run.
* **The sterile bit is a within-run bit** (§4). The engine has no cross-run
  store and this round did not build one.
* **Three mechanisms are in the tree and off**: `m34entry` / `m34skip` (§2),
  `m34drop` (§2.4), `m34probe` (§3). Each has its measurement in this directory.
  None of them is claimed to work.
* **This round's own first two designs are retracted in the text rather than
  deleted**: the wall prior read by the ranking rule (§1.1, 9 paired rounds,
  median 0.649 mm) and held over later slices (§5.2, 9 paired rounds, median
  0.137 mm and up to 3.952 mm on one seed). Both batteries are in
  `evidence/curve-*-priorfloor.json` and `evidence/curve-*-priorheld.json`.
* **Wall against work.** Every quality number here is a paired interleaved wall
  comparison over nine rounds with the within-arm spread reported beside it,
  except the reproduction and determinism checks, which are at a work budget.
  The box was shared with one other measurement agent for the whole campaign,
  and the 10 s overrun counts (mixed-61, 3 of 9 against 5 of 9, worst 10.17 s
  against 10.29 s) are inside that noise and are not a finding in either
  direction.
* **`0.002`, not `0.0005`.** Not comparable to the record lineage.
* **Nothing here touches the 21 mm gap to Sparrow's 150.165.** Grok's own
  estimate for items 1-4 was that they close wall-against-work, not the gap; the
  measurement here is that items 1, 3 and 4 close a slice of the *wall* and none
  of the millimetres.

---

## 8. Files

* `drivers/runlib.py` - the pinned CLI tail, the salt sets and the `0.002`
  allowance, repointed from `coordinator-v4/drivers/runlib.py`.
* `drivers/battery.py` - the paired interleaved arm battery, from
  `coordinator-v4/drivers/battery.py` plus the overrun column and the m34 slice
  aggregation.
* `drivers/firstslice.py` - the 18-cell first-slice wall census the prior is
  built from.
* `drivers/entrycensus.py` - one row per m34 *slice*: entry feasibility before
  and after the repair, entry depth loss, where the slice's wall went.
* `drivers/summarize.py` - the §5 tables.
* `drivers/reproduce.py` - flag-off against the base-commit binary, whole
  documents, at a work budget.
* `drivers/determinism.py` - two processes, whole documents.
* `drivers/gates.py`, `drivers/gatelib.py`, `drivers/docdiff.py` - the four
  pinned gates, from `se2-rigidity/drivers/` with `ROOT` repointed.
* `drivers/smoke.py` - one run with the action trace printed.
* `evidence/*.json`, `evidence/suite-*.log`.

Reproduce:

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental                          # gate binary
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,compression-schedule     # measurement binary

D=docs/experiments/m34-wall-price/drivers
OFF='m34wall=0,m34entry=0,m34skip=0,m34drop=0,m34probe=0,m34bit=0'

python3 $D/firstslice.py base-firstslice mixed-61,shapes-17,triangle-20 \
    0,1,2 10000 30000

for R in mixed-61:mixed61 shapes-17:shapes17 triangle-20:triangle20; do
  python3 $D/battery.py "curve-${R##*:}" 3 "${R%%:*}" 0,1,2 \
      "aheadat3:wall:3000:1:$OFF"   'bnewat3:wall:3000:1' \
      "aheadat10:wall:10000:1:$OFF" 'bnewat10:wall:10000:1' \
      "aheadat30:wall:30000:1:$OFF" 'bnewat30:wall:30000:1'
done
python3 $D/summarize.py evidence/curves-summary.json evidence/curve-*.json

M34_EXTRA='' python3 $D/entrycensus.py entry-measure \
    'm34wall=0,m34bit=0,m34entry=1,m34skip=0' \
    mixed-61,shapes-17,triangle-20 0,1,2 10000
for n in 2 3 4; do
  python3 $D/entrycensus.py "probe-$n" "m34wall=0,m34bit=0,m34probe=$n" \
      mixed-61,shapes-17,triangle-20 0,1,2 10000
done

python3 $D/gates.py final <measurement-binary> <outdir>
python3 $D/reproduce.py reproduce <base-binary> <measurement-binary> \
    mixed-61,shapes-17,triangle-20 0,1,2 40000000
python3 $D/determinism.py determinism mixed-61,shapes-17,triangle-20 0,1,2 \
    40000000
```
