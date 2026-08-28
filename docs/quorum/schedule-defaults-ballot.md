# Quorum: should the inherited schedule constants become the defaults?

Convened 2026-08-28 on `73b6549`. Two reviewers of three: **ox-alpha is down
provider-side** (`opencode run` returns `UnknownError / Unexpected server
error, ref err_10fa293a`), verified again at the time of writing. A two-model
quorum is a deviation from the campaign rule and is recorded as one.

The brief (`round 1`) and its corrections (`round 2`) are reproduced after the
ballots. **Two of the three premises I put to the quorum in round 1 were
false**, and I found that out by running the probe I should have run before
writing the brief. Both corrections are mine and both are material.

## Verdict

| question | Sol | Grok | outcome |
| --- | --- | --- | --- |
| `EXPLORE_SHRINK_STEP` 0.001 -> 0.032 | **NO** (post-selection) | **NO** (named forbidden rescue) | **refused** |
| `EXPLORE_TIME_RATIO` 0.8 -> 0.95 | **NO** (board is flat; 0.60 is better anyway) | **NO** (no-op on wall, trajectory change on fixed work) | **refused** |
| `Pacer::Wall` cap: unbounded -> bounded | **YES**, 200 | **YES**, 50 | **approved, value open** |
| re-pin the four identity gates | no - category error | no - category error | **untouched** |
| a schedule gate should exist | yes, statistical, wall-mode | yes, plus a production-path pin | **to be specified** |

### The refusal that settles it

Grok cited a pre-registered constraint and I verified it in the tree:

> **Bite-size fitted to 168.484** | 3 % (or "whatever reaches 168 in 80 bites")
> chosen after a scout run | 0.1 % / (0.0005, 0.00001) / 80/20 frozen from
> Sparrow defaults **before** the nine-seed wall. Changing them is a forbidden
> rescue.
>
> -- `docs/grok-review-12-reading-sparrow.md:370`, and carried in the source at
> `homotopy.rs:38`

`0.032` is 3.2 %, chosen after sweeping mixed-61. The rule was written in
advance precisely so that the difference between "fitted to a bar" and
"a budget-scaling mechanism" could not be used as an escape hatch. The step
does not move on this evidence, independently of how good the evidence is.

## Sol, round 1 (verbatim)

**Q1. PARTIAL.** Promote only (2): `Pacer::Wall::iteration_cap = Some(200)` for the bare-request wall profile. Keep `EXPLORE_SHRINK_STEP = 0.001` and `EXPLORE_TIME_RATIO = 0.8`. The cap is isolated from fixed-work ([mod.rs:3726](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:3726)), bounds a demonstrably pathological unbounded call, and preserves all four identity gates. The step is post-selection on one unsaturated request: eight values were swept, then the same nine seeds received a third repetition—not an independent holdout. Moreover, the advertised “current defaults” comparator uses cap 50 although the actual default is 0, so the headline −7.3 mm is not a clean default-vs-default contrast. Ratio 0.95 is rejected: its board is flat, while 0.60 has the better median and worst case; re-pinning for that would be indefensible.

**Q2. Budget-derived, but not yet.** I would accept a wall-only rule

\[
s(B,r)=\operatorname{clamp}\!\left(a(B_{\rm search}r_{\rm hard})^{-p},s_{\min},s_{\max}\right),
\]

where `r_hard` is a pinned bite-22/hard-state master-iteration rate, not cheap-prefix throughput. Budget alone is insufficient because last night’s speedup moved the optimum. Fit `a,p` jointly with the cap, using precommitted grids at 5/10/20/30/60/120 seconds on at least three requests with ≥20 mm certified headroom, discovery and held-out seeds separated. Promotion requires held-out regret ≤0.5 mm versus that budget’s best swept arm and no request-level median regression >1 mm. The three present points cannot fit it: the 20-minute point comes from a different engine and CPU, and previous linear/square-root cap laws already failed at five seconds. Until then, 0.032 remains a 10-second experimental override, not a universal constant.

**Q3. (c).** Keep all four identity gates byte-for-byte and add wall-mode statistical gates. Re-pinning would erase the only long-term proof that geometry/search semantics stayed fixed; those gates are not product benchmarks. The new wall gate should run fresh processes, nine seeds × at least three repetitions, pin zero invalid publications, bare-request wall distribution, median, tail, and transfer. The cap can become default only after that gate passes because it cannot affect fixed-work. Any future step promotion must be wall-policy-specific so fixed-work continues to use the legacy schedule; otherwise it is a semantic trajectory replacement masquerading as tuning.

