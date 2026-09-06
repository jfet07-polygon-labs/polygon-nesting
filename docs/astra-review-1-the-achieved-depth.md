# GPT-6 Astra review 1: closing the ten-second gap to Sparrow

Consulted 2026-09-06 at `model_reasoning_effort = xhigh` on `d0c459b`, on the owner's instruction
("ignora grok, usa codex astra xhigh"). The brief is reproduced after the reply.
Verbatim; nothing edited.

## Reply

**Q1. Run H1 first as the quality experiment, but do not accept the claimed causal magnitude yet. There are three problems with the premise and one separate code defect.**

First, the 4,077 refusals establish **discarded improving proxy states**, not discarded dual-valid layouts. The target guard executes before `work.exact_checkpoints` increments and before either authority runs. Consequently, it also cannot explain refusals *after* the 352 exact calls in your example funnel. Those are two different losses: entry rejection and failed certification. The microscope establishes the first; it does not certify the rejected states. [Publication entry gates](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:561), [microscope census](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/bite22-microscope/README.md:83).

Second, **82.5% is not an estimate of recoverable time**. `minRawPhi` and `proxyBandReached` accumulate across the entire bite, including retries. A state with a micrometre-scale positive top overhang has positive raw Φ: the top residual enters the sum of squared violations. A bite can reach zero, fail certification, disrupt, and subsequently spend hundreds of iterations elsewhere. Classifying that entire bite as churn attributes its approach cost and subsequent search to the publication gate. Measure time **after the first state that H1 actually certifies**, not all iterations in bites that eventually touched zero. [Bite aggregation](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1962), [top residual](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/broad_phase.rs:46), [raw fold](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:371).

Third, the quoted **159.297 mm belongs to step 0.032, cap 200**. The held-out table reports **159.480 mm for cap none**; the later signed cap-none arm reports **159.953 mm**. The current profile is indeed cap none. This does not undermine the coarse-step result, but it means the headline is not the historical score of the configuration being controlled. [Held-out table](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/holdout-and-head-to-head/README.md:14), [signed result](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/quorum/ics-schedule-round-spec.md:136), [current profile](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:3934).

The H1 implementation I would approve is:

- Keep an immutable aspiration `T` throughout each separation and its retries.
- Keep the 4 µm entry band and existing proxy improvement prefilter.
- Run the existing Exclusive scan, existing bounded repair, and untouched contract validator.
- Require the **final measured depth** to satisfy  
  `published_depth.is_finite() && published_depth < incumbent_depth - 0.001`.
- Only then return a successful publication, install its repaired poses, and set the next parent depth to that measured depth.
- Apply this policy to the shrink schedule. Keep genuinely locked-target cells under a separate strict-target policy.

The post-repair improvement predicate is essential. Today, the caller protects `best_exact` but **returns the publication even when it did not improve**; `separate` treats any returned publication as success. Deleting the target checks while relying only on the existing *pre-repair* improvement check can therefore advance the live parent to a worse depth after repair. The incumbent would remain protected, but the trajectory would not be monotone. [Caller’s explicit non-record return](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:646), [separation success](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1277).

Use the strict comparison above consistently: the current prefilter admits equality at exactly 1 µm improvement; the incumbent writer requires strictly more.

With that formulation:

| Concern | Finding |
|---|---|
| Worse incumbent or parent admitted | Prevented by the final improvement check. |
| Quality redefined | No. It remains measured raw-source depth of a dual-valid layout. |
| Proxy-legal parent | Prevented by installing `Publication.poses`, then rebuilding all geometry and rows. |
| GLS | No new update rule. Earlier success causes earlier weight resets and changes subsequent trajectories. |
| Pool and disruption | Earlier success discards a different pool and avoids some disruptions. This can help throughput and hurt exploration. |
| Repair scope | Keep it unchanged, including its existing use of `T` when computing pair-repair slack. Do not silently relax that third use of the target. |

The installation and reset already exist. The repair’s remaining target dependency is at the pair-slack calculation. [Publication installation](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:678), [successful explore handoff](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:2089), [repair slack](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:1132).

