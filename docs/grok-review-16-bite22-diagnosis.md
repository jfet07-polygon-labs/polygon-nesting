# Grok review 16 — the bite-22 diagnosis

Grok at `--reasoning-effort xhigh`, web search disabled. Same four questions as
[`sol-review-21-bite22-diagnosis.md`](sol-review-21-bite22-diagnosis.md),
answered independently.

He confirms the diagnosis with an explicit falsifier table, all of whose rows
the data already answers "no"; he rejects `mod.rs:2002` as the defect on the
same isomorphism Sol names; and he proposes a different mechanism from Sol's —
one sequential far-side sweep at the *entry* of a bite rather than a repair at
its *exit*. Verbatim below, including his own four corrections to the brief.

---

# Bite 22 — diagnosis review (Grok)

I re-read Round 4’s composed-arm r0 cells (`curve3` / `gate10` / `curve30` / `curve60`), `run_cutclose`, Sparrow `explore.rs` / `separator.rs` at `14f4868f`, Algorithm 12, and the three closed rounds after it. The table is real. Some of the surrounding sentences are not.

---

## 1. Diagnosis: confirm, with four corrections

**The 10-second median of 179.07 mm is the unpublished 22nd explore bite.** It is not a search plateau, not a throughput deficit, and not a compress-basin result. I confirm that.

Equal explore units, unequal publications, on the composed arm, repetition 0:

| seed | 10 s explore bites | 10 s depth | 10 s strikes / disruptions | 30 s explore bites |
| ---: | ---: | ---: | ---: | ---: |
| 2 | 85 | 166.84 | 3 / 2 | 125 |
| 3 | 99 | 165.42 | 0 / 13 | 115 |
| 6 | 74 | 169.35 | 0 / 17 | 108 |
| 0 | 46 | 170.44 | 2 / 0 | 120 |
| 1, 4, 5, 7, 8 | **21** | **179.07–179.08** | 0–2 / 0–1 | 21, 115, 118, **21**, **21** |

Every seed spends the same ~16.94 M explore units at 10 s. Four seeds turn those units into 46–99 legal contractions. Five seeds turn them into 21 publications and one unpublished width. That is a success-rate collapse at one width, not a machine that has run out of bites per second. After a seed clears 22 it flows (46, 74, 85, 99 at 10 s; 99–125 at 30 s). There is no second integer freeze point in the 10 s column. Bimodal freeze-at-21 vs flow is the opposite of a gradual plateau.

The 179.08 incumbent on a frozen seed is compress bite 23 from the 21-bite parent, not the failed explore target (~178.99). That matches the earlier per-bite autopsy: one 0.05 % compress publication, then compress also fails. The published number is the parent lattice, not a new basin.

**Specific falsifiers, and whether the data already falsifies:**

| Falsifier | Already in the table? |
|---|---|
| A 10 s frozen seed with `exploreBites ≠ 21` | **No.** All five are 21. |
| A ~179 mm 10 s cell with `exploreBites > 21` (a later explore barrier, or compress-only) | **No.** |
| A 10 s seed at ≤168.484 mm with `exploreBites == 21` (compress alone did the work) | **No.** The two qualifiers have 85 and 99 explore bites. |
| Freeze points spread across many bite ordinals at 10 s | **No.** Only 21 or “flowing.” |
| Frozen seeds idle (near-zero batches after the prefix) | **No.** Seed 7 at 30 s: 7 332 explore batches, 23 strikes, 7 disruptions, 28.15 s search, still 21 bites. |

The inference “21 successful explore publications ⇒ currently sitting on unpublished bite 22” is structural, not a table-reading trick: `explore_bites` increments only in the `Some(publication)` arm; the extra ~15.4 M explore units between the 3 s and 10 s cells have nowhere to go except that unpublished width and its retries.

I do **not** treat this as a newly sighted object. Economics already named “bite-22 hard states” and the 179 shelf; grok-13 already named “deadline-killed first `separate()` at `W ≈ 178.99`.” What the Round 4 table adds is the escape-time instrument on the *same* composed member, and one fact grok-13 did not have: **at 30 s, Algorithm 12 now fires on the frozen tail and still loses.**

**Four corrections, where I disagree with the write-up:**

