# Grok review 14 — the economics round (2026-08-23, three rounds)

Same quorum, Grok's side. Round 1 = independent design (froze the strike
literals); round 2 = after reading Sol, VERIFIED Sol's quantum derivation
number-by-number and signed it as the honestly-labeled treatment with the
paired control; round 3 = the closing vote. The two consultants swapped
positions across the exchange — the two-arm structure is what both could
sign. Verbatim.

# ROUND 1

## 1. Diagnosis, ranked by expected millimetres at 10 s

The 10 s distribution is **three regimes**, not one 7/9-at-179 lump.

| regime | seeds (this draw) | 10 s depth | bite 22 | what 30 s does |
|---|---|---|---|---|
| **welded at the 179 shelf** | 0, 1, 4, 5, 7, 8 | 179.07–179.08 | unpublished, `disruptions: 0` | 0, 1, 4, 5 publish and go to 161–165; **7 and 8 never leave** (5 disruptions, 5.6–6.5 k iters) |
| **escaped, compress crosses the bar** | 2, 3 | 167.95 / 167.32 | published (1283/3/1 and 137/0/0) | explore already under the bar by bite 82–83 |
| **escaped, second shelf at ~169.8** | 6 | 169.17 | published in 131 iters, 0 strikes | explore to 164.62 |

Control arm A (same binary, same seeds, different process) splits the lottery from the shelf:

| seed | wall 10 s | control A | |
|---|---:|---:|---|
| 0, 1, 5, 7, 8 | 179.07–179.08 | **identical** | not a lottery |
| 2 | 167.952 | 167.919 | stable qualifier |
| 3 | **167.315** | **169.212** | lottery across the bar |
| 4 | 179.081 | **175.005** | lottery on *whether bite 22 publishes at all* |
| 6 | 169.172 | 169.081 | stable miss, 0.7 mm |

Control quorum is **1/9**. Wall quorum is **2/9**. Expected 10 s qualification on this binary is about **1.5/9**. “2/9 is a near miss” is not supported.

### Rank 1 — (i) escape time at the 179 shelf

**This is the millimetres.** Six of nine seeds are 11 mm above the bar because bite 22 has not published when explore dies, and compress then runs from the bite-21 parent at ~179.17.

The rows name the mechanism more tightly than “needs more separations”:

- **Four of the six** sit at **exactly 2 strikes, 0 disruptions** (seeds 0, 4, 7, 8). Algorithm 12 is one strike short of firing. Three strikes need ≥600 non-improving master iterations; they have 893–1408 total on that bite, but improving/marginal windows ate the rest, and the explore deadline arrives first.
- **Seed 0 is 16 iterations from publishing without disruption.** 10 s: 1408 iters, 2 strikes, unpublished. 30 s, same seed, separate process: **1424 iters, 2 strikes, 0 disruptions, published.** That is ~70 ms at seed 0’s 4.36 ms/iter. After that publish, 30 s then cascades to 161.
- **Seed 5 does not need disruption at all.** 30 s publishes bite 22 at **1142 iters, 0 strikes, 0 disruptions**. 10 s had 854. Shortfall **288 iters ≈ 1.87 s** — almost exactly the compress window.
- **Seed 1 is not a 10 s conversion.** 30 s needed 3059 iters and 2 disruptions. 10 s had 809.
- **Seeds 7 and 8 are a different basin.** 30 s: 17/5 and 15/5 strikes/disruptions, still unpublished, still 179. Round 2 will not bring them under 168.484 at 10 s. Do not write a 10 s clause that requires 7/9.

Post-escape economics, also from the rows, not from theory: **10 s explore never reaches 168.484 even on the seeds that escape.** A 0.1 % cascade from ~178.99 to 168.484 is ~61 bites. Seed 2 had 132 post-bite-22 explore iters and ended at **171.78**; seed 3 had 629 and ended at **170.22**. Compress then dropped **3.83 mm and 2.91 mm** and that is why they qualified. Seed 6 had 843 post-22 explore iters, hit a **second unpublished bite at 169.82 with 4 disruptions / 5 attempts / 252 iters**, and compress only added 0.65 mm.

