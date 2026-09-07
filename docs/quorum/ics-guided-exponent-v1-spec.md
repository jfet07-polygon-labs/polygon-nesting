# Signed round: `ICS-guided-exponent-v1`

**Committed before any scored cell runs.** Nothing below may be edited after the first scored
cell; an inconvenient clause is a FAIL, not an amendment. The specification is GPT-6 Astra's
(review 6, Q11–Q14, `docs/astra-review-6-the-live-exponent.md`), transcribed with its own words
wherever a clause is quoted, on the owner's instruction that Astra replaces the three-model
quorum for this round. Executable hashes and the exact commands are appended under "Frozen
identities" before scoring begins, as the specification itself requires; that append is part of
the protocol, not an amendment.

## What this round changes, said plainly

It changes the objective the separation *ranks on*, prospectively: `guided = Σ w·v^p` with
`p = 1` instead of the frozen `Σ w·v²`, for every positive pair and boundary residual, in every
worker, in the relocate's candidate ranking, both CD stages and the pre-update tournament,
for the whole process. Raw Φ (`Σ v²`), the maximum violation, the 4 µm band, the strike minimum,
`gls_update`'s `v / v_max` recurrence and the 2^20 cap, the pool, the disruption, `publish.rs`,
the Exclusive kernel and `validate_placements_against_contract` keep their definitions; the
lexicographic rule "any clear pose beats every colliding pose" stays on raw. Both arms carry
the proxy margin of 8 µm (review 4: margin 8 in both arms of every separator experiment) and
the profile caps (Legacy 50 iterations per separation, Wall10s unbounded). The exponent is our
own design choice reconsidered in a prospective successor (Grok review 12 line 183–188 chose
`v²` for "one guided path"); it is not Sparrow's pole proxy and no code of Sparrow's is ported.

It trips no row of `docs/grok-review-12-reading-sparrow.md` §5.2: quality remains the published
raw-source depth of a dual-valid layout, the strip shrinks only after a dual-valid publication,
the constructor is never a child, no fixture seeds a scored cell (the scorer refuses the
`startedFrom`, `biteMicroscope` and `replay` tripwires), source rings only.

## Why p = 1, and how it was selected (development, consumed seeds 18–26)

- The bite microscope (`docs/experiments/overlap-ics/bite-microscope/README.md`) found the hard
  explore bite to be a pinned column that breaks only when its rows' GLS weights reach ~1e5
  under `w v²`; the replay exponent probe and the geometry instrument showed every lower
  exponent breaking it at weights three to four orders lower and within Astra's registered
  column-break deadlines, with the fork at control sweep 24 showing the price crossover at
  identical poses and weights (2 → 36 of 64 relocates under p = 1). The band-entry deadlines and,
  at p = 1, the evaluation halving were missed: **halving is a forecast outcome of this round,
  reported separately, not a promotion requirement** (review 6, Q10: a prospective policy change,
  the old failed forecast preserved).
- The registered 108-cell development screen (`docs/experiments/overlap-ics/guided-exponent/`,
  review 5b Q7) passed its five scorer gates on both profiles for p = 1 against the margin-8
  control (Legacy +4.192 mm 9/9, Wall10s +1.078 mm 7/9, evaluations per explore bite −19 / −17 %).
- The selection between p = 1 and p = 0.75 followed the rule frozen before the 54 additional
  cells (`guided-exponent/selection-rule.md`, review 6 Q12): on both archives (as-run primary and replacement) p = 1 passed the four
  eligibility conditions on both profiles and p = 0.75 did not (Wall10s worst seed −4.161 mm
  against the tail guard of −1.000; Wall10s explore elapsed time per published explore bite
  +10 %, published explore bites per cell 4.00 → 3.63, publication fraction 80.0 → 78.4 %; on the
  replacement archive also the Legacy tail, −1.542), so the rule's first branch applied:
  "exactly one eligible → select it". `H = D_1 − D_0.75` was recorded (Legacy median −1.373 mm,
  Wall10s −3.935 mm) but not reached. Documents and output: `docs/experiments/overlap-ics/
  guided-exponent/screen/` (`selection.txt`, `selection.json`, `spec-score.py`).