**Q4.** A precommitted HEAD-only factorial holdout: `(cap 0/200) × (step 0.001/0.032)`, ratio fixed at 0.8, five repetitions on unseen seeds 9–17 for mixed-61 and one previously untouched request first certified to have ≥20 mm headroom. I would change my vote on the step if the interaction arm beats cap-only by ≥3 mm paired median on both requests, has no >1 mm request-median regression, and remains zero-invalid.

**Q5. Confidence: 84/100.** Strongest argument against my vote: 0.032/200 is extraordinarily stable—27/27 below 160.6 mm—and preserving historical identity should not be allowed to keep the actual ten-second product roughly seven millimetres worse when a wall-specific policy could preserve both.

## Grok, round 1 (verbatim)

I'll read the experiment docs, the schedule constants, and the explore-loop code so the ballot is grounded in what's actually measured.The docs already show the 10s/30s optimum inverting; next I’ll check how the defaults, pacers, and identity gates actually bind.`EXPLORE_SHRINK_STEP` already reads `0.032` in source while tests still pin `0.001` — I'll reconcile that with the pacer and gate code.**Q1. PARTIAL.** Move (2) only: `Pacer::Wall::iteration_cap` `None → Some(50)`. Do not move (1) or (3). 200 is not a standalone default; it is the companion of step 0.032. At the step that would remain, 50 is the last fully-swept 10 s optimum; 200 was measured as the pair that lets a 3.2 % bite finish a separation. Unbounded wall is the one replicated defect (2/9, five times of five, Algorithm 12 firing once per run). A bound repairs a missing pacer, not a Table 1 constant. Fixed-work already has `iterations_per_separation`; wall does not.

(1) is the founding forbidden rescue: Grok review 12 §4.4 names “3 % (or any %) substituted for 0.1 % after seeing a number,” and `homotopy.rs` still calls shrink-step fitted to mixed-61 a forbidden rescue. 0.032 is 3.2 %, found by sweeping mixed-61. (3) is flat in the proposed regime (0.46 mm from 0.60–0.95, 18 cells) because explore already dies at ~5.1 bites; changing 0.8 → 0.95 is a no-op on wall and a trajectory change on `WorkPlanPacer`.

**Q2. Budget-derived**, if the step ever moves. A constant 0.032 is the same disease one level down, and the 10 s / 30 s inversion is the mechanism, not noise. At 30 s, 0.032 parks six of nine seeds on a 159–160 overshoot shelf (median **159.010**); 0.016 continues through it (median **157.773**, N=9×1). At 10 s the “win” is the same shelf: four of nine seeds sit at ~160, and the worst-column story is “everyone hits the wall in five bites.” Triple the wall and that floor is a trap. The pacer already holds `remaining_seconds`; shipping 0.032 is choosing not to use it.

I will not accept a closed form fit to {10 s: 0.032, 30 s: 0.016, 20 min: 0.001}. The wall-cap round already fitted `cap ~ 16√s` to two budgets and died at the third (5 s). Form I would accept: a **lookup over measured (T, step, cap) triples**, interpolated in log T, **engine-versioned** (this retune exists because 1.47× moved the last optimum; host seps/s is a hidden coordinate). Evidence to fit: 2-D (step × cap) at T ∈ {5, 10, 15, 30, 60} s, mixed-61, 9 seeds × 3 reps; then a **pre-committed holdout T = 20 s** that must beat both neighboring constants on median and worst. Until that holdout exists, the step stays 0.001.

**Q3. (d).** The four named gates are mode 20/22 of the previous engine (`docs/experiments/round-envelope-kernel/drivers/gatelib.py`: g1 mode 20, g2–g4 mode 22). They do not read `EXPLORE_SHRINK_STEP`, `EXPLORE_TIME_RATIO`, or `Pacer::Wall`. Re-pinning them is a category error; using them as the scare-cost of this change is the same error. Keep them untouched. Keep `tests.rs` `assert_eq!(EXPLORE_SHRINK_STEP, 0.001)` as the frozen 0.1 % pin. Add wall-mode statistical gates on mixed-61 at 10 s (9×3, median and worst, clauses committed before the next sweep) for the production path. If a step change ever ships, add a cutclose identity cell that forces `--shrinkstep=0.001` so the 0.1 % trajectory remains a regression, not a memory. Do not choose (a). Do not choose (b) for the cap: unbounded is already measured-bad, and the knobs already exist.