So (i) is two sequential millimetre events: **leave 179**, then **have enough remaining schedule (explore cascade and/or compress) to cover the last 3–11 mm**. Converting a welded seed without leaving time after the publish lands at 175–179, which is seed 4’s control draw.

**Expected 10 s mm if bite 22 publishes with ~1.5 s left:** ~8–11 mm (179 → 168-class), i.e. a qualifier. If it publishes with ~0 s left: ~0.1 mm (one compress consolation), still 179-class.

### Rank 2 — (iv) the 80/20 split, but not the way the question frames it

**Reject “earlier compress on stuck seeds.”** Compress on the six welded 10 s cells is one published 0.05 % bite (~0.09 mm, 2–7 iters, from the bite-21 parent) and one failed child at ~178.99 — the same shelf, **uniform-Y cut, 100/5 limits, no disrupt**. That is the 1.5 s being burned.

**Do not flip the split to 100 % explore either.** On the two qualifiers, compress *is* the bar-crossing phase at 10 s (+2.9 / +3.8 mm). At 30 s the opposite is true: six of seven escapees first go under 168.484 on **explore bite 82–83**; compress is a 0.03–1.17 mm polish except seed 1 (4.56 mm from 169.56). The owner’s “at 30 s compress digs 168→161–164” is true of the *band*, not of the crossing: explore already carried them through 168.

The honest schedule fact: **80/20 is correct for a seed that has left 179, and wasted for a seed that has not, and naive “stay in explore if unpublished” would steal compress from seeds 2 and 3** (their last explore bite is unpublished because the clock hit 2 iterations into the next 0.1 %, strikes 0, disruptions 0). Rank (iv) as a **conditional 1.5 s**, not as a retune of 0.8.

### Rank 3 — (ii) spawn/clone tax, unmeasured, estimated 8–20 % of barrier-to-barrier

Iteration counts vs search wall, 10 s:

| seed | master iters | ms/iter | sample evals / s | bite 22 |
|---|---:|---:|---:|---|
| 2 (escaped, qualifier) | 2035 | 3.77 | 2.43 M | published |
| 0 (16 short) | 1761 | 4.36 | 2.57 M | unpublished |
| 6 (second shelf) | 1697 | 4.53 | 2.45 M | published |
| 4 | 1399 | 5.50 | 2.73 M | unpublished |
| 3 (qualifier) | 1202 | 6.40 | 2.09 M | published |
| 1, 5, 7, 8 | 1163–1203 | 6.38–6.62 | 2.74–2.91 M | unpublished |

~1200–2000 eight-worker tournaments in ~7.67 s of search. Each one is `std::thread::scope` + **eight `IcsState` clones** + eight joins (`mod.rs:662`). Sparrow’s `separator.rs:52` keeps a persistent Rayon pool; they still `worker.load(&master_sol, &ct)` every iteration (`separator.rs:153`), so **clone-every-iter is shared, spawn-every-iter is not**.

I will not pretend the percentage is measured. A back-of-envelope (8 sequential `pthread_create`s + 8 deep clones of mixed-61 geometry/rows) is **0.4–0.9 ms of a 4.4–6.6 ms barrier**, roughly **8–20 %**. That is:

- seed 0’s 16-iter gap, by itself;
- a plausible 3rd strike on the 2-strike cells if the tax sits at the high end;
- **not** seed 1’s 2250-iter hole, and **not** seeds 7/8.

`ms/iter` varies 1.76× across seeds; `sampleEvaluations` per search-second is tighter (2.1–2.9 M/s). Work currency for a deterministic gate should not be wall, and should not be calibrated on the cheap first 21 bites.

**Rank (ii) as the time-axis implementation of (i), not as a separate millimetre source.** Killing a 3 % tax to save seed 0 is not a round. Killing a 15 % tax that lets disrupt fire on {0,4} and buys seed 5’s 288 iters is the round.

### Rank 4 — (iii) wall-mode lottery

