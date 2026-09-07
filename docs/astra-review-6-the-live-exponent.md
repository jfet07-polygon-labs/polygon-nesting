# GPT-6 Astra review 6, verbatim: the live exponent, the screen, the column from geometry

Reply of `gpt-6-astra` (`codex exec`, reasoning effort xhigh, read-only sandbox) to brief 6, 2026-09-07, verbatim. The brief follows the reply.

## Reply

**Choose (a): finish selection on development seeds, then validate one fixed exponent prospectively.** The evidence demonstrates a useful repricing mechanism in the captured jams. It does not confirm the registered fast-escape or halving forecasts, and the existing screen should not be relabelled an all-six-gates pass.

I reviewed `b4b32d0` read-only and recomputed the screen summaries from all 108 archived documents. The reported depth gains and regression guards check out. I ran no new cells.

**Q10. The mechanism is demonstrated locally; the numerical forecast failed.**

The fork establishes a specific causal fact: with geometry, weights and candidate poses held fixed, changing the exponent makes already-discovered finalists preferable to staying. The subsequent treatment trajectories release the entry column at much smaller actual weights and reach certifiable band-entry states. That is sufficient evidence for **quadratic pricing delaying an affordable rearrangement in these exposures**.

It does not establish that all 36 counterfactually preferred finalists would survive a treatment tournament. The fork follows the quadratic trajectory and re-scores its candidates; it does not propagate 36 treatment commits. Nor does one seed’s capsules establish the mechanism’s prevalence across seeds.

The detached certification closes the contract/kernel question for those states. Its scope needs one qualification: the replay incumbent is an infinite-depth placeholder, so its improvement gate is automatically satisfied. It establishes acceptance by the exact authorities at the target, rather than a complete live publication against the original incumbent. [Certification implementation](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/replay.rs:1531)

My previous wording needs an explicit correction:

| Original criterion | Present conclusion | Role I recommend prospectively |
|---|---|---|
| Demonstrated mechanism | Supported for repricing and release in the captured exposures | Required development prerequisite |
| Seed-balanced halving | Not established; bite-15 p = 1 target failed | Required for a **halving claim**, not promotion |
| Engineering benefit | Supported by the additional accounting below | Required |
| Positive paired-median depth gain, both profiles | Passes development screen | Required on validation seeds |
| Per-seed regression ≤ 1.000 mm | Passes development screen | Required on validation seeds |
| Integrity | Recorded checks support it; protocol qualifications remain | Required |

Q7 called all six “substantive.” I cannot retrospectively reinterpret that as an unconditional pass. **Making halving a forecast criterion rather than a promotion requirement is a prospective policy change.** Preserve the old failed forecast, including the separate −30% development work target.

There is also an accounting correction. The scorer divides **whole-cell evaluations, including compression**, by explore-bite count. It does not measure explore-only evaluations per bite. [Scorer](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/guided-exponent/screen/screen-score.py:24)

Using the archived, post-rerun documents:

| Measure, control → p = 1 | Legacy | Wall10s |
|---|---:|---:|
| Total evaluations per cell | 47.57M → 45.17M | 45.36M → 40.24M |
| Explore evaluations per published explore bite, charging failed bites | 373,874 → 300,100 | 9,123,571 → 7,342,272 |
| Published explore bites per cell | 102.67 → 120.48 | 4.00 → 4.33 |
| Explore publication fraction | 99.07% → 99.18% | 80.00% → 81.25% |
| Explore elapsed time per published explore bite | 73.10 → 62.30 ms | 1.878 → 1.732 s |

These support an operational efficiency benefit. They measure throughput along different trajectories, not the cost of solving identical bites.

**Q11. Register a single selected candidate and explicit promotion requirements.**

I would write the operative rule as follows:

> `ICS-guided-exponent-v1` compares control p = 2 with the single exponent selected by the development rule below. Both arms use margin 8, unchanged profile caps and the exact 5.0/5.0 contract. Seeds 27–35, both profiles, three repetitions: 108 new cells. Each cell starts a fresh eight-worker process from the bare request with the existing ten-second accounting.
>
> Freeze the executable, configuration, scoring implementation and complete execution manifest before the first cell. Run adjacent control/treatment pairs, rotating their order by seed and repetition. Complete the registered population without efficacy-based early stopping.

For profile \(h\), seed \(s\), define

\[
D_{a,h,s}=\operatorname{median}_{r=0,1,2}(\text{final valid depth}),\qquad
g_{h,s}=D_{2,h,s}-D_{p,h,s}.
\]

Positive gain means improvement. Require, **separately on both profiles**:

1. **Depth:** \(\operatorname{median}_s g_{h,s}>0\).
2. **Tail:** \(\min_s g_{h,s}\ge -1.000\) mm, using unrounded values.
3. **Engineering:** lower total evaluations per cell; lower exploration evaluations and exploration elapsed time per published explore bite; no reduction in published explore bites per cell or their publication fraction. Charge unsuccessful and deadline-truncated bites in the numerators and opportunity counts.
4. **Integrity:** complete, correctly identified cells; zero invalid publications; required default/replay identity checks; no unregistered configuration or timing changes.

The development mechanism prerequisite must also hold for the selected exponent. Halving and the old iteration deadlines remain separately reported forecast outcomes.

**Promote only if every required condition passes on both profiles.** Otherwise record which condition failed. Do not change p, caps, margin, seeds or the regression guard after inspecting validation results. A selected p = 0.75 failing validation does not authorize trying p = 1 on those same seeds as another unseen confirmation.

For the frozen margin-zero comparison, report both comparisons, with separate claims:

- **Treatment versus contemporaneous margin-8 control:** the effect of changing p conditional on margin 8.
- **Treatment versus archived frozen margin-zero cells:** the observed performance of the combined margin/exponent package relative to the frozen record.

The historical comparison cannot isolate the exponent or separate package effects from measurement-period effects. Preserve all 27 frozen cells per profile and identify their hashes before starting. Their outcomes are already known; the holdout is untouched by these treatments, not entirely unseen.

For a claim that the **package improves the frozen record**, require positive paired-median gains and the same 1.000 mm tail guard against that archived baseline too. Failure there need not erase a conditional exponent improvement, but it prevents the stronger package claim.

For Sparrow, preregister:

> This round tests whether the selected ICS successor improves ICS and reduces its observed distance from the archived Sparrow reference of 150.165 mm. It does not test population-level superiority over Sparrow.

Report the Wall10s median’s residual gap, its reduction from the frozen baseline, and the count of seed medians at or below 150.165. The frozen validation reference has a **9.550 mm** gap; the current development treatment median has an **8.580 mm** gap. Individual reference-level outcomes are useful milestones. They do not establish typical parity.

**Q12. Choose (a), with a direct comparison between the candidate exponents.**

The p = 0.75 evidence warrants selection work. It is not stronger on every reading: bite-15 band entry is 40 versus 32 for p = 1, and margin-zero bite-17 band entry is 30 versus 19. Its three-capsule evaluation result also comes from one seed, not a seed-balanced population.

Freeze this rule before the 54 additional development cells:

- An exponent is **eligible** only if it passes the declared development depth, tail, integrity and engineering requirements on both profiles.
- If exactly one exponent is eligible, select it.
- If neither is eligible, do not proceed to validation.
- If both are eligible, define the direct paired difference
  \[
  H_{h,s}=D_{1,h,s}-D_{0.75,h,s}.
  \]
  Select **p = 0.75 only if all three hold**:
  - Legacy median \(H>1.000\) mm;
  - Wall10s median \(H\ge0\);
  - no seed on either profile has \(H<-1.000\) mm.
- Otherwise select p = 1.

