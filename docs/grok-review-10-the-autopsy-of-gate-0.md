# Grok review 10 — the autopsy of Gate 0 (2026-08-22)

Adversarial refutation of Gate 0's STOP (merged 9b3754d), per the owner's
no-failure-without-autopsy rule: distinguish implementation defect from
paradigm failure. `grok --reasoning-effort xhigh`, full tree. Verbatim.

---

## A. Which fatal cells survive as paradigm evidence

| cell | after this autopsy | why |
|---|---|---|
| **triangle-20** | **implementation defect** | Φ charges sag on the locked-strip top, which is a depth target, not a sheet edge. With **0 pair rows**, one-piece GS can clear the remaining sheet residual once that two-sided squeeze is removed. Not a canary of mixed-61 concavity, and not a move-set kill. |
| **S1** | **implementation defect until the jump is real; then the open paradigm candidate** | Sag is 0, so Finding 1 does not apply. The trajectory recovered 433 → 4.5e-4 and then froze 12.6 µm from the band because the one specified topology change was evaluated, refused, and **spent**. That is not yet evidence that one-piece GS cannot close 12 µm. |
| **C175** | **still the separator test, but not yet run** | Shock, 2 s, and 3 seeds stay. What ran was PGS with the jump turned into a no-op (or, under `always`, a smash of 4 self-moves). Grok’s 168.484/no-jump T is **not** this cell; the arbitration made C175 the 10 % shock **with** the Round-1 jump. |
| S0 / soundness / throughput | **pass, unchanged** | Measure, publication path, and work currency are not on trial. |

**Nothing in the fatal set is presently paradigm evidence.** The first cell that can become paradigm is C175, after the jump matches Sol R2 §4. The second is S1, after that same jump has had a stall-scale chance at a 12 µm cluster. triangle-20 is not in that list.

---

## D. Finding by finding (wrongness called with a line)

### Finding 1 — triangle-20 sag: **confirmed as a defect; overstated as a rescoring false negative**

The split is in the code.

- Material sheet legality is `sheet_edge + sag` on the **physical** sheet (`validation/general_polygon.rs` 200–203, 530–545). Sheet is 2000×2700. The locked top at `T = 70.742` is not a sheet edge.
- Depth and the publication strip gate are sag-less: `deepest + sheet_edge_clearance_mm` (`state.rs` 365–373, `publish.rs` 214, 265–269).
- Φ’s four strip sides all use `contract.edge_clearance_mm()` = sheet_edge **+ sag** (`state.rs` 104–106, `energy.rs` 102, `broad_phase.rs` 26–36).

On triangle-20, `s = 0.25`. Driver JSON reports `sheetEdgeClearanceMm` as `edge_clearance_mm()` already (`overlap_ics_benchmark.rs` 420), so residual_split’s `depth − T + s = 0.11027` is the right top-row identity. Unused strip 0.140 mm and predicted phantom 0.110 mm match.

**Where the layer-2 reading is wrong:** the stall `max_g = 0.11765` is **not** the phantom. It exceeds 0.11027, so a left/right/bottom sheet row is carrying the max (`residual-split.json`; verification README §3.2). Re-scoring the committed stall with a sag-less top leaves `max_g` at 0.11765 — still 29× the 4 µm attempt band. The publication gate is not “blocked by a 0.25 phantom” at that snapshot; it is blocked by a real sheet residual. The second agent was right about that sentence.

**Where the layer-2 reading is right, and stronger than they stated:** the stall is a *fixed point of the wrong objective*. Phantom top (force −Y, `EDGE_NORMALS[3] = [0,−1]` in `state.rs` 208) presses the layout into the sheet floor while 0.140 mm of strip sits unused. That is a two-sided Y-squeeze. The frozen-rotation probe and the 2 M probe both freeze with 0 pair rows, which is the important fact: **with no pair coupling, every remaining boundary row is a one-piece move.** A piece with only a bottom (or side) residual accepts a step along its edge normal; `propose` cannot reject it for neighbour cost because there is no neighbour cost (`descent.rs` 160–207, `energy.rs` 232–254). The phantom is what can put top and bottom on the same piece and cancel that gradient. Remove it, and the canary’s own move set can express the residual.

