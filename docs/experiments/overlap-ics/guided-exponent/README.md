# The guided exponent, live: `--guidedexponent=<p>`

The knob (commit "The guided exponent: --guidedexponent=<p> ...", 6d55efb, loose ends f856d19)
makes the separation *rank* on `sum w v^p` instead of `sum w v^2`: `energy::fold` (the
tournament's winner) and `energy::incident_totals` (the relocate's candidates and both CD stages)
are now the `_with_exponent` variants at a process-level knob, `p = 2` instantiated as
`w * (v * v)` so the default is the frozen engine to the bit (bitcheck ALL_IDENTICAL against
frozen-2b5f16d and frozen-9c38526). Raw Φ (`sum v^2`), the max violation, the 4 µm band, the
strike meter, `gls_update`'s `v / v_max` growth, the pool, the disruption, `publish.rs`, the
kernel and `validate_placements_against_contract` are untouched; the lexicographic rule "any
clear pose beats every colliding pose" (`relocate::eval_cmp` on `raw`) stays. The exponent is a
landscape change of our own design (Grok review 12 line 188 chose `v^2` for "one guided path"),
not Sparrow's pole proxy: `v` is still the source-ring signed-gap residual. Why it exists: the
bite microscope's pinned column (`../bite-microscope/README.md`) and the replay exponent probe
there; GPT-6 Astra reviews 5 (Q3) and 5b (Q6–Q9, `docs/astra-review-5b-the-exponent.md`).

The verifier (workflow `ics-mechanisms-round`, adversarial read against the forbidden-rescue
table of `docs/grok-review-12-reading-sparrow.md` §5.2) refuted nothing and named four
non-refuting defects: the corpus cell's gradient probe would mix a `v^2` direction with a `v^p`
acceptance (the cell now refuses the knob, f856d19); a doc drift on `guided_term_with_exponent`
(fixed, f856d19); the knob tests share the existing `knob_lock` exposure of `ProxyMarginGuard`;
and one `Relaxed` atomic load per `fold` / `incident_totals` call on the default path
(trajectory-neutral under fixed work; only wall-mode timing could notice).

## Development measurement (dev seeds 18–26, one repetition, no margin, profile caps)

Both arms without the proxy margin, because the frozen `devbase` cells were measured without
it and this is one factor at a time; Legacy cap 50, Wall10s unbounded, ten seconds from the
bare request, one eight-worker process at a time. Documents: `guided-exponentfull-*.json.gz`
(p = 1, nine seeds) and `guided-exponent075-*.json.gz` (p = 0.75, seeds 18–20). The frozen
control cells are `devbase-*` under `/var/lib/t3/tmp/astra/dev` (the same documents as in the
proxy-margin experiment).

`analyse.py devbase guided-exponentfull` (p = 1, nine seeds), verbatim:

```
== profile legacy | devbase vs guided-exponentfull | 9 seeds, 9/9 cells ==
  devbase      median  164.954  mean  165.105  best  163.550  worst  167.157  iters/cell   2695  explore bites  99.8
  guided-exponentfull median  164.002  mean  164.685  best  162.132  worst  169.039  iters/cell   2708  explore bites 103.9
  paired median gain (A-B) +0.846 mm | B wins 6/9 | worst paired -4.484 | invalid publications 0
  per seed: 18:-1.12 19:+0.85 20:-1.12 21:+3.16 22:+0.65 23:+1.26 24:+2.38 25:-4.48 26:+2.22
== profile wall10s | devbase vs guided-exponentfull | 9 seeds, 9/9 cells ==
  devbase      median  159.861  mean  160.243  best  159.003  worst  165.237  iters/cell   1345  explore bites   4.9
  guided-exponentfull median  159.659  mean  160.147  best  154.848  worst  164.407  iters/cell   1904  explore bites   4.9
  paired median gain (A-B) +0.114 mm | B wins 5/9 | worst paired -5.404 | invalid publications 0
  per seed: 18:-0.36 19:-4.15 20:+0.11 21:+10.39 22:+0.70 23:-1.43 24:-5.40 25:+0.30 26:+0.71
```

`analyse.py devbase guided-exponent075` (p = 0.75, seeds 18–20), verbatim:

