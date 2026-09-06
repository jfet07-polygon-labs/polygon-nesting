# GPT-6 Astra review 4: same start, two separators

Consulted 2026-09-06 at `model_reasoning_effort = xhigh` on `9f46bce`, on the owner's instruction
("ignora grok, usa codex astra xhigh"). The brief is reproduced after the reply. Verbatim;
nothing edited. The mirror experiment (section E of the brief) completed after the brief was
sent; its result is in `docs/experiments/overlap-ics/sparrow-warm-start/README.md`, addendum 3.

## Reply

**Keep margin 8 and Legacy cap 50. The next experiment should distinguish unfinished local refinement from conflict transfer during the hard bite.** A changes the diagnosis substantially, but it does not yet distinguish those two mechanisms.

I reviewed `bcee244` and the available records read-only. The failed margin preflight remains unchanged; nothing here authorises spending seeds 27–35.

**Q1. The constructor drops out of the leading explanations; CD resolution and conflict transfer remain my first two tests.**

The common start establishes that our constructor supports a much better trajectory. Equal ordinary-bite iteration counts concentrate attention on the tail. They do **not** establish equal wall cost per iteration, and after the first bite the two engines are operating on different layouts—even at matching widths.

My revised order is:

1. **Fine CD exits with useful local descent remaining.**
2. **Relocates transfer blocking contacts to endpoints whose turns have passed or were never scheduled.**
3. **The weighted objective favours moves that resolve today’s conflicts while creating a less mobile arrangement.**
4. **Cap, pool and disruption amplify an unresolved separation.**
5. **Publication mismatch under margin 8**, now substantially demoted.

Ranks 1 and 2 remain a test order, not an established causal ordering. A strengthens their relevance to the hard-bite tail without choosing between them.

B and F are compatible. A disrupted bite can finish in a configuration whose next three small squeezes are easy, yet whose eventual attainable depth is worse. Conversely, large centroid displacement alone does not establish damage: useful rearrangement also moves pieces, and exchanging identical copies can inflate displacement without changing packing geometry. **F plus the warm-start results makes disruption-mediated basin degradation plausible; it does not isolate disruption from the other changes induced by the margin.**

For your five specific candidates:

| Candidate | What could persist into later bites | One discriminating measurement |
|---|---|---|
| **Container-origin commits** | A distant relocation clears a contact but changes neighbours, orientation freedom or which pieces cross the next cut. | On the **retained master trajectory**, attribute subsequent blocking-row births and persistence to each commit’s origin and actual displacement. Compare container, focused and stay-origin moves. |
| **Follower carry** | Carried pieces establish new external contacts even when the completed bite and its immediate successors are cheap. | Across each disruption, identify follower-created rows and measure which survive into the published parent and reactivate after subsequent cuts. Record followers separately from the swapped hosts. |
| **16 µm repair** | Tiny installed changes can close a clearance or change a contact’s classification. | Compare incident rows immediately before repair and after installation **at the same target**, then identify which changed rows block the next bite. Count installed repairs only. |
| **GLS-driven movement beyond the squeezed pieces** | A newly activated endpoint moves to improve weighted loss while spreading raw conflict elsewhere. | For pieces outside the initial conflict set, measure committed weighted-loss reductions accompanied by raw-loss increases or deferred row births; replay the implicated sweep with unit weights at equal work. |
| **Cap-50 truncation** | The subsequent retry can reach a different, fully legal but less mobile parent. | From an identical cap-boundary state, compare ordinary retry against bounded continuation, then measure the next identical cut from each successfully published parent at equal work. |

Three code distinctions constrain that interpretation.

