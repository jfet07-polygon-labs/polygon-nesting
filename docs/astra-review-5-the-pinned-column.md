# GPT-6 Astra review 5: the pinned column

Consulted 2026-09-07 at `model_reasoning_effort = xhigh` on `6b79557`, on the owner's instruction
("ignora grok, usa codex astra xhigh"). The brief is reproduced after the reply. Verbatim;
nothing edited. Written before the replay probes ran and before the escape-weight reading
(`docs/experiments/overlap-ics/bite-microscope/README.md`, addenda); those go to brief 5b.

## Reply

**The pinned-column explanation now leads, but the trace does not falsify finer CD or revisits. Finish those probes; the next objective experiment should test linear guided loss before adding a joint operator.**

I reviewed `32e32c1` read-only, including the escape addendum, and reconstructed weights from the archived sweep residuals. Margin 8, Legacy cap 50, Wall10s’s existing schedule, the failed preflight and seeds 27–35 retain their previous status.

**Q1. Your reading identifies the leading mechanism, but overstates what “unmoved” establishes.**

The analysis script classifies an exit as unmoved from its final displacement and angle. It then labels that result a local minimum where every ± step is worse. The trace contains no trial-comparison record establishing that stronger claim. Even rejecting every *tested* step would not establish rejection of smaller steps or every direction. [Classification](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/bite-microscope/analyse-trace.py:21)

Consequently:

- **The 174 moved exits establish termination at the limits, not useful descent remaining.**
- **The 108 unmoved exits establish failure of the sampled searches to change the committed pose, not mathematical single-piece local optimality.** Continuation must include them.
- Refining another member can change the objective confronting an unmoved piece. Revisits can therefore help even if its earlier search exhausted useful descent.
- Persistent positive rows include residuals already below the 4 µm band. Track actual band blockers separately from all positive contacts.

The escape itself is now much better established. **Sweep 37**, after 36 updates, moves piece 43 approximately 166 mm: raw incident loss rises from `0.0000610743` to `9.58187`, while guided loss falls from `10.6590` to `9.58187`. Six further sweeps reach zero at 43. This supports a weight-enabled rearrangement, although it does not establish that the same escape proposal was available earlier.

I would restore rank 1 if continuation clears the persistent blockers, directly or through neighbour changes, and materially accelerates whole-state certification. I would restore rank 2 if revisiting changed endpoints does that at equal work. Merely reducing an exit residual, or briefly clearing a row that immediately returns, is insufficient.

**Q2. The largest candidate difference is the collision quantifier, including how cheaply it values tiny persistent conflicts.**

The shared constructor is a common **initial** layout. After sixteen different sweeps-and-publications histories, bite 17 is not a matched separator input. The archived first-bite counts do not establish that Sparrow encountered and broke this exact column. That requires importing the hard-bite parent or equivalent capsule geometry into Sparrow. [Recorded comparison](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/sparrow-warm-start/first-bites.txt:1)

Here is my disposition of your six candidates:

