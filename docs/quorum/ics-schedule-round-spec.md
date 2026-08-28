# Signed round: `ICS-10s-coarse-v1` and `ICS-OPEN-02`, run as one battery

**Committed before the data exists.** Nothing in this document may be edited
after the first cell of the round runs; a clause that turns out inconvenient is
a FAIL, not an amendment. Both specifications are the reviewers' own words from
round 3 of `schedule-defaults-ballot.md`, transcribed, not paraphrased.

## Why there are two specifications and not one

The quorum converged on the route and split on the value.

Both ruled that a pre-registered forbidden rescue **can** be lifted, but only by
a later specification committed before the next cell - never retroactively by
the evidence that provoked it. Sol: *"Il freeze resta vero per il member chiuso
... La via legittima è una nuova specifica, un nuovo nome di member e dati
ancora vergini. I seed 9-17 ora sono evidenza esplorativa consumata: non possono
approvare il cambiamento."* Grok: *"A later specification, committed before the
next cell, can lift a freeze. This table cannot."*

They then split on which value such a specification may name.

- **Sol will sign `0.032`** under heavy clauses, as `ICS-10s-coarse-v1`, and
  authorises it for the ten-second wall profile only.
- **Grok refuses `0.032` at any evidence level** - *"Held-out confirmation of a
  post-selected `0.032` is still that number"* - and will sign only **`0.02`**,
  because it is the shipped `general_relaxed::initial_shrink_ratio`,
  independently motivated and absent from the ICS factorial that chose 0.032.

Both arms run in one battery. **Adoption follows each reviewer's own rule**: a
value is written only if the specification that admits it passes every one of
its own clauses. A pass of Sol's clauses does not license `0.032` over Grok's
refusal; it records that it would have passed.

## Already ratified, unanimously, and not part of this round

`Pacer::Wall::iteration_cap`: `None -> Some(50)`.

Both reviewers moved to 50 in round 3 and both retracted 200 explicitly. Sol:
*"Il mio voto per cap 200 è confutato ... a `step=0.001`, `50` batte `200` di
1.942 mm."* Grok: *"Default `Pacer::Wall::iteration_cap = Some(50)`. Do not
write 200."* This is a pacer the wall mode never had, it is isolated from
fixed-work mode, and it does not touch the four identity gates.

## Gate 0 - already run, and it passes

Sol's precondition: the second fixture must show, under the best legacy control,
a median at least **20 mm above a certified lower bound**, or the round is
invalid and the fixture may not be substituted.

`tests/fixtures/performance/quantity-expanded-74-request.json`, control
`(step 0.001, cap 50)`, seeds 18-26, ten seconds, exact 5.0/5.0:

| certified lower bound | median depth | headroom | invalid publications |
| ---: | ---: | ---: | ---: |
| 660.661 | 934.332 | **273.671 mm** | 0 |

**PASS.** It also satisfies Grok's parallel clause - a request outside
`{mixed-61, shapes-17, triangle-20}` first certified at >= 20 mm headroom at
ten seconds under the control - so the transfer requirement has a subject and
the round can reach PASS at all.

## The battery

`overlap_ics_benchmark --cell=cutclose`, exact 5.0/5.0 (`--edge=5 --pair=5`),
8 workers, `--orders=1`, **`--exploreratio=0.80`** (the real default, frozen for
this round), one fresh process per cell, machine otherwise idle.

Four arms:

| arm | step | wall cap | named by |
| --- | ---: | ---: | --- |
| **A** control | 0.001 | 50 | both |
| **B** control | 0.001 | none | Sol |
| **C** candidate | **0.02** | 50 | Grok (`ICS-OPEN-02`) |
| **D** candidate | **0.032** | none | Sol (`ICS-10s-coarse-v1`) |

Populations, all on **seeds 18-26**, which no arm of this campaign has used to
choose anything:

| fixture | walls | arms | repetitions |
| --- | --- | --- | ---: |
| mixed-61 exact-clearance | 7.000, 10.000, 15.000 s | A, C at all three; B, D at 10 s | 5 |
| quantity-expanded-74 | 10.000 s | A, B, C, D | 3 |
| shapes-17 `2000x2700-compact` | 10.000 s | A, C, D | 3 |
| triangle-20 `2000x2700-compact` | 10.000 s | A, C, D | 3 |

Statistic, Grok's: per-seed median of the repetitions, then median and worst of
those nine, paired against the control.

## Sol's clauses for `ICS-10s-coarse-v1` (arm D against the better of A and B)

All mandatory, at ten seconds:

1. mixed-61 paired median improvement **>= 4.000 mm**;
2. mixed-61 per-seed median wins **>= 8/9**;
3. mixed-61 absolute median **<= 161.000 mm**;
4. mixed-61 no per-seed regression **> 1.000 mm**;
5. quantity-74 paired improvement **>= 3.000 mm**, wins **>= 6/9**, no per-seed
   regression **> 1.000 mm**;
6. shapes-17 and triangle-20 median regression **<= 0.050 mm**;
7. zero invalid publications, and request-relative p95 **<= 10.250 s**.

FAIL of any clause: `0.032` remains an experimental override, becomes neither an
ICS profile nor a production contender, and **no second choice of step or cap
may be made in this round**. A PASS authorises the ten-second wall profile only,
never a thirty-second default, where `0.016` remains the better datum.

## Grok's clauses for `ICS-OPEN-02` (arm C against arm A)

At **each** of the three walls, all mandatory:

1. `invalidPublications = 0`;
2. candidate paired median **>= 3.0 mm** better;
3. per-seed wins **>= 7/9**;
4. worst **>= 2.0 mm** better.

Transfer: candidate median on the certified second fixture no worse than
**1.0 mm** against the control; the saturated fixtures are reporting-only.

Refuse the write after the run if any wall misses a clause, if transfer is
missing or regresses by more than 1.0 mm, or if any publication is invalid.

## What the proposer may not do

Choose a different step or cap after seeing this round; re-run a failing arm
with a different seed set; substitute a fixture; report a clause as "nearly
met"; or read a PASS of one specification as authority for the other's value.

---

# Result of the round

Run verbatim after this document was committed and pushed. Raw depths in
`evidence/signed-round.json`.

## `ICS-10s-coarse-v1` (Sol) - arm D, `step 0.032`, cap none - **PASS, every clause**

Better control by Sol's rule is A (`0.001`, cap 50), median 164.758 against B's
169.363.

| # | clause | required | measured | |
| --- | --- | ---: | ---: | :---: |
| 1 | mixed-61 paired median improvement | >= 4.000 mm | **+4.983** | PASS |
| 2 | mixed-61 per-seed median wins | >= 8/9 | **9/9** | PASS |
| 3 | mixed-61 absolute median | <= 161.000 mm | **159.953** | PASS |
| 4 | mixed-61 worst per-seed regression | <= 1.000 mm | **0.000** | PASS |
| 5 | quantity-74 paired median | >= 3.000 mm | **+37.227** | PASS |
| 5 | quantity-74 wins | >= 6/9 | **9/9** | PASS |
| 5 | quantity-74 worst regression | <= 1.000 mm | **0.000** | PASS |
| 6 | shapes-17 median regression | <= 0.050 mm | **+0.003** | PASS |
| 6 | triangle-20 median regression | <= 0.050 mm | **+0.002** | PASS |
| 7 | invalid publications | 0 | **0** | PASS |
| 7 | request-relative p95 | <= 10.250 s | **10.007** | PASS |

## `ICS-OPEN-02` (Grok) - arm C, `step 0.02`, cap 50, against A - **FAIL**

| wall | invalid | paired median (>= 3.0) | wins (>= 7/9) | worst better (>= 2.0) | |
| ---: | ---: | ---: | ---: | ---: | :---: |
| 7 s | 0 | +6.054 | 9/9 | +5.432 | PASS |
| 10 s | 0 | +3.301 | 8/9 | +2.185 | PASS |
| **15 s** | 0 | **+2.796** | 9/9 | +2.879 | **FAIL** |

Transfer on the certified fixture: candidate median **-16.615 mm** against the
control, against a requirement of "no worse than +1.0". Passes with room.

**One clause, at one wall, short by 0.204 mm.** Grok's refusal condition is
"refuse the write after the run if any wall misses a clause". It is not reported
as nearly met, it is not re-scoped to the two walls that carried it, and the
round is not re-run with a different seed set - all three are on the list of
things this document forbids the proposer to do.

## What was written

Nothing from this round. `EXPLORE_SHRINK_STEP` remains `0.001`.

The only default that moved is the one both reviewers ratified in round 3,
outside this round: `Pacer::Wall::iteration_cap` now defaults to **50**, with
`--itercap=0` preserved as the explicit unbounded arm so every pre-ratification
replay reproduces. Four pinned gates `ALL_PASS`; 839 `overlap-ics` and 1,104
workspace tests pass.

## The state this leaves

A pre-committed specification passed every clause it wrote - on seeds that chose
nothing, with a second fixture carrying 273.671 mm of certified headroom moving
**+37.227 mm at 9/9** - and its value is still not written, because the other
reviewer refuses that value categorically and the specification's own adoption
rule says a PASS records what would have happened rather than licensing it.

That is a 1-1 split on a two-model quorum, with the third reviewer down
provider-side. It is put to both in round 4 rather than resolved by the
proposer.
