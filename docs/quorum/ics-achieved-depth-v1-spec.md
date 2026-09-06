# Signed round: `ICS-achieved-depth-v1`

**Committed before any scored cell runs.** Nothing below may be edited after the
first scored cell; an inconvenient clause is a FAIL, not an amendment. The
specification is GPT-6 Astra's (review 1, §Q4), transcribed with its own words
wherever a clause is quoted, on the owner's instruction that Astra replaces the
three-model quorum for this round. Executable hashes, fixture hashes, compiler
and the exact commands are appended under "Frozen identities" before scoring
begins, as the specification itself requires; that append is part of the
protocol, not an amendment.

## What this round changes, said plainly

It changes acceptance semantics **prospectively**. `grok-review-12` §2 required
publication *at the new `T`*, persistence at that width, and forbade growing
`W`. This round replaces that rule for the arms it names:

> `T` is an aspiration. Accepted parent depths decrease strictly by more than
> 1 um. An unsuccessful separation never increases its aspiration. Only a
> completed dual-valid, improving publication establishes a new parent depth.

It does not reopen or reinterpret any previous verdict, and it does not trip any
row of `grok-review-12` §5.2's table: the strip is shrunk only after a
dual-valid publication, quality remains the published raw-source depth of a
dual-valid layout, the constructor is never a child, source rings only.

## Frozen implementation and authority

- Base: `d0c459b` (frozen head, arms O).
- Corrected base: `6aaa4e1` - adds only the rollback near-set restoration
  (arms R).
- H1: the corrected base plus the achieved-depth publication policy behind an
  opt-in knob, its **final strict improvement check after repair**
  (`published_depth.is_finite() && published_depth < incumbent_depth - 0.001`),
  and diagnostics (arms H). Both phases, explore and compress, enforce it.
- Excluded from this round: bounded candidate evaluation, inward proxy margins.

Unchanged: source geometry and rotation permissions; exact 5.0/5.0 clearance;
Exclusive radius 2.500, allowance zero; `validate_placements_against_contract`;
publication band 0.004 mm, cumulative displacement 0.016 mm per piece, repair
limit `4n`; constructor, sample counts, coordinate descent, GLS, pool policy,
disruption; explore ratio 0.80; compression range `(0.0005, 0.00001)`; eight
workers and `--orders=1`. Strict-target behaviour remains available for
locked-target regressions.

## Preflight, consumed seeds only, before any seed 27-35 cell

1. The rollback regression vector passes (it does: `6aaa4e1`).
2. Publication vectors: dual-valid above-`T` strict improvement; above-`T`
   physical-boundary or pair illegality; repair consuming the proxy gain;
   exactly 1 um final improvement; non-finite depth; repaired-pose installation
   and second-bite parent identity.
3. S0, locked S1 and triangle-20, FAST/HEAVY soundness, default-build
   isolation, and the four pinned engine gates preserved.
4. Two-process identity for fixed-work runs of each policy. Changes caused by
   the rollback correction are recorded; they are not required to reproduce
   the defective trajectory.
5. **Conversion census.** On corrected Legacy controls, seeds 0-8, one fresh
   ten-second process each, retain the first 32 distinct target-only rejected
   states per seed (or all, if fewer). Evaluate H1 afterward on detached
   copies, counting distinct `(target, pose digest)` opportunities. **Precondition:
   at least six of nine seeds with a newly certified strict improvement, and
   median per-seed conversion at least 50 % among seeds with opportunities.**
   Missing opportunities are reported and do not count. Failure stops the
   quality battery.

The instrumentation must distinguish actual refusal reasons from the
counterfactual `wouldStrictTargetRefuse` flag; the H arms must not inherit a
census that reports the removed target gate as an actual refusal.

## Arms

| arm | source | explore step | wall cap | publication |
| --- | --- | ---: | ---: | --- |
| O-L | frozen head `d0c459b` | 0.001 | 50 | existing |
| O-W | frozen head `d0c459b` | 0.032 | none | existing |
| R-L | corrected `6aaa4e1` | 0.001 | 50 | strict target |
| R-W | corrected `6aaa4e1` | 0.032 | none | strict target |
| H-L | corrected + H1 | 0.001 | 50 | achieved depth |
| H-W | corrected + H1 | 0.032 | none | achieved depth |

O-L/O-W measure whether the correction altered the baseline. R-L/H-L and
R-W/H-W isolate H1. H1 is selected by its own publication-policy knob, never by
`--arm=treatment`, which selects the work-based strike experiment.

The frozen-head control cells already run on seeds 27-35 before this document
(three unrotated repetitions, `/var/lib/t3/tmp/astra/base`) are **superseded
and not scored**; the O arms are re-run inside the rotated battery below.

## Population and walls - every scored cell on seeds 27-35, bare request, fresh process

