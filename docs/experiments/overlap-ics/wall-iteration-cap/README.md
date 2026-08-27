# The knob that was never there

`Pacer::Wall::iteration_cap()` returns `None`. It always has. Wall mode - the
mode a bare request runs in - has never bounded how long one `separate` call may
run, so on a hard bite a single separation consumes 800-1,400 master iterations
and a ten-second budget buys **about one attempt at that bite**.

The pool restore and the Algorithm-12 disruption happen only **between**
attempts. With one attempt they never happen at all. At ten seconds the entire
escape mechanism this campaign built, debugged and re-verified is never switched
on even once.

This directory measures what happens when it is.

## The measurement

mixed-61, `--orders=1 --workers=8 --edge=5 --pair=5`, bare ten-second wall
requests, nine seeds, five repetitions per arm, one fresh process per cell,
machine otherwise idle. The only difference between arms is
`--itercap`: `0` is the historical unbounded behaviour and is the default
everywhere.

The two clauses are the ones the retired ten-second gate was written with:
**at least 5 of 9 at or below 168.484 mm**, and **median at or below
168.484 mm**.

| cap | reps | median of medians | worst rep median | under-bar per rep | **both clauses met** |
| --- | ---: | ---: | ---: | --- | :---: |
| **none (today)** | 5 | 169.672 | 179.072 | 2, 2, 2, 2, 2 | **0 of 5** |
| **50** | 5 | **168.382** | 168.580 | **5, 6, 5, 6, 4** | **4 of 5** |
| 150 | 5 | 168.678 | 169.598 | 5, 4, 4, 5, 4 | 2 of 5 |

The unbounded arm returns **2 of 9, five times out of five**. That is a floor,
not variance. `cap = 50` meets both clauses on four of five repetitions; its one
miss is 4/9 at a median of 168.580, so it is a good knob and not a certainty.

Per-seed medians across repetitions:

| seed | none | cap 50 | gain |
| ---: | ---: | ---: | ---: |
| 8 | 179.082 | 168.590 | **+10.492** |
| 7 | 179.082 | 171.135 | **+7.947** |
| 4 | 179.081 | 172.157 | +6.924 |
| 1 | 179.081 | 172.211 | +6.870 |
| 0 | 169.561 | 164.001 | +5.560 |
| 6 | 169.070 | 165.849 | +3.221 |
| 5 | 169.079 | 168.131 | +0.949 |
| 3 | 165.401 | 165.154 | +0.247 |
| 2 | 165.068 | 168.382 | **-3.314** |

**All five seeds the campaign called frozen move.** Seeds 7 and 8 - the two that
nothing tried in this campaign had ever shifted by a micrometre, and the two the
T-row's Gate 0 died on this same day - gain 7.9 and 10.5 mm. Eight of nine seeds
improve; seed 2 regresses by 3.3 mm.

## What was changed

One thing. `Pacer::Wall` gains an `iteration_cap` and `iteration_cap()` returns
it when non-zero. `0` is the historical behaviour and remains the default, so a
build that does not name the knob is byte-for-byte the engine that existed
before.

Nothing frozen moved: not `200 / 3 / 100 / 5 / 0.98`, not
`EXPLORE_SHRINK_STEP`, not `COMPRESS_SHRINK_RANGE`, not the sample counts, not
the 80/20 explore-compress split, not `workers = 8`, not the relocate operator,
not GLS, not the constructor, not the publication path, not the 4 um band, the
16 um cap or the `4n` row budget.

## Why this is a mechanism and not a retune

Sparrow's `iter_no_imprv_limit = 200` and `strike_limit = 3` are Table 1
parameters, and the paper's §11.3 states plainly that they were tuned for
twenty-minute runs on a 7950X and **not re-tuned for other time limits**. At
twenty minutes an unbounded separation is harmless: thousands of them run
anyway. At ten seconds exactly one runs.

We inherited the constants faithfully and inherited the assumption underneath
them without noticing it: that the loop gets enough turns for a restart policy
to matter. The cap is not a new value for a tuned constant; it is the bound the
wall pacer never had, which the fixed-work pacer has always had
(`iterations_per_separation`), and whose absence made every restart-based
mechanism dead code at the production budget.