**The serious algorithmic risk is earlier acceptance of an inferior parent geometry.** Every accepted parent can improve its predecessor while the resulting run loses to the control. T-row repair already demonstrated this: accepting earlier states altered the prefix and reversed seeds 2 and 3. Removing its downward repair avoids that particular cascade; it does not remove the earlier-parent risk. [Measured reversals and explanation](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/t-row-repair/gate0/README.md:58).

**Does H1 trip the forbidden-rescue table?**

For **§5.2’s nine rows: no**, provided the implementation above preserves certification, caps, source geometry, and publication-triggered shrinking. Your reading of “Giveback hiding” and “Proxy-legal parent” is correct.

But **it changes an explicitly frozen rule elsewhere in that document**. The reading requires publication “at the new `T`,” persistence at that width, and forbids growing `W`. H1 permits `T < D_new < D_parent`; the internal search width increases from the attempted target when the achieved layout is installed. Calling that wholly unchanged would conceal a real change in acceptance semantics. [§5.2 table](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/grok-review-12-reading-sparrow.md:366), [publication requirement](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/grok-review-12-reading-sparrow.md:192), [fixed-width persistence and prohibition](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/grok-review-12-reading-sparrow.md:244).

The new specification should expressly replace that rule:

> `T` is an aspiration. Accepted parent depths decrease strictly by more than 1 µm. An unsuccessful separation never increases its aspiration. Only a completed dual-valid, improving publication establishes a new parent depth.

That is a defensible prospective experiment, not a retroactive PASS for the closed member. The campaign already recognizes prospective specifications as the route for changing a freeze. [Recorded rule](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/quorum/ics-schedule-round-spec.md:12).

**The honest prediction is substantially weaker than “4×.”**

From 182.976 mm, approximately **198 nominal 0.1% publications** reach 150.16451 mm. If each publication loses the entire 4 µm lip, the simple recurrence

\[
D_{k+1}=0.999D_k+0.004
\]

requires approximately **203 publications**. Thus, roughly doubling the present 103 explore publications could be enough *if the later states remain separable*. Fourfold throughput is unnecessary.

Conversely, the same recurrence accumulates about **0.725 mm of lip after 200 bites**. The lip is small per bite, but not globally negligible. Actual repair giveback must be measured separately.

My conditional engineering forecast is **3–8 mm improvement over today’s coarse control**, if most first target-rejected states certify and the saved time persists at greater density. This is a planning range, not a confidence interval.

The falsifiers are:

- Fewer than **50%** of sampled distinct target-only rejections certify under H1: abandon the “mostly discarded legal progress” premise.
- Less than **50% of explore wall** lies after first counterfactually successful H1 checkpoints: abandon the simple “H1 doubles bite throughput by removing churn” explanation.
- More publications but insufficient measured depth improvement: reject the quality hypothesis regardless of the funnel improvement.

**Demanding `T − 4 µm` in the proxy is a useful alternative, but I rank it below H1.** In exact arithmetic, band entry then implies pre-repair depth at most `T`. It does **not** imply post-repair depth at most `T`; that final guard remains necessary. It also asks the separator to solve a harder problem and can prevent entry to the band altogether.

A fair comparison would retain the original nominal cut and acceptance target `T`, introduce a separate proxy ceiling `B = T − 0.004`, and use `B` consistently in boundary scoring and sampling. It is not T-row repair, but neither is it a free numerical alignment. I would not add it to the first quality battery.

---

**Q2. Beyond H1, I would fund one optional mechanism. First, fix the independently discovered rollback defect.**

`restore_keeping_weights` restores poses, geometry, pair rows, and boundary rows—but **does not restore `state.near`**. `incident_totals` relies on `near` containing exactly the positive pair rows. Consequently, restoring a collision that disappeared in the abandoned state can leave it absent from incident scoring and colliding-piece selection. [Restore implementation](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:2502), [incident scoring invariant](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:410).

The concrete regression vector is:

1. Snapshot a state containing a positive pair collision.
2. Move that pair apart and rebuild, making its near-set entries disappear.
3. Change live weights.
4. Restore the snapshot while retaining those weights.
5. Verify that both incident folds and colliding-piece selection recognize the restored collision.

The existing rollback test changes weights but does not exercise a changed near set. [Existing test](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/tests.rs:1560).