- **The 9,735 container commits include discarded workers.** I verified that total in the dev files; the tournament sums every worker’s work before selecting its winner. It is an expenditure count, not 9,735 rearrangements inherited by the layout. The microscope must follow selected workers and subsequent rollbacks. [Tournament accounting](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1015)
- **GLS does not move a currently clear piece merely because it has large weights.** `relocate` skips nonpositive incident raw loss. Also, “not translated by the cut” does not mean unaffected: the stationary endpoint of a squeezed pair is directly affected. Record both cut membership and initial conflict membership. [Relocate guard](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/relocate.rs:704)
- **A capped unresolved state cannot become the next bite’s parent.** Publication installs repaired, dual-valid poses and resets weights. What can persist is tight legal geometry, including contacts that violate the inflated proxy clearance—not an unpublished half-resolved child or inherited GLS weights. [Parent installation](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:2090)

Repair is consequently a low-ranked explanation for F’s displacement: roughly 100 installed publications allow at most roughly 1.6 mm of cumulative repair displacement per piece under the cap. It could trigger later motion; it cannot directly supply hundreds of millimetres.

**Q2. Retain cap 50 as the existing Legacy control. Any cap change is a separate prospective schedule treatment.**

D supports keeping the fuse. It does not establish that 50 is optimal, or provide a predictor of which capped states deserve more time.

There is a useful concrete example in the records: margin-8 seed 20’s bite 21 finishes in **80 iterations** under cap 200 or unbounded, versus **119 with two disruptions** under cap 50. Continuation sometimes avoids expensive retries. Seed 26 shows why extending every attempt is a poor policy.

Accordingly:

- **Legacy:** margin 8, cap 50 in both separator arms.
- **Wall10s:** retain its existing unbounded cap in both arms. D tested Legacy’s 0.1% bite; it does not justify adding cap 50 to the 3.2% profile.
- **Cap 200, unbounded Legacy, or adaptive extension:** each needs an explicitly named prospective schedule treatment. None should enter the separator control as an unscored “precondition.”

Margin 8 removes a demonstrated certification confound. Increasing the cap reallocates work between continuation and restarting, with demonstrated large regressions. Those are different reasons for changing an experiment.

**E’s raw outputs arrived during this review.** The files report:

| Sparrow start, recomputed depth | Our explore best | Our final incumbent |
|---|---:|---:|
| 164.36118 mm | 159.80803 | **159.77808** |
| 150.09032 mm | 150.09032 | **150.00506** |

These are seed-20 Legacy, cap-50 runs **without the margin**, reporting dual-valid starts and zero invalid publications. The new `--start` implementation’s verification remains provisional from my perspective. [164 mm run](/var/lib/t3/tmp/astra/warm/ours-from-sparrow-2s-legacy-s20.json), [150 mm run](/var/lib/t3/tmp/astra/warm/ours-from-sparrow-final-legacy-s20.json)

The second run retains and slightly improves the deep incumbent. Its first explore bite spends **447 iterations, with zero band entries**, before exploration ends. That particular failure precedes publication entirely. E further demotes “our authority cannot hold Sparrow’s depth”; it does not establish margin-on behaviour.

**Q3. Yes: a reduced trace plus two detached probes can distinguish ranks 1 and 2 on the observed bite. Five aggregate columns alone cannot.**

Use **seed 20, Legacy, eight workers, margin 8, cap 50**, with other mechanisms unchanged. Define the first hard bite prospectively as the first explore bite reaching **34 master iterations**, preserving the previous `>33` threshold.

Buffer from every bite’s beginning. Discard ordinary bites; retain the first qualifying bite’s complete history and the **next three explore bites**, including failures. Emit after timing. If fewer exist, report that explicitly.

This is the reduced field list:

| Record | Fields |
|---|---|
| **Existing cell header** | Reuse binary/request identities, seed, profile, workers and resolved knobs; add trigger and truncation/absence status. |
| **Bite entry** | Bite ID, parent/target depth, cut-moved mask, initial positive-incident-piece mask, replay-state reference. |
| **Selected sweep** | Attempt and iteration IDs, winner ordinal, initial piece order, end-of-sweep blocking row IDs/residuals, total evaluations charged across all workers. |
| **Committed relocate** | Piece/order position, sample origin, `moved`, centroid displacement vector and angle change, incident raw/guided loss before and after. Include ran-but-unmoved relocates. |
| **Incident row changes** | Stable pair-or-boundary row ID, before/after violation, other endpoint’s status: `later / visited / absent / boundary`. Include changed persistent rows, not just births and deaths. |
| **Fine-CD exit** | `limits / iteration-cap`, exit incident maximum and raw loss, final translation/angular steps, candidate-pair count. |
| **Control transition** | Actual separation stop; restore/retry state reference; swapped/follower IDs for disruption; publication reference and installed repair pose deltas. |