Measured, not inferred: seed 3 flips 1.90 mm across the bar; seed 4 flips 4.08 mm from welded-179 to 175 (still not a qualifier); five welded seeds are bit-stable at 179. Lottery **does not create the 7/9-at-179 outcome**. It decides whether a *post-escape* seed is 167 or 169, and whether seed 4’s 2-strike cell completes. That is why a wall 10 s ≥*n*/9 clause is not a gate. It is why (b) exists. Millimetres-at-10 s from killing the lottery: **~0 expected, large variance**. Gate-credibility: first.

---

## 2. What to fund, in order, with pre-committed gates

This is a **cost-and-schedule** round on the frozen member, not a new operator.

### Wave 0 (same day, no semantic engine change) — measure, then decide (a)

**Fund the measurement. Do not just build the pool.** Sol 18 recorded the tax as unmeasured and forbade folding it into the strike rerun. That rerun is over. The cell is now due.

FAST cell, 200 master iterations, eight workers, **at the 179-shelf density** (not bites 1–21): report `clone_ns / spawn_join_ns / sweep_ns / gls_ns / barrier_to_barrier_ns` as fractions, two processes, quiet box, SHA recorded.

Pre-committed build rule, written before the number:

| measured fraction of barrier-to-barrier | what wave 1 builds |
|---|---|
| spawn+join < 0.08 **and** clone < 0.15 | **skip (a)**; spend the round on (b) and the (d) exception |
| spawn+join ≥ 0.08 | persistent **eight OS threads**, spawned once per `run_cutclose`, barrier, **ordinal join**, no Rayon |
| clone ≥ 0.15 (additional) | persist worker *slots* (`clone_from` into preallocated `IcsState`s). Not in-place mutation, not COW |

No Rayon in `overlap_ics`. Sparrow’s pool is a cited difference; importing Rayon is a new dependency and Rayon’s completion order is the determinism bug this member already pre-named. Eight parked threads + condvar/barrier is the whole of (a). Workers still receive a copy of the master each iteration — that is also what `worker.load` does.

### (b) plancal-style fixed-work 10 s — always fund, first-class, not optional

The campaign already has: `Budget::FixedWork`, two-process replay of wall ordinals (`replayAllBitIdentical: true` on all nine), and the `plancal` protocol (probe keyed on a **counter**, persisted per box, 60/60 under load on the old stack).

**The 10 s PASS/FAIL clock of round 2 is work, not wall.** Wall 3/10/30 remains the reported curve and cannot pass or fail.

Calibration, pre-committed:

- Currency: **master iterations of the eight-worker tournament** as the cap the pacer uses, with `sampleEvaluations` recorded beside it (tighter across seeds, but strikes are denominated in iterations).
- Probe: **400 master iterations at the 179 shelf** (constructor + 21 published 0.1 % bites, then a fixed-work separate at that `W`), not from `D*`. Calibrating on bites 1–21 will overstate iters/s by ~1.5–2× and recreate 7/9-at-179 *inside* “deterministic 10 s”.
- Persist `icscal=<path>` keyed on `probe_iterations / probe_seconds`, headroom 0.97, same shape as `plancal`. Constructor still runs fully (deterministic, charged, uncapped). The cap is search work for the remainder of a 10.000 s quiet-box budget.
- Two-process identity of the work-capped trajectory is a FAST stop, not a report.

This is the time axis as *readability* (the lottery dies) and the only way a ≥5/9 clause is a clause.

### (c) strike limits — **do not retune 200/3, 100/5, 0.98**

The honest equal-work statement: **200 master iterations of an eight-worker tournament already is Sparrow’s work unit.** Their `iter_no_imprv_limit = 200` counts `move_items_multi` calls, not milliseconds. Converting 200 × (their ms/iter) / (our ms/iter) into a smaller integer is **equal-wall**, and equal-wall after seeing 2/9 is tuning-to-pass. It also makes the separator *more impatient* in search work than the paper, which is a different member.

A conversion through measured evals/iter would require a Sparrow run on mixed-61. That binary is out of tree; linking it is a forbidden rescue. An optional **read-only** diagnostic against `/var/lib/t3/tmp/sparrow-bench` may be recorded as a citation. It does not license a knob change in the same round.