Restore the snapshot’s near lists along with its raw rows. This is correctness work, with **no promised millimetre gain**. In particular, Legacy’s 50-iteration cap normally prevents reaching the 200-iteration in-separation strike rollback; its terminal rollback is followed by a cold pool installation. I would budget **0 mm expected Legacy gain**, with an unmeasured effect on uncapped and locked-target runs. Do not use this finding to explain the 82.5% bucket.

The optional mechanism is **bounded candidate evaluation**:

| Required item | Proposal |
|---|---|
| Fact it acts on | Every candidate currently rebuilds its incident rows fully. `BestSamples` exposes an acceptance upper bound, but `evaluate` never receives one. |
| Mechanism | Accumulate nonnegative weighted row contributions and stop once the candidate is provably unable to win its comparison. Preserve equality rules, candidate order, and exact evaluation of every surviving candidate. |
| Predicted effect | **0.5–1.5 mm at 10 s**, conditional on H1 making useful progress throughput-limited. This is not additive with H1’s forecast. |
| Falsifier | Abandon it as the next quality investment if representative full-sweep replay gains **<15% wall throughput**, or if a separately registered subsequent wall test gains **<0.5 mm**. Any decision mismatch falsifies the claim of trajectory preservation. |
| Sparrow relationship | Inspired by bounded sample evaluation, already attributed to Sparrow in `BestSamples`; independently implemented on our nonnegative signed-gap rows. No imported geometry or source functions. |
| Cost | Approximately **3–5 engineering days**, mainly scratch-state handling, CD comparison semantics, and proving that rejected candidates cannot leave stale caches. |

The unused seam is concrete. [Candidate evaluation](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/relocate.rs:514), [upper bound and pool acceptance](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/relocate.rs:292).

Start with pool candidates; extend to CD only after its pairwise comparisons and accept-equal behavior are preserved. A partial score must never be stored as an exact score. Use the existing accumulation order or a conservative rounding bound.

Also, **4.68 M versus 3.74 M evaluations/s does not establish the useful-throughput advantage you suggest**. Using the supplied counts:

- ICS: approximately **11.7 K relocates/s**.
- Sparrow: approximately **14.2 K moves/s**.

Their evaluation is also different work, on different collision representations. Your master iteration rate is lower. The actionable advantage is the ability to evaluate source-faithful geometry quickly; it still has to become productive publications.

I would not fund another GLS scalar, constructor, pool-spread, or worker-count variation now. The committed negative ledger already covers substantial versions of those proposals. [Prior scalar and GLS negatives](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/sol-review-20-sparrow-informed-next-round.md:24).

---

**Q3. The 11.8% does not distinguish landscape, proposal coverage, or GLS. First establish that the separator is seeing its actual collisions.**

At every rollback and subsequent worker entry, compare the maintained near sets and incident folds against an independent full-row reconstruction. **One mismatch invalidates a landscape diagnosis for that continuation.**

After that, the most useful single experiment is a **frozen-state missed-improvement census**, not another end-to-end parameter sweep.

For each consumed seed, capture the first state with 50 consecutive master iterations without a 2% raw improvement while `max_violation > 0.004`. Keep poses, target, rows, weights, and stream ordinals. On detached copies:

- Record the actual candidates generated by the existing relocate.
- Add a fixed, denser single-piece probe bank using existing contact directions, translations, and allowed rotations.
- Evaluate every candidate’s **raw and current-weighted** loss with cold-consistent rows.
- Record whether each improving candidate was generated, retained, and committed.
- Replay promising alternatives to the same publication authority under an identical fixed quota.

Interpret the resulting matrix:

| Observation | Supported diagnosis |
|---|---|
| Ordinary proposals contain improving moves that the implementation fails to retain | Selection or implementation defect. |
| Added probes find weighted descent where ordinary proposals do not | Proposal coverage or CD resolution. |
| Raw-improving candidates exist but current weights reject them; weight-neutral replay converts more states | Evidence against the current weighting behavior. |
| Neither bank finds single-piece descent | A sampled coordinate trap; landscape versus coordinated move-set remains unresolved. |

**There is no single scalar that uniquely separates all three.** Changing the objective changes which moves are acceptable; changing the move set changes which barriers matter. A finite probe bank cannot prove that an escape does not exist.