| Candidate | Reading and discriminating measurement |
|---|---|
| **(i) Maximum over boundary and pair rows** | Real throttling mechanism. A boundary supplies the maximum in **17 of the 36 pre-escape sweeps**. Replaying just the weight recurrence on the recorded residual sequence, using the pair maximum for pair updates, makes final weights on `(0,6)`, `(6,43)`, `(6,44)` **4.77×, 3.69×, 3.63×** larger. This is a fixed-residual counterfactual, not a trajectory result. **Measurement:** the escape/stay score crossover under those two weight histories, using identical candidate poses. Including container conflicts is not itself an established Sparrow difference: our source ledger says its tracker also updates container-item entries. |
| **(ii) Ranking by `w v²`, growing by `v/v_max`** | My leading candidate. Squaring makes tiny residuals extremely cheap relative to fresh millimetre overlaps. Changing growth to `v²/v_max²` would actually **slow** smaller rows’ growth: the ratio becomes smaller. **Measurement:** rerank identical candidates with `Σwv` versus `Σwv²`, then propagate each independently through the tournament and separation. Measure certification work, not merely earlier departure. |
| **(iii) Four edge rows versus container hazards** | The relevant difference is representation and loss, not simply weighted versus unweighted. Our ledger describes Sparrow’s container-item loss as a weighted bbox-area term; our four edges independently retain weights and contribute squared distances. Its evaluator also has an `Invalid` category our finite poses lack. **Measurement:** for the same entry and candidate poses, record pair contribution, container contribution and validity classification under each implementation. The exact pinned evaluator is needed before prescribing a hard-container policy. |
| **(iv) Full incident cost at a distant sample** | Charging prospective weighted collisions is GLS’s intended acceptance rule, including Sparrow’s paper Algorithm 7. The problem may be their relative prices. **Measurement:** at frozen pre-escape states, record the best container candidate’s score minus stay’s score, plus whether an acceptable candidate loses during finalist selection or the worker tournament. Back-insert the observed escape pose separately to distinguish discovery from acceptance. |
| **(v) Sixteen versus continuous orientations** | This is not the documented distinction. **Both sample sixteen orientations and then refine continuously.** Our trace itself contains continuous rotations. Differences can remain in candidate coverage, rotation permission, sample-box coordinates and CD streams. **Measurement:** keep the fifty translation draws fixed, substitute continuous sampled angles, and count additional candidates that beat stay after the same refinement budget. |
| **(vi) Accept-equal finds no equality** | Our comparison accepts `Less` and `Equal`; there is no reversed comparison defect here. Equality’s frequency depends on the landscape and arithmetic. **Measurement:** recover actual `Less/Equal/Greater` counts and score deltas for the unmoved calls, including the finer continuation. Do not infer them from endpoint displacement or introduce an equality tolerance. |

The relevant implementation anchors are [GLS recurrence and source ledger](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:547), [sample comparison](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/relocate.rs:194), and [orientation sampling](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/relocate.rs:1038).

There is a stronger version of the quantifier hypothesis than “Sparrow is linear.” Its paper quantifier uses a **square root of a decaying pole-overlap proxy, multiplied by a shape penalty**, and collision detection gates it to zero. It deliberately gives minor detected collisions appreciable cost. It is neither our squared distance nor simply linear penetration. [Algorithms 3–4](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/sparrow-paper.txt:417)

For the observed piece-43 escape, the unweighted new/old cost ratios are:

| Cost applied to the recorded incident residuals | New/old ratio |
|---|---:|
| Sum of squares | **156,889** |
| Sum of residuals | **416** |

These ratios illustrate the weight threshold if the old rows shared a multiplier and fresh rows stayed at one. Actual weights differ. They support testing linear scoring; they do **not** predict a seventeen-pass result.