## Frozen implementation and authority

- Base of the round: `1e913f4` on `engine/topology-archive-search` (the knob 6d55efb, its
  loose ends f856d19, the pinned fractional power, the replay-geometry instrument and its
  verifier fixes 0cb1b2b, the NaN rule in `eval_cmp` / `guided_beats` 7fe3df5, and the
  tests-only brace fix 1e913f4). Suite at the base: 862 passed, 0 failed. None of the commits after the
  screen's frozen binary `f856d19` changes a measured path: the pinned power routes every
  `v^p` through one non-inlined `energy::guided_power` (the arithmetic is one libm routine with
  one run-time argument everywhere; at p = 2 the product path is untouched), and the NaN rule
  only orders unordered comparisons, which no finite trajectory produces.
- Executable: `/var/lib/t3/tmp/astra/frozen-1e913f4` (sha256 `8734404a249e06fb57c387a9e1d310147a5b924b255378ff880ed59f189227b2`), built with `cargo build --release --example
  overlap_ics_benchmark -p polygon-nesting-core --features overlap-ics` from the base, on this
  machine (x86_64, the toolchain named under "Frozen identities"). Identity checks, both
  appended under "Frozen identities": (a) the default path bit-identical to `frozen-f856d19`
  on the three fixed-work bitcheck cells; (b) the **treatment path** (`--proxymargin=8
  --guidedexponent=1`) bit-identical to `frozen-f856d19` on the same three cells
  (`bitcheck-flags.py`, review 6 Q12: "default p = 2 identity alone does not establish treatment
  identity"); (c) the replay identity gate at p = 2 and at `none` on the bite-15 capsule (43/43
  with relocates, evaluations and stream equal).
- Authority: GPT-6 Astra (review 6), on the owner's instruction "ignora grok, usa codex astra
  xhigh". The owner's constraints stand: the exact 5.0/5.0 contract and the validator untouched,
  nothing copied from Sparrow, no polygon simplification, no pole proxy.

## Arms

| arm | flags | profile caps |
|---|---|---|
| Control `A` | `--proxymargin=8` (p = 2) | Legacy 50 / Wall10s unbounded |
| Treatment `B` | `--proxymargin=8 --guidedexponent=1` | identical |

"The exponent is fixed for the entire process, across both phases, all workers, retries and
publications" (review 5b Q7). No other flag differs between the arms. No CD limit, sampling
count, equality acceptance, weight cap, boundary margin, publication rule or profile schedule
changes; any later smoothing, boundary-specific exponent or adaptive p is another treatment.

## Population and walls — every scored cell on seeds 27–35, bare request, fresh process

- Seeds **27–35**, both profiles, three repetitions: **108 new cells**. The virgin population:
  no treatment of any round has run on them; the frozen engine's own cells on them (p = 2,
  margin 0, `/var/lib/t3/tmp/astra/base/`, 27 per profile, executable `4df261ed7e94`) exist and
  are the archived baseline of the "package" comparison below; their outcomes are already known,
  so "the holdout is untouched by these treatments, not entirely unseen" (review 6).
- Each cell: `--cell=cutclose --request=tests/fixtures/mixed-61/mixed61-request-exact-clearance.json
  --edge=5 --pair=5 --mode=wall --wall=10.0 --orders=1 --workers=8 --arm=control
  --profile=<legacy|wall10s> <arm flags> --seed=<s>`, one fresh eight-worker process, the
  existing ten-second accounting (constructor charged inside the wall), the bench lock held for
  the whole round so no two measurements overlap.