```
== profile legacy | devbase vs guided-exponent075 | 3 seeds, 3/3 cells ==
  devbase      median  164.954  mean  164.898  best  164.262  worst  165.478  iters/cell   2779  explore bites  98.7
  guided-exponent075 median  160.698  mean  161.297  best  159.192  worst  164.002  iters/cell   4497  explore bites 125.3
  paired median gain (A-B) +4.256 mm | B wins 3/3 | worst paired +1.476 | invalid publications 0
  per seed: 18:+5.07 19:+1.48 20:+4.26
== profile wall10s | devbase vs guided-exponent075 | 3 seeds, 3/3 cells ==
  devbase      median  159.773  mean  159.630  best  159.256  worst  159.861  iters/cell   1135  explore bites   5.0
  guided-exponent075 median  159.047  mean  159.160  best  154.391  worst  164.043  iters/cell   4036  explore bites   5.0
  paired median gain (A-B) +0.726 mm | B wins 2/3 | worst paired -4.181 | invalid publications 0
  per seed: 18:+4.87 19:-4.18 20:+0.73
```

The quick A/B at p = 1 on seeds 18–20 alone read −0.414 mm (1/3) on Legacy and −0.645 mm
(0/3) on Wall10s; the nine-seed measure above supersedes it.

Work, from the documents (`relocateEconomics.sampleEvaluations`, explore bites, master
iterations; A = devbase, B = treatment):

| profile | treatment | cells | evaluations per explore bite A → B | explore bites per cell | master iterations per cell | evaluations per cell | give-ups A → B |
|---|---|---|---|---|---|---|---|
| Legacy | p = 1 | 9 | 437 633 → 385 328 (−12 %) | 99.8 → 103.9 | 2695 → 2708 | 43.7 M → 40.0 M | 1535 → 1307 |
| Legacy | p = 0.75 | 3 | 444 323 → 279 493 (−37 %) | 98.7 → 125.3 | 2779 → 4497 | 43.8 M → 35.0 M | 588 → 336 |
| Wall10s | p = 1 | 9 | 9 166 933 → 7 761 497 (−15 %) | 4.9 → 4.9 | 1345 → 1904 | 44.8 M → 37.9 M | 737 → 780 |
| Wall10s | p = 0.75 | 3 | 8 884 701 → 6 604 385 (−26 %) | 5.0 → 5.0 | 1135 → 4036 | 44.4 M → 33.0 M | 169 → 104 |

(Give-ups are the no-margin exact-call refusals "outside the 4 µm band or no sheet slack"; with
margin 8, which both arms of the registered screen carry, they are zero in every cell measured
so far.)

## Against the falsifiers registered with the mechanism