If wave 0 + (a) still cannot complete 3×200 inside the explore window, the round **reports that as a remaining fact**. It does not quietly drop 200 to 80. A later “impatient strike” member can be funded separately, with its own gate.

What *is* in scope for (c) is not a new number: it is **making 200/3 actually fire inside our 6.14 s explore**, which is (a) plus the (d) exception.

### (d) compress-phase design — one exception, not a new 80/20

Keep `EXPLORE_TIME_RATIO = 0.8` and TimeBased `(0.0005, 0.00001)` verbatim.

**Fund one schedule exception, named before the wall:**

> If explore is inside an **unpublished** bite **and** that bite already has `masterIterations ≥ 200` or `strikes ≥ 1` or `disruptions ≥ 1`, do not start compress. Continue explore until the bite publishes, or until the **full** remaining wall. Compress still runs from the last dual-valid parent on whatever time is left.

That is “Algorithm 12 gets a chance before we spend 20 % retrying the same shelf without it.” It is not a 0.8 fitted to mixed-61.

The 200 threshold is the existing strike unit, and it is what **protects seeds 2 and 3**: their last unpublished explore bites are 2 iterations, 0 strikes, 0 disruptions — they still compress, and that compress is why they are under 168.484.

FAST vector, mandatory if (d) ships: a fixture whose last explore bite is unpublished at 2 iterations **must still enter compress**. Pre-named defect of (d): escape-priority eats the 20 % on a cascade seed and 10 s lands at 171.

**Do not** enter compress *earlier* on stuck seeds.

### (e) what the rows name that the question did not

1. **Seed 0’s 16-iteration gap** — the existence proof that a small tax is millimetres for at least one seed, and that “2 strikes” is not always “needs disrupt.”
2. **The 2-strike pile-up** on {0, 4, 7, 8} at 10 s vs 5 disruptions on {7, 8} at 30 s still unpublished — disrupt is necessary and not sufficient.
3. **Seed 6’s second shelf at 169.82** (4 disruptions, 252 iters, unpublished) is the 0.69 mm miss, not leftover 179. The (d) exception applies here too.
4. **Seeds 2, 3, 6 have identical bite-22 rows at 10 s and 30 s** (1283/3/1, 137/0/0, 131/0/0). Bite 22 on those seeds is not a lottery; the lottery is *after* escape (seed 3) or on the 2-strike cells (seed 4).
5. **10 s bar-crossing is compress; 30 s bar-crossing is explore bite 82–83.** Any schedule change has to preserve the first and not assume the second will happen in 10 s.
6. **`exactAttempted` still counts bites-with-attempts.** Split the counter this round if instrumentation is open. It is not a trajectory change and it is still deferred if it would dirty the wall schema comparison; I would split it, with both names in the document.
7. Constructor 2.31–2.35 s is 23 % of 10 s and stays charged, uncompensated (arbitration 3). Out of scope.

**Order:** wave 0 measure → (b) always → (a) iff the threshold fires → (d) exception with its tripwire → (c) numbers frozen. Do not land (a) and a 200-retune in one diff.

---

## 3. The gates (pre-committed reading for round 2)

New funding, new §0, committed before any wall or work-cap second. The closed member’s 3/9 at wall 10 s is **not** this round’s judge.

**Round validity.** FAST fully green, including the new cells in §5. One release binary, feature `overlap-ics`, SHA in every document. Bare mixed-61. Eight workers. Seeds 0..=8. Constructor charged, uncapped. Floor unchanged from the rerun §7 (S0 bit-for-bit, 1k/10k zeros, throughput literals, four pinned gates on default and feature-compiled-unarmed, default-build isolation, `jagua-rs`/`Xoshiro`/`rand::` absent, first-bite canary still licenses the battery). Any invalid publication, any seed, any budget: **FAIL**, defect hunt, not a retarget.