So: **yes**, strip-top must use the sag-less depth inset; **yes**, true sheet edges (L/R/B) keep `edge+sag`; **no**, mixed-61 S1/C175 are not this bug (`flatteningSagToleranceMm: 0.0` in those documents).

Sol R2 §4 says edge clearance follows the contract, “including sag exactly as the contract does.” The contract’s sag is a **sheet** rule. Applying it to `T` is the mis-read. The publication pipeline already arbitrated the other way.

### Finding 2 — jump neutered: **confirmed, and the proposed stall-scale gate is necessary but not sufficient**

Sol R2 §2 (and the converged spec): 16 relocations, “one bounded local **sweep** from each,” choose by guided Φ, **“commits for a full epoch even if raw Φ temporarily worsens.”** Staying put is not in the choice set.

Three independent neuterings, all in `descent.rs`:

1. **Commit default is the opposite of the spec.** `jump_commits_unconditionally: false` at line 87; line 368 requires `improved_guided` otherwise. The comment at 357–362 even says a guided-improvement test “cannot change a topology at all.” That is not a knob reading. That is disabling the mechanism and then measuring its absence.
2. **“Local sweep” is four proposals of the same piece** (`jump_local_proposals: 4` at 85; loop at 342–346). A sweep in this module is `n` pieces (`sweep`, 216–242). Four self-moves after a strip teleport do not settle neighbours. Guided Φ of that state almost never beats the pre-jump local minimum — which is why `jumpsImprovingGuided = 0` on S1, C175 (all three seeds), and triangle-20, while random-T with allowance 8 records **4/8** improving. The jump *can* improve guided Φ on a wild state; it cannot, as implemented, on a near-min.
3. **Allowance is spent on the no-op.** `jumps_spent += 1` at 265, *then* `jump()`. Guided-commit restores the original pose (368–370) and the trajectory has no jump left for the rest of the quota. Every fatal cell shows `jumpProposals: 16` and `jumps: 1`: the mechanism was *evaluated once and discarded*.

**The stall-scale gate is the right commit semantics**, and Grok R1’s stall (`max g > 0.05 mm`) is independent support. Derive the threshold from the publication band, do not fit it to S1: `25 * EPSILON_GRID_MM = 0.1 mm`. Above it: strip relocation, real local sweep, unconditional commit. Below it: **do not** run a strip teleport (S1 `always` is the measurement: 12.6 µm → 2.55 mm, `basin-jump-always.json`).

**What Finding 2 gets wrong:** “turn commit on and C175 is the test the spec prescribed.” README §4 already ran that. Under `always`, C175 is still 0/3, with two seeds at Φ 925 and 3359. Unconditional commit of a **4-self-move teleport** is a smash at both scales. C175 must be re-run under the stall-scale gate **and** a real local sweep. Commit-bit alone is not the experiment.

Also: `jump()` only snapshots `state.poses[piece]` (line 291). That is only safe because local proposals do not move anyone else. A spec-correct n-piece settle **must** clone/restore the full `IcsState` between the 16 candidates or candidate 2 starts from candidate 1’s wreckage.

### Finding 3 — S1 fixed point: **(c) is not a bookkeeping defect; the mechanism is (a) plus GLS equalization**

`(c)` as “rows counted on one side, or the escalated row not incident” is **false**. `incident_guided` (`energy.rs` 232–254) folds every pair involving the piece, both index orders, and all four of **that piece’s** edge rows. `rebuild_piece_rows` (158–187) updates those pairs and those edges; penalties persist (only `violation_mm` / `contact` are overwritten on pairs; `measure_edges` copies `previous[edge].penalty` at 107). `guided_update` (326–337) **does** escalate edge rows. Sweep order visits every piece (`energy.rs` 368–381). The escalated row’s piece is proposed.

Why weight ×10 does nothing, from the same functions:

S1’s active set is four similar rows (2 pair at 11.7 µm, 2 edge at 12.6 µm). Utility `u = v/(1+p)` keeps them in lockstep, so 3,040 then 32,549 increments equalize `p` instead of picking a winner (`maxGuidedPenalty` 923 → 9,885, same `max_g` bits, same 1,044 accepted moves). Incident guided energy of a violation-transfer is then invariant to a global scale on those weights. Squared-hinge makes the transfer worse: moving 12 µm off a 12 µm wall onto an 11.7 µm pair multiplies that pair’s `v²` by ~4, and with `p` similar the step is rejected (`descent.rs` 204, strict `<`).

That is **(a)**, classic Gauss–Seidel deadlock, with GLS doing what GLS does on a tied active set. Not a comparison bug.

**(b)** is not the cause. `ladder_top = max(c_pair/4, median/128, 8 µm)` (`descent.rs` 79–82) is 1.25 mm here; rungs halve to 0.25 µm. 12 µm sits on the ladder.

**(d)** as “never selects the unlocking piece”: the sweep **does** select every positive-energy piece. The freeze is the complement: `if before <= 0.0 { return false }` (`descent.rs` 161–163) plus strict decrease means a **zero-energy** neighbour, the piece that would have to step aside, is immovable. With only four active rows the cluster is 2–3 pieces pressed against the strip, 7.53 µm too deep (`150.17299` vs `T = 150.16547`). That is a cooperative compaction of ~8 µm, which one-piece strict-incident-decrease cannot name.

SE(2) coupling is a contributing inefficiency (triangle-20 frozen-θ: 66,863 accepts vs 175, still jammed). There is **no** S1 frozen-θ probe; do not promote coupling to the cause.

**Minimal spec-consistent remedy, in order:**

1. Make the jump real (Finding 2). On S1 it will fire at the µm stall and must be **suppressed as a strip teleport**.
2. Below the 0.1 mm threshold, the same 16-sample jump at **stall scale** (ball radius derived, e.g. `max(4 * max_g, ladder_top)`, not fitted to 12.6 µm), still with a real local sweep. That is jump **type**, which the spec already calls a knob.
3. Do **not** lead with accept-equal (a zero-energy piece still cannot move: `0+ε` is not `< 0`). Do not invent an incident-guided bug. SOR / chain translation / two-endpoint PGS are paradigm amendments; they wait until (1)+(2) have been measured.

The basin table (works at 0.25 mm / 1°, dead at 0.5 mm / 2°) is the shape of the **one-piece** field. It is not yet the shape of the specified solver, because the specified solver includes a jump that never committed.

### Finding 4 — C175’s 2 s clause: **stands; it was calibrated for a working jump; do not weaken it**

C175 is not a 12 µm deadlock. Seed 0: Φ 461 → 40, `max_g` 5.36 → 2.10 mm, **22 pair + 21 edge**, **2,473 accepted moves**, 2,673/3,277 stalled, one no-op jump, 1.44 s for 200 k proposals. The 2 M fixed-point probe was **not** run here. This is unfinished millimetre separation with the barrier-crossing shot already spent on a 4-self-move teleport.

The arbitration (converged spec, delta 1): C175 is **the** fatal separator; C168 is diagnostic. Grok’s T was 168.484 with **no** jumps. That is not what was implemented. You do not get to read a no-jump failure of a with-jump cell as paradigm.

After Findings 1–3: **C175 stands as-is** — same `0.10(D₀−L)`, same three seeds, same 200 k / 2 s, same “≥1 strict dual-valid non-constructor child.” The extra 0.25 mm / 1° seed perturb (`overlap_ics_benchmark.rs` 556–561) is Sol R2’s “distinct deterministic affine perturbations,” and 0.25 mm is the basin-passable scale. It does not excuse the cell. Do not relax the clock, do not shrink the shock, do not add jumps. If it still fails after a spec-correct jump, **that** is the paradigm separator.

---

## B. Frozen fix list for the Gate 0 re-run

No other knobs. No fitting to S1’s 12.635 µm or C175’s 2.1 mm.