For the last row, the next diagnostic must compare coordinated paths and alternative rankings on the **same saved states**, with eventual dual-valid conversion as the outcome. It does not license a live area proxy. Sparrow’s paper itself distinguishes collision detection, collision quantification, and search strategy; its successful combination does not identify which component explains this stall. [Paper §4](https://arxiv.org/html/2509.13329v3#S4).

---

**Q4. Proposed immutable specification: `ICS-achieved-depth-v1`.**

This specification changes acceptance semantics prospectively. It does not reopen or reinterpret any previous verdict.

**Frozen implementation and authority**

Base source is `d0c459b913ef0c6caa2611a4486427850ebdecb4`.

The corrected base adds only the rollback near-set restoration described above. H1 adds the achieved-depth publication policy, its final strict improvement check, and necessary diagnostics. Bounded candidate evaluation and inward proxy margins are excluded.

Unchanged:

- Source geometry and rotation permissions.
- Exact 5.0/5.0 clearance.
- Exclusive radius 2.500, allowance zero.
- `validate_placements_against_contract`.
- Publication band 0.004 mm, cumulative displacement 0.016 mm/piece, repair limit `4n`.
- Constructor, sample counts, CD, GLS, pool policy, disruption.
- Explore ratio 0.80; compression range `(0.0005, 0.00001)`.
- Eight workers and `--orders=1`.

The achieved policy must enforce strict final improvement in **both phases**. Strict-target behavior remains available for locked-target regressions.

**Preflight, using consumed seeds only**

Before any seed 27–35 cell:

1. Pass the rollback vector above, including preserved live weights and correct colliding-piece selection.
2. Pass publication vectors covering:
   - dual-valid above-`T` strict improvement;
   - above-`T` physical-boundary or pair illegality;
   - repair consuming the proxy gain;
   - exactly 1 µm final improvement;
   - non-finite depth;
   - repaired-pose installation and second-bite parent identity.
3. Preserve S0, locked S1 and triangle-20, FAST/HEAVY soundness, default-build isolation, and four pinned engine gates.
4. Establish two-process identity for fixed-work runs of each policy. Changes caused by the rollback correction are recorded; they are not required to reproduce the defective trajectory.
5. On corrected Legacy controls, seeds **0–8**, one fresh ten-second process each, retain the first **32 distinct target-only rejected states per seed**, or all if fewer occur. Evaluate H1 afterward on detached states. Count distinct `(target, pose digest)` opportunities.

The mechanism precondition is **at least six of nine seeds with a newly certified strict improvement**, and **median per-seed conversion at least 50%** among seeds with opportunities. Missing opportunities are reported and do not count as conversions. Failure stops the quality battery.

The instrumentation must distinguish actual refusal reasons from the counterfactual `wouldStrictTargetRefuse` flag. The H1 arm must not inherit a census that continues to report the removed target gate as an actual refusal.

**Arms**

| Arm | Source behavior | Explore step | Wall cap | Publication |
|---|---|---:|---:|---|
| O-L | Frozen head | 0.001 | 50 | Existing |
| O-W | Frozen head | 0.032 | none | Existing |
| R-L | Rollback corrected | 0.001 | 50 | Strict target |
| R-W | Rollback corrected | 0.032 | none | Strict target |
| H-L | Rollback corrected | 0.001 | 50 | Achieved depth |
| H-W | Rollback corrected | 0.032 | none | Achieved depth |

O-L/O-W measure whether the correction altered the baseline. R-L/H-L and R-W/H-W isolate H1.

Do not use `--arm=treatment` to select H1: that existing flag selects the work-based strike experiment. Add a separate publication-policy option. [Existing arm dispatch](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/examples/overlap_ics_benchmark.rs:2585).

**Population and walls**

Every scored cell uses **seeds 27–35**, a bare request, and a fresh process.

| Fixture | Requested walls | Arms | Repetitions per seed |
|---|---|---|---:|
| mixed-61 exact-clearance | 10.000 s | All six | 5 |
| mixed-61 exact-clearance | 7.000 and 15.000 s | R-L, R-W, H-L, H-W | 5 |
| quantity-expanded-74 | 10.000 s | R-L, R-W, H-L, H-W | 3 |
| shapes-17 compact | 10.000 s | R-L, R-W, H-L, H-W | 3 |
| triangle-20 compact | 10.000 s | R-L, R-W, H-L, H-W | 3 |

That is **954 scored processes**.

The fixture paths are:

- `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`
- `tests/fixtures/performance/quantity-expanded-74-request.json`
- `tests/fixtures/shapes-17/2000x2700-compact/request.json`
- `tests/fixtures/triangle-20/2000x2700-compact/request.json`

Quantity-74’s previous Gate 0 is accepted: certified lower bound 660.661 mm, control median 934.332 mm, certified headroom 273.671 mm. This establishes a non-saturated test subject; it is not an expected treatment gain. [Gate 0 record](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/quorum/ics-schedule-round-spec.md:44).

At ten seconds, use the explicit profile settings. At seven and fifteen seconds, run the coarse schedule as a **named research override** with the same frozen values. This specification authorizes those diagnostic cells; it does not broaden `Wall10s`’s existing profile guard or authorize a seven-/fifteen-second default.

Run one cell at a time on the same otherwise-idle machine. No concurrent builds. Within each `(fixture, wall, seed, repetition)` block, rotate the listed arm order left by `(seed − 27 + repetition) mod arm_count`; reverse it on odd repetitions. Repetition numbering starts at zero. Freeze executable hashes, compiler, features, fixture hashes, commands, and this order before scoring.

**Clock and quality**

Start the request clock at decoded-request entry. Charge constructor, engine preparation, separator, and publication work.

For requested wall \(w\), score only the deepest improvement certified **by \(w\)**:

\[
d(w)=\min\{D_j:\text{publication }j\text{ completed both authorities by }w\}.
\]

The constructor is an eligible floor only after its own certification. No completed legal layout by the deadline gives that cell infinite depth. Never count a publication completed after the deadline merely because the process returned within an overrun allowance.

Record request-relative publication completion timestamps. The driver currently has separate request and loop clocks; their distinction is already documented. [Clock accounting](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/examples/overlap_ics_benchmark.rs:2795).

Independently revalidate **every saved publication**, including late publications, outside the measured solve. Save placements and recompute raw depth. Do not enable expensive post-run revalidation inside the timed cell and then subtract its cost retrospectively. [Existing revalidation timing caveat](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/examples/overlap_ics_benchmark.rs:1293).

**Statistics**

For each arm \(a\), fixture, wall, and seed \(s\):

\[
m_{a,s}=\operatorname{median}_{r} d_{a,s,r}.
\]

All quality clauses use these **nine seed medians**. Repetitions are not additional independent seeds.

At mixed-61 ten seconds, define the conservative reference

\[
b_s=\min(m_{\mathrm{O\!-\!L},s},m_{\mathrm{O\!-\!W},s},
         m_{\mathrm{R\!-\!L},s},m_{\mathrm{R\!-\!W},s}).
\]

This is explicitly an envelope of controls, not a deployable policy. It prevents a regression in the correction from making H1 easier to pass.

For treatment \(h\), define paired gain \(g_s=b_s-m_{h,s}\). A win requires \(g_s>0.001\) mm. “Worst paired gain” means \(\min_s g_s\), not the difference between two arms’ worst absolute depths.

**PASS clauses—each treatment must satisfy its own complete conjunction**

At mixed-61 ten seconds:

1. Median seed depth **≤156.000 mm**.
2. Median paired gain against the reference envelope **≥3.000 mm**.
3. Wins on **at least 8/9 seeds**.
4. Worst paired gain **≥−1.000 mm**.

At **each** of seven and fifteen seconds, against that treatment’s matched corrected strict-target control:

5. Median paired gain **≥0.000 mm**.
6. Worst paired gain **≥−1.000 mm**.

On quantity-74, against the per-seed minimum of R-L and R-W:

7. Median paired gain **≥3.000 mm**.
8. Wins on **at least 6/9 seeds**.
9. Worst paired gain **≥−1.000 mm**.

On **each** of shapes-17 and triangle-20, against the same corrected-control envelope:

10. Median paired regression **≤0.050 mm**.
11. No seed-median regression **>1.000 mm**.

Across all required populations:

12. **Zero invalid publications**, zero authority/cap violations, and zero non-improving H1 parent installations.
13. Request-relative return-time p95 **≤ requested wall + 0.250 s**, evaluated separately for each arm/fixture/wall using nearest-rank p95.
14. All preflight and isolation requirements remain satisfied.

These are proposed materiality thresholds, not values inferred from unseen seeds.

A passing treatment earns **PASS-ADVANCE**, authorizing a ten-second research profile. It has not necessarily closed the Sparrow gap.

It earns **PASS-SPARROW-REFERENCE** only if it also has:

- median seed depth **≤150.16351 mm**; and
- **at least 8/9 seed medians ≤150.16351 mm**.

That establishes a robust result below the pinned legal layout. It does **not** establish superiority to Sparrow’s seed distribution; one Sparrow seed cannot supply that comparison.

If both treatments pass, select the lower ten-second median seed depth; an exact tie selects H-L. This selection rule is fixed now. Neither treatment can borrow the other’s passing clauses.

**Refusal condition**

Any failed mandatory clause refuses promotion of that treatment. A mechanism-only improvement, more bites, an improved best run, or a fifteen-second success cannot rescue it.

No step/cap changes, fixture substitution, selected-seed exclusion, replacement repetitions, post hoc wall restrictions, or addition of bounded evaluation after the first scored cell. A defect discovered after scoring begins invalidates the affected campaign result; these seeds remain consumed. Append results and the refusal, without amending the specification.

---

**Q5. Implementation order and parallel work**

The order is **rollback integrity → publication policy → causal replay → virgin battery → optional bounded evaluation**.

| Workstream | Ownership | Can proceed independently |
|---|---|---|
| A: rollback correction | `restore_keeping_weights`, focused regression vectors | Immediately; estimated half-day including validation. |
| B: H1 publication policy | `publish.rs`, policy-specific tests | In parallel with A; approximately one day. Do not edit schedule wiring yet. |
| C: harness and evidence | Benchmark driver, new round specification, evaluator, offline revalidator | In parallel with A/B; approximately one–two days. |
| D: bounded-evaluation prototype | Separate worktree; `relocate.rs`/`energy.rs` | Can be built concurrently, but remains outside this round’s binaries. |

One integrator should own the subsequent `mod.rs` schedule wiring and shared diagnostic types. Merge A before wiring B, so both corrected control and H1 inherit exactly the same rollback behavior. Workstream C should verify commands and executable identities rather than modifying solver behavior.

I rank the work by expected decision value: **H1 first for quality; rollback repair first for correctness; bounded evaluation next; inward proxy margin diagnostic-only; no new landscape or GLS treatment until the frozen-state census identifies a missed escape.**

Seeds 27–35 remain untouched. No repository files were changed.

---

## Brief

# Design consultation: close the 8.5 mm gap to Sparrow at ten seconds, and then beat it

You are GPT-6 Astra at maximum reasoning, consulted as the design partner for
the next round on a strip-nesting engine. The owner has asked for exactly this:
take inspiration from Sparrow (arXiv 2509.13329), unblock the campaign, and if
possible do better than Sparrow. Read the repository freely; everything below is
verifiable in it. Be adversarial with my hypotheses - the last three rounds of
this campaign were each saved by a reviewer refusing a premise I had asserted.

Repo root: `/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a`, branch
`engine/topology-archive-search`, head `d0c459b`. The engine is
`crates/polygon-nesting-core/src/search/overlap_ics/` (feature `overlap-ics`,
research-only, not on the shipped path). Sparrow's reading is
`docs/grok-review-12-reading-sparrow.md`; its source is partially at
`/var/lib/t3/tmp/sparrow-bench/src/` (optimizer only; `sample/`, `eval/`,
`quantify/` are absent locally - rely on the reading for those).

## Hard constraints (owner's, standing)

1. **Never copy Sparrow.** Reading it is authorised; porting functions verbatim
   is forbidden; everything learned must be cited. `jagua-rs` stays unused in
   this engine. No polygon simplification, no pole overlap-area proxy.
2. **The exact 5.0/5.0 mm clearance contract is untouched**, and so is
   `validate_placements_against_contract`. Quality is published raw-source depth
   of a dual-valid layout (Exclusive kernel + contract validator), never a proxy.
3. Sparrow's 150.16451 mm ten-second layout is contract-legal (Gate A,
   `docs/grok-review-9`). The comparison is apples to apples.