This compares p = 0.75 directly with p = 1. Subtracting their separate median gains against control is not equivalent. The 1.000 mm selection hurdle is a declared practical preference, not a statistical significance threshold. Select one p for both profiles; keep p = 0.5 outside this selection.

Sharing the existing controls is acceptable for **development selection**, with two conditions:

First, the 54-cell option must use the same frozen live executable and configuration as the existing arms. The screen used `f856d19`; the pinned-power change came later. Do not silently combine newly compiled treatment cells with old controls. Freeze and verify the final prospective executable separately; default p = 2 identity alone does not establish treatment identity or equal wall overhead.

Second, explicitly acknowledge that the added arm is measured later. Shared historical controls do not recreate a contemporaneous three-arm experiment.

The seven replacements were also a deviation from the stated rule: that rule specified load-average exceedance, which flagged no cells; replacement instead used build overlap. Both analyses agreeing is reassuring, but does not remove the deviation. [Perturbation record](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/guided-exponent/screen/exp1-perturbation-rule.txt:1)

For selection, I would name the original as-run archive as primary and require the selection to agree using the replacement archive. If they disagree, retain p = 1 provided it remains eligible under both; otherwise select neither.

**Q13. Freeze the implementation, accounting and observation rules—not additional rescue knobs.**

The specification should fix:

- **Arithmetic:** executable hash, compiler/options, target and math-library environment; the multiplication path at p = 2 and the shared power path otherwise. Verify the selected treatment’s arithmetic and replay identity as well as the default. `inline(never)` is an implementation measure, not a portability guarantee.
- **Objective semantics:** exact zero, residual units, row order, comparison/equality policy, every worker’s objective, pre-update tournament ordering, weight cap, and the unchanged raw/band/strike definitions. No smoothing, normalization, boundary-specific p or adaptive exponent.
- **Timing and administration:** constructor charging, phase allocation, deadline handling, output overhead, process isolation and a manifest of paired cells. Define contamination from independently logged events before reading scores; preserve originals and rerun the whole affected pair under a fixed rule. Do not exclude cells because their work count looks low.
- **Scoring:** require exactly three uniquely identified repetitions per seed and arm; verify request, contract, executable and knobs; support the actual archive format; reject missing or nonfinite required fields. The current scorer can skip unreadable files and defaults missing invalid-publication counts to zero. Those defaults are unsuitable for validation.

Two implementation details deserve correction before the freeze:

- Replay metadata still hardcodes `capturedExponent: 2.0`; it must describe the source capsule’s exponent when treatment captures are used. [Benchmark metadata](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/examples/overlap_ics_benchmark.rs:4554)
- The live comparison still maps an unordered floating-point comparison to equality. Rejecting nonfinite p does not establish finite scores. Prove the upstream finite-score invariant or reject nonfinite scores before comparison; do not allow NaN to become an accepted equality. [Comparison](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/relocate.rs:221)

Record the observed hazards as diagnostics, with denominators:

| Diagnostic | Record |
|---|---|
| Column churn | Releases, reformations, connected sweeps and relocate opportunities |
| Conflict concentration | Blocking-row count, raw Phi, maximum residual, and weighted improvements that increase raw loss |
| Distant moves | Displacement/rotation and fresh contacts, separately for all-worker expenditure and retained master commits |
| Cleanup | Work from release to band, exact-check outcome, retries and subsequent bites |
| Failed bites | Count, elapsed time, evaluations, iterations and actual stop reason |
| Numerical behaviour | Equal comparisons/accepted equal moves, tiny positive residuals, capped rows and time at cap |

Keep expensive geometry diagnostics outside scored cells. Declare their sampling rule before validation—for example, repetition 0 for every seed/profile, retaining the first control explore bite reaching 34 iterations and reporting absence without substitution. Diagnostic results must not alter the candidate or promotion thresholds.

There are also small factual corrections: the fork archive shows p = 1 winning finalists for piece **6 in 2/8 workers**, and piece **45 in 8/8**. The total 36 is correct. And the WHY comment’s “p < 2 is concave” is false: p = 1 is linear; concavity applies below 1.

