# Brief for the third voice

You hold the deciding seat in a three-model quorum. The other two are split 1-1
and neither will break the tie. You were appointed by a rule committed before
you were invoked (`third-voice-appointment.md`). Read the repository if you want
to check anything; everything below is verifiable in it.

Answer **Q1 to Q4**. All four, or the ballot is incomplete and the seat passes
to the next candidate.

## The question

Does `EXPLORE_SHRINK_STEP` move from `0.001` to **`0.032`**, as the ten-second
wall profile of the ICS engine only?

## The engine and the constant

`search::overlap_ics` reproduces Sparrow (arXiv 2509.13329) without copying it.
`EXPLORE_SHRINK_STEP = 0.001` is Sparrow's `shrink_step`, one of four Table 1
constants inherited whole. The paper's §11.3 says they were tuned for
**twenty-minute** runs and never re-tuned. This project's target is a
**ten-second** bare request.

Each explore bite shrinks the strip by that fraction and then runs a separation.
A fixed wall buys a roughly fixed number of separations, so the depth reached is
about `start * (1 - step)^n`: a finer step only shrinks the base.

## The case for `0.032` (Sol's position)

A specification, `ICS-10s-coarse-v1`, was written by Sol, committed and pushed
**before its first cell ran** (`ics-schedule-round-spec.md`), and executed
verbatim on seeds 18-26, which had chosen nothing. It passed **every** clause:

| clause | required | measured |
| --- | ---: | ---: |
| mixed-61 paired median improvement | >= 4.000 mm | **+4.983** |
| mixed-61 per-seed wins | >= 8/9 | **9/9** |
| mixed-61 absolute median | <= 161.000 mm | **159.953** |
| mixed-61 worst per-seed regression | <= 1.000 mm | **0.000** |
| quantity-74 paired median | >= 3.000 mm | **+37.227** |
| quantity-74 wins | >= 6/9 | **9/9** |
| shapes-17 / triangle-20 median regression | <= 0.050 mm | **+0.003 / +0.002** |
| invalid publications | 0 | **0** |
| request-relative p95 | <= 10.250 s | **10.007** |

`quantity-expanded-74` is a second, independent fixture with **273.671 mm of
certified headroom** above its own lower bound, certified under the control
*before* the treatment ran. Both other corpus fixtures are solved to within
13 um of a certified lower bound, so they can only show "no regression".

Separately, at a matched ~10.6 s wall on held-out seeds, ICS at `0.032` beats
the engine this repository actually ships (`general_relaxed`) by **+6.893 mm
paired median, 9/9**, while ICS at `0.001` **loses** to it by 3.081 mm.

## The case against (Grok's position)

`docs/grok-review-12-reading-sparrow.md:370`, written **before any wall number
existed** and carried in the source at `homotopy.rs:38`:

> **Bite-size fitted to 168.484** | 3 % (or "whatever reaches 168 in 80 bites")
> chosen after a scout run | 0.1 % / (0.0005, 0.00001) / 80/20 frozen from
> Sparrow defaults **before** the nine-seed wall. Changing them is a forbidden
> rescue.

`0.032` is 3.2 %, and it was found by sweeping mixed-61 and reading the numbers.
Grok's position is that the rule has no exception clause and was written
precisely so that "it is a mechanism, not a fit" could not be used as the escape
hatch: *"held-out confirmation of a post-selected `0.032` is still that
number."* Grok signed a different specification for `0.02` - the value this
repository's **shipped** engine already uses as `initial_shrink_ratio`,
independently motivated and absent from the sweep that chose `0.032` - and that
specification **failed** its own clauses. Grok refuses to re-scope it.

Two further facts Grok relies on: the optimum is budget-dependent (`0.032` at
10 s, `0.016` at 30 s, Sparrow's `0.001` at 20 minutes), so `0.032` is not a
general constant; and at 15 s the coarse arm plateaus around 161.7 mm while the
control catches up to 164.0, which he reads as a shelf rather than a floor.

## Ballot

**Q1.** Does `EXPLORE_SHRINK_STEP` move to `0.032` as the ten-second wall
profile? Answer **WRITE**, **REFUSE**, or **ABSTAIN**. An abstention leaves the
split unbroken and nothing is written; it is a legitimate answer but say so
explicitly rather than by omission.

**Q2.** Give your reasoning against the strongest version of the side you did
not take. If you voted WRITE, answer the forbidden-rescue rule directly: what,
if anything, distinguishes a pre-committed specification passed on virgin seeds
from the rescue the rule forbids - and if nothing does, say so and change your
vote. If you voted REFUSE, say what evidence could ever move the constant, or
state plainly that none can.

**Q3.** Is the outcome that a research engine's faithful configuration is 3 mm
*worse* than the shipped engine, while a forbidden configuration is 7 mm better,
a reason to (a) promote the engine, (b) keep it research-only, or (c) something
else? One sentence of justification.

**Q4.** Confidence 0-100, and the single strongest argument against your own
vote.

Under 600 words. No preamble.