**Q4.** Neighborhood of the SLA, not another saturated fixture. mixed-61, exact 5.0/5.0, 8 workers, `--orders=1`, idle machine, one fresh process per cell. Walls **7 s, 10 s, 15 s**. Arms `(step 0.001, cap 50)`, `(0.016, 200)`, `(0.032, 200)`. 9 seeds × 3 reps. Pre-commit: 0.032/200 must win **median and worst at all three walls**, or it does not ship as the ten-second default. The 30 s inversion already exists; this asks whether it starts inside the production neighborhood (constructor tax ~2.3 s already means “10 s” is not a point).

**Q5. 74.** Strongest argument against: the product is a ten-second request, 27/27 cells at median 159.483 / worst 160.523 with 0 invalids is the largest legal quality move on the board, and this vote leaves ~8 mm on a flag because of a thirty-second inversion that is not the SLA. If no caller ever passes 30 s, that is process theater. Cap 50 is also timid on the faster engine (anecdotal ~2 mm for 200 at step 0.001, not a 27-cell sweep).

## Round 2: the corrections I owed them

# Round 2: the brief's crux was false. Re-vote.

I gave you a premise and it does not survive contact with the code. Two
corrections, both mine, both material. Revise your ballot.

## Correction 1: changing `EXPLORE_SHRINK_STEP` does NOT re-pin the four gates

I claimed it did, and made that the crux of Q3. Measured instead of asserted:

- built with `EXPLORE_SHRINK_STEP = 0.032`: all four gates reproduce, exact raw
  depths and fingerprints (`g2` 159.09233022733062, `g3` 159.07876040364795,
  `g4` 164.0375677990678), `ALL_PASS: true`;
- built with `EXPLORE_SHRINK_STEP = 0.4` - a forty per cent bite, absurd by any
  reading - **all four still reproduce identically**.

The reason is in `lib.py::GATES`: every gate runs
`examples/general_request_benchmark` in mode 20 or 22 **from a pinned parent
layout at a fixed target depth with an allowance**. They are relocate/repair
identity cells. They never run the explore homotopy.

So the cost I told you about does not exist. But the corollary is worse than the
cost was: **nothing in the pinned gate set covers the explore schedule at all.**

## Correction 2, and it is the big one: the ICS engine is not on the shipped path

`search::overlap_ics` is behind `#[cfg(feature = "overlap-ics")]`, and
`crates/polygon-nesting-core/Cargo.toml` has `default = []`. Outside its own
module the only things that name it are `search::overlap_ics_meter` and
`examples/overlap_ics_benchmark.rs`. `general_request_benchmark` - the binary
the gates run, and the closest thing here to a production request - imports
`general_fast`, `general_relaxed`, `portfolio` and `shadow_rescore`, and never
`overlap_ics`.

**A bare production request does not execute a single line of the code these
constants live in.** My Q1 as written - "change the shipped defaults so that a
bare request gets this behaviour" - was not a description of a possible change.

The shipped `general_relaxed` path has its own schedule constants:
`initial_shrink_ratio: 0.02`, `minimum_shrink_ratio: 0.001`
(`general_relaxed.rs:627`).

## What is unchanged

Every measurement in the round-1 brief stands; it was taken through
`overlap_ics_benchmark`, which is the engine those numbers describe. The
contact-pruning speedup (1.47x, bit-identical) is in that same engine. 1,059
cells, 56,108 publications, `invalidPublications: 0`.

## Re-vote on the real question

**Q1'. Disposition.** Pick one and defend it:
(a) promote the ICS engine toward the production path, with this schedule
    finding as one of its arguments;
(b) keep ICS research-only but move *its* defaults, so its own benchmark and any
    future comparison run at the honest ten-second setting;
(c) leave ICS defaults alone and treat the finding as a *transferable claim* to
    test against `general_relaxed`'s `initial_shrink_ratio` / 
    `minimum_shrink_ratio` on the shipped path;