1. **“Every seed reaches 21 explore bites in under 0.61 s” is false.** Seed 1 at 3 s is **19** bites / 179.5295 mm, in 0.603 s. The 3 s search budget is only 0.672 s. Seed 1 finishes the prefix between 3 s and 10 s, then freezes at 21. The prefix is *cheap*, not seed-identical. Seeds 2 and 5 already have a disruption in that prefix. 3 s depths among 21-bite seeds span 179.031–179.082 mm, and `finalPoseDigest` is distinct per seed. Constructor fingerprint `a791c397…` is identical; the 21-bite parents are not one layout.

2. **`compressBites: 2` is not two successful compress publications.** In `run_cutclose`, `compress_bites += 1` on every started compress bite; `explore_bites += 1` only on publish. Frozen 10 s cells are 21 explore pubs + 1 compress pub + 1 failed compress start = 22 publications. That is the 179.082 lattice, not two extra shrinks.

3. **“94 % of a 10-second budget” is the wrong denominator.** Frozen seed 7: ~0.56 s of prefix search, then 6.44 s search total. That is ~91 % of *that cell’s search wall* after the prefix, and ~80 % of search units, sitting on unpublished 22. It is not 94 % of the 10 s request (constructor and the 20 % compress allocation still run). Separately, the 3 s→10 s explore-unit ratio of 0.912 is **the same for all nine seeds** because they all exhaust the plan. That ratio does not isolate bite 22. Bite counts at equal units do.

4. **Clearing bite 22 is necessary for the median, not sufficient for the 10 s quality clauses.** 168.484 / 182.976 at 0.1 % is ~83 successful explore bites. Seeds 0 and 6 **won the 22-lottery at 10 s** (46 and 74 bites) and still sit at 170.44 and 169.35. The median is 179.07 because five seeds never left 22; the 5/9 clause would still fail if those five escaped like seed 0. The 30 s pass *is* “7/9 won that lottery inside 30 s” — at 30 s, everyone who cleared 22 is well under 168.484. Do not import that sentence back onto 10 s.

The 30 s column also updates the old “disrupt never fires” story. Round 4 seed 7/8 at 30 s: 7 and 6 disruptions, still `exploreBites == 21`. Disruptions only exist on the explore fail path, and the 3 s prefix had zero of them, so those tickets were spent on bite 22. **The lottery is the first `separate()` of that width. Algorithm 12 is the consolation, and on this member it has a tail past 30 s-equivalent work.**

---

## 2. `mod.rs:2002` `None => break` — paradigm, not a bug

I disagree with treating line 2002 as the defect.

Sparrow `explore.rs:32–82` at `14f4868f` is one loop:

```text
while !term.kill():
    local_best = sep.separate(term, ...)
    if total_loss == 0.0:
        shrink; clear pool
    else:
        pool
        if pool.len >= max_conseq_failed_attempts: break   # default unbounded
        rollback(selected, None); disrupt
```

There is no outer “bite” loop. Shrink happens **on success**. A failed `separate` never ends the phase; it pools, disrupts, and loops. The phase ends when `term.kill()` is true at the top of the `while`.

Our isomorphism is:

| Sparrow | Us |
|---|---|
| `while !term.kill()` | the **inner** `loop` at a fixed width (`mod.rs:1781`) |
| shrink on success | outer loop’s `explore_bite` before `separate` |
| `max_conseq_failed_attempts` default `None` | `attempts_exhausted` is `false` when `attempts_per_bite == 0` (Wall and every gate Calibrated cell) |
| return `feasible_sols` when time is up | `None => break` at 2002, then compress from the last exact parent |

On the production CutCloseRelocate path, the inner loop leaves unpublished only on `Deadline` or `phase_done` (both mean the explore allocation is spent). Struck or Refused with remaining units does **not** take the 2002 arm; it pools, disrupts, and calls `separate` again at the same `W`. That is Algorithm 12’s `else`.

2002 is Algorithm 11’s phase return: explore hands compress the last dual-valid parent. Making 2002 continue the **outer** loop after an unpublished bite would shrink again without legalizing `W` — the opposite of persist-at-`W`.

Two micro-differences, neither of which is the 10 s median:

- Sparrow still takes the `else` (pool, disrupt) after a timed-out `separate`, then the `while` sees `kill` and exits. We skip that idle disrupt. One swap with no following `separate` does not move 179.07.
- Sparrow’s `term.kill()` is checked on the **outer strike** `while` (`separator.rs:83`), not inside the 200-iteration inner `while`. We read the calibrated/wall deadline every master iteration. That can convert a would-be `Struck` on the crossing batch into `Deadline`, and `phase_done` then skips disrupt. That is a terminator *placement* difference, not “one failed bite aborts explore while budget remains.”