4. mixed-61, exact clearance, 8 workers, `--orders=1`, bare ten-second wall
   request, one fresh process per cell. Seeds 0-26 are consumed; **27-35 are
   virgin** and are what the next round will be judged on.

## Where we are

| | median at 10 s | mean | best |
| --- | ---: | ---: | ---: |
| Sparrow, seed 0 (their log) | 150.165 | | |
| ICS `Wall10s` profile (step 0.032, cap none) | 159.297 | 158.966 | 154.582 |
| ICS `Legacy` (step 0.001, cap 50) | 165.287 | 165.445 | 163.053 |
| shipped `general_relaxed` at a matched 10.6 s | 165.904 | 166.713 | 162.847 |

Held-out seeds 9-17, five repetitions, `docs/experiments/overlap-ics/holdout-and-head-to-head/`.

## What Sparrow's ten seconds actually are (its own log)

`--global-time 10 --workers 8 --min-item-separation 5`, seed 0:

- LBF constructor at **214.027** - 31 mm *worse* than our constructor's 182.976.
- Explore 0-8 s: **0.1 % multiplicative bites, hundreds of them, most
  `[s:0,i:0] (S)` - legal again after ONE separator iteration** - down to
  150.796. First failure only at 150.646 after 241 iterations.
- Compress 8-10 s: 0.05 % -> 0.009 %, final 150.165.
- Last explore separation: `evals/s: 3742 K, evals/move: 263, moves/s: 14.2 K,
  iter/s: 460, #workers: 8`.