- Order: adjacent control/treatment **pairs** per (profile, seed, repetition), the pair's order
  rotated by seed and repetition (`(seed + rep) mod 2`: even → A then B, odd → B then A), the
  manifest of pairs written and committed before the first cell (`screen/manifest-v1.txt`).
- "Complete the registered population without efficacy-based early stopping."
- Contamination, defined now from an independently logged event: the load sampler
  (`load-v1.log`, one `/proc/loadavg` sample every 5 s, started before the first cell) and the
  bench-lock discipline. A **pair** is re-run whole if any sample inside either of its cells'
  ten-second windows shows a 1-minute load average above 10.0, or if any cargo invocation is
  found to have run outside the bench lock during either window (from the agents' transcripts);
  originals are preserved beside the replacements and both scores are reported. No cell is
  excluded or re-run for any other reason, and never because its work count looks low.
- Expensive geometry diagnostics stay outside the scored cells: after the 108 cells, one
  `--bitemicroscope=1` cell per (profile, seed) at repetition 0's order position for the control
  arm only (its trigger: the first explore bite reaching 34 iterations; absence reported without
  substitution), unscored, refused by the scorer through its tripwire. Diagnostic results do not
  alter any threshold.

## Clock and quality

Ten seconds of wall from the bare request; quality is the final valid published depth
(`outcome.depthMm`, the raw-source depth of a dual-valid layout); a cell whose document lacks a
finite depth or the `invalidPublications` field is a refusal of the round, not a zero.

## Statistics

For profile h, seed s and arm a, `D_{a,h,s}` = median over the three repetitions of the final
valid depth; `g_{h,s} = D_{2,h,s} − D_{1,h,s}` (positive = treatment deeper). Work per cell from
the documents: total sample evaluations (`outcome.work.sampleEvaluations`); explore evaluations
= Σ over explore bites of `strikeMeter.chargedWorkSampleEvaluations` (failed and truncated bites
charged); published explore bites; explore bites started; `wall.loopExploreSeconds`. Aggregates
per (arm, profile) over the 27 cells: explore evaluations per published explore bite (sum /
sum), explore elapsed time per published explore bite (sum / sum), published explore bites per
cell, publication fraction (published / started). Scorer: `spec-score.py`, frozen at the round's
base commit.

## PASS — required, separately on both profiles (review 6 Q11)

1. **Depth:** `median_s g_{h,s} > 0`.
2. **Tail:** `min_s g_{h,s} ≥ −1.000 mm`, unrounded.
3. **Engineering:** lower total evaluations per cell; lower explore evaluations and lower
   explore elapsed time per published explore bite; no reduction in published explore bites per
   cell or in their publication fraction.
4. **Integrity:** exactly three uniquely identified repetitions per seed and arm; every document
   verified (request sha256, contract, executable sha256, profile, seed, `proxyMarginUm = 8`,
   `guidedExponent` absent in A and 1 in B, no tripwire); zero invalid publications; the three
   identity checks above passed; no unregistered configuration or timing change.

**Promote only if every required condition passes on both profiles.** Otherwise record which
condition failed. "Do not change p, caps, margin, seeds or the regression guard after
inspecting validation results. A selected p failing validation does not authorize trying
another p on those same seeds as another unseen confirmation."

## Reported, not required

- **The package against the frozen record:** the treatment against the archived frozen cells
  (p = 2, margin 0) with the same depth and tail conditions; a pass supports the claim "the
  margin-8 + p = 1 package improves the frozen record"; a failure "need not erase a conditional
  exponent improvement, but it prevents the stronger package claim". The historical comparison
  cannot isolate the exponent or separate package effects from measurement-period effects.
- **Sparrow:** "This round tests whether the selected ICS successor improves ICS and reduces
  its observed distance from the archived Sparrow reference of 150.165 mm. It does not test
  population-level superiority over Sparrow." Reported: the Wall10s median-of-seed-medians, its
  residual gap to 150.165, the frozen reference's gap (9.550 mm on these seeds), the reduction,
  and the count of seed medians at or below 150.165.
- **Forecast outcomes** (the old registered forecasts, preserved and scored, never promotion
  requirements): the evaluation halving (explore evaluations per published explore bite at most
  half the control's) and the −30 % development work target.
- **Diagnostics with denominators** (review 6 Q13, from the documents and the unscored
  microscope cells): failed bites (count, elapsed, evaluations, iterations, stop reason);
  column churn, conflict concentration, distant moves, cleanup and numerical behaviour where
  the microscope cells expose a hard bite.

## Refusal

The round is refused as a whole, before any depth is read, if: any scored document carries a
tripwire; any (arm, profile, seed) has other than three uniquely identified repetitions; any
document fails a verification; the frozen identities were not appended before the first cell;
or a cell ran outside the bench lock's serialization. A refused round is recorded as refused,
not re-run under a changed rule.

## Frozen identities (appended before the first scored cell, as the protocol requires)

- Base commit: `1e913f4` on `engine/topology-archive-search` (suite: 862 passed, 0 failed).
- Executable: `/var/lib/t3/tmp/astra/frozen-1e913f4`, sha256
  `8734404a249e06fb57c387a9e1d310147a5b924b255378ff880ed59f189227b2`, byte-identical to the build
  from 0cb1b2b (the only later change is tests-only); rustc 1.97.1 (8bab26f4f 2026-07-14), cargo
  1.97.1, x86_64, `cargo build --release --example overlap_ics_benchmark -p polygon-nesting-core
  --features overlap-ics`.
- Request: `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`, sha256
  `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3`; contract pair 5.0 / edge 5.0
  (`--edge=5 --pair=5`).
- Identity checks, verbatim from `evidence/ics-guided-exponent-v1/freeze-1e913f4.log`:

```
    Finished `release` profile [optimized] target(s) in 0.03s
built 8734404a249e06fb
frozen-1e913f4 sha256 8734404a249e06fb57c387a9e1d310147a5b924b255378ff880ed59f189227b2
== default bitcheck vs frozen-f856d19
seed 0 IDENTICAL  depth 181.517305  poses same
seed 3 IDENTICAL  depth 181.517305  poses same
seed 7 IDENTICAL  depth 181.517305  poses same
ALL_IDENTICAL
== treatment bitcheck (p = 1, margin 8) vs frozen-f856d19
flags: --proxymargin=8 --guidedexponent=1
seed 0 IDENTICAL  depth 181.322019  poses same  (guidedExponent 1.0, proxyMarginUm 8)
seed 3 IDENTICAL  depth 181.347459  poses same  (guidedExponent 1.0, proxyMarginUm 8)
seed 7 IDENTICAL  depth 181.342611  poses same  (guidedExponent 1.0, proxyMarginUm 8)
ALL_IDENTICAL
== control bitcheck (margin 8) vs frozen-f856d19
flags: --proxymargin=8
seed 0 IDENTICAL  depth 181.391461  poses same  (guidedExponent 2.0, proxyMarginUm 8)
seed 3 IDENTICAL  depth 181.351461  poses same  (guidedExponent 2.0, proxyMarginUm 8)
seed 7 IDENTICAL  depth 181.398335  poses same  (guidedExponent 2.0, proxyMarginUm 8)
ALL_IDENTICAL
== replay identity exponent:2
replay identity (exponent:2): 43/43 PASS, 0 FAIL (extended gate: scalars, relocates 43/43, evaluations 43/43; stream 43/43)
== replay identity none
replay identity (none): 43/43 PASS, 0 FAIL (extended gate: scalars, relocates 43/43, evaluations 43/43; stream 43/43)
rustc 1.97.1 (8bab26f4f 2026-07-14)
cargo 1.97.1 (c980f4866 2026-06-30)
x86_64
FREEZE_V1_DONE
```

- Scorer: `docs/experiments/overlap-ics/guided-exponent/screen/spec-score.py` at this commit;
  the treatment-identity check `bitcheck-flags.py` beside it; runner `spec-run.sh`, diagnostic
  runner `spec-diag.sh` and the pair manifest `manifest-v1.txt` under `evidence/ics-guided-exponent-v1/`.
- Cells: `/var/lib/t3/tmp/astra/v1/v1-<A|B>-<profile>-r<rep>-s<seed>.json`, archived gzipped under
  `evidence/ics-guided-exponent-v1/cells/` after the round; the load log `load-v1.log` beside them.
- Scoring commands, fixed now:
  `python3 spec-score.py /var/lib/t3/tmp/astra/v1 v1 --control A --treatment B --knobs A='proxymargin=8' B='proxymargin=8,guidedexponent=1' --seeds 27..35 --frozen /var/lib/t3/tmp/astra/base base --json v1-score.json`
  (and once more with `--originals` if any pair is re-run under the contamination rule).

# Result: NOT PROMOTED — the Legacy tail fails on seeds 29 and 31; Wall10s passes all four conditions; the package improves the frozen record on both profiles

Appended 2026-09-07 after scoring; nothing above was edited. The round ran 10:18:20–10:36:26 UTC
from the committed manifest, 108 cells, no pair re-run: the load sampler's maximum inside any
cell window was 8.33 and no cargo invocation ran during the round (`evidence/contamination.txt`).
Scored with the fixed command (`evidence/v1-score.txt`, `v1-score.json`; cells under
`evidence/cells/`).

| required, per profile | Legacy | Wall10s |
|---|---|---|
| 1 depth: median g | **+2.343 mm** (7/9) PASS | **+0.851 mm** (6/9) PASS |
| 2 tail: min g ≥ −1.000 | **−3.336 (seed 29), −1.929 (seed 31): FAIL** | −0.523 (seed 28) PASS |
| 3 engineering | total evaluations −1.6 %, explore evaluations per published bite −9.9 %, elapsed per published bite −7.5 %, bites 110.5 → 119.4, fraction 99.10 → 99.17 %: PASS | −10.5 %, −15.8 %, −5.3 %, 4.00 → 4.22, 80.00 → 80.85 %: PASS |
| 4 integrity | 0 invalid, all verified: PASS | PASS |

**Verdict under the specification: DO NOT PROMOTE** (failed: Legacy tail). Per-seed medians
(control | treatment), Legacy: 27 164.005 | 159.028; 28 164.004 | 161.662; 29 160.809 | 164.145;
30 164.343 | 162.963; 31 160.364 | 162.293; 32 164.009 | 161.018; 33 163.063 | 159.016;
34 163.243 | 159.019; 35 159.009 | 158.987. Wall10s: 27 159.031 | 154.421; 28 159.053 | 159.576;
29 159.418 | 158.156; 30 159.126 | 159.497; 31 159.432 | 154.941; 32 159.664 | 159.021;
33 159.997 | 159.028; 34 159.873 | 159.022; 35 159.014 | 159.060.

What the two failing seeds are: the repetitions within each (seed, arm) agree to a tenth of a
millimetre (seed 29: control 160.81 / 160.64 / 161.28, treatment 163.16 / 164.14 / 164.16; seed
31: control 160.37 / 160.36 / 160.35, treatment 162.31 / 162.29 / 162.29), so these are not
repetition noise but the two basins the Legacy profile lands in per seed (164 or 159–161), and
on these two seeds the control's trajectory reaches the deeper basin and the treatment's does
not. The treatment wins the other seven seeds by 0.02–4.98 mm. The tail guard was written for
exactly this and it is not amended.

**Reported, not required.**
- *The package against the frozen record* (p = 2, margin 0, the archived 27 cells per profile,
  executable `4df261ed…`): Legacy median +5.015 mm (8/9, min −0.143), Wall10s +1.821 mm (7/9, min
  −0.022); both pass the depth and tail conditions: **the margin-8 + p = 1 package improves the
  frozen record on both profiles.** This comparison cannot isolate the exponent (the margin is in
  both arms of the required comparison and only in the treatment here) or separate package
  effects from measurement-period effects.
- *Sparrow* (residual-gap claim only): Wall10s treatment median of seed medians 159.022 mm, gap
  to 150.165 = 8.857 mm, against the frozen reference's 9.550 (reduction 0.693 mm); Legacy
  161.018, gap 10.853 (frozen 14.770, reduction 3.917). Seed medians at or below 150.165: 0/9 on
  both profiles (individual cells: Wall10s seeds 27 and 31 sit at 154.4 and 154.9).
- *Forecast outcomes*: the halving is missed (explore evaluations per published explore bite
  −9.9 % Legacy, −15.8 % Wall10s); the −30 % development work target is missed.
- *Diagnostics*: the 18 unscored control-arm microscope cells are appended below when they land.

**Integrity notes.** Executable `frozen-1e913f4` (sha256 `8734404a…`) in every document;
request sha256 and contract equal across the 108 cells and the frozen record; exactly three
repetitions per (arm, profile, seed); zero invalid publications; zero checkpoint refusals of any
kind in both arms; the three identity checks passed before the first cell.

## Diagnostics (appended after the result; unscored)

The 18 unscored control-arm cells (`--proxymargin=8 --bitemicroscope=1`, one per profile and
seed, run after the 108 scored cells, refused by the scorer through their tripwire) all found a
trigger bite: on Legacy the first explore bite reaching 34 master iterations is bite 15–21 on
every seed, on Wall10s it is bite 1 (its bites are all long). Each trigger bite's entry capsule
was then replayed from the same state at p = 2 (the control, identity gate live) and at p = 1
(the treatment on the *same* bite), 100 iterations maximum, `--certify=1`
(`evidence/ics-guided-exponent-v1/diagnostics/`, `v1-replay-diag.sh`):

| profile, seed (bite) | p = 2: band entry, evaluations, column break, max column-row weight at break | p = 1: band entry, evaluations, column break, weight |
|---|---|---|
| Legacy 27 (21) | none in 100, —, 21, 1.5e4 | 85, 1 427 804, 50, 7.9e2 |
| Legacy 28 (15) | 43, 596 758, 21, 1.5e4 | 16, 198 496, 14, 7.0e1 |
| Legacy 29 (21) | 89, 1 836 564, 32, 3.0e5 | 72, 1 388 091, 4, 9.8 |
| Legacy 30 (16) | 38, 822 400, 16, 1.1e4 | 42, 487 031, 16, 2.2e2 |
| Legacy 31 (16) | 61, 1 054 359, 25, 3.7e4 | 39, 542 011, 13, 7.2e1 |
| Legacy 32 (21) | 80, 1 510 451, 19, 6.1e3 | none in 100, —, 12, 4.1e1 |
| Legacy 33 (15) | 46, 658 152, 26, 4.2e4 | 25, 253 389, column avoided |
| Legacy 34 (21) | none in 100, —, 22, 4.8e4 | 69, 1 112 194, 20, 2.8e2 |
| Legacy 35 (21) | 95, 2 162 463, 20, 6.2e4 | 83, 1 464 325, 19, 3.5e2 |
| Wall10s 27 (1) | 50, 1 361 098, 16, 7.1e2 | 32, 947 257, 4, 6.4 |
| Wall10s 28 (1) | 35, 1 225 751, 12, 1.9e2 | 85, 1 475 577, 5, 3.8 |
| Wall10s 29 (1) | 81, 2 489 046, 31, 8.8e3 | 20, 559 070, 3, 1.9 |
| Wall10s 30 (1) | 46, 1 323 581, 22, 1.9e3 | 68, 1 323 528, 15, 6.4e1 |
| Wall10s 31 (1) | 47, 1 912 544, 14, 2.6e2 | 31, 1 015 555, 7, 1.1e1 |
| Wall10s 32 (1) | 62, 2 666 422, 14, 3.8e2 | 42, 1 161 794, 5, 7.7 |
| Wall10s 33 (1) | 50, 1 528 653, 27, 2.9e3 | 35, 680 495, 2, 3.0 |
| Wall10s 34 (1) | 87, 2 017 083, 17, 6.2e2 | 57, 1 239 302, 12, 4.7e1 |
| Wall10s 35 (1) | 51, 1 371 452, 13, 2.6e2 | 34, 938 910, 11, 2.2e1 |

Over the 15 bites where both exponents reach the band within 100 iterations, p = 1 enters the
band earlier on 12 (median ratio 0.67 of the control's iterations) and with fewer evaluations
on 14 (median ratio 0.59); two Legacy bites the control cannot finish in 100 iterations (seeds
27, 34) the treatment finishes, and one (seed 32) the reverse. The column (entry-graph reading;
here it forms at iteration 1–4 rather than at entry on all but two capsules, so these are
trajectory descriptions in review 6's sense) breaks at p = 1 at weights one to four orders of
magnitude below the control's on every bite, and on Legacy seed 33 never forms. The band-entry
states certify through the unchanged publication path in 16/18 (p = 2) and 17/18 (p = 1) cases;
the misses are the no-band cases. Temporary releases are again more frequent under p = 1 on
most bites. This is the mechanism gate's evidence across seeds: the repricing is not one seed's
accident. It does not speak to the two failing Legacy seeds' verdicts, which are decided over
the whole ten seconds by which basin the trajectory reaches, not by this one bite: on seed 29
the treatment resolves the control's hard bite in 72 iterations against 89, and still finishes
the cell 3.3 mm shallower.

## Corrections from review 7 (appended; the result above is not edited)

- "The repetitions within each (seed, arm) agree to a tenth of a millimetre" is inaccurate:
  seed 29's within-arm ranges are 0.637 mm (control) and 0.999 mm (treatment); the separation
  between the arms is nevertheless complete on seeds 29 and 31 (every treatment repetition
  worse than every control repetition, by more than 1.87 and 1.91 mm), so the tail failures
  stand without that claim.
- "Two basins" is a hypothesis, not a finding: "different retained packing configurations and
  retry histories are plausible contributors, but the endpoint depths do not establish exactly
  two basins or isolate the cause of these regressions". Review 7's wording of the outcome:
  "At ten seconds, p = 1 changes the search trajectory, with favourable but nonuniform depth
  effects. Wall10s passes all four per-profile conditions; Legacy fails the tail condition on
  seeds 29 and 31."
- The seed-29 replay reading (72 against 89 iterations) runs both trajectories past Legacy's
  live cap of 50 and from the control's entry, which is not the treatment's own entry; it
  supports cheaper detached continuation from that state, not either live attempt's retries.
  From the scored cells: on seed 29's bite 21 the treatment spends 281 iterations and 5
  disruptions against the control's 126 and 2, and publishes 101–102 explore bites per cell
  against 120–121.
- The three statements the branch may carry (review 7, Q16): (a) "ICS-guided-exponent-v1: not
  promoted; the required Legacy tail condition failed." (b) "On Wall10s, p = 1 passed every
  per-profile condition of the prospective ICS-guided-exponent-v1 round against contemporaneous
  p = 2 control, with margin 8 in both arms. The round's joint promotion requirement failed."
  (c) "In the prospectively specified comparison against the archived frozen record, the
  margin-8 + p = 1 package passed the depth and tail conditions on both profiles", always with
  "the historical comparison cannot isolate the exponent or separate package effects from
  measurement-period effects" beside it.