So: **the analogue of `while !term.kill()` is the per-width inner loop, not the outer bite loop. 2002 is not why five seeds sit at 21 bites.** They sit there because that inner loop never publishes, and the allocation ends inside the first (or first-plus-one) `separate()` of width 22.

---

## 3. One mechanism: a post-cut far-side sweep before the first `separate()` of an explore bite

I will **not** name “make Algorithm 12 reachable at 10 s” (bite-local cap, impatient strikes, steal compress, skip 2002). That class has already lost.

- Economics treatment (work-strikes): at 10 s, seeds 4/7/8 got 3 strikes and **1 disruption** and stayed at 21 bites / 179.08. Seed 2 **regressed** from 48 bites to 21. Policy not promoted.
- Round 4 composed 30 s: seeds 7/8 got **7 and 6 disruptions** on bite 22 and still did not publish.
- 80/20 is frozen. Stealing compress from seeds 2/3 is how you destroy the only two 10 s qualifiers.

The table says the 10 s coin flip is **whether the first `separate()` of the 22nd centre-cut publishes**. Seeds that win (0, 2, 3, 6) mostly win inside that call (seed 0: 46 bites, **0 disruptions**). Retries are not the 10 s lever.

**Mechanism, one sentence:** after `homotopy::explore_bite` (centre cut, far-side translate, weight reset — unchanged) and **before** the first `separate()` of that width, run **one sequential Gauss-Seidel relocate pass over the cut-moved pieces that are still colliding**, then hand that state to the existing eight-worker `separate()`.

Grounding:

- Shelf probe (the campaign’s own 400-iteration bite-22 witness): `movedPieces = 34`, `widthAfterMm = 178.9865`, `minRawPhi = 2.00e-4`, `proxyBandReached = false`, `strikes = 0`, `disruptions = 0`. The first separate starts from the raw seam and burns 400 eight-worker tournaments without entering the band.
- The cheap prefix is 21 legal centre-cuts of a constructor packing. Those cuts do not rearrange. Bite 22 is the first seam the sequential relocate does not trivially close. Min-cut already changed *who* translates and made the 30 s median **worse**. This does not change membership, shrink, cut coordinate, GLS restore, strike literals, sample counts, or 80/20. It spends one Algorithm-5 pass on the 34 pieces the homotopy actually moved, before spending 91 % of leftover explore on eight clones of the unmoved seam.
- Sparrow `worker.rs:37–40` shuffles the colliding set. We do too (`colliding_permutation`). The deviation is declared: **order the first post-cut pass as far-side-first**, once, not as a new field, cut, or tracker policy.

Not a programme: no second cut, no disrupt injection, no Φ change, no visit-quota allocator (conflict-cluster is closed), no pool-rebase.

---

## 4. Gate that kills this proposal

Reuse the existing shelf probe, not a 10 s battery: **400 master iterations at bite 22**, from the 21-bite exact parent, composed plan, seeds **1, 4, 5, 7, 8** (the Round 4 10 s freeze set). Control is today’s first-separate. Treatment is cut → one far-side sweep → first-separate.

**NO before any quality battery if any of these fail:**

1. **Install.** On every seed, the sweep’s visit set is exactly `{i : i was moved by this `explore_bite` and is colliding after the cut}`. Empty visit set on a seed with `moved_pieces > 0` is `AUTOFAIL`.
2. **Not a no-op.** Post-sweep pose digest ≠ post-cut digest on every seed. Identity ⇒ the pass did not run; stop.
3. **Does not worsen the entry.** Post-sweep raw Φ < post-cut raw Φ on every seed. A raise on any freeze-set seed is `AUTOFAIL`.
4. **Cheapens the 400-iter shelf, or it is closed.** After the same 400 master iterations: treatment `minRawPhi` strictly below control on **at least 3 of those 5 seeds**, **or** treatment publishes and control does not on **at least 1 of 5**.

**Pre-declared “this did not work”:** (1)–(3) green and (4) red — the sweep changed the entry and did not cheapen the first 400 iterations of bite 22. Close it. No 10 s last-chance, no extra disrupt, no “maybe 800 iterations.” If (4) is green and a later 10 s battery still prints median 179.07 with 5/9 seeds at `exploreBites == 21`, that is a quality miss of a mechanism that engaged: the first-separate landscape moved and the lottery did not. Same closure. Do not then reopen 200/3, 80/20, or pool-rebase to rescue it.