1. **Strip-top clearance ≠ sheet-edge clearance.** `boundary_residuals` / `measure_edges`: L/R/B keep `edge_clearance_mm()`; **top** uses `sheet_edge_clearance_mm` (the depth inset). Sites: `broad_phase.rs` 26–36, `energy.rs` 91–128, `state.rs` 104–106. Same split in `descent.rs` 294–299 (jump box `high_y`), `publish.rs` 550–608 (repair `sheet_slack` on the strip top), `corpus.rs` 173 / 224 (independent score; mixed-61 unaffected, keep the conventions honest).
2. **Homotopy floor matches Φ’s true sheet edges.** `homotopy.rs` 73 and 121 use sag-less `sheet_edge_clearance_mm` as the floor while Φ’s bottom uses edge+sag. On triangle-20 that **manufactures** bottom residuals of up to 0.25 mm. Use `edge_clearance_mm()` as the floor. Do not change pinned `W` values. `L` on mixed-61 is unchanged (`s = 0`).
3. **Jump matches Sol R2 §4.** Sites, all `descent.rs`:
   - `jump_local_proposals`: one **n-piece** `sweep` (or a frozen small multiple), not 4× `propose(piece)`.
   - Snapshot/restore **full** `IcsState` between the 16 candidates (line 291 is not enough once neighbours move).
   - `jump_commits_unconditionally` default **true above threshold**.
   - Stall-scale gate, derived: if `max_g > 25 * EPSILON_GRID_MM` (0.1 mm), strip relocation + local sweep + unconditional commit; if `max_g ≤ 0.1 mm`, **no strip teleport**. Below threshold, same 16 samples in a ball of radius `max(4 * max_g, ladder_top)` around the current pose (jump type, not a new family of moves).
   - **Do not increment `jumps_spent` for a suppressed or uncommitted jump** (line 265). The one-shot is for an installed relocation.
4. **Instrument the residual.** `RowCensus` must split L/R/B/T counts and maxima. The verification round correctly refused “a single global translation would legalize” without this. Do not claim it until the document prints it.
5. **Not in this round:** accept-equal, SOR, chain/component translation, two-endpoint PGS, SE(2) decoupling, extra jump allowance, C175 shock or budget, homotopy schedule.

FAST tier still fails until S1 republishes; that is the two-tier discipline working.

---

## C. Pre-commit reading (so the fix round cannot be tuned)

Cell definitions, `W` pins, C175 budget 200 k, `stalls_before_jump = 2`, `jump_samples = 16`, band 4 µm, repair 16 µm: **frozen**. Threshold 0.1 mm and micro-ball `max(4*max_g, ladder_top)`: **frozen, derived**. One re-run of the committed battery. If a cell fails, you name which remaining defect it is; you do not add a knob.

| cell | pre-commit | remaining fail means |
|---|---|---|
| **triangle-20** | **Must PASS** (dual-valid child inside 70.742, repair ≤ 16 µm, giveback ≤ 0.050 mm). | If `max_g` still sits on **top** at ~0.11 mm: clearance fix missed a caller (treat as an incomplete (B)(1)). If 0 pairs and **one-sided** L/R/B: still implementation (move accepted but not far enough — look at homotopy floor). If 0 pairs and **opposite** sides on the **same** piece: then, and only then, the canary becomes move-set evidence. |
| **S1** | **Not predicted.** Finding 1 cannot save it. | Strip-jump must **not** fire (`jumpProposals` stays 0 once `max_g < 0.1`, or only micro-ball samples). If it still freezes at 12.6 µm / 0 accepts after the micro-jump: **paradigm candidate** — basin of one-piece GS on mixed-61 between 0.25 mm and 0.5 mm. If it republishes with repair ≤ 16 µm inside 150.16547: previous FAIL was the no-op jump. |
| **C175** | **Not predicted.** Same 0/3 criterion: ≥1 strict dual-valid non-constructor child, 0 invalid publications, all three seeds ≤ 2 solver seconds. | `jumpsImprovingGuided` may still be 0 (unconditional commit does not require guided improvement). What must **not** be 0 is an **installed** relocation on at least the trajectories that stall above 0.1 mm. If 0/3 after that: **paradigm separator**, stop before `homotopy.rs`. If Φ explodes again as under `always`: local sweep still is not a sweep — incomplete (B)(3), not a family kill. |
| S0 / corpus / throughput | Must remain PASS. Any breakage is a regression, not a new Gate 0 question. | |