Keep one compact replay capsule containing poses, weights, target and stream/descent state at each retained bite entry and non-reconstructible reset. It is replay payload, not another statistical census. A deterministic replay can recover the selected CD call’s axis and stream position without logging every trial.

The indispensable details are **row identity, endpoint scheduling status and replayability**. Without them, “cleared 2, created 2” cannot distinguish repeatedly transferring a seam from repeatedly failing to clear it.

Implementation rules:

- Buffer private worker traces, retain the selected worker after merging, and account for later restores. Loser work still belongs in the cost denominator.
- Compare relocate entry with its final committed rebuild. Scratch candidate installations are not conflict births.
- Origin is ancestry, not movement distance: stay-origin CD can move; container-origin does not prove a distant final move.
- Record actual stop reasons. The existing `attempts` field counts failed separations; it is not the total number of separation calls.

After the timed cell, perform cold/cache agreement and two probes:

1. **Fine-CD continuation:** replay nonzero limit exits with the proposed finer termination, then propagate through the sweep. Does it clear the persistent blockers cheaply and bring the whole state into the band?
2. **Deferred-endpoint revisit:** replay implicated sweeps with a bounded revisit queue, leaving ordinary CD unchanged. Does it remove blockers transferred to unavailable endpoints at equal evaluation work?

If continuation resolves the same persistent rows, rank 1 gains support. If deferred births dominate and revisits resolve them, rank 2 gains support. If both help, measure their effects separately; they may be two stages of the same failure. If neither helps, promote the objective/landscape hypothesis.

This first cut identifies mechanisms **in its captured exposure**. It cannot establish their nine-seed prevalence or prove the cause of F’s final basin difference.

**Q4. Keep `--cdfinish`; amend its evaluation toward the tail and add a regression guard.**

The mechanism remains conditional on successful detached continuation:

- Default off and bit-identical off.
- After ordinary fine CD exits on its limits with nonzero incident loss, continue from its **accepted pose and saved walk state**.
- Translation limits become `min(existing, band/4)`: currently **1 µm**.
- Angular resolution gives at most 1 µm displacement at the farthest source vertex from the centroid.
- Spend at most **64 additional ± candidate pairs**.
- Stop on incident zero, the finer limits, or the additional budget.
- Preserve weighted comparisons and equality acceptance; charge every evaluation.

The CD floor remains a termination scale, not a quantisation grid. Its size motivates the probe; only the probe establishes missed improvement.

My falsifiers are:

1. **Mechanism:** continuation rarely clears the implicated persistent rows or improves whole-sweep band conversion. Then do not build the live knob.
2. **Halving claim:** retain the previously stated 50% requirement on a prespecified, seed-balanced common-entry population, including failed opportunities. A smaller improvement does not substantiate that claim.
3. **Engineering benefit:** fewer master iterations without lower total evaluation work and measured wall at maintained completion rate fails the efficiency claim. Include ordinary bites that pay continuation overhead.
4. **Depth:** on consumed seeds 18–26, require positive paired-median gains on both profiles before proposing an unseen-seed round.
5. **Tail protection:** for the proposed three-repetition development screen, add **no per-seed-median regression greater than 1.000 mm** on either profile. D shows why positive paired medians alone are inadequate.
6. **Integrity:** any invalid publication or default-off trajectory mismatch stops the treatment.

The added tail guard is prospective for this new treatment. It changes no previous verdict. Keep each profile’s cap fixed throughout this comparison.