So Sparrow closes ~64 mm in 8 s at 0.1 % - about 350 successful bites, ~44 per
second, at roughly one master iteration each.

## What our ten seconds are

`Legacy` (0.1 %, cap 50), 45 held-out cells, from the signed round's JSON:

- constructor 0.61 s; explore loop 6.9 s; 4.68 M evals/s (**faster than
  Sparrow**); 288 master iterations/s; 400 evals per relocate.
- **103 explore bites in 6.9 s, 14.9/s**; median 4 iterations per bite, but max
  427, and 34 pool retries per run.
- 182.976 -> 165.169 at the end of explore.

`Wall10s` (3.2 %, cap none): 5 explore bites at 62 iterations median, then 29
compress bites; 182.976 -> 160.652 -> 159.953.

## The diagnosis, and it is measured, not inferred

Every bite record carries `minRawPhi`, `proxyBandReached`, `exactBandEntries`,
`exactCheckpointCalls`, `published`. Splitting all explore master iterations of
the 45 `Legacy` cells three ways:

| class | bites | master iterations |
| --- | ---: | ---: |
| absorbed, <= 4 iterations | 48.6 % | 5.6 % |
| **separator stall**: `minRawPhi > 0` or never entered the band | 10.5 % | 11.8 % |
| **churn**: `minRawPhi = 0`, band entered, publication refused | 40.9 % | **82.5 %** |