**Time clause (the 10 s axis).** PASS iff **≥5 of 9** seeds publish a strict non-constructor dual-valid child with raw-source depth **≤ 168.484 mm** at the **calibrated work-cap 10 s** of (b). Wall 10 s is reported and **cannot pass or fail**. 5, not 3: 3 is a lottery we have already drawn at 1–2/9; 30 s shows seven seeds *can* leave 179; the named 10 s conversions are {2, 3} plus {0, 5, 6}. 7/9 at 10 s would require {1, 4, 7, 8}; the 30 s rows refuse that.

If wave 0 skips (a) and (d) does not ship, **drop the time clause to ≥4/9** rather than pretend 5 is available from (b) alone. Do not keep 5 and skip the mechanisms that buy it.

**Millimetre clause (the 30 s axis).** Separate work-cap or wall 30 s, all nine, non-interpolated:

- median of 9 **≤ 163.00 mm** (current median **164.00**; 1 mm is a real move from more 10 s-rate work spilling into 30 s, not a hold);
- **≥7/9 ≤ 168.484** (already true; a regression here is a defect in this round);
- best is reported, not a clause (current 161.05).

**60 s.** All nine, reported, **not a clause**. It is the asymptote read: do 7/8 ever leave 179, and is 161 still descending. Without it we will argue about 150.165 from 30 s tea leaves.

**150.165 does not enter any clause.** It remains the horizon. Best 30 s is 10.9 mm from it; seeds 7/8 have not left the constructor’s first shelf; Sparrow’s 150.165 is constructor-free on a different start. Putting it in a gate is how a funded cost-and-schedule round becomes a failed different member.

**Control.** Interleaved AB/BA stays diagnostic. 168.484 stays absolute.

**Failure license for *this* round.** One named line-level defect with a red/green vector → one repair, one rerun of *these* clauses, then stop. A valid round that misses ≥5/9 at the work-cap, or misses the 30 s median, **closes this funding**; it does not license bite-size search, GLS retune, worker-count change, or a new operator.

---

## 4. Scope discipline — explicitly not in round 2

Frozen verbatim, and a diff that touches them is a different proposal:

- Relocate: 25 focused + 50 container, 16 orientations + CD wiggle, 3 finalists, two-stage axis CD, accept-equal, no leftover `after < before`, no `ladder_top` cap.
- GLS: 1.2 + 0.8·(v/v_max) active, 0.95 inactive, floor 1.0, all rows every master iteration.
- Disruption: Alg. 12 fail path only, 75 % hull-area cutoff, guaranteed interior witness, cap n.
- Eight workers, ordinal merge, equal work, no early cancel.
- Explore bite 0.1 %, centre cut, far-side `ty` only; compress TimeBased 0.0005→0.00001, uniform-Y cut.
- `EXPLORE_TIME_RATIO = 0.8` as the default (the (d) exception is the only schedule delta).
- Strike literals **200 / 3 / 100 / 5 / 0.98**.
- Publication: Exclusive `r = 2.500`, allowance 0, untouched contract validator, repair ≤ 16 µm, dual-valid to advance `W`.
- Constructor configuration, charged, no internal wall cap.
- Pool-restore weight policy (declared frozen difference vs Sparrow’s tracker rebuild).
- Worker count. Publication band. `proxy_depth > T` semantics.

**Not in the round at all:** new operators (joint PGS, component-Y, chain, homotopy bisection, jump); bite-size changes; GLS changes; 8 workers re-read as a rescue; v3 / m34 / old stack as a lane; jagua / Xoshiro / pole proxy / simplification; Sparrow fixture as a seed; 150.165 as a clause; fitting 0.8 or 200 to mixed-61 / 168.484; in-place shared-state workers; Rayon.

**Allowed and small:** split `exactAttempted` (instrumentation); the spawn/clone counters the FAST cell needs; `icscal` file format.

---

## 5. Workflow, FAST additions, pre-named defect

**Agents.**

| wave | owner | ships |
|---|---|---|
| 0 | evidence | spawn/clone/sweep FAST cell; 179-shelf probe; `icscal` write path; optional out-of-tree Sparrow iter/s citation |
| 1 | core | persistent eight threads **iff** wave 0 threshold; ordinal merge unchanged; unit vector: two-process fingerprints identical to `thread::scope` on a short fixed-work cell |
| 1b | schedule | pacer reads `icscal`; (d) exception behind the same `ScheduleConfig`; no other homotopy edit |
| 2 | evidence / red-team | FAST union below; then work-cap 10 s nine-seed gate; wall 3/10/30 reported; 60 s diagnostic; AB/BA; HEAVY floor |