**Q5. The next 24 hours should produce one causal result and, conditionally, one isolated treatment.**

1. **Hours 0–6: build the reduced microscope.** Verify diagnostic off/on fixed-work identity and reconstruction. Finish archiving and verifying `--start`; add the two margin-8 mirror diagnostics when ready.
2. **Hours 6–10: capture and replay.** Run the prescribed seed-20 margin-8 trace. A separate margin-zero trace can expose the historical 37-iteration bite without treating the two diverged trajectories as matched inputs. Run the two detached probes.
3. **Hours 10–14: test the chosen explanation against seed 26.** Predeclare this follow-up because D already identifies it as the fuse-dependent failure. Report missing exposure rather than replacing the seed.
4. **Hours 14–20: implement only the supported mechanism.** If continuation works, build `--cdfinish` and complete identity/soundness checks. If it fails, pursue the supported alternative; do not bundle CD, scheduling and GLS.
5. **Hours 20–24: run the consumed-seed development screen if ready.** Two arms × two profiles × nine seeds × three repetitions = **108 ten-second cells**. Rotate arm order, use per-seed repetition medians, and report work, completed bites, depth and regressions. If the mechanism gate failed, spend this time explaining the failed probes.

Run one timed eight-worker cell at a time, with no concurrent builds or replay jobs. Leave seeds 27–35, cap tuning and the 80/20 phase allocation alone during this sequence.


## The brief, verbatim

# Brief 4 for GPT-6 Astra: same start, two separators; and what the cap does now that the churn is gone

Same setting as brief 3 (branch `engine/topology-archive-search`, worktree
`/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a`, read-only, xhigh). Your review 3 is
committed verbatim at `docs/astra-review-3-the-separator.md`. Your three corrections are accepted
and recorded: the loose-region Sparrow mean is 2.2 not 1.3 (gap about 7.8x); `minRawPhi` bounds
`v_max` by `sqrt(Phi)` and does not locate the plateau; Legacy's tail is the 50-iteration
`WorkCap` -> pool -> disruption path, not the 200-iteration strike. The failed margin preflight
is preserved unamended and seeds 27-35 are untouched. New evidence since your review, in the
order it was obtained.

## A. Sparrow warm-started from our layouts (`docs/experiments/overlap-ics/sparrow-warm-start/`)

Sparrow's `read_spp_input` accepts a solution file (it needs `solution.layout.density`). Our
layouts were converted by a rigid fit (1e-12 mm) and pass Sparrow's own validator. Every run:
`-t 10 -s 0 --min-item-separation 5 --workers 8`, explore best at 8 s:

| Sparrow started from | width | explore best |
|---|---|---|
| its own LBF (archived) | 214.027 | 150.796 |
| **our constructor** (seed-independent) | 182.976 | **149.195** |
| our Legacy incumbents, seeds 18/20/22/24/26 | 163.6-166.2 | 152.692 / 155.966 / 154.639 / 157.268 / 156.378 |
| our Wall10s incumbents, seeds 18/20/24 | 159.0-159.8 | 155.012 / 156.451 / 152.919 |
| its own layout after 2 s (`-t 2`) | 165.501 | 150.8 (the archived path) |

From our constructor Sparrow does 183 -> 167.6 in the first second (89 bites), -> 158.7 in the
second (54 bites). Ours from the identical layout: about 100 bites in 7.5 s to 163.6-167.2.
Bite by bite on the identical start (`first-bites.txt` there): bites 1-16 cost the same in
both engines (Sparrow 1,4,2,2,2,2,1,5,2,3,5,1,2,4,2,2 passes; ours 3,1,2,1,1,1,2,3,2,1,6,3,2,6,6,4
master iterations); bite 17 at 180.07 costs Sparrow 17 passes and us 37 iterations; the next
three cost Sparrow 13, 1, 1 and us 11, 11, 14. Over width >= 175 on seed 20: medians 2 and 2,
means 3.5 and 32.2, max 17 and 1145.