**Q14. Use the common-entry column for the primary escape claim; retain longest-lived as a secondary trajectory description.**

For a common-entry replay, freeze the row identities belonging to bottom-to-top paths in that entry state. Follow those same rows under every exponent. Specify the graph’s positive-residual threshold, observation cadence, path-enumeration limit and disconnection/reformation rule.

If no column exists at entry, report **entry exposure absent**. The current “entry” analysis falls back to the first later formation; label that separately because formation now depends on the treatment trajectory. [Column definitions](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/replay.rs:690)

Keep longest-lived, with its deterministic earliest-tie rule, as a secondary measure of each trajectory’s most persistent spanning structure. Different exponents can select different row sets, so their longest-lived break times are not automatically matched escapes.

This changes the historical forecast verdict:

- Entry-column reading: control **29**, p = 1 **16**.
- Longest-lived reading: control **37**, p = 1 **26**.

You cannot use the second reading to reproduce 37 and the first to pass the treatment deadline of 20. Under the reading that reproduces 37, p = 1 misses that deadline. The geometry correction should remain explicit.

Finally, **“column avoided” is changed trajectory formation**, exactly as proposed. On the margin-zero capsules, report that no bottom-to-top column appeared at the observed committed sweep boundaries before band entry. Do not assign escape time zero, or include avoidance as a successful escape in a halving statistic. A run ending before band entry provides only horizon-limited absence or censored release evidence.

## Brief 6, as sent

## Brief 6: the exponent on the live path, the registered screen, the column from geometry

Read after your review 5b. Same setting (ICS research engine on `engine/topology-archive-search`,
worktree `/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a`, ten seconds on mixed-61 with
the exact 5.0/5.0 contract, Sparrow at 150.165 in the same ten seconds). Everything below is
committed; the files to read are named at the end. Seeds 27-35 remain untouched by any
treatment; the frozen engine's own cells on them exist (27 per profile, no margin, p = 2:
Legacy median-of-medians 164.935, Wall10s 159.715).

### 1. The knob (commits 6d55efb, f856d19; pinned power b497b6f)

`--guidedexponent=<p>`, single interface as you asked in Q9: `energy::fold` and
`energy::incident_totals` are now the `_with_exponent` variants at a process-level knob
(`0 < p <= 2`, default 2), `p = 2` instantiated as `w * (v * v)` so the default is the frozen
engine to the bit (bitcheck ALL_IDENTICAL against two frozen binaries). Raw Phi, max violation,
the band, the strike minimum, `gls_update`, the pool, the disruption, `publish.rs`, the kernel
and the validator are untouched; `eval_cmp`'s "clear beats colliding" stays on raw. The corpus
cell refuses `p != 2` (its gradient probe walks the gradient of `sum w v^2`). The adversarial
verifier (forbidden-rescue table, row by row) refuted nothing. Your Q7 item 3: every `v^p`
now goes through one `#[inline(never)] guided_power`, because LLVM rewrites `powf(x, 0.5)`
with a constant exponent into a correctly rounded sqrt while a run-time exponent calls libm's
pow, and the fork's readings at the constant `{2, 1, 0.75, 0.5}` differed by one ULP from the
same rows folded at a run-time `p`. The exponent is recorded in the document (`guidedExponent`,
present only when `!= 2`) and the capsule path refuses a replay whose `p` disagrees with the
document's.

### 2. Development measure without the margin (one repetition, `docs/experiments/overlap-ics/guided-exponent/README.md`)

Both arms without the margin (the frozen `devbase` cells have none; one factor at a time):

