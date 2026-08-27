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

## It does not transfer to shapes-17, and shapes-17 has a worse problem

The severe test against "you tuned it on your own fixture" is the two fixtures
that were never looked at while the knob was found. On `shapes-17`, ten seconds,
nine seeds, two repetitions, the cap does **nothing**: medians 200.349 against
200.348, per-seed gains of +0.002 mm.

Looking at why turned up something larger:

| fixture | constructor | after 10 s | explore bites |
| --- | ---: | ---: | ---: |
| mixed-61 | 182.976 | ~168.4 with the cap | ~90 |
| **shapes-17** | 200.651 | **200.347** | **1** |

**On shapes-17 the engine completes one explore bite and stops** - every seed,
both arms - and gains three tenths of a millimetre in ten seconds. That is not
restart starvation: the *first* shrink is already infeasible there. It is a
different and larger failure, on a fixture in the standing corpus, and this is
the first measurement that looks at it.

So the knob's claim has to be written narrowly: it is a **mixed-61** result,
large and repeated; it is **neutral** on shapes-17, neither helping nor harming.

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