So: the constructor's basin is good (Sparrow's best on this fixture starts from it); the layouts
our separator leaves at 165 are basins from which even Sparrow reaches only ~155; and the
ordinary bite costs the same in both engines while the hard bite costs us 2x and its aftermath
is where we lose.


Then from our **margin-8** Legacy incumbents (seeds 18/20/22/24/26, starts 164.0 / 164.0 / 157.4
/ 167.1 / 163.3): Sparrow reaches **152.453 / 152.454 / 151.563 / 151.042 / 151.649**. So the
margin-8 path leaves basins almost as good as Sparrow's own (150.8 from its own 165.5), while
the no-margin path leaves basins worth 152.7-157.3. Section F says why.

## B. The bite records around the hard bite (seed 20, Legacy, `outcome.bites[]`)

devbase (no margin), bite 22 at 179.17: 1145 master iterations, **22 attempts, 22
disruptions, 319 band entries, 2 exact calls**, `minRawPhi` 0, published at the end. That is
your correction in the data: raw Phi reaches 0, the unchanged-pose digest suppresses the exact
call, the refused-Phi=0 rule ends the separation, the cap-50 attempt goes to the pool, the
pool restore + disruption (swap two large pieces, carry followers) runs, and the cycle repeats
22 times inside one bite. With `--proxymargin=8` the same seed's bites 15-24 show one exact
call and one band entry each; its worst, bite 21 at 179.07, is 119 iterations = 2 attempts
of 50 + 19, with 2 disruptions and strike-meter batches 88 none / 8 marginal / 26 substantial.
First 45 bites, total master iterations: devbase 1449, margin 4 306, margin 8 377.

Aggregate over the nine Legacy cells: bites that contain a disruption are 73/898 (devbase) and
27/938 (margin 8); the three bites *after* a disrupted bite are not harder than the three
before (median iterations 12 -> 9 devbase, 16.5 -> 7 margin 8). So "the disruption scrambles
the layout and the next bites pay" is not supported at the bite scale, and what makes our
165 mm layouts worse basins than Sparrow's is still unmeasured.

## C. Margin 8, nine seeds, both profiles (`docs/experiments/overlap-ics/proxy-margin/README.md`, addendum)

Legacy: paired median **+0.949 mm, 8/9 wins**, worst -2.592, give-ups 0, explore bites per cell
99.8 -> 104.2. Wall10s: +0.439 mm, 5/9, worst -1.085, give-ups 0. Against margin 4: +0.698
(5/9) and +0.515 (5/9). Not a round; recorded.

## D. The cap, now that the churn is gone (`m8cap0`, `m8cap200` vs `proxy-margin8full`, Legacy, seeds 18-26, one repetition)