Spec commit first (this consultation + the other consultant’s + the new §0), then wave 0, then 1/1b, then 2. Wave 1 does not start until the spawn/clone document exists.

**FAST additions** (keep the existing 13, including first-bite canary as the wall/work-cap license):

1. **`spawn-tax`** — 200 iters at 179-density; fractions committed; this is the (a) go/no-go.
2. **Pool merge, two-process** — if (a) ships: each worker seed, each master snapshot, winning ordinal, pose+weight fingerprint after every master iteration, two processes. Completion order must be invisible. If (a) skipped, this stage is N/A and FAST stays 13+the others.
3. **Work-cap identity** — calibrated 10 s, two processes, stripped documents identical. FAIL here is a member fail; do not run the nine-seed battery.
4. **Compress-entry tripwire** — if (d) ships: unpublished last-explore at 2 iterations still compresses; unpublished last-explore at ≥200 iterations does not.
5. **Bite-22 strike progress** — fixed-work 800-iter separate at `W ≈ 179`, seed 0: `strikes ≥ 1`. Locks the 2 % predicate so it cannot silently regress while we move the pool.

**Most likely implementation defect of round 2, pre-named:** **completion-order leak in the persistent pool** — merge by who finished, or a channel that reorders workers, or Rayon `min_by_key` without the ordinal. The current `thread::scope` joins in ordinal order on purpose (`mod.rs:672-676`). Reused threads plus a queue are how that sentence dies. Two-process merge FAST is the tripwire; a unit test in one address space is not enough.

**Second, almost as likely:** **`icscal` probe run on bites 1–21.** Work-cap then looks deterministic and still welds six seeds at 179 because the budget was bought with cheap iterations.

**Third, if (d) ships:** **escape-priority steals compress from a cascade seed** (the 2-iter unpublished case). The tripwire in FAST addition 4 is not optional.

---

**What I would sign.** Wave 0 measure; (b) always; (a) iff ≥8 % spawn or ≥15 % clone; (d) as the unpublished-and-already-striking exception only; 200/3 frozen; ≥5/9 at *work-cap* 10 s; 30 s median ≤ 163.00 and ≥7/9 ≤ 168.484; 60 s reported; 150.165 still the horizon; operator family frozen verbatim. That is both of the owner’s axes. It is not a new homotopy, and it does not pretend the closed 3/9 wall-10 s member still has a license.


# ROUND 2 — exchange

**I sign the economics round.** The closed 2/9 stays closed. This is a newly authorized equal-work round, not a reopening of `CutCloseRelocate`. Durable text: [`docs/grok-review-14-the-economics-round.md`](docs/grok-review-14-the-economics-round.md).

Sol’s headline numbers check out against `wall.json` and Sparrow `separator.rs` at `14f4868f`: 11 203–19 131 evals/iteration on the stuck 10 s cells, 1.63 M / 85–145 batches, constructor 2.31–2.35 s, search-phase median 2.73 M evals/s, ~48 % parallel efficiency, compress 0.0896–0.0935 mm on the shelf vs 0.65 / 2.91 / 3.83 mm after escape.

---

## CONVERGE