(d) record and do nothing.

**Q2'. Is the finding even about ICS?** The claim is general: "the depth a bare
request reaches is set by how many successful separations fit in the wall, and
an inherited twenty-minute shrink step is 32x too fine at ten seconds." State
whether you believe that generalises to the shipped path, and what would falsify
it there.

**Q3'. Gates.** Given that no pinned gate covers any schedule, on either engine:
what gate should exist? Be concrete - instance, budget, statistic, threshold,
repetitions - or say that no gate should exist and why.

**Q4'. Did I stop too early or too late?** The proposer spent a night measuring
an engine that is not shipped. Say plainly whether that was the right use of the
night given the stated goal ("the same quality at ~10 seconds from a bare
request"), and what the first move this morning should be.

**Q5'. Confidence** 0-100 and the strongest argument against your own vote.

Under 800 words.

## Correction 3, found while writing this: the shipped path already does it

`general_relaxed.rs` around lines 4582 and 4820-4858:

```text
shrink_ratio = initial_shrink_ratio                     // 0.02, at start
on improving exact-valid epoch:  shrink_ratio = initial_shrink_ratio        // reset up
on valid non-improving epoch:    shrink_ratio = max(shrink_ratio*0.5, 0.001)
on rejected / infeasible:        shrink_ratio = max(shrink_ratio*0.5, 0.001)
otherwise:                       shrink_ratio = max(shrink_ratio*0.75, 0.001)
```

That is a **start-large, halve-on-failure, reset-on-success** schedule, opening
at `0.02` - inside the `0.016-0.032` band the ICS sweep independently measured
as optimal at ten seconds - with a floor at exactly Sparrow's `0.001`.

So the finding does not need to be transferred to the shipped path. The shipped
path has had it all along, and the ICS engine - built to reproduce Sparrow
faithfully - inherited a bare twenty-minute constant and thereby regressed
against this project's own older engine.

Weigh this in Q1' and Q2'. In particular: does it make (b) nearly free and
(a) much more interesting, or does it make the whole night's result a
rediscovery of something the repository already knew?

## Sol, round 2 (verbatim)

**Q1'. Disposition — (a).** Promote ICS toward production as a feature-gated contender, not immediately as the default. The schedule principle is not novel—the shipped engine already implements start-large/back-off at [general_relaxed.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:4582)—but the exact-valid **159.483 mm median at 10 s** is new product-relevant evidence. Merely changing a research benchmark’s defaults would preserve an artificial leaderboard without advancing the bare-request goal. Use `0.032/200` as the candidate’s frozen 10-second configuration; do not claim it is universally optimal.

**Q2'. Is the finding about ICS?** Literally, yes: it diagnoses ICS inheriting a twenty-minute constant. Generally, it confirms a principle the shipped engine already knew: coarse successful contractions early, smaller ones after failure. It does **not** license transplanting `0.032` or `200` into `general_relaxed`; its operators and failure feedback differ. The general explanation would be falsified there if a contemporaneous 10-second A/B—existing adaptive `0.02→0.001` versus forced `0.001`—showed no increase in exact-valid improving epochs and no paired depth benefit on mixed-61. Also trace the ratio: if it immediately collapses to `0.001`, the apparent corroboration is merely dead code-path semantics.

**Q3'. Gates.** Keep the four identity gates unchanged; they test relocate/repair, not scheduling. Add an actual bare-request schedule/promotion gate:

- Actual production entry point, mixed-61, exact 5.0/5.0 contract, 10.000-second capture.
- Control versus production entry plus feature-gated ICS contender, interleaved AB/BA.
- Seeds 0–8, three fresh processes per seed: 27 paired cells.
- PASS only with zero invalid publications, paired median gain **≥3.000 mm**, treatment wins **≥21/27**, and no paired regression worse than **1.000 mm**.
- Request-relative completion p95 **≤10.250 s**; quality is the last exact-valid publication at or before 10.000 s.
- Transfer floor: shapes-17 and triangle-20, nine seeds each, paired median regression no worse than **0.050 mm**.

This is statistical because wall scheduling is the behavior under review. Add a small fixed-work trace-digest canary for implementation determinism, but do not mistake it for the product gate.

**Q4'. Timing.** The night was productive but sequenced incorrectly. Repository-path discovery should have preceded optimization: the work did not improve a shipped request. Still, it was not merely rediscovery. It quantified why the faithful ICS configuration was wrong for ten seconds, demonstrated a roughly 7.3 mm median gain, preserved exact validity across 56,108 publications, and exposed a credible production contender. The first move this morning should be to commit the production-integration specification and gate above, then wire ICS as an isolated contender under the real request clock. No further benchmark-only tuning first.

**Q5'. Confidence — 90/100.** Strongest argument against: mixed-61 is the only unsaturated quality fixture, while both transfer fixtures cannot reveal upside. Production integration and budget competition may erase the apparent gain, leaving an expensive duplicate of `general_relaxed` whose scheduling insight was already present.

## Grok, round 2 (verbatim)

I'll re-read the shipped schedule, the ICS defaults, and the gate drivers so the round-2 ballot is grounded in the corrected path, not the round-1 premise.The path split is confirmed; next I’ll read the adaptive-explore README, shipped `general_relaxed` schedule, and wall-cap evidence so Q1' and the 50-vs-200 disagreement rest on measurements.The shipped path already opens at 0.02; I’ll check production 10s quality, gate coverage, and the 50-vs-200 cells so the ballot can treat that as a measurement, not a preference.**Q1'. (b).** Correction 3 kills (c): `general_relaxed` already opens at `0.02`, resets on an improving exact-valid epoch, halves on valid-non-improving / rejected / infeasible, decays `0.75` otherwise, and floors at `0.001` — inside the `0.016–0.032` band ICS spent a night rediscovering. That is a rediscovery of a *policy this repository already ships*, not a transferable claim still waiting to be tested.

(d) leaves the one replicated defect: `Pacer::Wall::iteration_cap` is still `0`, so Algorithm 12 still fires about once per ten-second run (2/9, five of five). (a) is more interesting as *diagnosis* — ICS’s 10 s FAIL was in part a twenty-minute Sparrow constant on a ten-second clock, while production already opened at 2% — but this night is not a promotion brief. `overlap-ics` is Gate0-stopped; the 10 s quality gate is retired against retune; `Cargo.toml` still forbids a portfolio route until the Round-2 gate passes, and that gate failed. Keep ICS research-only. Move *its* wall cap from unbounded to a bound. Do not write `0.032` into `EXPLORE_SHRINK_STEP`; CLI overrides already exist. (b) is nearly free for the cap. It is not a license to ship Sol’s pair.

**Q2'.** The economics generalise; the prescription does not need to. Depth-from-separations-in-the-wall is why `initial_shrink_ratio = 0.02` exists. The finding is about ICS because ICS is the engine that *lacks* that policy. Falsifier on the shipped path: mixed-61 exact 5/5, 8 workers, 10.0 s, 9 seeds × 3, `initial_shrink_ratio` ∈ `{0.001, 0.02, 0.032}` with the existing halve/reset rule otherwise unchanged. If `0.001` wins or ties median *and* worst, “32× too fine” is ICS-specific (continuous `separate` vs epoch/lane). If `0.02` and `0.032` are flat, the ICS `0.016`-vs-`0.032` distinction does not transfer. I predict `0.001` loses and `0.02 ≈ 0.032`.

**Q3'.** A gate should exist: nothing currently covers any schedule, on either engine. Not a `159.483` quality pin — that launders a sweep.

- Keep `tests.rs` `assert_eq!(EXPLORE_SHRINK_STEP, 0.001)`.
- Pin the production schedule: `mixed_61_probe.initial_shrink_ratio == 0.02` and `minimum_shrink_ratio == 0.001` (those numbers are presently only struct defaults).
- **ICS-WALL-10:** `overlap_ics_benchmark --cell=cutclose`, mixed-61 exact 5/5, 8 workers, `--orders=1`, wall `10.000` s, seeds `{7,8}` × 5 reps, one process per cell. PASS iff `invalidPublications = 0` on all ten and every cell’s raw depth `≤ 175` mm. Unbounded seed 8 is `179.082` five of five; cap 50 moved it. This does not reopen `5/9 ≤ 168.484`.
- If a step override ever becomes a default, a cutclose identity cell that forces `--shrinkstep=0.001`.

**Q4'.** Too late on the wrong engine for the stated goal; the right night for ICS-research. 1,059 cells after the pair was clear; never asked whether a bare `general_request_benchmark` already opens at 2%. First move this morning: 9×3 mixed-61 10 s head-to-head, exact 5/5, 8 workers, idle machine — shipped `general_relaxed` defaults vs ICS `0.001`/unbounded vs ICS `--shrinkstep=0.032 --itercap=200`. That is the comparison that tells you whether `159.483` is production-relevant. Then, if anything in ICS code, default the wall cap. Not another `(step, cap)` cell. Not a promotion spec.

**Q5'. 71.** Strongest argument against: 27/27 legal cells at median **159.483** / worst **160.523** is the largest 10 s quality move on the board, the shipped 10 s band in this campaign has lived well above that, and (b) files it under a research feature. If the head-to-head confirms it, this vote is how you waste the next week.

**Cap 50 vs 200.** We are talking past each other. Sol’s 200 is the companion of `0.032`. Mine is the companion of frozen `0.001` (wall-cap round’s last fully-swept 10 s optimum; 200 at `0.001` is the ~2 mm anecdote, not a 27-cell sweep). Settling measurement: the 2×2 at 10 s *and* 30 s, mixed-61, 9×3, arms `(0.001, 50)`, `(0.001, 200)`, `(0.032, 50)`, `(0.032, 200)`. I predict: at 10 s, `0.032/200` wins median and worst; `0.032/50` loses on retries (same shape as `0.016/50`’s 19 retries vs 6); `0.001/200` beats `0.001/50` by ~2 mm, not 8. At 30 s, `0.032/200` stays on the 159–160 shelf already measured. Concession: if `0.001/200` wins median *and* worst at 10 s by ≥2 mm vs `0.001/50`, I abandon 50 as the default at frozen step and accept 200. If `0.032/50` matches `0.032/200`, the pairing dies. I still will not write `0.032`.

## Round 1 brief, for the record

# Quorum ballot: should the three inherited schedule constants become the defaults?

You are one of three independent reviewers. Rule on a specific, narrow question.
Answer in the ballot format at the end. Be adversarial: the proposer wants a
"yes" and you are here to find the reason it should be "no".

## Repository and where to look

Repo root: `/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a`
Branch: `engine/topology-archive-search`, head `73b6549`. Read-only.

Relevant files:
- `crates/polygon-nesting-core/src/search/overlap_ics/homotopy.rs`
  (`EXPLORE_SHRINK_STEP`, `COMPRESS_SHRINK_RANGE`, `EXPLORE_TIME_RATIO`, and the
  process-level overrides `set_explore_shrink_step`, `set_adaptive_step_ceiling`,
  `set_compress_start_step`)
- `crates/polygon-nesting-core/src/search/overlap_ics/mod.rs` (the explore loop,
  around line 1780-2100; `set_wall_iteration_cap` and `Pacer::Wall`)
- `crates/polygon-nesting-core/src/search/overlap_ics/{contact,energy,broad_phase}.rs`
  (the contact pruning landed last night)
- `docs/experiments/overlap-ics/faster-engine-retune/README.md` (the measurements)
- `docs/experiments/overlap-ics/contact-pruning/README.md`
- `docs/experiments/overlap-ics/adaptive-explore-step/README.md` (a falsified
  hypothesis, kept)
- `docs/experiments/overlap-ics/wall-iteration-cap/README.md` (the prior round
  whose conclusions this partly reverses)

## Background you need

The engine is a strip-packing optimiser (`CutCloseRelocate`) that reproduces
Sparrow (arXiv 2509.13329) without copying it. Four constants were inherited
from Sparrow's Table 1. The paper's §11.3 states they were tuned for
**twenty-minute** runs on a 7950X and never re-tuned for other time limits. The
production target for this project is a **ten-second bare request**.

Three of the four have now been examined under measurement at ten seconds:

| constant | Sparrow / current default | measured best at 10 s |
| --- | --- | --- |
| `EXPLORE_SHRINK_STEP` | 0.001 | **0.032** |
| `Pacer::Wall` iteration cap | none (0) | **200** |
| `DEFAULT_EXPLORE_TIME_RATIO` | 0.8 | flat 0.60-0.95 |
| `COMPRESS_SHRINK_RANGE` opening | 0.0005 | flat 0.0005-0.0100 |

## The measurement (mixed-61, exact 5.0/5.0 clearance, 8 workers, `--orders=1`)

Bare wall-clock requests, one fresh process per cell, machine otherwise idle,
nine seeds, three repetitions = 27 cells per arm.

Ten seconds:

| configuration | median | mean | best | worst |
| --- | ---: | ---: | ---: | ---: |
| current defaults + the night's *code* speedups (cap 50, step 0.001) | 166.781 | 166.6 | 162.690 | 169.4 |
| **cap 200, step 0.032, ratio 0.95** | **159.483** | **158.689** | **154.557** | **160.523** |

Thirty seconds, nine seeds, `cap 200, step 0.016`: median 157.773, mean 157.640,
best 154.524.

The step's optimum is **budget-dependent**: 0.032 at 10 s, 0.016 at 30 s,
0.001 at Sparrow's 20 minutes. The cap moves with it (bite size and separation
budget are one parameter: at step 0.032, `cap = 50` is the *worst* arm on the
board at 19 retries/run against 6 for `cap = 200`).