| profile | treatment | seeds | paired median gain | wins | evaluations per explore bite | give-ups A -> B |
|---|---|---|---|---|---|---|
| Legacy | p = 1 | 18-26 | +0.846 mm | 6/9 | -12 % | 1535 -> 1307 |
| Wall10s | p = 1 | 18-26 | +0.114 mm | 5/9 | -15 % | 737 -> 780 |
| Legacy | p = 0.75 | 18-20 | +4.256 mm | 3/3 | -37 % | 588 -> 336 |
| Wall10s | p = 0.75 | 18-20 | +0.726 mm | 2/3 | -26 % | 169 -> 104 |

p = 1 without the margin misses the registered work falsifier (-30 % evaluations per explore
bite) and shows single-repetition losses of 4-5 mm on three seeds per profile (Legacy 25:
-4.48; Wall10s 19: -4.15, 24: -5.40). p = 0.75 on three seeds takes Legacy from 164.95 to
160.70 with every seed improving. I record it as a development finding, per your Q9, and did
not substitute it into the screen.

### 3. The registered 108-cell screen (`docs/experiments/overlap-ics/guided-exponent/README.md`, `screen/`)

Exactly as registered in your Q7: control `--proxymargin=8` (Legacy cap 50 / Wall10s
unbounded), treatment `--proxymargin=8 --guidedexponent=1`, seeds 18-26, three repetitions,
arm order rotated per repetition, one fresh eight-worker process per cell under the bench
lock, ten seconds from the bare request, per-seed repetition medians, frozen binary. Scored
with `screen-score.py` (in `screen/`) against the five gates it can read:

| profile | paired median gain (per-seed medians) | wins | worst seed | evaluations per explore bite | explore bites per cell | invalid | give-ups |
|---|---|---|---|---|---|---|---|
| Legacy | **+4.192 mm** (164.006 -> 160.025) | 9/9 | +0.548 | 459 062 -> 371 865 (-19 %) | 103.6 -> 121.5 | 0 / 0 | 0 / 0 |
| Wall10s | **+1.078 mm** (159.867 -> 158.745) | 7/9 | -0.627 (seed 22) | 9 071 040 -> 7 544 341 (-17 %) | 5.0 -> 5.3 | 0 / 0 | 0 / 0 |

All five pass on both profiles: positive paired median, no per-seed-median regression greater
than 1.000 mm, zero invalid publications, fewer evaluations per explore bite, bites per cell
not lower. Wall10s seed 24's median is 150.04 (seed 21: 153.03), the first ten-second depth
of ours at Sparrow's level on any seed; its losses are -0.63 and -0.26. Bite anatomy on
Legacy: bites per second 11.0 -> 13.1, iterations per explore bite median 8 -> 7 and 90th
percentile 32 -> 28, bites reaching 34 iterations 9.2 % -> 6.8 %, publications per cell
107 -> 136; on Wall10s the failed bites run longer under p = 1 (median 335 -> 719 iterations,
27 failed bites in each arm) and the wins are two basin escapes.

