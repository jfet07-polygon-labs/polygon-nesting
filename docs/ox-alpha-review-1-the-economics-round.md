# ox-alpha review 1 — the economics round (2026-08-23, two rounds)

The owner added ox-alpha (oxalpha.com; reasoning-first, no effort knob —
maximum thinking is native; accessed via `opencode run --pure -m
opencode-go/ox-alpha-free --agent plan`) as the third quorum member.
Round 1 = fully independent design against the same brief, blind to Sol and
Grok (it read the raw per-bite evidence and Sparrow's source itself);
round 2 (their round 3) = the closing vote. Its round-1 position landed on
work-denominated strikes + mandatory paired control independently — the
tie-breaking vote that shaped the two-arm resolution. Verbatim.

# ROUND 1 (independent)

# Consultation — ox-alpha, third consultant, CutCloseRelocate round-2 design

**What I read before answering:** `docs/experiments/overlap-ics/cutclose-rerun/README.md` in full; the raw per-bite rows in `evidence/wall.json` (all 27 cells), `evidence/control-ab-ba.json`, `evidence/round1-bites-red.json`; `docs/cutclose-relocate-spec.md`; the engine (`crates/polygon-nesting-core/src/search/overlap_ics/mod.rs` — `tournament` at :646-717, `separate` at :744-860, `run_cutclose` at :883-1135, `Pacer` at :1632-1757, `SeparateLimits`/`observe_raw` at :1256-1348); Sparrow `optimizer/separator.rs` and `config.rs` at `/var/lib/t3/tmp/sparrow-bench/src/`; `docs/experiments/sparrow-mixed61/README.md`; `docs/experiments/robust-plan/README.md` (the plancal methodology).

**One transparency note before anything else:** the tree already contains `docs/grok-review-14-the-economics-round.md`, dated today, which is a converged Sol–Grok position on this exact brief. My analysis below was derived from the primary rows first; where it touches that document I say so explicitly, including one place I refine it (§Q1, seed 6) and one place I dissent (§Q3, the 4/9 contingency).

---

## Q1. Where the 10 s millimetres were lost, ranked

**The evidence shows** a trajectory that is deterministic in work-space and cut by the clock, nothing else. Proof from the committed rows: the cumulative master-iteration count at which a seed crosses a width milestone is *identical across budget cells* — seed 6 crosses <175 mm at exactly 296 batches in both its 10 s and 30 s cells, seed 3 at 479 in both, seed 2 at 1463 in both — and the fixed-work replay is bit-identical (`wall.json:replayAllBitIdentical: true`). Wall mode only chooses *where in the deterministic sequence the clock cuts*. Every mechanism below is a statement about what the cut point lands on.

**Escape draws, in batches (from `wall.json`, explore-phase cumulatives):**

| seed | batches to <175 | batches to ≤168.484 | classified |
|---|---:|---:|---|
| 6 | 296 | 1,562 | crosses early, **descent-limited after crossing** |
| 3 | 479 | 1,048 | qualified at 10 s |
| 5 | 1,315 | 1,566 | pure continued search (0 strikes, 0 disruptions at 30 s) |
| 2 | 1,463 | 1,881 | qualified at 10 s |
| 0 | 1,607 | 1,930 | closest shelf seed (2/3 strikes at 10 s, deadline mid-cycle) |
| 4 | 2,142 | 2,805 | disruption-mediated at 30 s |
| 1 | 3,300 | 3,848 | disruption-mediated at 30 s |
| 7 | >6,544 | never | different basin (5 disruptions at 30 s, still 179.08) |
| 8 | >5,708 | never | different basin (still 179.06 at 30 s) |

Explore capacity at 10 s is 0.8 × (10 − 2.31..2.35) ≈ **6.14 s**, i.e. 946–1,508 batches at the measured per-seed rates of 154–246 explore batches/s. Required uniform throughput multiple *k* to fit each draw inside the explore window (keeping 80/20): **seed 0: k≥1.28 · seed 6: k≥1.46 · seed 5: k≥1.58 · seed 4: k≥2.34 · seed 1: k≥4.07**. Seeds 7/8 have no finite *k* inside any plausible round.

**Ranked by expected millimetres at 10 s:**

1. **(ii) Per-iteration cost — the dominant lever, and larger than the spawn tax alone.** Our 8-worker cells sustain **2.04–2.91 M evals/s** (`relocateEconomics.sampleEvaluations / searchSeconds`, all 18 cells); Sparrow on the same box prints **3.742 M evals/s at 460 iter/s** (`sparrow-mixed61/README.md:72-73`) — a **1.3–1.8×** rate gap, with our parallel efficiency at ~48 % (single-worker canary 715 K/s × 8 = 5.72 M ceiling, median sustained 2.73 M). Batch cost is state-dependent: **9.1 K–19.1 K evals/batch** on the welded cells (~245–255 evals/relocate, matching Sparrow's 263/move grain). Every unit of *k* here deepens every seed's cut point down its own deterministic trajectory. At k≈1.3, seed 0 converts (quorum 4); at k≈1.5–1.6, seeds 6 and 5 convert (**quorum 6**). Note the spawn+clone itself (`mod.rs:648-666`: 8 state/descent clones + `scope.spawn` × 8 per batch) is only *part* of the 52 % inefficiency — clone bandwidth, sync and unequal sweep cost sit there too. Which is exactly why the census must precede the build (§Q2a).
2. **(i) Shelf escape-time — the phenomenon the levers act on.** Six seeds burned 809–1,408 batches on the single bite-22 shelf (93 % of seed 0's explore phase) for 0–2 strikes, **never reaching `Struck`**, because one strike-out costs 600 non-improving batches ≈ **6.7–11.5 M evals ≈ 2.6–4.5 s** — half the explore window — so Algorithm 12 fired **zero times at 10 s** on all six (rows confirm `disruptions: 0` everywhere) while firing 2–17 times at 30 s. The mechanism is demonstrated; the window withholds it.
3. **(iv) The 80/20 split — real waste, but arithmetically inert for the quorum.** On the six welded cells, compress consumed ~1.5 s (~20 % of search) to buy **0.090–0.094 mm**. Handing it to explore scales capacity only k≈1.25: seed 0 escapes at ~the last batch and has **no descent time left** (needs 1,930 batches; capacity would be ~1,880); nobody else converts. Reallocating compress buys zero qualifiers. Freeze it.
4. **(iii) The wall-mode lottery — not an independent loss mechanism.** Same binary, same seed, two processes (`control-ab-ba.json` vs `wall.json`): Δ = **+1.90 mm** (seed 3), **−4.08 mm** (seed 4), ≤0.09 mm on the other seven. The variance concentrates exactly on seeds whose escape draw sits within jitter distance of the deadline — it is the cut-point shadow of (i), and it costs gate integrity, not expected depth. Cured by the work-plan, not by an engine change.

**A refinement to the standing regime table:** seed 6 is a third case, not "fast cascade." It crosses the shelf almost immediately (296 batches) but its bar-crossing needs 1,562 explore batches — at 10 s the 80/20 split amputated the last ~491 and its compress phase then spent 626 batches gaining nothing below 169.17. No strike policy targets seed 6; only added capacity or a longer effective explore window does. It is the watch-seed for the capacity clause, not the strike clause.

---

## Q2. What I fund, in order

**(c) first as *design*, (b) as *instrument*, (a) behind a measured gate, (d) frozen.**

- **(c) Escape economics — fund, with the equal-patience derivation.** The honest non-tuning re-derivation is: preserve *Sparrow's patience measured in his own currency*, not our iteration literals. His same-machine print gives 3.742 M evals/s ÷ 460 iter/s ≈ 8.13 K evals per separator iteration, so one of his 200-batch strike cycles ≈ **1.63 M evals** (compress half: **0.815 M**). Re-denominate: debit each batch's full eight-worker `sample_evaluations` on `RawObservation::None`, reset on `Substantial`, pause on `Marginal` (`observe_raw` untouched); strike at quantum, overshoot ≤ 1 batch; counts stay **3 / 5**; improving-strike reset stays 0.98. On our shelf costs that fires at **85–145 of our batches** — a strike-out in ~1.7–2.4 s instead of 2.6–4.5 s, so 2–3 full disruption lifecycles fit in one explore window. Caveat, named and accepted: his 460 iter/s was printed on an easy separate (width ~150.6), so the quantum fires earlier on our heavy shelf than it did for him. That earlier fire *is* the mechanism under test — decided by the paired control, never by the absolute number.
- **(b) plancal-style fixed-work 10 s — fund; it is the round's load-bearing instrument.** Today's `Budget::FixedWork` quotas bites/attempts/per-separation iterations, **not total work** (`mod.rs:1633-1638`) — a hard shelf still consumes unbounded work under it. What's needed is a total-work pacer in the existing `Pacer` shape: currency `U = sample_evaluations + B·batches + E·publication_attempt_calls + R·repair_rows + D·disruption_moves`, weights from timing-only microbenchmarks on all three fixtures, **rejected if wall-prediction error exceeds 10 % on any transfer fixture**; calibration persisted off-line keyed on a deterministic probe (currency version + binary semantic key + workers), read/write separate keys, min-rule write, out-of-band falls back loudly. Compression decay advances by consumed compress-work fraction, never a live clock. Precedent: 60/60 reproducible under load-14 on the old stack (`robust-plan/README.md` §9). This kills caveat-2's lottery and makes every future comparison attribution-clean. Build it **before** quality runs; it changes no semantics at fixed U.
- **(a) Persistent worker pool — measure first, build behind a predicate.** Do not assume the tax: the census must bucket, on frozen easy and hard-shelf states at workers 1/2/4/8 identical fixed work — (a) slot/state/descent preparation, (b) dispatch/join, (c) sweeps, (d) merge+GLS, (e) exact attempts. **Build iff (a)+(b) ≥ 10 % of hard-state wall.** Shape: one local Rayon pool of 8 (already `rayon = "1"` in `Cargo.toml`), persistent preallocated slots, `clone_from` reuse, `pool.install` per tournament — Sparrow's own stated reason is cache/scheduler warmth (`separator.rs:52-57`), which a per-batch `scope.spawn` forfeits. Forbidden by name: global pool, job-owned pool, bare `par_iter`, `find_any`/`find_first`, early cancellation. Merge stays min-guided-Φ stable-by-ordinal collected in ordinal order. **Promote iff** fixed-work bit-identity vs ephemeral executor (winner ordinal, guided bits, fingerprint, per-batch work delta, whole document; ≥1,024 batches incl. a strike, a pool restore, a disruption), p50 ≥1.15× on the mixed-61 shelf, geomean ≥1.10× over three fixtures, no fixture worse than 5 %, RSS +≤10 %. If it doesn't ship, the round loses capacity, not correctness.
- **(d) Compress — freeze entirely this round; shadow counters only** (exact-valid mm/unit per phase; work since last explore publication; unresolved-bite work at the boundary; time-to-first strike/disruption/shelf-publication). My §Q1(iv) arithmetic independently shows reallocation converts nobody, and conditional allocation would need a request-independent trigger that cannot be written without fitting mixed-61.
- **(e) What the rows add that wasn't named:** (1) the seed-6 third regime above; (2) **batch-cost variance is 2.1× across states** (9.1 K–19.1 K), which means any iteration-denominated knob carries that noise across states — a second, independent argument for eval-denominated strikes *and* total-work pacing; (3) the **deterministic-prefix property** is a standing asset: any wall-mode run is a prefix of the fixed-work trajectory, so post-hoc replay diagnosis of any cell is possible without re-running it — worth a driver flag; (4) the `exactAttempts` split (calls vs bites-with-attempt) ships now as counters only, sums reconciling against seed 2's 1,313-behind-174.

---

## Q3. The gates (pre-committed, before any number)

**Mechanical floor first, mandatory, stop-not-report:** the entire rerun Part III floor verbatim (S0 bit-for-bit, S1/triangle locked-T relocate regressions, 1k/10k soundness zeros, four pinned engine gates default + compiled-unarmed + armed, default-build isolation, jagua-rs/Xoshiro/rand:: absent, five suites, two-binary determinism, FAST 13 stages with the **first-bite canary as a stop**).

**Time clause — at the deterministic calibrated-work 10 s-equivalent plan** (bare mixed-61 request, seeds 0..=8, workers=8, quiet box):
PASS iff **all**: ≥**5/9** publish strict non-constructor dual-valid children ≤**168.484**; therefore median ≤168.484; every publication Exclusive r=2.500 allowance 0 + untouched contract validator; **each seed bit-identical across two processes**; end-to-end quiet-box wall p95 ≤ 10.000 s over ≥5 repetitions × 9 seeds (wall reported as the distribution it remains — quality is deterministic, wall is not); **paired equal-work control mandatory**: treatment vs the frozen 200/3/100/5/0.98 arm under the identical plan must gain ≥2 qualifying seeds or ≥1.000 mm paired median, else the absolute result is a draw, not a policy win. Per-seed paired table reported; seed 5 is the named watch (its escape used 0 strikes — a work-strike that kills a continued-search escape is the regression the median could hide).

**One dissent I want voted on.** Grok review 14 withdraws any 4/9 fallback as a pass clause. I hold a narrow version of it: **if the persistent executor fails its own mechanical build-or-promote predicates (measured before any quality number exists), the time clause reads ≥4/9 — by this sentence, now, not retroactively.** Rationale from the draws: 5/9 requires converting three of {0, 6, 5, 4} at k≈1.3–2.3; without the executor's capacity those conversions lean entirely on strike-policy redirection, and my honest expectation there is 3–4 qualifiers. A bar you privately expect to fail burns a funded round to manufacture a second kill; a contingency keyed to a *mechanical event* is still pre-commitment. If the vote goes against me, I sign 5/9 flat and report 4/9 as the failure it would be.

**mm clause:** 30 s cell under the same plan: median ≤ **163.00461** (current median 164.00461, minus 1.000 — an asymptote-probing ask, deliberately ≈1 σ of the escaper spread); ≥**7/9** ≤168.484 as a no-regression floor (already true — it gates only regression); paired median improvement ≥1.000 mm over the control; zero invalid publications; no shapes-17/triangle-20 median regression >1 mm at equal work. Add a nine-seed **60 s diagnostic and time-to-depth survival curves, reported not gated** — the survival curves are what make the asymptote legible.

**150.165 stays the horizon, not a clause.** Independent grounds: our best depth at *any* budget ever is 161.05 (30 s, seed 0) — still 10.88 mm away; Sparrow's number is constructor-free while ours charges a 2.31 s constructor against the wall (arbitration 3, documented asymmetry); no measured path connects any funded lever here to that number. Gating on it would be fitting to an external scalar.

---

## Q4. Frozen verbatim (cannot creep in)

Relocate operator (25+50 samples, 16 angles, 3 finalists, coordinate descent, accept-equal, no leftover-after-before); GLS multipliers and update schedule; disruption semantics, follower witness, pool selection; **0.1 % explore bite size**; compress range/cut semantics/**80-20 share**; workers=8; constructor and start state; publication band, repair cap, Exclusive kernel, contract validator; `observe_raw`'s Substantial/Marginal/None classification and the 0.98; seeds 0..=8; **bar 168.484 absolute** (the control neither rescues nor kills); no new operators, no chain/two-endpoint member, no joint-PGS/component-Y, no pole proxy/simplification/Xoshiro, no Sparrow or jagua code in the tree, no seed substitution or outcome-selected reruns. The failure-license discipline itself stays: a new line-level defect found mid-round is *reported* and repaired only under its own red/green license.

## Q5. Workflow, FAST, pre-named defects

**Roles/waves (4 roles, 3 waves):** (1) spec/profile agent commits formulas, thresholds, timing census — no engine edits; (2) parallel: executor agent owns the persistent-executor module + equivalence tests, meter agent owns strike-work debit + total-work pacer primitives — **neither touches `mod.rs`**; (3) one integration agent, sole owner of `mod.rs`/Pacer wiring/schema; (4) evidence agent runs drivers and commits evidence, edits no engine code after the gate text freezes.

**FAST additions:** ephemeral↔persistent K=1,024 bit-identity on a hard shelf incl. strike/restore/disruption; forced scheduling-order perturbation with ordinal merge intact; **batch-two-delta vector** (batch 2's aggregate equals the sum of the eight batch-two deltas, not cumulative slot totals); strike meter with variable batch costs and overshoot ≤1 batch; calibrated-plan hit/miss/version/out-of-band/clock-poison tests; eight-worker hard-shelf throughput (retire the single-worker canary as the only throughput signal); attempt-counter reconciliation; existing S0/first-bite/default-build isolation.

**Most likely implementation defect, pre-named and ranked:** (1) **persistent-slot leakage / double-debit** — a reused non-winner retaining poses, GLS weights, descent stream, or an un-zeroed `WorkVector` (`mod.rs:680` consumes fresh vectors today); it would make work-strike and plancal accounting *stable but false*, which is the worst failure class this round has; (2) completion-order leaking into winner/work-debit (tripwire: the perturbation vector; fallback: eight parked OS threads + barrier); (3) total-work overshoot past one batch or a live clock influencing a hit-trajectory — breaks the two-process identity clause; (4) currency weights mispriced >10 % on a transfer fixture — depth stays deterministic while the wall promise silently dies (caught only by the reject rule; do not skip it).

---

**Position summary for the vote:** fund work-denominated strikes (quantum 1.63 M / 815 K, counts 3/5, paired control mandatory), the total-work calibrated plan, and the executor behind the 10 % census gate; freeze compress and everything in Q4; gate at ≥5/9 deterministic-10 s + 30 s median ≤163.00461, with my narrow mechanically-triggered 4/9 contingency offered for vote and droppable without affecting the rest.


# CLOSING VOTE

YES. My funded position was work-denominated strikes shipped with a paired control mandatory; §Vote-1 delivers both, and strengthens the control from observational to a full arm — Sol's frozen literals 200/3/100/5/0.98 running on the identical executor and pacer, so strike semantics are the only delta between arms, which is the cleanest attribution instrument available. The promotion clause also closes the one hole I'd otherwise flag: a treatment clearing the absolute 5/9 while failing attribution cannot be promoted, because unattributed gain might belong to noise or the executor rather than the impatient policy; reverting the member to Sol's policy on failure settles a semantic disagreement by measurement instead of by vote count, and the no-second-guess clause on 1_630_000/815_000 prevents post-hoc retuning of the quanta against the nine fixtures. Honest labeling preserves the distinction Grok verified number-by-number (Sparrow's counter wraps iterations; the derived quanta denominate work). Nothing here contradicts any item I signed; it buys falsifiability at no extra scope, and the draw-on-attribution-failure outcome is conservative in exactly the right direction.

— ox-alpha