I could read the pinned separator online; the local Sparrow checkout was unavailable, and the other pinned files could not be fetched. Its separator shows ordinary competitive sweeps, GLS updates and rollback—no special pinned-column operator. Claims above about its tracker and sampler are distinguished as the repository’s source ledger, supported where applicable by the local paper. [Pinned separator](https://github.com/JeroenGar/sparrow/blob/14f4868fcd7e97036700dbebaf193fb159180aa9/src/optimizer/separator.rs)

**Q3. My ranking adds an objective-only candidate ahead of the proposed operators.**

1. **Linear guided loss on our existing residuals**, testing the severe price difference above.
2. **A bounded coordinated change that alters relative geometry**, especially middle-member rotation with neighbour accommodation.
3. **Additional GLS escalation associated with unsuccessful moves.**
4. **Forced container relocation selected by raw overlap.**

Two qualifications materially change your operator proposals.

First, rigid sideways translation of the entire column preserves its internal pair separations and its bottom/top residuals. It cannot directly relieve those constraints. It may create room for subsequent relative movement, but that benefit needs a propagated test. Likewise, rotating only the middle member remains a single-piece move unless neighbours move with it. A coordinated candidate must be installed atomically and scored on **every affected row, including new external contacts**, counting each row once.

Second, the proposed raw-score rescue is incomplete. If stay remains eligible, it beats the observed escape by approximately **157,000×** under raw squared loss. If relocation is compulsory, this becomes an explicit worsening move and an earlier disruption mechanism. It may work, but needs that description and corresponding regression scrutiny.

Its detector is also too restrictive: before the escape, all four core members `0,6,43,44` ran and remained unmoved together only at sweeps **24, 28 and 30**. Requiring two consecutive such sweeps misses this exposure.

Faster escalation does violate the frozen GLS rule. The code explicitly identifies the published constants as frozen; applying an extra update only to unsuccessful pieces changes the schedule even if the literal constants remain unchanged. Joint moves and forced relocations also change the frozen member. These are prospective successor experiments, with explicit exceptions, rather than repairs that rescue an earlier verdict. [Frozen GLS constants](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:530), [frozen member](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/cutclose-relocate-spec.md:20)

For rank 1, I propose detached **`--probe=linear`**, followed conditionally by **`--guidedlinear=1`**, default off:

- Guided row cost becomes `w*v` for **both pairs and boundaries**.
- Raw `Σv²`, maximum violation, collision membership, GLS recurrence, publication and schedule retain their present definitions.
- Sample ranking, both CD stages and worker selection use the same linear guided objective.
- Apply this consistently in [`energy::fold`](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:386) and [`incident_totals`](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:425); audit their consumers in `relocate.rs` and [`Engine::tournament`](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1053).
- Record the objective mode and its units in results and replay identities. Preserve the default-off arithmetic path.

This tests our own linear residual objective. It does not port Sparrow’s quantifier.

The review-4 Q4 falsifiers carry forward:

1. **Mechanism:** failure to resolve persistent blockers and reduce work to certification rejects the premise. Earlier jumps followed by equally expensive cleanup also fail.
2. **Halving:** require at least **50% reduction** on a prespecified, seed-balanced population of common-entry opportunities. Fix the work horizon beforehand; retain failures in the population rather than analysing successful completions alone.
3. **Engineering:** require lower total evaluation work **and measured wall time**, at maintained completion rate. Include loser workers and ordinary bites.
4. **Depth:** on seeds **18–26**, require positive paired-median depth gains on **both profiles**.
5. **Tail protection:** no per-seed repetition-median regression **greater than 1.000 mm**, on either profile.
6. **Integrity:** any invalid publication, default-off trajectory mismatch or cold/incremental inconsistency stops the treatment. Capsule starts remain diagnostic.

Follow seed 20 with the already prescribed seed-26 exposure. If supported, retain the **108-cell** development screen: two arms × two profiles × nine seeds × three repetitions, with fixed profile caps, rotated arm order and one timed eight-worker process at a time.

**Q4. Finish the two probes first, then select the live mechanism.**

They are already being built, and the trace’s strongest exclusion—“continuation cannot act on unmoved pieces”—is unproven.

For **either** probe, this would move it ahead of linear scoring: from the margin-8 bite-entry capsule, certify within **342,990 all-worker evaluations**, half the recorded 685,980, while demonstrating the claimed causal path.

- **CDFinish:** the gain must come from continuation clearing or unlocking the implicated blockers.
- **Revisit:** the gain must come from servicing changed endpoints. Compare against spending the same extra work on ordinary continuation; extra sampling alone does not establish a queue benefit.

At comparable work per sweep, **43 → at most 21 sweeps** is the corresponding iteration target, but evaluations remain essential because both probes add work. The historical margin-zero corroboration would be **37 → at most 18**, with at most **250,340 evaluations**.

These are prospective decision thresholds, not observed results. Smaller improvements can justify later optimization without displacing the leading jam mechanism. Require `--probe=none` identity before interpreting any treatment; do not combine the probes. If neither materially resolves the blockers, run the isolated linear probe next.

**Q5. Yes, the boundary margin removes 16 µm of vertical capacity beyond the nominal contract; that warrants the deferred ablation now.**

With sag zero, the allowed vertical material span changes from

\[
(T-\text{inset})-\text{edge}
\quad\text{to}\quad
(T-\text{inset}-m)-(\text{edge}+m),
\]

a reduction of **`2m = 16 µm`**. That is extra proxy reserve, not a contractual requirement. Pair margins add their own geometric burden. The two top pieces do not make the sheet reserve 24 µm: each bottom-to-top path pays one bottom and one top reserve.

It does not follow that removing the boundary margin clears the jam. At sweep 29, for example, pair `(0,6)` remains at **4.285 µm** and `(6,43)` at **6.020 µm**. Removing edge reserve on those unchanged poses leaves pair blockers outside the band.

There is also a concrete reason to retain some top reserve: with zero top margin, a state admitted by the 4 µm band may exceed nominal `T` and fail the strict target check before repair. Pair-only margin can therefore exchange separation work for publication churn. [Boundary residuals](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/broad_phase.rs:53), [publication entry](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:561)

Run the pair-only **detached ablation** from identical capsules: pair margin 8, boundary margin zero, rebuilt caches, identical nominal target and unchanged publication. Measure certification work, above-target refusals, boundary failures and repair cascades. It now deserves testing; it does not yet justify replacing the all-row margin-8 control or combining margin changes with linear scoring.


## The brief, verbatim

# Brief 5 for GPT-6 Astra: the microscope says the hard bite is a pinned column

Same setting (branch `engine/topology-archive-search`, worktree
`/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a`, read-only, xhigh). Your review 4 is
committed verbatim (`docs/astra-review-4-same-start-two-separators.md`). Its plan was followed:
margin 8 and cap 50 kept on Legacy, Wall10s untouched, the reduced microscope built exactly to
your Q3 list (`--bitemicroscope=1`; commit "The bite microscope"; module
`overlap_ics/microscope.rs`; bit-identical off, trajectory-identical on, replay capsules at
bite entry and after every reset), the two prescribed cells traced (seed 20, Legacy, eight
workers, cap 50, margin 8 and margin 0). The replay harness with your two detached probes
(`--cell=replay --probe=none|cdfinish|revisit`) is being implemented by a workflow agent now;
its identity gate is that `--probe=none` reproduces the traced sweeps bit for bit. Everything
below is in `docs/experiments/overlap-ics/bite-microscope/` (README, the gzipped documents,
`analyse-trace.py` and its two readings). Seeds 27-35 untouched; the failed margin preflight
unamended.

## 1. The trigger bite under margin 8: bite 15, 180.195 -> 180.015, 43 iterations, published at 43

End-of-sweep maximum violation (um), iteration by iteration:

```
88.8 72.1 52.3 43.5 30.8 26.5 25.9 41.3 41.4 41.4 41.4 28.1 37.4 15.9 20.4 14.8 58.6 86.1 10.1 23.7
26.6  7.8 19.4 31.0 34.0  9.6  9.0 14.3  6.1  6.1 95.2 28.9 10.7 50.9 10.7 10.7 1815.2 1134.0 582.6
385.5 88.9 17.1 0.0
```

Blocking rows at the end of each sweep (rowIdScheme decoded):

| row | present (of 43) | residual median / last (um) |
|---|---|---|
| edge(piece 0, bottom) | 36 | 3.7 / 5.2 |
| pair(0, 6) | 35 | 5.1 / 3.6 |
| pair(6, 44) | 34 | 4.3 / 4.3 |
| pair(6, 43) | 34 | 5.7 / 4.8 |
| edge(piece 44, top) | 32 | 6.1 / 3.1 |
| edge(piece 43, top) | 31 | 5.6 / 5.6 |
| pair(8, 45) | 29 | 8.2 / 1.0 |
| edge(piece 2, bottom) | 28 | 6.1 / 0.6 |

One object: a column 0 - 6 - 43/44 standing from the sheet's bottom edge to the strip top
(and 2 - 8 - 45 beside it). Fine-CD exits in the bite: 339 committed relocates, 282 with a
nonzero exit (83 %), median 7.8 um, all by `limits`. **108 of the 282 are unmoved**: after 75
samples and both descents the relocate committed the entry pose because every +- candidate was
strictly worse; these are strict local minima of the weighted incident objective at a median
5.4 um, on pieces 0 (22 times), 6 (17), 43 (16), 44 (15), 45 (10). The other 174 moved and
were stopped by the limits (median exit 14 um; final translation step 37 um against a 60 um
limit; candidate pairs median 19). Changed rows by the other endpoint's status: later 201,
visited 176, absent 37, boundary 78; conflict births by status: visited 28, absent 37, later
23, boundary 11. Evaluations: 685 980 over all workers, 87 285 for the winners; relocates per
sweep median 8.

The bite resolves at iteration 36: the maximum jumps to 1.8 mm (a member of the column takes a
distant sample with a real overlap, presumably because its rows' weights after 36 updates make
that the lower weighted cost), and seven iterations later the state is proxy-zero.

## 2. Without the margin: bite 17, 180.070 -> 179.890, 37 iterations (the historical bite)

Same column: pair(0, 6) 20 iterations at 30 um, edge(0, B) 20 at 25 um, pair(2, 8) 19, pair(6,
43) 19, edge(43, T) 19, edge(2, B) 18, pair(6, 48) 18, pair(8, 45) 17, residual medians 25-30
um; maximum series 81 60 68 53 35 55 39 50 46 45 45 37 34 34 35 53 33 33 then the jump to
2118 um at iteration 19, then 18 iterations to 0. 199 of 246 exits nonzero (median 33.7 um),
62 unmoved (median 29.9 um) on pieces 2, 43, 0, 45, 48, 6. On the identical constructor layout
Sparrow resolved this bite in 17 passes and its next three in 13, 1, 1
(`docs/experiments/overlap-ics/sparrow-warm-start/first-bites.txt`).

## 3. The three bites after (margin 8): 4, 26, 16 iterations

The 26-iteration bite blocks on edge(piece 44, top), pair(6, 44), edge(piece 0, bottom)... the
column again, 23-26 of 26 iterations. Under margin 0: 11, 11, 14, blocking on pair(2,8)/
pair(8,45)-type rows.

## Questions

**Q1.** Does the trace settle your ranks 1 and 2 before the probes run? My reading: the step
floor holds the 174 moved exits but not the bite; the pinned column holds the bite, and neither
a finer continuation nor a revisit queue moves a piece whose every single-piece step is worse.
Say where that reading is wrong, and what the probes must show for you to keep rank 1 or 2.

**Q2.** The column is broken, thirty iterations late, by GLS weight growth. Sparrow runs the
same Algorithm 8 constants and breaks the same column on the same layout in seventeen passes.
Name the candidate differences you can see in the code that would make its weights bite sooner
or its sampler find the exit earlier: (i) our `v / v_max` uses the current maximum over ALL
rows including the boundary rows, so a large boundary residual elsewhere throttles the column
rows' growth; (ii) our ranking uses `w v^2` while the growth uses `v / v_max`; (iii) the
column's boundary rows (bottom edge, strip top) are weighted rows for us and container hazards
for Sparrow; (iv) our `evaluate` of a distant container sample charges the piece's full
weighted incident cost at the new place, so an escape must beat a weight of 1.2^k times a
5 um residual; (v) our 16 sampled orientations against Sparrow's continuous rotation for a
column member that could turn; (vi) the fine CD's accept-equal on a pinned piece finds no
Equal because the entry pose is compared with `Ordering::Greater` on weighted cost. One
measurement each, from the capsule replays if possible.

**Q3.** The mechanism for a pinned column, ranked, with the forbidden-rescue table in view: a
jam detector that, when the same rows have blocked for k sweeps with all their pieces unmoved,
relocates one member with container samples scored on raw overlap rather than weighted cost;
a bounded joint move of the column (translate the column sideways, or rotate its middle
member) evaluated on the whole column's rows; a faster weight escalation for rows whose piece
was unmoved this sweep (a constant change: is that the rescue row "GLS constants are the
paper's"?); Sparrow's own answer if you can read it in the pinned separator. For the one you
rank first: the opt-in knob, the code sites, and the falsifier on seeds 18-26 in the form of
review 4 Q4 (mechanism, halving, engineering, depth on both profiles, the 1.000 mm per-seed
guard, integrity).

**Q4.** Should the replay probes still run first (they are being built), or does the trace
already move the next workflow to Q3's mechanism? If the probes should run, state the number
from each that would change your Q3 ranking.

**Q5.** The column reaches from the bottom edge to the strip top. Under margin 8 the strip
top for the proxy is `T - inset - 0.008` and the bottom is `edge + 0.008`; the column's members
are charged the margin at both ends. Is the margin making this particular jam 16 um harder
than the contract requires, and is that a reason to apply the margin to pair rows only (your
review 2 deferred that ablation)?
