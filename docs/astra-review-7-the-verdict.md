# GPT-6 Astra review 7, verbatim: the round is not promoted, the Wall10s-only successor, the thirty seeds, the deeper failed cut

Reply of `gpt-6-astra` (`codex exec`, reasoning effort xhigh, read-only sandbox) to brief 7, 2026-09-07, verbatim. The brief follows the reply.

## Reply

**The “not promoted” verdict is correct. A Wall10s-only successor is legitimate, but the Legacy “two basins” explanation is still a hypothesis. The next microscope should examine the deeper failed cuts.**

I reviewed `12fbc65` read-only, reproduced the scorer’s verdict and both historical package comparisons, independently recomputed the depth and engineering summaries, and checked the 36 diagnostic replays. The frozen specification is unchanged before the appended result. I ran no new cells.

For **Q15**, there is more to read before assigning the Legacy failures to basins.

First, correct “the repetitions … agree to a tenth of a millimetre.” Seed 29’s within-arm ranges are **0.637 mm for control and 0.999 mm for treatment**. Nevertheless, the regression is consistent across repetitions: every treatment result is worse than every control result, with even the closest separation exceeding 1.87 mm. Seed 31 likewise has complete separation, exceeding 1.91 mm. The tail failures stand without the inaccurate repeatability claim.

Second, the scored cells already expose a relevant difference:

| Legacy observation | Control p = 2 | Treatment p = 1 |
|---|---:|---:|
| Seed 29, bite 21, all three repetitions | 126 iterations; 2 disruptions | 281 iterations; 5 disruptions |
| Seed 29, published explore bites per cell | 120–121 | 101–102 |
| Seed 31, published explore bites per cell | 123–124 | 112 |
| Seed 31, total explore disruptions per cell | 4 | 4 |

These are calculations from the [scored archive](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/quorum/evidence/ics-guided-exponent-v1/cells). Seed 29 warrants tracing the cap, pool restoration and disruption sequence. Seed 31 shows why disruption counts alone cannot explain the result.

The replay qualification matters here: **72 versus 89 iterations on seed 29 extends both trajectories beyond Legacy’s live cap of 50.** The replay explicitly substitutes its own iteration horizon. Its result supports cheaper detached continuation from that control entry; it does not reproduce either live attempt’s subsequent retries. Furthermore, that control entry is not the treatment’s actual entry. [Replay semantics](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/replay.rs:1835)

My wording would be:

> At ten seconds, p = 1 changes the search trajectory, with favourable but nonuniform depth effects. Wall10s passes all four per-profile conditions; Legacy fails the tail condition on seeds 29 and 31. Different retained packing configurations and retry histories are plausible contributors, but the endpoint depths do not establish exactly two basins or isolate the cause of these regressions.

Trace where the paths acquire different retained geometry, retry histories and completion costs, then follow those differences through publication and compression. A common-policy continuation from the resulting parents, under a fixed diagnostic work budget, would help distinguish poorer configurations from slower progress within ten seconds. None of this is needed to settle the existing verdict.

For **Q16**, the branch may carry these statements:

| Claim | Wording I would use |
|---|---|
| **(a)** | “ICS-guided-exponent-v1: not promoted; the required Legacy tail condition failed.” |
| **(b)** | “On Wall10s, p = 1 passed every per-profile condition of the prospective ICS-guided-exponent-v1 round against contemporaneous p = 2 control, with margin 8 in both arms. The round’s joint promotion requirement failed.” |
| **(c)** | “In the prospectively specified comparison against the archived frozen record, the margin-8 + p = 1 package passed the depth and tail conditions on both profiles.” |

For (c), retain the qualification immediately alongside it: **the historical comparison cannot isolate the exponent or separate package effects from measurement-period effects**. “Prospective” accurately describes the specification and treatment evaluation; it should not imply a contemporaneous package experiment. These distinctions follow the [registered comparison rules](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/quorum/ics-guided-exponent-v1-spec.md:136).