Registered before the run (workflow args, verbatim in the commit's provenance): any invalid
publication; the default path not bit-identical; sample evaluations per explore bite not falling
by at least 30 % on Legacy seeds 18–26; a paired-median depth regression on either profile;
and, recorded for the screen, any per-seed repetition-median regression greater than 1.000 mm.

| falsifier | p = 1 (nine seeds) | p = 0.75 (three seeds) |
|---|---|---|
| invalid publication | none (0 / 0) | none (0 / 0) |
| default path bit-identical | ALL_IDENTICAL | same binary |
| evaluations per explore bite −30 % on Legacy | **fails**: −12 % | passes: −37 % |
| paired-median depth regression | none: +0.846 / +0.114 | none: +4.256 / +0.726 |
| per-seed regression > 1.000 mm (single repetition here, not a median) | Legacy 18, 20, 25 (−1.12, −1.12, −4.48); Wall10s 19, 23, 24 (−4.15, −1.43, −5.40) | Wall10s 19 (−4.18) |

So p = 1, Astra's named value, does not deliver the work reduction the mechanism promised on
the live path: the trickle is shorter (the median gain is positive on both profiles) but the
evaluations per bite fall by an eighth, not a third, and single-repetition regressions of 4–5
mm appear on three seeds per profile. p = 0.75 on three seeds delivers the promised work
reduction and a Legacy gain of 4.3 mm with every seed improving, taking Legacy from 164.95 to
160.70, which is where the Wall10s profile sits; on Wall10s it wins two of three with one
4 mm loss. These are single repetitions on consumed seeds: a development finding, not a
result. Review 5b Q9 is explicit that a fractional exponent found this way does not substitute
its winner into the registered 108-cell screen ("Selecting the best of 1, 0.75 and 0.5 on seed
20 would still be fitting to seed 20") and asks for "a separate selection/validation plan"
before further treatment results. The registered screen (control p = 2 against p = 1, margin
8 and the profile caps in both arms, nine seeds, three repetitions, rotated order) runs as
registered; its result is appended below when it lands, and the p = 0.75 finding goes to
Astra as a development finding with the question of how to validate it prospectively.

## The registered development screen: control p = 2 against p = 1, margin 8 and the profile caps in both arms

Registered in review 5b Q7 before any cell ran, run on 2026-09-07 from 08:04 to 08:27 UTC with
`screen/screen.sh` (`frozen-f856d19`, sha256 82c6d5b3ccc9, bit-identical to frozen-9c38526
on the three bitcheck cells): two arms (A `--proxymargin=8`; B `--proxymargin=8
--guidedexponent=1`), both profiles (Legacy cap 50, Wall10s unbounded), seeds 18–26, three
repetitions, arm order A,B on even repetitions and B,A on odd, one fresh eight-worker process
per cell holding the bench lock for the whole screen, ten seconds from the bare request.
Scored with `screen/screen-score.py` against the five gates it can read (positive paired median
of the per-seed repetition medians; no per-seed-median regression greater than 1.000 mm; zero
invalid publications in B; fewer sample evaluations per explore bite; explore bites per cell
not lower). The 108 documents are `screen/exp1-*.json.gz`.

`screen/score-after-rerun.txt`, verbatim:

```
== exp1 legacy: A cells 27, B cells 27
   depth: A median-of-medians 164.006 mean 163.863 | B 160.025 mean 160.386
   paired gain (A-B) median +4.192 mm, B wins 9/9, worst +0.548, per seed: 18:+4.99 19:+5.10 20:+2.17 21:+5.84 22:+0.61 23:+4.19 24:+4.69 25:+0.55 26:+3.15
   evaluations per explore bite: A 459062  B 371865  (-19 %); explore bites per cell: A 103.6  B 121.5; master iterations per cell: A 2107 B 2165
   invalid publications: A 0 B 0; give-ups: A 0 B 0; publications per cell: A 108.3 B 135.7
   [PASS] depth: positive paired median
   [PASS] tail: no per-seed-median regression > 1.000 mm
   [PASS] integrity: zero invalid publications in B
   [PASS] engineering: fewer evaluations per explore bite
   [PASS] engineering: explore bites per cell not lower
== exp1 wall10s: A cells 27, B cells 27
   depth: A median-of-medians 159.867 mean 159.630 | B 158.745 mean 157.230
   paired gain (A-B) median +1.078 mm, B wins 7/9, worst -0.627, per seed: 18:+1.08 19:+2.46 20:+0.31 21:+6.97 22:-0.63 23:+0.43 24:+9.98 25:-0.26 26:+1.26
   evaluations per explore bite: A 9071040  B 7544341  (-17 %); explore bites per cell: A 5.0  B 5.3; master iterations per cell: A 957 B 1483
   invalid publications: A 0 B 0; give-ups: A 0 B 0; publications per cell: A 21.0 B 33.8
   [PASS] depth: positive paired median
   [PASS] tail: no per-seed-median regression > 1.000 mm
   [PASS] integrity: zero invalid publications in B
   [PASS] engineering: fewer evaluations per explore bite
   [PASS] engineering: explore bites per cell not lower
SCREEN PASS (mechanism and halving gates are scored from the traces, not here)
```

**The perturbation and the re-run.** The replay-geometry instrument's implementer compiled in
its own worktree without the bench lock twice while the screen ran (08:05:16–08:05:51 a
library build, 08:21:17–08:21:33 the example). `screen/exp1-perturbation-rule.txt` was
registered before scoring: it names a load-average rule (1-minute average above 10.0 inside a
cell's window), but the sampler only started at 08:07 and from then on no sample exceeded
10.0 (a 16-second build does not move a 1-minute average that far), so the registered rule
flags nothing; the transcript's build windows overlap seven cells (A-legacy-r0 seeds 18–22,
B-legacy-r2 seeds 24–25), none of which fell below 80 % of its arm's median sample
evaluations. Those seven were set aside (`screen/perturbed-originals/`) and re-run at
08:30–08:32 with the same script (it skips existing cells). Both scores are archived: as run
(`screen/score-as-run.txt`: Legacy +4.192 mm, 8/9, worst −0.002 on seed 25; Wall10s
identical to the above) and after the re-run (above: Legacy +4.192 mm, 9/9, worst +0.548).
Every gate passes either way; Wall10s has no re-run cell.

**Per-seed medians (A | B).** Legacy: 18 164.00 | 159.02; 19 165.12 | 159.88; 20 164.00 |
161.83; 21 165.87 | 160.02; 22 158.20 | 156.86; 23 164.01 | 159.81; 24 167.10 | 162.47;
25 164.01 | 164.01 (as run; 163.46 after the re-run); 26 163.32 | 160.17. Wall10s: 18 160.09 |
159.02; 19 159.98 | 157.53; 20 159.06 | 158.74; 21 160.00 | 153.03; 22 159.29 | 159.92;
23 159.87 | 159.44; 24 160.02 | 150.04; 25 159.17 | 159.43; 26 159.19 | 157.92. The Wall10s
seed-24 median of 150.04 is the first ten-second depth of ours at Sparrow's level (150.165
from its own constructor, 149.195 from ours) on any seed.

**What moved, bite by bite (all 108 cells, before the re-run).** Legacy: explore bites per
cell 102.8 → 122.5 (bites per second 11.0 → 13.1), master iterations per explore bite median
8 → 7 and 90th percentile 32 → 28, bites reaching 34 iterations 9.2 % → 6.8 % of bites, the
share of all iterations spent in bites longer than 100 iterations 22 % → 25 %, publications
per cell 107 → 136. Wall10s: explore bites per cell 5.0 → 5.3, iterations per explore bite
median 53 → 47 but 90th percentile 335 → 718 (the failed bites run longer under p = 1: median
335 → 719 iterations, 27 failed bites in each arm), publications per cell 21 → 34. So on
Legacy the gain is more, slightly shorter bites and many more publications, not a shorter
tail; on Wall10s the wins are two basin escapes (seeds 21 and 24) and the losses are within a
millimetre.

**Against the six gates of review 4 Q4 / 5b Q7.** The five the scorer reads pass on both
profiles. The mechanism gate and the halving gate are for the traces: the replay-geometry
instrument (column break from committed geometry, the sweep-24 fork under four exponents,
the extended identity gate, the detached publication check) is built and its readings are
appended below when they land.

**What this screen does not say.** It is a development screen on consumed seeds. It compares
p = 1 with margin 8 against p = 2 with margin 8; the no-margin measure above gave p = 1 only
+0.85 / +0.11, so the margin and the exponent interact (the margin removes the exact-call
churn; the exponent shortens the trickle), and the prospective successor must carry margin 8
in both arms as review 4 prescribed. The p = 0.75 development finding stands beside it, not
in it.

**The trace evidence for the mechanism gate** is in `../bite-microscope/README.md`, section
"The column from committed geometry, the fork at sweep 24, the certification": every treatment
breaks the pinned column at weights three to four orders of magnitude below the control's and
within review 5b's column-break deadlines; the fork at the end of control sweep 24 shows 36 of
64 relocates (all of the entry column's members 2 and 8) with a finalist that beats the stay
pose under p = 1 and 2 under p = 2, at the same poses and weights; the band-entry states
certify through the unchanged publication path. The band-entry deadlines and, at p = 1, the
evaluation halving are missed.

## The selection between p = 1 and p = 0.75 (review 6 Q12; `selection-rule.md`, frozen first)

The p = 0.75 arm (`exp1-C`, 54 cells, same frozen executable, 09:54–10:03 UTC, three cells
re-run by the registered load rule and their originals kept in `screen/contaminated-originals/`)
and the corrected scorer `screen/spec-score.py` (explore-only evaluations and elapsed time per
*published* explore bite with failed bites charged, exact repetition and identity checks, refusal
instead of defaults; it reproduces every figure of Astra's recomputation) give, on both archives
(`screen/selection.txt`, `selection.json`):

| against the margin-8 control | p = 1 (B) Legacy / Wall10s | p = 0.75 (C) Legacy / Wall10s |
|---|---|---|
| paired median gain | +4.192 / +1.078 mm | +3.094 / +0.135 mm |
| worst seed | +0.548 (as run −0.002) / −0.627 | −1.542 (as run −0.818) / **−4.161** |
| explore evaluations per published explore bite | −19.7 % / −19.5 % | −24.9 % / −16.5 % |
| explore elapsed time per published explore bite | −14.8 % / −7.8 % | −12.4 % / **+10.0 %** |
| published explore bites per cell | 102.7 → 120.5 / 4.00 → 4.33 | 102.7 → 117.2 / 4.00 → **3.63** |
| publication fraction | 99.07 → 99.18 % / 80.0 → 81.3 % | 99.07 → 99.15 % / 80.0 → **78.4 %** |
| the four required conditions | PASS on both profiles | FAIL: Wall10s tail and engineering (and the Legacy tail on the replacement archive) |

`H = D_1 − D_0.75` per seed: Legacy median −1.373 mm (min −4.985), Wall10s median −3.935 /
−4.590 mm (min −9.050): p = 0.75 is deeper than p = 1 on most seeds of both profiles, but its
Wall10s losses of 4 mm on seeds 18 and 23 against the control, its longer failed bites and its
lower publication fraction there fail the eligibility conditions. **Decision, identical on the
as-run (primary) and the replacement archive: p = 1 (only p = 1 is eligible).** The
single-repetition, no-margin finding of +4.26 mm at p = 0.75 on three Legacy seeds did not
survive three repetitions with the margin on both profiles, which is what the rule was for.
The prospective specification names p = 1.