| fixture | walls | arms | repetitions per seed |
| --- | --- | --- | ---: |
| mixed-61 exact-clearance | 10.000 s | all six | 5 |
| mixed-61 exact-clearance | 7.000 s and 15.000 s | R-L, R-W, H-L, H-W | 5 |
| quantity-expanded-74 | 10.000 s | R-L, R-W, H-L, H-W | 3 |
| shapes-17 `2000x2700-compact` | 10.000 s | R-L, R-W, H-L, H-W | 3 |
| triangle-20 `2000x2700-compact` | 10.000 s | R-L, R-W, H-L, H-W | 3 |

954 scored processes. Fixture paths:
`tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`,
`tests/fixtures/performance/quantity-expanded-74-request.json`,
`tests/fixtures/shapes-17/2000x2700-compact/request.json`,
`tests/fixtures/triangle-20/2000x2700-compact/request.json`.
quantity-74's Gate 0 is accepted (certified lower bound 660.661, control median
934.332, headroom 273.671 mm): a non-saturated subject, not an expected gain.

At ten seconds the W arms use the explicit `wall10s` profile. At seven and
fifteen seconds the coarse schedule runs as a **named research override** with
the same frozen values (`--shrinkstep=0.032 --itercap=0 --exploreratio=0.80`);
this authorises those diagnostic cells and does not broaden the `Wall10s`
guard or authorise a seven- or fifteen-second default.

One cell at a time, otherwise-idle machine, no concurrent builds. Within each
`(fixture, wall, seed, repetition)` block the listed arm order is rotated left
by `(seed - 27 + repetition) mod arm_count` and reversed on odd repetitions;
repetitions number from zero.

## Clock and quality

The request clock starts at decoded-request entry and charges constructor,
engine preparation, separator and publication work. For requested wall `w`,
the score is the deepest improvement **certified by `w`**:
`d(w) = min { D_j : publication j completed both authorities by w }`. The
constructor is an eligible floor only after its own certification; a cell with
no completed legal layout by the deadline scores infinite depth; a publication
completed after the deadline never counts, whatever the overrun allowance.
Request-relative completion timestamps are recorded. Every saved publication,
late ones included, is independently revalidated outside the timed solve.

## Statistics

Per arm `a`, fixture, wall, seed `s`: `m_{a,s} = median_r d_{a,s,r}`. All
quality clauses use these **nine seed medians**; repetitions are not additional
seeds. At mixed-61 ten seconds the conservative reference is the envelope
`b_s = min(m_{O-L,s}, m_{O-W,s}, m_{R-L,s}, m_{R-W,s})` - explicitly an envelope
of controls, not a deployable policy, so that a regression in the correction
cannot make H1 easier to pass. For treatment `h`, paired gain
`g_s = b_s - m_{h,s}`; a win is `g_s > 0.001`; "worst paired gain" is
`min_s g_s`, never the difference of two arms' worst absolute depths.

## PASS - each treatment must satisfy its own complete conjunction

At mixed-61 ten seconds:
1. median seed depth **<= 156.000 mm**;
2. median paired gain against the envelope **>= 3.000 mm**;
3. wins on **>= 8/9** seeds;
4. worst paired gain **>= -1.000 mm**.

At **each** of seven and fifteen seconds, against the treatment's matched
corrected strict-target control:
5. median paired gain **>= 0.000 mm**;
6. worst paired gain **>= -1.000 mm**.

On quantity-74, against the per-seed minimum of R-L and R-W:
7. median paired gain **>= 3.000 mm**; 8. wins **>= 6/9**; 9. worst paired gain
**>= -1.000 mm**.

On **each** of shapes-17 and triangle-20, against the same corrected-control
envelope: 10. median paired regression **<= 0.050 mm**; 11. no seed-median
regression **> 1.000 mm**.

Across all required populations: 12. **zero invalid publications**, zero
authority/cap violations, zero non-improving H1 parent installations; 13.
request-relative return-time p95 **<= requested wall + 0.250 s**, nearest-rank,
per arm/fixture/wall; 14. all preflight and isolation requirements still hold.

A passing treatment earns **PASS-ADVANCE**: a ten-second research profile. It
earns **PASS-SPARROW-REFERENCE** only if also median seed depth
**<= 150.16351 mm** and **>= 8/9** seed medians **<= 150.16351 mm** - a robust
result below the pinned legal layout, not superiority to Sparrow's seed
distribution, which one Sparrow seed cannot supply. If both treatments pass,
the lower ten-second median seed depth is selected; an exact tie selects H-L.

## Refusal

Any failed mandatory clause refuses promotion of that treatment. A
mechanism-only improvement, more bites, a better best run, or a fifteen-second
success cannot rescue it. No step or cap change, fixture substitution,
selected-seed exclusion, replacement repetition, post hoc wall restriction, or
addition of bounded evaluation after the first scored cell. A defect discovered
after scoring begins invalidates the affected result; the seeds remain
consumed. Results and any refusal are appended, never amended.