Transfer test on the two corpus fixtures never looked at while tuning, 9 seeds,
10 s, `frozen` = cap 50 / step 0.001:

| fixture | arm | median | best | worst | certified lower bound |
| --- | --- | ---: | ---: | ---: | ---: |
| shapes-17 | frozen | 200.350 | 200.348 | 200.350 | 200.347 |
| shapes-17 | tuned | 200.351 | 200.347 | 200.353 | 200.347 |
| triangle-20 | frozen | 70.251 | 70.250 | 70.254 | 70.250 |
| triangle-20 | tuned | 70.254 | 70.251 | 70.263 | 70.250 |

Both fixtures are within 13 um of a bound the request itself certifies as
unreachable-below. mixed-61 has ~43 mm of headroom above its bound (115.839).
triangle-20's median is **3 um worse** under the tuned arm; shapes-17 neutral.

Validity: across every cell run that night - 1,059 cells, 56,108 publications -
`invalidPublications` was **0**. The exact-clearance contract validator
(`validate_placements_against_contract`) is untouched.

## The proposal being balloted

Change the shipped defaults so that a **bare** request gets this behaviour:

1. `EXPLORE_SHRINK_STEP: 0.001 -> ???`
2. `Pacer::Wall::iteration_cap`: `None -> Some(???)`
3. optionally `DEFAULT_EXPLORE_TIME_RATIO: 0.8 -> 0.95`