A Wall10s-only successor is legitimate. The Legacy failure ends this candidate’s claim to promotion across both profiles; it does not end research on the exponent. Register Wall10s as the successor’s sole promotion scope, preserve the failed joint result, and validate independently. The nine Wall10s seeds support choosing that next question; they cannot also serve as its fresh confirmation.

For **Q17**, I recommend **30 new seeds, three repetitions, two arms, Wall10s only: 180 cells**, approximately 30 minutes of cell budgets.

Thirty is motivated principally by the tail requirement. Under independent uniform seed sampling, a treatment with a 10% probability of a greater-than-1 mm seed-level regression would escape detection on all 30 seeds with probability \(0.9^{30}=4.24\%\). With nine seeds that probability is 38.74%. This is a tail-screen rationale, not a power calculation for confirming a 0.851 mm median gain.

Draw them as follows:

1. Freeze the candidate, rules, seed-generation procedure and an exclusion registry containing every previously exercised seed for this fixture—including diagnostics and archived baselines.
2. Generate one fresh, recorded random nonce. Use a fixed, documented SHA-256 counter procedure to produce unsigned 64-bit seed candidates; reject duplicates and excluded values, retaining the first 30 admissible seeds. Record the nonce and complete draw log.
3. Commit the seed list and execution manifest before running either arm. Randomize pair order and balance control-first versus treatment-first positions across repetitions.
4. Complete the population with the existing depth, tail, engineering and integrity requirements, plus the independently determined whole-pair contamination rule. No difficulty-based substitutions or additional seeds after inspecting results.

Use consumed seeds for further mechanism development. Keep this population untouched until the candidate is frozen. The resulting claim remains specific to the fixed fixture, request and execution regime.

For **Q18**, the next exposure is now identifiable from the scored evidence.

The Wall10s microscope examined **bite 1, targeting 177.121 mm**. Across all 27 treatment cells:

- Twenty-one publish four explore bites and fail on the next cut, roughly **160.6 → 155.5 mm**.
- The six cells on seeds 27 and 31 publish a fifth bite and fail on the next cut, roughly **155.5 → 150.5 mm**.
- The final unpublished bite accounts for **79.5% of treatment exploration evaluations**, versus 62.1% for control.
- Those failed bites never reach the proxy band and make no exact attempts. There are **zero explore disruptions in either Wall10s arm**.

The first bite accounts for only **3.2% of treatment exploration evaluations**. These calculations locate the dominant unresolved work; a larger failed-bite share can arise because earlier savings let the engine spend longer on the difficult cut.

**My next mechanism hypothesis is that releases at the deeper target transfer or recreate conflict, preventing affordable local progress from becoming a deeper valid parent.** The existing temporary-release observations motivate that hypothesis, but do not establish it at these depths.

I would specify the next microscope on consumed seeds 27–35, both arms, repetition 0: capture the first explore cut targeting at most 156 mm; if it publishes, also retain the next cut targeting at most 151 mm. Record absence without substitution. Capture each arm’s own entry and follow the actual attempt to publication or its live stopping boundary.

It should first establish:

- **What persists:** blocking rows, residuals, weights, connected structures and their reformation. Separate a released original column from newly created blockers.
- **Where useful moves disappear:** candidate discovery, refinement, worker commit and tournament retention, with all-worker expenditure.
- **What releases accomplish:** work from release to band and exact acceptance, then the geometry of the published parent and difficulty of its next cut.
- **Why the attempt ends:** remaining wall allowance, strikes, rollbacks and actual stop reason, keeping unfinished trajectories in the denominator.

Replay both objectives from each captured entry, with identity checks. Equal seed and bite ordinal do not imply equal state, and another 100-iteration horizon would censor much of the observed deeper work.

If those traces show individual moves repeatedly transferring the obstruction between neighbours, a bounded coordinated move that changes their relative geometry becomes the next justified probe. If useful candidates already exist but are discarded, investigate their selection first. The microscope needs to demonstrate that distinction before another operator or schedule change is selected.