Kept from the T-row round's calibration sweep, which found the same thing from
the other side: given twelve retry attempts the **closed member** closes explore
bite 22 on eight of nine seeds. Bite 22 was never impossible. At ten seconds it
simply never got the attempts.

## Thirty seconds: it does not cost the asset, it completes it

The campaign's current asset is Round 4's thirty-second result - median
162.94241 mm, **7 of 9** at or below 168.484, with seeds 7 and 8 the two that
hold it there. The obvious risk of a cap tuned at ten seconds is that it wrecks
that. Measured on the same instrument, nine seeds, bare thirty-second requests:

| cap | median | best | under 168.484 |
| --- | ---: | ---: | ---: |
| none | 164.001 | 160.538 | **7/9** |
| **50** | 164.001 | **160.047** | **9/9** |

The cap does not cost the thirty-second result. It **finishes** it: seeds 7 and
8 clear the bar and the arm goes 9 of 9, at an unchanged median and a better
best.

That is worth naming precisely, because it is the same clause by a different
road. The T-row specification signed earlier the same day added exactly one
tightening to the thirty-second battery - *"seeds 7 and 8 individually at or
below 168.484 mm"* - as the clause the mechanism claimed to move. The T-row's
Gate 0 died on those two seeds without converting either of them. This reaches
the clause the specification was written for, without the mechanism the
specification was written about.

## Thirty seconds, per seed: the two that never moved

The two repetitions above are medians. The per-seed numbers are where the result
actually lives:

| seed | none | cap 50 | |
| ---: | ---: | ---: | ---: |
| 7 | 173.583 | **164.000** | **-9.583** |
| 8 | **179.013** | **164.001** | **-15.012** |

Seed 8 is still at 179 after **thirty** seconds under the closed member -
exactly where it sits after three. Under the cap it lands at 164.001. These are
the two seeds that held Round 4 at 7 of 9, that the T-row specification named as
the one clause it claimed to move, that the T-row's Gate 0 died on without
converting either, and that nothing tried in this campaign had shifted by a
micrometre.

## The other two fixtures are already at their certified floor

The severe test against "you tuned it on your own fixture" is the two corpus
fixtures never looked at while the knob was found. On both, ten seconds, nine
seeds, two repetitions, the cap does **nothing**: shapes-17 medians 200.349
against 200.348, triangle-20 70.254 against 70.251, per-seed gains of +0.002 to
+0.006 mm.

**A first draft of this section read that as a failure to transfer and went
looking for a second defect.** It was wrong, and the correction is the more
interesting result. `lower_scale_mm` is the request's own certified lower bound
on achievable depth - `max(total area / usable width, tallest piece's supporting
width)` plus the two edge clearances - and it is a *bound*, not a target:

| fixture | certified lower bound | reached in 10 s | headroom left |
| --- | ---: | ---: | ---: |
| mixed-61 | 115.839 | 164.001 | **48.162 mm** |
| shapes-17 | 200.347 | 200.347 | **0.001 mm** |
| triangle-20 | 70.250 | 70.251 | **0.001 mm** |

**shapes-17 and triangle-20 are each solved to within one micrometre of a
certified lower bound.** There is nothing left to win on either, and no knob can
find it. shapes-17's single explore bite is not a stall - it is the engine
arriving and stopping. triangle-20 spends 44 bites getting there, both arms,
and lands on the bound.

So the corpus offers exactly one fixture with headroom, and that is the fixture
the knob moves. The honest statement is not "it does not transfer": it is **the
cap helps where there is something to win and is neutral where there is not**,
which is the only transfer test this corpus is capable of giving.

## What this is not, yet

These are wall runs, which is the right instrument for a claim about a bare
ten-second request, and they are repeated rather than single. They are **not** a
signed gate: no specification was pre-committed for this, no quorum has ruled on
it, `50` is a constant found by sweeping and not derived from the budget, the
ten-second arm misses on one repetition in five, and the thirty-second reading
is one repetition so far.

The honest form of the knob is almost certainly budget-derived rather than
constant, precisely because the defect it repairs is a constant that did not
scale. That is the next thing to establish, not something to assert here.

---

# Part two: the cap has an optimum, and it is not the only twenty-minute constant