## The cost, and it is the crux

The four pinned regression gates (`g1` 206.869 / `8a7737381238fa4d`,
`g2` 159.09233022733062 / `fa01012af1d559ae`, `g3` 159.07876040364795 /
`e28fba007f8031d4`, `g4` 164.0375677990678 / `49f094d7e59a9008`) are **identity**
gates: each pins a raw depth *and* a placement fingerprint. They run in
**fixed-work** mode (`WorkPlanPacer`), not wall mode.

- The wall iteration cap does **not** affect fixed-work mode (that pacer already
  has `iterations_per_separation`), so change (2) should leave the gates alone.
- `EXPLORE_SHRINK_STEP` and `EXPLORE_TIME_RATIO` **do** affect the fixed-work
  trajectory, so changes (1) and (3) re-pin all four gates.

Re-pinning identity gates destroys the campaign's continuity record: those four
numbers are how every change for months has been proven not to have moved the
trajectory.

## What is NOT established

- `0.032` and `200` were found by **sweeping**, not derived. Curing a constant
  that failed to scale with another constant is the same disease one level down.
  The budget-dependence is measured at exactly two budgets (10 s, 30 s) plus one
  inherited data point (20 min).
- No specification was pre-committed for this. It is not a signed gate.
- One fixture has headroom. The other two are saturated, so the transfer test
  can only show "neutral", never "helps".