## Brief 7, as sent

## Brief 7: the prospective round ICS-guided-exponent-v1 is scored — not promoted, on the Legacy tail alone

Read after your review 6. Everything is committed on `engine/topology-archive-search` (worktree
`/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a`); the files are named at the end.

### 1. What was done since review 6, in order

- Corrections you asked for before the freeze: `eval_cmp` orders a NaN as the worse side and
  the tournament's new `guided_beats` never lets a NaN win (7fe3df5; finite paths untouched,
  bitcheck identical, a test); `capturedExponent` reads the capsule's own exponent and the live
  capture records the knob (0cb1b2b, with the certify test against the live path's own refusal
  string, the unreached fork made explicit, the negative identity test, the wording); the
  "p < 2 is concave" wording and the piece-6 / piece-45 counts corrected with your credit; the
  scorer rewritten (`spec-score.py`: explore-only evaluations and elapsed time per *published*
  explore bite with failed bites charged, exactly three uniquely identified repetitions,
  request / contract / executable / knob verification, refusal instead of defaults; it
  reproduces your recomputation figure for figure).
- The selection, under the rule frozen before the 54 p = 0.75 cells (`selection-rule.md`, your
  Q12 verbatim; same frozen executable `f856d19`; three cells re-run by the registered load
  rule with originals kept): p = 1 eligible on both archives; p = 0.75 not eligible (Wall10s
  worst seed −4.161 mm; Wall10s explore elapsed time per published bite +10 %, published bites
  per cell 4.00 → 3.63, publication fraction 80.0 → 78.4 %; on the replacement archive also
  the Legacy tail −1.542). H = D_1 − D_0.75 was recorded (Legacy median −1.373, Wall10s −3.935)
  and not reached. Decision: p = 1, identical on both archives.
- The specification `docs/quorum/ics-guided-exponent-v1-spec.md`, your Q11–Q14 transcribed:
  committed (b63699d) before the first cell with the frozen identities appended (base 1e913f4,
  862 tests; executable `frozen-1e913f4` sha256 8734404a…; bit-identical to the screen's
  `f856d19` on the default path, on the treatment path p = 1 / margin 8 and on the control path
  margin 8 over the three fixed-work cells; replay identity 43/43 at p = 2 and at `none`); the
  pair manifest ((seed + rep) mod 2 rotation) committed with it; the contamination rule from
  the independently logged load samples, whole pairs, never for a low work count.
- The round: 108 cells, 10:18–10:36 UTC, no pair re-run (max load in any cell window 8.33, no
  cargo invocation during the round), scored with the fixed command.

### 2. The verdict (`docs/quorum/ics-guided-exponent-v1-spec.md`, "# Result", appended, nothing amended)

| required, per profile | Legacy | Wall10s |
|---|---|---|
| depth: median g | +2.343 mm (7/9) PASS | +0.851 mm (6/9) PASS |
| tail: min g ≥ −1.000 | **−3.336 (seed 29), −1.929 (seed 31): FAIL** | −0.523 (seed 28) PASS |
| engineering | total evaluations −1.6 %, explore evaluations per published bite −9.9 %, elapsed per published bite −7.5 %, published bites 110.5 → 119.4, fraction 99.10 → 99.17 %: PASS | −10.5 %, −15.8 %, −5.3 %, 4.00 → 4.22, 80.00 → 80.85 %: PASS |
| integrity | PASS | PASS |

**DO NOT PROMOTE** (failed: Legacy tail). Per-seed medians, Legacy (control | treatment): 27
164.005 | 159.028; 28 164.004 | 161.662; **29 160.809 | 164.145**; 30 164.343 | 162.963; **31
160.364 | 162.293**; 32 164.009 | 161.018; 33 163.063 | 159.016; 34 163.243 | 159.019; 35 159.009
| 158.987. Wall10s: 27 159.031 | 154.421; 28 159.053 | 159.576; 29 159.418 | 158.156; 30 159.126
| 159.497; 31 159.432 | 154.941; 32 159.664 | 159.021; 33 159.997 | 159.028; 34 159.873 | 159.022;
35 159.014 | 159.060.