| # | Vote |
|---|---|
| **1. Strikes** | **DISAGREE** that 1.63 M is the *same frozen iteration semantics*. **AGREE** it is the honest equal-work reading of Sparrow’s patience on this machine, not a mixed-61 fit. **SIGN** the paired control as mandatory. |
| **2. Substrate** | **Rayon local pool.** Two-process merge-identity is the tripwire. Parked OS threads are the FAST-failure fallback, not a second design. |
| **3. Executor gate** | **Merge.** Build iff prep+dispatch ≥ 10 % of hard-state wall. Grok’s 8 %/15 % split is withdrawn as a predicate (kept as diagnostic columns). Promotion = Sol’s 1.15× / 1.10× / ≤5 % / ≤10 % RSS / bit-identity. |
| **4. Compress** | **Sol.** Full freeze + shadow counters. Grok’s §2(d) remainder-handoff is withdrawn. |
| **5. Gates** | **Union, one refusal:** 5/9 does **not** drop to 4/9 if the executor is skipped. |
| **6. Facts** | **Fold all of them.** Seed 7/8 may not be required. Seed 5 is a continued-search escape, not strike-starvation. |
| **7. Workflow** | **Sol’s 4 roles / 3 waves.** FAST union + both defect names + Sol’s batch-two-delta vector. |

### Vote 1, in full

200 master batches is what the closed member froze. 1.63 M sample-evaluations is not that sentence. I will not sign “200 always meant 1.63 M.”

It is also not a retune. The quantum is Sparrow’s same-machine `3.742e6 / 460 × 200`, named 1.63 M; compress is half; counts stay 3/5; `observe_raw`’s 2 % Substantial / Marginal / None rule does not move. None-batches debit all eight workers, which is already how `WorkVector` charges and how Sparrow sums `SepStats`. Per-move currencies match (Sparrow 263 evals/move; our wall 245–254).

The one caveat, not a veto: those 3742 K / 460 were printed on the first explore-failure separate at 150.646 mm in 0.9 s, not on a 179 mm jam. Our shelf iterations are heavier, so 1.63 M fires at 85–145 of *our* batches. That earlier fire is the mechanism under test. The paired control is what decides whether the millimetres are from it.

Control arm = frozen 200/3/100/5/0.98, identical executor and calibrated plan. Treatment must gain **≥ 2 qualifying seeds or ≥ 1.000 mm paired median**, else an absolute 5/9 is a draw. Seed 5 is the watch: 854 / 0 strikes / unpublished at 10 s → 1 142 / 0 / published at 30 s. A work-strike that disrupts that path is a regression the median can hide.

**KNOB, frozen here:** `1_630_000` / `815_000`. Not a more “precise” 1 626 957 — 3742 K is already a truncated `usize` from `f32`. No second guess if attribution fails.

---

## The spec we both sign

**Name.** `CutCloseRelocate` economics round. Fund three changes only: work-denominated strikes, a persistent executor if profiling clears its gate, a persisted calibrated-work pacer. Freeze the operator and 80/20 compression.

**Frozen.** Relocate 25+50 / 16 angles / 3 finalists / CD / accept-equal; disruption and pool; GLS; 0.1 % explore bites; compress range and 80/20; workers=8; constructor; publication band / Exclusive r=2.500 / validator; `observe_raw`’s 2 % classifier. Executor and planner must be trajectory-preserving at fixed work.

**Strikes (the one quality-semantic change).** After each completed master batch, classify with `observe_raw`: Substantial resets accumulated None-work; Marginal updates the snapshot and adds nothing; None adds that batch’s all-eight-workers `sample_evaluations`. Strike at the quantum; overshoot ≤ one batch. Outer 2 % improving-strike reset unchanged. Strikes stay local to one `separate`. An already-publishable entry still publishes before any sweep.

**Executor.** Profile easy and bite-22 hard states, workers 1/2/4/8, identical fixed work; time prep, dispatch/join, sweeps, merge+GLS, exact/repair. **Build iff** prep+dispatch ≥ 10 % of hard-state wall. **Promote iff** bit-identical vs ephemeral for ≥ 1 024 batches (including strike, pool restore, disruption), ≥ 1.15× shelf p50, ≥ 1.10× geomean over mixed-61 / shapes-17 / triangle-20, ≤ 5 % any-fixture regression, ≤ 10 % RSS. Implementation: local Rayon pool of 8, persistent slots, `clone_from`, ordinal merge. Forbidden: global Rayon, job pool, bare `par_iter`, `find_any` / `find_first`, early cancel. If two-process merge-identity is red, fall back to eight parked OS threads + barrier; do not ship a completion-order winner. If the executor is not built, 5/9 does not drop.