- An adaptive rule was tried and **falsified by its own control** (a plain larger
  constant beat it on every column) - see the adaptive-explore-step doc. So
  "make it adaptive instead" already has one corpse.

## Ballot - answer exactly these

**Q1. Move the defaults at all?** `YES` / `NO` / `PARTIAL`. If `PARTIAL`, say
which of the three.

**Q2. Constant or budget-derived?** If the step should be a function of the wall
budget, give the functional form you would accept and say what evidence would be
needed to fit it. If a constant, give the number and defend it against the
charge that it is the same mistake at a different scale.

**Q3. The gates.** Choose and defend one: (a) re-pin all four; (b) keep the four
as they are and gate the new behaviour behind a non-default flag indefinitely;
(c) keep the four *and* add new wall-mode statistical gates alongside; (d)
something else.

**Q4. What single piece of evidence, not yet collected, would most change your
vote?** Be specific enough that it can be run.

**Q5. Confidence** 0-100, and the strongest argument *against* your own vote.

Keep it under 900 words. No preamble.

---

# The two false premises, and how they were caught

## 1. "Changing the step re-pins the four gates"

Asserted in the round-1 brief as the crux of Q3. Probed instead of asserted:

- built with `EXPLORE_SHRINK_STEP = 0.032`: all four gates reproduce exactly,
  `ALL_PASS: true`;