## What the cap actually buys

Medians over 45 cells per arm, ten seconds, mixed-61:

| cap | explore bites | retry attempts | **disruptions** | strikes |
| --- | ---: | ---: | ---: | ---: |
| **none** | 36 | **3** | **1** | 2 |
| 50 | **79** | **23** | **14** | 0 |
| 150 | 72 | 17 | 11 | 0 |

**Under the closed member, Algorithm 12 fires once in a whole ten-second run.**
The two-large-item swap this campaign implemented from the paper, whose strike
predicate it found and repaired with red-to-green evidence, and whose behaviour
it verified cell by cell, gets a single turn at the production budget. With the
cap it gets fourteen.

Strikes fall to zero because the cap (50) acts below the no-improvement limit
(200): the separation now ends on the bound rather than on the strike counter.
At ten seconds the frozen `200 / 3` is not being fought, it is being reached
by a bound that acts first.

## The cap has a real optimum

Three repetitions per cap, nine seeds each (`evidence/cap-refinement/`):

| cap | median of medians | under-bar per rep | both clauses |
| ---: | ---: | --- | :---: |
| 15 | 171.652 | 1, 1, 1 | 0/3 |
| 25 | 171.264 | 2, 2, 2 | 0/3 |
| 35 | 169.097 | 4, 4, 4 | 0/3 |
| **50** | **168.230** | **5, 6** | **2/2** |
| 70 | 168.361 | 5, 5 | 2/2 |
| 150 | 168.678 | 5, 4, 4, 5, 4 | 2/5 |

Monotone up to about 50 and flat-to-worse after. This is a genuine trade and not
a free parameter: too short and a separation accomplishes nothing before it is
cut, too long and the restart never happens. `50` is the measured optimum.

## The 80/20 split is the same kind of constant

`DEFAULT_EXPLORE_TIME_RATIO = 0.8` is Sparrow `consts.rs` and Table 1, tuned for
twenty minutes. At ten seconds it hands 20 % of the budget to a phase whose
steps are 0.05 % decaying to 0.001 %, against explore's flat 0.1 % - on a layout
with 48 mm of certified headroom still ahead of it. That is polishing something
that is nowhere near finished.

Three repetitions, nine seeds, `cap = 50` (`evidence/explore-ratio/`):

| ratio | median of medians | best ever | under-bar per rep | both clauses |
| ---: | ---: | ---: | --- | :---: |
| **0.80** (Sparrow) | 168.387 | 164.002 | 6, 5, 5 | 3/3 |
| 0.90 | 167.906 | 163.602 | 5, 5, 5 | 3/3 |
| **0.95** | **167.603** | **163.400** | **6, 6, 6** | **3/3** |
| 1.00 | 167.687 | 163.696 | 6, 6 | 2/2 |

`0.95` wins on every column. Note that `1.00` - no compress phase at all - is
slightly *worse* than `0.95`, so the compress phase does earn its keep at ten
seconds; it just does not earn a fifth of the budget.

## Where ten seconds stands now

| | median | best | under 168.484 |
| --- | ---: | ---: | ---: |
| this morning's engine | 169.672 | 165.162 | **2/9**, five times of five |
| `cap = 50`, `ratio = 0.95` | **167.603** | **163.400** | **6/9**, three times of three |

**-2.07 mm of median and 2/9 to 6/9**, from two bounds that were never wrong -
they were simply inherited from a twenty-minute schedule and applied to ten
seconds without anyone asking whether they still meant the same thing.

Round 4's own ten-second best was 165.42489. This reads **163.400**.

## Thirty seconds with both bounds: nine of nine, twice

`cap = 50, ratio = 0.95` against the closed member, nine seeds, two repetitions,
bare thirty-second requests (`evidence/combined-30s/`):

| arm | rep | median | best | under 168.484 | seed 7 | seed 8 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| base | 0 | 164.005 | 160.573 | **7/9** | 179.082 | 179.082 |
| base | 1 | 164.006 | 161.030 | **7/9** | 179.082 | 179.011 |
| **tuned** | 0 | 164.001 | 160.849 | **9/9** | **161.904** | **164.001** |
| **tuned** | 1 | 164.003 | 161.142 | **9/9** | **162.013** | **164.003** |