---

# Bite 22 — diagnosis review (Grok)

I re-read Round 4’s composed-arm r0 cells (`curve3` / `gate10` / `curve30` / `curve60`), `run_cutclose`, Sparrow `explore.rs` / `separator.rs` at `14f4868f`, Algorithm 12, and the three closed rounds after it. The table is real. Some of the surrounding sentences are not.

---

## 1. Diagnosis: confirm, with four corrections

**The 10-second median of 179.07 mm is the unpublished 22nd explore bite.** It is not a search plateau, not a throughput deficit, and not a compress-basin result. I confirm that.

Equal explore units, unequal publications, on the composed arm, repetition 0:

| seed | 10 s explore bites | 10 s depth | 10 s strikes / disruptions | 30 s explore bites |
| ---: | ---: | ---: | ---: | ---: |
| 2 | 85 | 166.84 | 3 / 2 | 125 |
| 3 | 99 | 165.42 | 0 / 13 | 115 |
| 6 | 74 | 169.35 | 0 / 17 | 108 |
| 0 | 46 | 170.44 | 2 / 0 | 120 |
| 1, 4, 5, 7, 8 | **21** | **179.07–179.08** | 0–2 / 0–1 | 21, 115, 118, **21**, **21** |

Every seed spends the same ~16.94 M explore units at 10 s. Four seeds turn those units into 46–99 legal contractions. Five seeds turn them into 21 publications and one unpublished width. That is a success-rate collapse at one width, not a machine that has run out of bites per second. After a seed clears 22 it flows (46, 74, 85, 99 at 10 s; 99–125 at 30 s). There is no second integer freeze point in the 10 s column. Bimodal freeze-at-21 vs flow is the opposite of a gradual plateau.

The 179.08 incumbent on a frozen seed is compress bite 23 from the 21-bite parent, not the failed explore target (~178.99). That matches the earlier per-bite autopsy: one 0.05 % compress publication, then compress also fails. The published number is the parent lattice, not a new basin.

**Specific falsifiers, and whether the data already falsifies:**

| Falsifier | Already in the table? |
|---|---|
| A 10 s frozen seed with `exploreBites ≠ 21` | **No.** All five are 21. |
| A ~179 mm 10 s cell with `exploreBites > 21` (a later explore barrier, or compress-only) | **No.** |
| A 10 s seed at ≤168.484 mm with `exploreBites == 21` (compress alone did the work) | **No.** The two qualifiers have 85 and 99 explore bites. |
| Freeze points spread across many bite ordinals at 10 s | **No.** Only 21 or “flowing.” |
| Frozen seeds idle (near-zero batches after the prefix) | **No.** Seed 7 at 30 s: 7 332 explore batches, 23 strikes, 7 disruptions, 28.15 s search, still 21 bites. |

The inference “21 successful explore publications ⇒ currently sitting on unpublished bite 22” is structural, not a table-reading trick: `explore_bites` increments only in the `Some(publication)` arm; the extra ~15.4 M explore units between the 3 s and 10 s cells have nowhere to go except that unpublished width and its retries.

I do **not** treat this as a newly sighted object. Economics already named “bite-22 hard states” and the 179 shelf; grok-13 already named “deadline-killed first `separate()` at `W ≈ 178.99`.” What the Round 4 table adds is the escape-time instrument on the *same* composed member, and one fact grok-13 did not have: **at 30 s, Algorithm 12 now fires on the frozen tail and still loses.**

**Four corrections, where I disagree with the write-up:**

1. **“Every seed reaches 21 explore bites in under 0.61 s” is false.** Seed 1 at 3 s is **19** bites / 179.5295 mm, in 0.603 s. The 3 s search budget is only 0.672 s. Seed 1 finishes the prefix between 3 s and 10 s, then freezes at 21. The prefix is *cheap*, not seed-identical. Seeds 2 and 5 already have a disruption in that prefix. 3 s depths among 21-bite seeds span 179.031–179.082 mm, and `finalPoseDigest` is distinct per seed. Constructor fingerprint `a791c397…` is identical; the 21-bite parents are not one layout.

2. **`compressBites: 2` is not two successful compress publications.** In `run_cutclose`, `compress_bites += 1` on every started compress bite; `explore_bites += 1` only on publish. Frozen 10 s cells are 21 explore pubs + 1 compress pub + 1 failed compress start = 22 publications. That is the 179.082 lattice, not two extra shrinks.