Integrity note, recorded before scoring (`screen/exp1-perturbation-rule.txt`): the
instrument implementer compiled twice without the bench lock while the screen ran; seven
cells overlapped those builds (none fell below 80 % of its arm's median work). They were set
aside and re-run; both scores are archived and both pass (as run: Legacy +4.192, 8/9, worst
-0.002 on seed 25).

Margin and exponent interact: p = 1 gives +0.85 without the margin and +4.19 with it on the
same seeds and profile. The margin removes the exact-call churn; the exponent shortens the
trickle; each alone was worth about a millimetre.

### 4. The column from committed geometry, the fork, the certification (`docs/experiments/overlap-ics/bite-microscope/README.md`, last section; `replay.rs`)

Your Q6-Q8 instruments are built: committed relocates per iteration, the column's rows with
their actual GLS weights, core poses, the extended identity gate (poses / weights / stream
fingerprints, relocates and evaluation counts against the trace: 43/43 at p = 2 and at
`none`), `resolvedSeed`, `--fork=<sweep>`, `--certify=1`, and `column-break.py`.

Two readings of the column, both from the blocking graph (pieces and the four strip edges;
rows with violation > 0). *Entry graph:* the only bottom-to-top path in the capsule's own rows
is edge(2,B)-pair(2,8)-pair(8,45)-edge(45,T); the 0-6-43/44 chain I described is not
bottom-to-top at entry and connects at iteration 1. *Longest-lived:* the union of both chains
(8 rows, members 0, 2, 6, 8, 43, 45), formed at 6, resident 31 iterations, breaks at **37** at
p = 2, your registered "reproduce 37".

Bite 15, margin 8, 50 iterations maximum, against your table:

| p | column break, entry graph (deadline) | longest-lived | band entry (deadline) | evaluations (target 342 990) | max column-row weight at the break | temporary releases |
|---|---|---|---|---|---|---|
| 2 | 29 | 37 (37) | 43 (43) | 685 980 | 1.6e5 / 2.5e5 | 2 |
| 1 | 16 (20) | 26 | 32 (27) | 462 936 (misses) | 113 / 576 | 9 |
| 0.75 | 12 (16) | 12 | 40 (23) | 288 357 (meets) | 12.4 | 12 |
| 0.5 | 11 (12) | 11 | 49 (19) | 230 022 (meets) | 11.8 | 10 |

Every treatment meets its column-break deadline and breaks the column at weights three to four
orders below the control's; none meets its band-entry deadline; the halving holds at
p <= 0.75, not at p = 1. In your reading rules: "earlier release at much smaller actual
old-row weights" without "band entry within the table"; at p = 1 the escape comes 13 iterations
earlier and the band 11 earlier (evaluations -33 %), at p = 0.75 the escape 17 earlier and the
band 3 earlier (evaluations -58 %, the post-break iterations are cheap). The treatments release
and re-form column rows four to six times more often than the control.

The fork at the end of control sweep 24 (identity 25/25 to the fork; column-row weights
1.9e4-3.9e4, twelve blocking rows, max 31 um): in sweep 25 the eight workers ran 64 relocates
and a candidate beats the stay pose in **2** relocates under p = 2, **36** under p = 1 (all
finalists), **63** under p = 0.75, **64** under p = 0.5, at identical poses and weights. For the
entry column's members 2 and 8 the crossover is complete in every worker (and in most for 45);
0 and 6 stay under p = 1 everywhere; 43 and 44 leave in some workers. The arithmetic: piece 2's
stay pose scores 0.3-0.5 under `sum w v^2` (weights ~2e4 on 3-5 um residuals) and 108-135 under
`sum w v`; the finalist that beats it carries a fresh overlap of raw 750-1200 mm^2 (10-30 mm
of penetration, 700-1000 mm away, weight 1), priced 750-1200 under `v^2` and 27-35 under `v`.

`--certify=1` runs the unchanged path (`Engine::attempt_publication` -> `publish::attempt` ->
`validate_placements_against_contract`) on the band-entry state and installs nothing: bite 15
publishes at p = 1 (180.0066, target 180.0146, contract and kernel valid, zero repair rows,
one exact call) and at p = 0.75 / 0.5 (180.0056). Bite 17 (margin 8): breaks at 25 (1.4e5) /
19 (104) / 8 (6.2) / 12 (11), band 26 / 30 / 13 / 18. Margin-0 bite 17: p = 2 forms a
three-path column at 4 and breaks it at 21 (2.6e4), band 37; p = 1 breaks at 17 (121), band
19; p = 0.75 and p = 0.5 never form a bottom-to-top column ("column avoided"), band 30 / 45.

### Questions

**Q10. The gates.** Five of your six pass on both profiles in the registered screen. On the
mechanism gate: is the fork's price crossover (2 -> 36 of 64 at identical poses and weights,
complete for the entry column's members) together with the break at weight 113 against 1.6e5
the "demonstrated mechanism" you asked for, given that the band-entry deadlines were missed
and the cleanup consumed part of the saved work? On the halving gate: p = 1 does not halve
anything (-33 % on bite 15, -19 % on bite 17, -57 % on the margin-0 bite; -19 % per explore
bite in the screen). Does the screen stand as a pass with the halving gate failed, i.e. which
gates are necessary for promotion and which are the falsifiers of the *forecast* only?

**Q11. Promotion.** I propose the prospective successor `ICS-guided-exponent-v1` on the
virgin seeds 27-35: control `p = 2, margin 8`, treatment `p = 1, margin 8`, profile caps,
three repetitions, rotated order, one process at a time, the same six gates, the per-seed
regression guard of 1.000 mm, the specification committed before the first cell and only
appended. Write or correct its falsifiers and its decision rule. Two sub-questions: (a) the
frozen engine's own cells on 27-35 (p = 2, margin 0) exist; should the specification report
the treatment against both (margin-8 control for the exponent's effect, frozen control for
the absolute record) and what claim does each support? (b) what is the pre-registered
statement about Sparrow: Wall10s at p = 1 is at 158.7 median on the dev seeds against
Sparrow's 150.2, so "beats Sparrow" is not on the table for this round; what is?

**Q12. p = 0.75.** The development finding (three seeds, one repetition, no margin: +4.26 on
Legacy 3/3, -37 % evaluations; traces: halving met on all three capsules, column avoided on
the margin-0 bite) is stronger than p = 1's on every reading we have. Your Q9 asks for "a
separate selection/validation plan" before further treatment results. Which of these: (a) a
second registered dev screen, 54 new cells (p = 0.75 against the same margin-8 control, the
control cells shared), then one prospective specification that names a single p chosen by a
rule declared before the screen; (b) go prospective with p = 1 now and treat p = 0.75 as a
later successor v2 with its own dev screen; (c) one prospective specification with three arms
(2, 1, 0.75) and a predeclared decision rule, 162 cells. I lean to (a): the evidence base
would then match p = 1's before any virgin seed is spent, and a rule such as "the lower p if
it passes the same five gates and its Legacy paired median exceeds p = 1's by more than the
tail guard" can be written now. Say which and write the rule.

**Q13. Hazards seen.** (i) Temporary releases 9-12 against 2: the sublinear objective churns
the column's rows; (ii) the accepted finalists sit 700-1000 mm away with 10-30 mm of
penetration, i.e. the concentration hazard is the escape; (iii) Wall10s failed bites run
twice as long at p = 1; (iv) the compiler's sqrt specialisation. What else must the
specification freeze, and should any of these become a recorded diagnostic on the virgin
seeds (counts, not gates)?

**Q14. The two columns.** The entry-graph reading finds 2-8-45; the longest-lived finds the
union with 0-6-43 and reproduces your 37. Which reading should the prospective specification
name for any trace-level claim, and is "column avoided" on the margin-0 bite (p <= 0.75) the
"changed trajectory formation" you described, to be reported as such and not as an escape?

### Files

- `docs/experiments/overlap-ics/guided-exponent/README.md` (+ `screen/` with the 108 documents,
  the seven originals, `screen.sh`, `screen-score.py`, the perturbation record, both scores)
- `docs/experiments/overlap-ics/bite-microscope/README.md`, last section, and `geometry/`
  (documents, `demo-geometry-all.sh`, `column-break-output.txt`, `demo-log.txt`),
  `column-break.py`
- `crates/polygon-nesting-core/src/search/overlap_ics/energy.rs` (`fold_with_exponent`,
  `incident_totals_with_exponent`, `guided_term_with_exponent`, `guided_power`),
  `mod.rs` (`set_guided_exponent`, the WHY comment), `replay.rs` (`ColumnReport`,
  `ForkRelocate`, `certify_band_entry`), `examples/overlap_ics_benchmark.rs`
  (`--guidedexponent`, the replay arm)
- `docs/astra-review-5b-the-exponent.md` (your review 5b, verbatim)