A pass obtained by widening the band, raising C175’s budget, setting `jump_allowance > 1` on mixed-61, or turning sag off globally is not a pass.

---

## Same-class latent defects (clearance, bookkeeping, acceptance)

- **One `edge_clearance_mm()` on all four sides** is the bug’s blast radius: Φ, jump sample box, repair slack, corpus independent score. Jump `high_y = T − (edge+sag) − R` also **collapses the sample box** on triangle-20 (piece `max_radius` ~ tens of mm, usable strip height 60.24 mm, so `low_y > high_y` and `mix` at 385–387 returns the midpoint). All 16 “relocations” of a large triangle become **the same point**, θ varying. Circumradius-in-AABB is the wrong feasible region; the spec asked for positions **in the strip**.
- **Jump state snapshot is one pose.** Latent until local sweep is real (`descent.rs` 291).
- **`jumps_improving_guided` counts “best candidate beat baseline,” not “a relocation was installed”** (`mod.rs` 361–364 vs `jump` returning `improved_guided` even when pose is restored). The evidence documents cannot tell a no-op from a committed guided improvement without reading `jumpcommit` and the pose digest. Name the counter for what it is, or record `jumpCommitted`.
- **Acceptance comparison is not a defect.** Incident-only, strict `<`, zero-energy early return: all spec-faithful, all deadlock-prone. Do not “fix” them in this re-run.
- **SE(2)-coupled ladder** (`descent.rs` 175–191): real inefficiency, not the S1 mechanism without an S1 frozen-θ probe. Out of scope.
- **C175’s extra 0.25 mm / 1°** is slightly beyond “affine shock by `0.10(D−L)`” and is not material.
- **No L/R/B/T census** in `RowCensus` (`energy.rs` 416–444): instrumentation hole that let README §3.1 over-claim a global translation.

I did not find a one-sided pair-row or “escalated row not incident” bug. Incremental vs cold rebuild is tested (`tests.rs` 429–457) and the 10 k corpus recorded 0 incremental mismatches.

---

## Force correlation: 82.71 % is not a field weakness on the spec’s population

The fatal clause is scored on `compressed` (Sol R2 §3’s 1/3/10 % residual + predeclared SE(2)): **5001/5001**. That is the population C175 is drawn from. The local force is a descent direction there.

The two added families:

- **grazing (83.9 %):** Φ is Σv², the independent score is Σv. README’s logged miss (`phi 0.004723 → 0.004478` while `active 0.027 → 0.030` on a 15 µm step) is exactly that near-zero trade. Benign artefact of two different aggregations at the publication scale, not a wrong normal.
- **containment (20 %):** MTV of a small piece inside a large one is the host’s extent (`contact.rs` 150–158, min-translation axis). The independent score is deepest-interior-vertex. Different quantities; the spec already has a separate containment clause, which passed 589/589. Known MTV limitation, already isolated.

Folding the three families into 82.71 % and calling it a field failure would be scoring a clause the spec did not write. Hull-as-one-cell SAT (Grok’s named fallback if force-correlation missed) is **not** licensed by this number: the scored population did not miss.

---

**Bottom line.** Gate 0’s STOP is correct as a *process* stop: do not build `homotopy.rs` on this binary. It is not correct as a *family* kill. triangle-20 is a one-line clearance bug sitting on a 0-pair residual the move set can express. S1 and C175 are measurements of a jump that evaluated 16 samples, committed none, and spent the allowance. Re-run the same fatal set with the list in B frozen. Only a C175 0/3 *after that*, or an S1 12 µm freeze *after* a stall-scale micro-jump, would be paradigm.