`Wall10s`: 62.2 % churn in explore, 40.6 % in compress. `step 0.02`: 48.4 %.

The funnel of one cell: 766 band entries -> 352 exact-authority calls -> 120
publications. **Two exact calls in three refuse.**

Which gate refuses is already instrumented
(`docs/experiments/overlap-ics/bite22-microscope/README.md` §3, feature
`ics-publish-census`): **`proxy_depth > target_depth_mm` is the whole
difference.** Across nine cells, all but four of 4,077 above-target refusals
were layouts that *beat the incumbent*, by 0.175-0.180 mm - a full 0.1 % bite of
real, dual-valid progress - refused for exceeding the bite's own self-set target
`T` by `0 < excess <= 4 um`. The 4 um is the proxy band: `separate` calls the
exact authority when `max_violation <= 4 um`, and a 4 um violation on the strip
top *is* a 4 um depth overhang. The engine then spends hundreds of iterations
trying to land a micrometre under `T` instead of a micrometre over it.

`publish.rs::attempt`, the two lines:

```rust
if proxy_depth > state.target_depth_mm && !t_row_eligible { return None; }        // line 577, pre-gate
...
if published_depth > state.target_depth_mm { /* refuse: "the target is immutable" */ } // line 906, post-repair
```

and on the next line of the pre-gate, the improvement gate that already exists:
`if proxy_depth > incumbent_depth_mm - limits.minimum_improvement_mm { return None; }`.
The outer loop already adopts the **achieved** depth on success
(`width_mm = publication.raw_source_depth_mm`, mod.rs ~2083).