The repetitions within each (seed, arm) agree to a tenth of a millimetre (seed 29: control
160.81 / 160.64 / 161.28, treatment 163.16 / 164.14 / 164.16; seed 31: 160.37 / 160.36 / 160.35
against 162.31 / 162.29 / 162.29): the Legacy profile lands per seed in one of two basins (164,
or 159–161) and on these two seeds the control's trajectory reaches the deeper one and the
treatment's does not. The other seven seeds gain 0.02–4.98 mm. The guard was written for this
and is not amended.

Reported, not required: against the archived frozen record (p = 2, margin 0, the 27 cells per
profile that pre-date every treatment) the package gains +5.015 mm on Legacy (8/9, min −0.143)
and +1.821 on Wall10s (7/9, min −0.022), passing both conditions: **the margin-8 + p = 1
package improves the frozen record on both profiles**, with the caveats you wrote. Sparrow:
Wall10s median of seed medians 159.022, residual gap 8.857 mm against the frozen 9.550
(reduction 0.693); Legacy 161.018, gap 10.853 (frozen 14.770); seed medians at or below 150.165:
0/9 on both (cells at 154.4 and 154.9 on Wall10s seeds 27 and 31). Halving and the −30 %
forecast: missed.

### 3. Diagnostics (unscored, control arm, margin 8, one `--bitemicroscope=1` cell per profile and seed)

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


### Questions

**Q15. The verdict and the two seeds.** The specification's answer is "not promoted", and I
record it so. Is there anything in the two failing seeds that the trace instruments should
read before any next step (the replays above are the first reading), or is the correct
statement simply that p = 1 changes which basin a Legacy seed lands in, with a favourable but
not uniform effect, and that the Wall10s profile, the ten-second profile the owner cares
about, passes the round's four conditions while Legacy does not?

**Q16. What this permits.** With the required comparison failed on one profile and passed on
the other, and the package comparison passed on both: which of these statements may the
branch carry, in your words? (a) "ICS-guided-exponent-v1: not promoted"; (b) "on the Wall10s
profile the exponent passes a prospective round"; (c) "the margin-8 + p = 1 package improves
the frozen record prospectively on both profiles". And is a profile-specific successor
(Wall10s only, its own prospective round on a new virgin population) a legitimate next
specification, or does the Legacy failure end the exponent line?

**Q17. Seeds.** Seeds 27–35 are now consumed by this treatment. The next prospective round
needs a new virgin population; propose its size and how it is drawn (the fixtures and the
request are fixed; the seed is the only free parameter).

**Q18. The next exposure.** The remaining gap on Wall10s is 8.9 mm to Sparrow's 150.165. From
the diagnostics above and everything before: what is the next mechanism to trace, and what
does its microscope need to show first? My reading of the two failing Legacy seeds and of the
virgin-seed trigger bites is in section 3.

### Files

- `docs/quorum/ics-guided-exponent-v1-spec.md` (specification, frozen identities, "# Result",
  "## Diagnostics"); `docs/quorum/evidence/ics-guided-exponent-v1/` (the 108 documents, the 18
  microscope documents and their replays, `v1-score.txt` / `.json`, `load-v1.log`,
  `contamination.txt`, manifest, runners, `freeze-1e913f4.log`)
- `docs/experiments/overlap-ics/guided-exponent/` (`selection-rule.md`, `screen/selection.txt`,
  `screen/spec-score.py`, `screen/bitcheck-flags.py`, the C arm's documents)
- commits 7fe3df5, 0cb1b2b, 1e913f4, b63699d, f11e3b0
- `docs/astra-review-6-the-live-exponent.md` (your review 6, verbatim)