**Pacer (after executor and strike freeze).** `Budget::FixedWork` is not enough. Currency

```text
U = sample_evaluations
  + B × master_batches
  + E × actual_publication_attempt_calls
  + R × repair_rows
  + D × disruption_moves
```

`B/E/R/D` from timing-only microbenchmarks on all three fixtures, conservative rounding; reject if wall prediction error > 10 % on any transfer fixture. File pins request hash, currency version, binary/feature key, workers=8, executor implementation, per-phase safe units/s. Read/write keys stay separate. No live probe on a gated trajectory. Allocate 80/20 by calibrated units; compress decay follows consumed compress-work, not a clock. Stop only between master batches. Quality is deterministic; wall remains a distribution.

**Compress.** Frozen. Shadow counters only (mm/unit per phase; work since last explore publication; unresolved-bite work at the phase boundary; time/work to first strike, disruption, shelf publication). No mixed-61 readiness threshold.

**§0 — 10 s calibrated-work, seeds 0–8, workers=8, quiet box. PASS iff all hold:**

1. ≥ 5/9 exact-valid ≤ 168.484 mm
2. median ≤ 168.484 mm
3. every publication Exclusive r=2.500, contract-valid
4. each seed bit-identical across two processes
5. quiet-box p95 ≤ 10.000 s over 5×9
6. paired control: ≥ 2 extra qualifying seeds **or** ≥ 1.000 mm paired median

The 4/9 fallback is not a pass. Controlled-load reports overruns; it cannot promise a wall bound without the excluded governor.

**30 s:** median ≤ 163.00461 mm; ≥ 7/9 ≤ 168.484 mm (no-regression; already true); paired median ≥ 1.000 mm; no shapes-17 or triangle-20 median regression > 1 mm at equal work; zero invalid publications.

**60 s reported, not gated. 150.165 is the horizon, not a clause.**

**Three regimes (no clause may require 7/8):**

| regime | seeds | what the round may claim |
|---|---|---|
| Fast cascade | 2, 3; 6 near | already across bite 22 |
| Strike-starved shelf | 0, 1, 4, 5, 7, 8 at 10 s | work-strikes target 0, 1, 4. Seed 5 needs ~288 more continued-search iterations, not a strike |
| Different basin | 7, 8 | five disruptions at 30 s, still unpublished |

**Workflow.** (1) spec/profile census, no quality edit; (2) executor agent ∥ meter agent, neither owns `mod.rs`; (3) one integration agent owns `mod.rs` / Pacer / schema; (4) evidence runs drivers and does not edit the engine after the gate text is frozen.

**FAST union.** Existing stages, plus: K=1 024 ephemeral/persistent identity; scheduling-order perturbation; batch-two-delta accounting (batch two’s aggregate = sum of eight *batch-two* deltas, not cumulative slot totals); strike meter with variable batch costs; calibrated-plan hit/miss/version/clock-poison; eight-worker hard-shelf throughput; actual-attempt vs bites-with-attempt reconciliation.

**Pre-named defects.** Completion-order leak; persistent-slot leakage / double-debit (`mod.rs:680` currently consumes fresh vectors — a persistent vector not zeroed makes work-strike “stable” but false); probe-on-cheap-bites; compress-steal.

---

Remaining refusal, if Sol still holds it: **4/9 as a pass**, and **any claim that 1.63 M is the same frozen iteration semantics**. Everything above is the joint spec.


# ROUND 3 — the closing vote

**YES.**

This is the structure I already signed in review 14 §1 / S2: treatment is the work-denominated impatient policy (quanta frozen at `1_630_000` / `815_000`, counts 3/5, `observe_raw`’s 2% classifier untouched), labeled as distinct from the closed member’s iteration semantics — I still will not sign any sentence that says 200 always meant 1.63M — and Sol’s literals 200/3/100/5/0.98 stay alive as the equal-work control under the same executor and pacer, with promotion only on ≥ 2 qualifying seeds or ≥ 1.000 mm paired median, else an absolute 5/9 is a draw and the control remains the member, with no second guess on the quanta.

Signed: Grok, economics-round closing vote, 2026-08-23.