**Sparrow never has this problem because for Sparrow the strip width is a
hazard inside the proxy: `loss == 0` is "feasible", and the width is exactly
what was asked.** We advance only on a dual-valid publication *at or below* an
aspiration the proxy was allowed to miss by 4 um.

## My hypothesis H1, which you should attack first

**Publish at the achieved depth.** Keep every exact check. Replace the two
`> target_depth_mm` refusals with the improvement gate that is already there:
a dual-valid layout with `published_depth < incumbent - 1 um` publishes at
`published_depth`, `W` becomes that depth, the next bite is cut from it. `T`
becomes the bite's aspiration, not its acceptance bar. A bite that closes
0.176 mm instead of 0.180 mm is a success, not a failure to be retried through
the pool and Algorithm 12.

Predicted effect: the 82.5 % churn collapses; at 0.1 % the engine should
approach Sparrow's one-iteration-per-bite regime with ~4x the bites per second,
and at 3.2 % the 62 iterations per bite should fall toward the absorbed cost.

What I want from you on H1: (a) is it a rescue in disguise - does it let a
worse layout through anywhere, does it change what "quality" means, does it
interact with GLS weights, the pool, or the disruption in a way that hurts;
(b) what is the honest control and the falsifier; (c) is there a *better*
formulation - e.g. should the proxy simply demand `T - 4 um` on the top row so
that band entry implies `depth <= T`, and how would that compare.

The campaign's pre-registered forbidden-rescue table is at
`docs/grok-review-12-reading-sparrow.md` §5 (line ~367). Read it and say
plainly whether H1 trips any row. My reading: it trips none - "Giveback hiding
in re-legalization" is met (shrink only after dual-valid; quality is published
raw depth), "Proxy-legal parent" is met (publication is still the trigger).
"Widening the band" is not what this is: the band stays 4 um, the contract
stays exact, the layout that publishes is exactly as legal as before. But I
have been wrong about premises three times in this campaign and you are here
to be the fourth check.

## What is already tried and closed (do not re-propose without a new reason)

- T-row repair (push top pieces down to meet `T`): FAILED Gate 0, reversed two
  seeds; `docs/t-row-repair-spec.md`, `docs/experiments/overlap-ics/t-row-repair/`.
- Wall iteration cap: written, 50 at the frozen step; 200 refuted on holdout.
- Explore step: 0.032 written as the `Wall10s` profile (2-1 quorum); 0.02 failed
  its own spec by 0.204 mm at 15 s; adaptive step beaten by its own control.
- Explore ratio, compression range: flat. Pool spread (0.25) and patience (200):
  Sparrow's values win. 16 workers: worse (hybrid P/E cores). `f64::hypot`:
  slower than `libm`.
- Engine speed: contact pruning 1.47x, axis cache 1.27x, near set 1.04x, all
  bit-identical; SAT is still ~half of an evaluation, `closest_feature` mostly
  pruned away.

## What I want from you

**Q1.** Attack H1 as above. Then rank it against anything better you can name.

**Q2.** Beyond H1: name up to four mechanisms, each with (i) the measured fact
it acts on, (ii) the predicted effect in mm at 10 s, (iii) the falsifier - the
number that would make you abandon it, (iv) whether it copies Sparrow or is
merely inspired, and (v) implementation cost in the engine as it stands. I am
especially interested in what would let us go *past* 150 with an engine that
already evaluates faster than theirs: our 4.68 M evals/s against their 3.74 M.

**Q3.** The 11.8 % of stalls (`minRawPhi > 0` for hundreds of iterations, the
"bite-22" class): is that the Φ landscape (signed-gap SAT vs their area proxy),
the move set, or GLS? What single measurement distinguishes them?

**Q4.** Write the specification for the round: control arms, treatment arms,
virgin seeds 27-35, repetitions, walls, the second fixture
(`tests/fixtures/performance/quantity-expanded-74-request.json`, 273.671 mm of
certified headroom, passed Gate 0), transfer floors on shapes-17 and
triangle-20, the PASS clauses, and the refusal condition. I will run it verbatim
and I will not amend it after the first cell.

**Q5.** Order of implementation and what can be built in parallel by
independent agents in separate worktrees without stepping on each other.

Be specific, cite line numbers where you read them, and prefer one mechanism
you can defend over four you cannot. No preamble.