Nine of nine on both repetitions. Seed 7 goes 179.082 to **161.904 and 162.013**
- **-17.1 mm** - and seed 8 to 164.001 and 164.003, **-15.1 mm**, on both. Under
the closed member they are pinned at 179 after thirty seconds, exactly where they
sit after three.

The median is unchanged, 164.005 against 164.001, and the best pays 0.28 mm,
160.573 against 160.849. That is the honest shape of the trade: the bounds buy
the tail and cost a little at the very top.

This is the clause Round 4 could not reach and the clause the T-row
specification was written for and failed to move.

---

# Part three: the shrink step, and where the day ends

## The step is the one Sparrow got right

`EXPLORE_SHRINK_STEP = 0.001` is Sparrow `config.rs`'s `shrink_step`, the third
Table 1 value in this file. Three repetitions, nine seeds, ten seconds, at
`cap = 50, ratio = 0.95` (`evidence/shrink-step/`):

| step | median of medians | best ever | under-bar per rep | both clauses |
| ---: | ---: | ---: | --- | :---: |
| **0.0007** | 167.737 | 163.699 | **7, 7, 7** | 3/3 |
| **0.0010** (Sparrow) | **167.687** | **163.487** | 6, 6, 6 | 3/3 |
| 0.0015 | 167.782 | 165.175 | **7, 7, 7** | 3/3 |
| 0.0020 | 168.222 | 165.387 | 5, 5 | 2/2 |

Unlike the other two, this one is nearly flat, and Sparrow's value holds the
best median and the best best. The only thing that moves is the *count*:
`0.0007` and `0.0015` each put one more seed under the bar than `0.0010` does,
on every repetition. So it is a real trade and a small one - **0.05 mm of median
for one seed** - and it is reported as such rather than claimed as a win.

`0.0020` is the first setting that clearly loses: 41 bites instead of 87, and
each one a bigger shock than a 50-iteration separation can absorb.

## Where the day ends

Ten seconds, mixed-61, nine seeds, three repetitions of the tuned arm and five
of the base:

| | median | best | under 168.484 |
| --- | ---: | ---: | ---: |
| the engine this morning | 169.672 | 165.162 | **2/9**, five of five |
| `cap = 50, ratio = 0.95` | 167.687 | **163.487** | 6/9, three of three |
| `cap = 50, ratio = 0.95, step = 0.0007` | 167.737 | 163.699 | **7/9**, three of three |

Thirty seconds, two repetitions each (`evidence/full-config-30s/`):

| | median | best | under 168.484 | seed 7 | seed 8 |
| --- | ---: | ---: | ---: | ---: | ---: |
| base | 164.005 | 160.573 | **7/9** | 179.082 | 179.082 |
| tuned, step 0.0010 | 164.001 | 160.849 | **9/9** | 161.904 | 164.001 |
| tuned, step 0.0007 | 164.000 | **161.017** | **9/9** | **161.017** | 164.951 |

**Seed 7 was the worst of nine at 179.082 this morning. At thirty seconds it is
now the best of nine at 161.017**, on both repetitions.

## Regression floor

Fresh build, everything above compiled in and every default preserved:

- four pinned regression gates: **4/4, `ALL_PASS: true`**;
- default workspace suite: **1,104 passed, 0 failed**;
- `overlap-ics` suite: **839 passed, 0 failed**.

Every knob added here defaults to the historical behaviour - `itercap = 0` is
unbounded, `exploreratio` defaults to `EXPLORE_TIME_RATIO`, `shrinkstep = 0`
means `EXPLORE_SHRINK_STEP` - so a build or a run that does not name them takes
exactly the path it always took.

## What is still true

- **None of this is a signed gate.** No specification was pre-committed, no
  quorum has ruled, and three constants were chosen by sweeping.
- **`50` and `0.95` are swept, not derived.** Curing a constant that failed to
  scale with another constant is the same disease one level down. The honest
  form is budget-derived, and that is the next piece of work, not a claim here.
- **Sparrow is still ahead.** 150.165 at ten seconds against 163.699. The
  distance did not fall today. What fell is the belief that a wall stood in
  front of it, and a "permanent" retirement decided on a wrong diagnosis.