3. **“94 % of a 10-second budget” is the wrong denominator.** Frozen seed 7: ~0.56 s of prefix search, then 6.44 s search total. That is ~91 % of *that cell’s search wall* after the prefix, and ~80 % of search units, sitting on unpublished 22. It is not 94 % of the 10 s request (constructor and the 20 % compress allocation still run). Separately, the 3 s→10 s explore-unit ratio of 0.912 is **the same for all nine seeds** because they all exhaust the plan. That ratio does not isolate bite 22. Bite counts at equal units do.

4. **Clearing bite 22 is necessary for the median, not sufficient for the 10 s quality clauses.** 168.484 / 182.976 at 0.1 % is ~83 successful explore bites. Seeds 0 and 6 **won the 22-lottery at 10 s** (46 and 74 bites) and still sit at 170.44 and 169.35. The median is 179.07 because five seeds never left 22; the 5/9 clause would still fail if those five escaped like seed 0. The 30 s pass *is* “7/9 won that lottery inside 30 s” — at 30 s, everyone who cleared 22 is well under 168.484. Do not import that sentence back onto 10 s.

The 30 s column also updates the old “disrupt never fires” story. Round 4 seed 7/8 at 30 s: 7 and 6 disruptions, still `exploreBites == 21`. Disruptions only exist on the explore fail path, and the 3 s prefix had zero of them, so those tickets were spent on bite 22. **The lottery is the first `separate()` of that width. Algorithm 12 is the consolation, and on this member it has a tail past 30 s-equivalent work.**

---

## 2. `mod.rs:2002` `None => break` — paradigm, not a bug

I disagree with treating line 2002 as the defect.

Sparrow `explore.rs:32–82` at `14f4868f` is one loop:

```text
while !term.kill():
    local_best = sep.separate(term, ...)
    if total_loss == 0.0:
        shrink; clear pool
    else:
        pool
        if pool.len >= max_conseq_failed_attempts: break   # default unbounded
        rollback(selected, None); disrupt
```

There is no outer “bite” loop. Shrink happens **on success**. A failed `separate` never ends the phase; it pools, disrupts, and loops. The phase ends when `term.kill()` is true at the top of the `while`.

Our isomorphism is:

| Sparrow | Us |
|---|---|
| `while !term.kill()` | the **inner** `loop` at a fixed width (`mod.rs:1781`) |
| shrink on success | outer loop’s `explore_bite` before `separate` |
| `max_conseq_failed_attempts` default `None` | `attempts_exhausted` is `false` when `attempts_per_bite == 0` (Wall and every gate Calibrated cell) |
| return `feasible_sols` when time is up | `None => break` at 2002, then compress from the last exact parent |

On the production CutCloseRelocate path, the inner loop leaves unpublished only on `Deadline` or `phase_done` (both mean the explore allocation is spent). Struck or Refused with remaining units does **not** take the 2002 arm; it pools, disrupts, and calls `separate` again at the same `W`. That is Algorithm 12’s `else`.

2002 is Algorithm 11’s phase return: explore hands compress the last dual-valid parent. Making 2002 continue the **outer** loop after an unpublished bite would shrink again without legalizing `W` — the opposite of persist-at-`W`.

Two micro-differences, neither of which is the 10 s median:

- Sparrow still takes the `else` (pool, disrupt) after a timed-out `separate`, then the `while` sees `kill` and exits. We skip that idle disrupt. One swap with no following `separate` does not move 179.07.
- Sparrow’s `term.kill()` is checked on the **outer strike** `while` (`separator.rs:83`), not inside the 200-iteration inner `while`. We read the calibrated/wall deadline every master iteration. That can convert a would-be `Struck` on the crossing batch into `Deadline`, and `phase_done` then skips disrupt. That is a terminator *placement* difference, not “one failed bite aborts explore while budget remains.”

So: **the analogue of `while !term.kill()` is the per-width inner loop, not the outer bite loop. 2002 is not why five seeds sit at 21 bites.** They sit there because that inner loop never publishes, and the allocation ends inside the first (or first-plus-one) `separate()` of width 22.

---

## 3. One mechanism: a post-cut far-side sweep before the first `separate()` of an explore bite

I will **not** name “make Algorithm 12 reachable at 10 s” (bite-local cap, impatient strikes, steal compress, skip 2002). That class has already lost.