- built with `EXPLORE_SHRINK_STEP = 0.4` - a forty per cent bite - **all four
  still reproduce identically**.

`lib.py::GATES` runs `examples/general_request_benchmark` in mode 20 or 22 from
a **pinned parent layout at a fixed target depth**. They are relocate/repair
identity cells and never touch the explore homotopy. Grok's word for using them
as the cost of this change is the right one: a category error.

## 2. "A bare request would get this behaviour"

`search::overlap_ics` is behind `#[cfg(feature = "overlap-ics")]` and
`Cargo.toml` has `default = []`. Outside its own module only
`search::overlap_ics_meter` and `examples/overlap_ics_benchmark.rs` name it.
`general_request_benchmark` imports `general_fast`, `general_relaxed`,
`portfolio` and `shadow_rescore`. **A production request executes no line of the
code these constants live in.**

## 3. Found while writing the correction: the shipped path already has the policy

`general_relaxed.rs:4582` and `4820-4858`:

```text
shrink_ratio = initial_shrink_ratio                       // 0.02 at start
improving exact-valid epoch:  shrink_ratio = initial_shrink_ratio
valid non-improving:          max(shrink_ratio * 0.5,  0.001)
rejected / infeasible:        max(shrink_ratio * 0.5,  0.001)
otherwise:                    max(shrink_ratio * 0.75, 0.001)
```

Start large, halve on failure, reset on success, floor at Sparrow's `0.001`,
opening at `0.02` - **inside the `0.016-0.032` band the ICS sweep spent a night
rediscovering**. The ICS engine, built to reproduce Sparrow faithfully,
inherited a bare twenty-minute constant and thereby regressed against this
project's own older engine.

One caveat that neither reviewer had: the pinned driver tail in `lib.py` passes
`relaxed-initial-shrink-ratio = 0.005`, not the struct default `0.02`, so what
the gates exercise is not what `GeneralRelaxedSettings::default()` holds.

# What the quorum ordered next

Both reviewers, independently, made the same first move the priority, and it is
not a tuning cell.

**Grok Q4'.** A 9x3 mixed-61 ten-second head-to-head, exact 5.0/5.0, 8 workers,
idle machine: shipped `general_relaxed` defaults, against ICS at
`0.001`/unbounded, against ICS at `--shrinkstep=0.032 --itercap=200`. *That is
the comparison that tells you whether `159.483` is production-relevant.*

**Sol Q4 (round 1), already running.** A pre-committed factorial
`(cap 0/200) x (step 0.001/0.032)`, ratio fixed at the real default `0.8`, five
repetitions on **held-out seeds 9-17** - the round-1 sweeps were discovery on
seeds 0-8 and the "third repetition" was not a holdout.

**The cap disagreement is a measurement, and Grok pre-committed a concession:**
if `(0.001, 200)` beats `(0.001, 50)` on median *and* worst at ten seconds by
>= 2 mm, Grok abandons 50 and accepts 200. Sol's factorial as launched tests
`cap 0` against `cap 200`; the `cap 50` arm has to be added on the same
held-out seeds for the concession clause to be decidable.

**Grok's shipped-path falsifier**, runnable with no code change because
`general_request_benchmark` already exposes `relaxed-initial-shrink-ratio`
positionally: sweep it over `{0.001, 0.02, 0.032}`, 9 seeds x 3, and see whether
`0.001` loses. Grok predicts it does, and that `0.02 ~ 0.032`.