Same binary, same seeds, Legacy, `--proxymargin=8`, one repetition; A = cap 50 (the profile's),
B = the arm named:

| arm | median | mean | worst | paired median vs cap 50 | wins | worst paired | explore bites / cell | iters/bite median / mean / max | disruptions | attempts |
|---|---|---|---|---|---|---|---|---|---|---|
| cap 50 (`proxy-margin8full`) | 164.010 | 163.860 | 167.102 | - | - | - | 104.2 | 8 / 15.7 / 735 | 68 | 77 |
| cap 200 | 164.008 | 164.343 | 172.061 | +1.603 | 6/9 | -8.758 (s26) | 99.4 | 10 / 18.5 / 2018 | 15 | 24 |
| cap 0 (unbounded, strikes only) | 164.028 | 165.706 | **179.095** (s26) | +0.597 | 5/9 | -15.792 | 91.0 | 10 / 21.5 / 2555 | 2 | 11 |

Per seed vs cap 50, cap 200: 18:+1.83 19:-0.36 20:+2.10 21:+1.75 22:-6.86 23:+0.00 24:+4.35
25:+1.60 26:-8.76; cap 0: 18:+1.82 19:-4.19 20:+1.34 21:+1.43 22:-3.53 23:-0.02 24:+0.60
25:+1.73 26:-15.79. Give-ups 0 everywhere, invalid 0. Reading: the cap is not the lever; it is
the fuse. Without it one seed sits in a single bite of 2555 iterations for most of the wall
(s26 ends at 179.095, twenty millimetres shallower). With 200 the good seeds gain 1.6-4.4 mm
and two seeds lose 7-9 mm. The stall that the cap truncates is the thing to fix; truncating it
later helps the seeds that were going to resolve and ruins the ones that were not.

## E. The mirror experiment (our engine started from Sparrow's layouts; diagnostic `--start` flag, workflow in progress)

Pending at the time of writing: the diagnostic `--start` flag is being implemented and verified by a workflow agent; if its result arrives before you answer, it will be appended as a postscript. The question it answers: from Sparrow's own 164.368 layout (its `-t 2` output), how deep does our engine get in ten seconds, and does it hold Sparrow's 150.090 layout (its `-t 10` output) or immediately lose depth to the repair/publication path?

## F. Piece displacement from the constructor layout

Per-piece centroid displacement between the constructor layout (182.976) and the layout an engine
reaches, computed on our placements (Sparrow's converted back by the inverse fit; its
rotation/mirror columns are conversion artefacts and are omitted):

| layout | depth | centroid displacement median / mean / max (mm) | pieces moved > 20 mm |
|---|---|---|---|
| Sparrow, 2 s from our constructor | 160.334 | 52 / 387 / 1641 | 44/61 |
| Sparrow, 10 s from our constructor | 149.195 | 339 / 509 / 1628 | 54/61 |
| ours, devbase Legacy s20 | 164.954 | **562** / 605 / 1794 | 58/61 |
| ours, devbase Legacy s18 | 164.262 | 404 / 460 / 1550 | 52/61 |
| ours, devbase Wall10s s20 | 159.773 | 599 / 629 / 1770 | 57/61 |
| ours, margin 8 Legacy s20 | 164.005 | **45** / 351 / 1635 | 39/61 |

The no-margin path moves the median piece 400-600 mm to gain 18 mm of depth; Sparrow moves it
52 mm to gain 23 mm and 339 mm to gain 34 mm; our margin-8 path moves it 45 mm to gain 19 mm.
The scrambling is the refused-Phi=0 -> pool -> disruption cycle of section B (22 disruptions in
one bite, 304 disruptions over nine devbase cells against 72 with margin 4 and 68 with margin
8), and it is what made the devbase layouts poor basins for Sparrow. With the margin the
layouts stay near the constructor's structure and Sparrow takes them to ~151.5.

## Questions

**Q1.** Does A change your ranking? In particular: the ordinary bite costs the same, the hard
bite costs 2x, and our layouts after ~100 bites are worse basins than Sparrow's after ~250.
Name what in our resolution of a hard bite could degrade the layout for later bites when the
disruption itself does not (B): container-origin commits (about 10 per bite on Legacy, 9735
over nine cells), the follower carry, the 16 um repair moves, GLS-driven moves of pieces the
bite did not squeeze, or the cap-50 truncation leaving half-resolved seams that the next bite
inherits. One measurement each, from the bite records we have or from the microscope.

**Q2.** Given D (and E if present), what is the right treatment of the cap under the margin,
and is it a schedule change that must go to a prospective spec on its own, or a precondition
the separator experiments should simply adopt (as you adopted margin 8)?

**Q3.** The microscope: your Q2 spec is large. Is there a first cut that decides your ranks 1
and 2 with a tenth of the fields (per committed relocate: origin, displacement, incident rows
cleared/created, CD exit reason and residual), buffered and emitted only for the first hard
bite and the three bites after it? If yes, write that reduced field list; it is the next
workflow run.

**Q4.** Unchanged from review 3 or amended: the `--cdfinish` mechanism and its falsifiers.

**Q5.** The order of the next 24 hours on one 16-core box, given everything above.