- Economics treatment (work-strikes): at 10 s, seeds 4/7/8 got 3 strikes and **1 disruption** and stayed at 21 bites / 179.08. Seed 2 **regressed** from 48 bites to 21. Policy not promoted.
- Round 4 composed 30 s: seeds 7/8 got **7 and 6 disruptions** on bite 22 and still did not publish.
- 80/20 is frozen. Stealing compress from seeds 2/3 is how you destroy the only two 10 s qualifiers.

The table says the 10 s coin flip is **whether the first `separate()` of the 22nd centre-cut publishes**. Seeds that win (0, 2, 3, 6) mostly win inside that call (seed 0: 46 bites, **0 disruptions**). Retries are not the 10 s lever.

**Mechanism, one sentence:** after `homotopy::explore_bite` (centre cut, far-side translate, weight reset — unchanged) and **before** the first `separate()` of that width, run **one sequential Gauss-Seidel relocate pass over the cut-moved pieces that are still colliding**, then hand that state to the existing eight-worker `separate()`.

Grounding:

- Shelf probe (the campaign’s own 400-iteration bite-22 witness): `movedPieces = 34`, `widthAfterMm = 178.9865`, `minRawPhi = 2.00e-4`, `proxyBandReached = false`, `strikes = 0`, `disruptions = 0`. The first separate starts from the raw seam and burns 400 eight-worker tournaments without entering the band.
- The cheap prefix is 21 legal centre-cuts of a constructor packing. Those cuts do not rearrange. Bite 22 is the first seam the sequential relocate does not trivially close. Min-cut already changed *who* translates and made the 30 s median **worse**. This does not change membership, shrink, cut coordinate, GLS restore, strike literals, sample counts, or 80/20. It spends one Algorithm-5 pass on the 34 pieces the homotopy actually moved, before spending 91 % of leftover explore on eight clones of the unmoved seam.
- Sparrow `worker.rs:37–40` shuffles the colliding set. We do too (`colliding_permutation`). The deviation is declared: **order the first post-cut pass as far-side-first**, once, not as a new field, cut, or tracker policy.

Not a programme: no second cut, no disrupt injection, no Φ change, no visit-quota allocator (conflict-cluster is closed), no pool-rebase.

---

## 4. Gate that kills this proposal

Reuse the existing shelf probe, not a 10 s battery: **400 master iterations at bite 22**, from the 21-bite exact parent, composed plan, seeds **1, 4, 5, 7, 8** (the Round 4 10 s freeze set). Control is today’s first-separate. Treatment is cut → one far-side sweep → first-separate.

**NO before any quality battery if any of these fail:**

1. **Install.** On every seed, the sweep’s visit set is exactly `{i : i was moved by this `explore_bite` and is colliding after the cut}`. Empty visit set on a seed with `moved_pieces > 0` is `AUTOFAIL`.
2. **Not a no-op.** Post-sweep pose digest ≠ post-cut digest on every seed. Identity ⇒ the pass did not run; stop.
3. **Does not worsen the entry.** Post-sweep raw Φ < post-cut raw Φ on every seed. A raise on any freeze-set seed is `AUTOFAIL`.
4. **Cheapens the 400-iter shelf, or it is closed.** After the same 400 master iterations: treatment `minRawPhi` strictly below control on **at least 3 of those 5 seeds**, **or** treatment publishes and control does not on **at least 1 of 5**.

**Pre-declared “this did not work”:** (1)–(3) green and (4) red — the sweep changed the entry and did not cheapen the first 400 iterations of bite 22. Close it. No 10 s last-chance, no extra disrupt, no “maybe 800 iterations.” If (4) is green and a later 10 s battery still prints median 179.07 with 5/9 seeds at `exploreBites == 21`, that is a quality miss of a mechanism that engaged: the first-separate landscape moved and the lottery did not. Same closure. Do not then reopen 200/3, 80/20, or pool-rebase to rescue it.

---

**Bottom line.** The table is a geometry-and-search fact at explore bite 22, not a counter-artefact, once you drop the “every seed, 0.61 s, 94 %, 2 compress pubs” overclaim. Line 2002 is not the bug; the inner loop is already Algorithm 12, and Algorithm 12’s tickets have already been shown not to buy 10 s. The next spec should test one cheap pass over the 34 pieces the 22nd cut actually moved, and should be willing to die on the 400-iteration shelf probe.