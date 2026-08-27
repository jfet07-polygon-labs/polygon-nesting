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

---

# Round 2 — cross-exchange

Grok was sent the publication-gate census (which neither reviewer had when
they answered round 1) together with Sol's complete round-1 answer, and asked
to converge, to name the one mechanism in final form, to merge the two gates,
and to say honestly which bar the mechanism is aimed at. Verbatim below.

# Bite 22 — cross-exchange, round 2

## 1. Converge: **(a) I adopt theirs.**

The census does not sit evenly between the two proposals at the clock that actually refuses bite 22. It classifies the refusal.

On the 10 s wall microscope, unpublished explore bite 22 is in-band and then discarded **entirely** by `proxy_depth > T`:

| seed | bite-22 master iters | band entries | exact checkpoint calls | published |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 1,090 | 1 | **0** | no |
| 4 | 1,599 | 3 | **0** | no |
| 7 | 1,114 | 1 | **0** | no |
| 8 | 1,151 | **215** | **0** | no |

The whole-run `called` counts on those freeze seeds are the cheap prefix (plus one compress consolation). They are not failed exact attempts at ordinal 22. Compress bite 24 is the same seam: seed 7 has 44 in-band entries and 0 calls; seed 8 has 215 and 0. Pose-digest is 0–3; the improvement gate is 0; the excess is 1.5–4.0 µm, bounded by the band that admitted it; every above-target refusal is a full 0.1 % bite better than the incumbent.

That is Sol’s terminal lip, counted. It is also why I withdraw the post-cut far-side sweep from this specification. My 400-iteration shelf probe still has `proxyBandReached = false`. That reading is true of the **first third** of a 10 s bite-22 `separate()`. It does not describe the 10 s refusal: by the end of that call the band has been reached, and every in-band state is then thrown away at `publish.rs:364` before Exclusive, before frozen-θ repair, and before the contractual validator. A Gauss-Seidel pass over the 34 cut-moved pieces has no witness in this census. The lip has thousands.

I do **not** claim the entry-side operator is refuted. I claim it is not what this round should fund. If Gate 0 fails because the new path **never fires** on ≥3 freeze seeds (leftover work never in band), that is a new-spec question about entry, not a retune of this repair, and not a rescue of this gate.

The 400-iteration unreadiness **does** kill my proposed gate as the instrument for *this* operator: a terminal repair that has not yet been offered a band entry cannot cheapen `minRawPhi` in a window where it does not run. That is a gate-merge point, not a reason to keep the sweep.

Diagnosis and `mod.rs:2002` stay as we both had them. One disagreement of wording, not of mechanism: I still would not call the 10 s median a throughput deficit. Equal explore units, five seeds at 21 publications, is a success-rate collapse at one width. More work raising P(escape) is the 30 s / 60 s column, which both of us already said.

## 2. Mechanism, one sentence

When `max_g ≤ 4 µm` and `0 < proxy_depth − T ≤ 4 µm`, clone the state and run the existing frozen-θ, ≤16 µm, ≤4n-row Gauss-Seidel repair with the locked-strip top **injected as a repair boundary** (Exclusive kernel, 4 µm band, 16 µm cap, target `T`, and contract validator unchanged), then publish only if the result is dual-valid and `raw_depth ≤ T`.

That is not “delete `publish.rs:364`.” Sol is right that the Exclusive scan uses the physical sheet (`inset_box` is sheet inset, not `T − depth_top_inset`), so a 1.5–4 µm proud state can already be Exclusive-admissible and still die at `published_depth > T`. The new operation is the T-row. The current boundary corrector already pushes in `[0, −1]` and the loop already re-scans; it has never been shown a strip-top shortfall because the pre-gate returns `None` first.

Frozen, as both of us already required: no `2002` change, no 80/20 steal, no 200/3 retune, no tracker-rebase, no persistent lanes, no millimetre legalizer.

## 3. The gate, final form

**Instrument (Sol’s, not mine).** The nine Round 4 composed bite-21 exact parents, leftover explore work of that arm’s 10 s plan, mixed-61, `--orders=1 --workers=8 --edge=5 --pair=5`. Control is today’s `attempt()`. Shadow is the T-row repair, allowed to publish. ComputeIgnore runs the same repair and discards it.

I am dropping the 400-iteration shelf probe, the visit-set / digest / Φ-drop install checks, and the “cheapen `minRawPhi` on 3/5 **or** publish on 1/5” clause. Those were the right NO-conditions for an entry sweep. They are the wrong NO-conditions for a terminal repair.

**Seed sets, unchanged.** Control closures `{0, 2, 3, 6}`. Freeze set `{1, 4, 5, 7, 8}`.

**PASS only if all of these hold:**

1. **Partition.** Control publishes bite 22 on `{0, 2, 3, 6}` and does **not** publish bite 22 on `{1, 4, 5, 7, 8}`. I do **not** require SHA identity with the historical Round 4 `gate10` JSON unless the driver continues a recorded engine checkpoint. The required fact is the closed/open partition under this leftover-work driver, plus two-process identity of *these* cells.
2. **Install.** On every freeze seed whose control leftover census records `aboveTarget ≥ 1` at bite 22, Shadow must enter the T-row path at least once. Zero entries on a lip witness is wiring, `AUTOFAIL`. A freeze seed with `aboveTarget = 0` cannot count as a conversion; it is a miss.
3. **Conversion.** Shadow publishes bite 22 on **at least 3 of 5** freeze seeds `{1, 4, 5, 7, 8}`.
4. **Cause.** Every new bite-22 success is from a state the current three pre-gates would have classified as in-band, improving, and `proxy_depth > T` only. `ics-publish-census` is the witness. A conversion that went through today’s `proxy_depth ≤ T` path does not count.
5. **No reverse.** None of `{0, 2, 3, 6}` loses its bite-22 publication; none of those four ends worse than control.
6. **Authority.** Every publication has `raw_depth ≤ T`, per-piece displacement ≤ 16 µm, Exclusive `r = 2.500`, contract-valid, independently revalidated; zero invalid publications.
7. **Cost / identity.** ComputeIgnore matches control on poses, publications, fingerprints, work vector and pacer charges after stripping timing and the diagnostic record, and keeps at least 95 % of control’s paired rate. Two-process identity on every seed.

**Pre-declared FAIL, and now closed:** fewer than three freeze conversions; any reverse; no causal depth-gated witness; T / 16 µm / Exclusive / contract broken; ComputeIgnore diverges or loses more than 5 % rate; two-process identity fails. Reading: the 4 µm depth lip is not legalizable with the existing frozen-θ repair. No 3/10/30 quality battery. The 10-second quality gate stays retired. The far-side entry sweep is not licensed as a rescue of this gate.

Where I disagree with Sol’s gate text:

> “il controllo riproduce bit-for-bit le quattro chiusure note `{0,2,3,6}`”

The closed/open partition, yes. Bit-for-bit against the Round 4 cell files, only if the driver is a recorded checkpoint continuation. A parent restart is a new RNG/GLS stream.

## 4. Sufficiency: **30-second quality improvement. 10 s is reported, not claimed. Do not reopen the retired 10-second gate.**

Where I disagree with Sol:

> “Un PASS licenzia soltanto una nuova batteria end-to-end col vecchio bar non ammorbidito”

I agree the bar is not softened. I disagree that a mechanical PASS licenses a 10-second reopening attempt. The unsoftened 10 s clauses are still `5/9 ≤ 168.484` and median `≤ 168.484`. Clearing bite 22 is necessary for those clauses and not sufficient, which both of us already said, and the census makes the arithmetic worse rather than better.

- 168.484 / 182.976 at 0.1 % is ~83 successful explore bites; from the bite-21 parent that is still ~62 publications.
- Seeds 0 and 6 **already** win the 22-lottery at 10 s (46 and 74 bites) and finish at 170.44 and 169.35.
- Seed 5 on the wall microscope **does** publish bite 22 (1,142 iterations, `minRawPhi = 0`) and then reaches 41 explore bites / **175.00 mm** — twenty bites of cascade, still 6.5 mm above the bar.
- Freeze bite 22 costs ~1,100–1,600 master iterations. The first 400 do not enter the band. A T-row conversion at the first in-band state is therefore a late-clock event on a budget that has almost nothing left for a 62-bite cascade.
- So even a perfect 3/5 or 5/5 conversion at this seam can easily print a 10 s column that is still 2/9, median still 179-class or 175-class. That would be a quality miss of a mechanism that engaged, and it would not be a reason to reopen 200/3, 80/20, or the sweep.

What this mechanism can honestly move is the **30 s frozen tail**. Round 4 already passes 7/9 at 30 s; the two unpublished 21-bite cells are seeds 7 and 8, the same seeds that sit on 5,885 in-band / 0 exact calls. That is the lip with thousands of shots and leftover time to cascade. After a Gate 0 PASS, the quality battery is the composed 30 s arm under the standing unsoftened 30 s clauses (median ≤ 163.00461 mm, 7/9 ≤ 168.484 mm, paired gain ≥ 1 mm, zero invalid), **plus** a tail clause that is a tightening, not a softening: seeds 7 and 8 at or below 168.484 mm. 10 s cells are run and reported against 5/9 / median / 168.484. They do not reopen the retired gate, and a 10 s 5/9 miss does not close a 30 s tail conversion.

Gate 0 remains allowed to say NO on the 10 s leftover clock. That clock is where control and Shadow actually differ on `{1,4,5,7,8}` (at 30 s leftover, control already closes 1, 4 and 5, and 3/5 would be vacuous). A 10 s leftover FAIL still means this repair is closed. It does not mean “try the sweep on the same gate.”

---

# Ballot on the frozen specification

Grok returned a single `AMEND` on the same sha256, and it is a soundness
repair rather than a preference. Verbatim:

**AMEND.** In §4, replace

`paired median gain >= 1.000 mm over the T-repair-off control;`

with

`paired median gain >= 1.000 mm over the Round-4 frozen-member control (the standing pairing), not over T-repair-off;`

Without that replacement the thirty-second battery is unsound as a test of this mechanism. Seeds 7 and 8 are the eighth and ninth of nine; converting both to `168.484 mm` — the movement §0 and §4 themselves name as the claim — leaves the median at the current fifth seed, `162.94241 mm`. Seed 8’s own 60-second cell lands at `167.563 mm`. A 1 mm paired-median demand over T-repair-off therefore fails the honest success of the tail clause. The standing 1 mm is composed versus the frozen member, already `1.232 mm` at this median. Asking it again of this repair over itself-off tests a median promise neither the tail clause nor the ten-second-residual Gate 0 makes.


---

# Autopsy — the T-row as built

Same charge, answered independently. Verbatim:

**Paradigm, not a bug. The implementation is the mechanism I signed. The finding explains a miss; it does not void the instrument. Nothing on this mechanism is licensed next.** These wall runs are not Gate 0 and I am not closing Gate 0. The structural result is still enough to answer the four charges.

---

## 1. Paradigm. The 16 µm cap and the 4 µm per-row guard are not the same quantity, and refusing a 12 µm row with 244 rows unspent is the repair’s declared competence, not a defect in it.

The 4 µm band is **derived grid disagreement**, not a walk budget. `EPSILON_GRID_MM` is `2 * ceil(sqrt(2) * 1 µm)`: the most `GridSet::of` can move two rings toward each other. The module header states the competence in one sentence: a source-faithful Φ at zero may disagree with exact geometry **at grid scale and nowhere else**; a repair that returns half a millimetre is a broken proxy wearing a legalizer’s coat, and the checkpoint is discarded. Grok 9, which this engine still runs under, said the same thing as a publication rule: *repair only inside `ε_grid = 4 µm`, freeze θ, cap 16 µm/piece, else discard the checkpoint. 16 µm is quantization.* The unit tests lock both sides: a 3 µm pair deficit publishes (`a_four_micrometre_deficit_is_repaired_inside_the_same_strip`); a 0.5 mm deficit is thrown away even after the attempt band is widened so the repair is what refuses (`a_half_millimetre_deficit_is_discarded_rather_than_legalized`).

Those two numbers do different jobs:

- **Per-row guard.** This residual is a grid-scale disagreement. If a single constraint is 12 µm infeasible, the state is not “nearly legal in the proxy-vs-exact sense.” `repair_one_row` classifies that row as outside competence and returns `None` for the whole repair. That is an admissibility test on the residual, not a step size.
- **Per-piece cap.** Gauss–Seidel may apply several *in-band* corrections as rows interact. Each accepted correction is `shortfall + guard`, so a max-band row already moves 8 µm. `4 * ε_grid = 16 µm` is room for about two such corrections on one piece, not a licence to spend 16 µm on one packing move.

A 12 µm pair shortfall would itself demand `correction = 16 µm`. After the T-row has already spent ~6–8 µm on the proud pieces (the 11-row checkpoint: 8 µm spent, depth pulled 4.55 µm *below* a target it was 1.6 µm above — that is `shortfall + guard` on a 1.6 µm overhang), the remaining cap on those pieces is ~8 µm. The guard is refusing a row that would also fail the cap on the pieces that created it. `blockedDisplacementCap = 0` on the tail is not unused headroom; it is the guard firing first. Lift the guard and a large slice of the 8–16 µm mass becomes a cap refusal, and the entire 16–32 µm bucket (882 of seed 7’s 4,284) is already `correction = 20–36 µm > 16`.

**Bounded step and re-scan would be a different operator.** The loop already re-scans after every *accepted* in-band row. That is what the `4n` budget is for. Applying `min(shortfall, 4 µm)` to a 12 µm row and continuing would turn the guard into a step size and the micro-corrector into a small PGS. The inflation test exists to forbid that. I will not call “the budget was unspent” a bug: the budget is for iterating in-band rows, not for chopping a row the classifier has already rejected.

Two corrections to the charge.

- **“All 244 rows unspent” is true as “never exhausted” (`blockedRowBudget = 0` on every seed) and overstated as “every attempt starts at row 0.”** The 11-row checkpoint you cite is a repair that *did* spend in-band rows, then hit an out-of-band pair. Leftover budget is the classifier firing, not a stuck loop.
- **The 812-row `≤4 µm` bucket is not a second mystery.** `blocking_row` re-derives the first failing row and the closed-form criticals and then stops. It does not reproduce `sheet_slack`, and on pairs it also does not reproduce the displacement cap. Those 812 passed the guard and then had no remaining strip room along the pair normal — the T-row having already spent the Y slack the pair now wants back. Same packing as the 8–16 µm mass, at a distance the guard still accepts. I agree they are not explained *by the guard*. I do not agree they are unattributed in kind.

The 4 µm band was never “we will absorb 4 µm of depth by creating 12 µm of overlap and then legalizing that too.” On the tail, `max_g ≤ 4 µm` already allows pairs that are 4 µm infeasible in the proxy; the T-row then pushes the front by `shortfall + guard ≈ 6–8 µm` (three failing boundaries per eligible state on seed 7: 12,824 / 4,284). That is a structured Y-shock, not a grid disagreement. The guard is doing the job it was derived for.

---

## 2. Yes. `inset[3] = min(inset[3], T − depth_top_inset + expansion)` is the faithful reading of “the locked-strip top injected as a repair row.” I do not want a separate row type for this miss.

I signed, in round 2, *“injected as a repair **boundary**”*, and I wrote why: the current boundary corrector already pushes in `[0, −1]` and the loop already re-scans; it has never been shown a strip-top shortfall because the pre-gate returns `None` first. Tightening the kernel box *is* showing it that shortfall. Sol’s “repair row, rechecking that row and every exact pair and boundary row after each correction” is the same GS loop that already exists.

The coordinate is the grid encoding of `raw_depth ≤ T`, not a heuristic.

- `raw_source_depth_mm = max source y + sheet_edge_clearance_mm`
- `depth_top_inset_mm = sheet_edge_clearance_mm` on this contract, so `raw_depth ≤ T` iff `max_y ≤ T − depth_top_inset`
- `boundary_admissible` requires `max_y_grid + radius ≤ high_y` with `radius = expansion_mm`
- `t_row_far_y = T − depth_top_inset + expansion` is the `high_y` that states the same inequality

On mixed-61 the physical sheet top is 2697.5 mm against a strip top near 176 mm, so `min` is always the strip. A piece 2 µm proud of `T` has top slack ~2,498 µm against millimetres on the other three sides. Binding-side picks the top. **Starvation behind three other sides is a real design question and this autopsy refutes it as the failure mode:** `blockedOnBoundary = 0` on seeds 7 and 8. The T-row is processed and it clears. A separate row type that the binding-side rule cannot starve would have produced the same Y-push, then the same pair cascade. Pair admissibility does not use the box; tightening `inset[3]` does not shrink the pair world. The cascade is the push.

I will not recommend a separate T-row type that pushes only the continuous excess, without `+ guard`. That would be a different correction formula than the existing boundary row, which is always `shortfall + guard`. We signed the existing repair. The 1.6 µm → 4.55 µm-below overshoot is that formula, and it is load-bearing for how large the pair shortfalls become. Changing it is a new operator.

One small infidelity, not the miss: `eligibleWithTRow` can be strictly less than `eligible` (seed 8 at 30 s: 1,510 / 1,527; seed 7: 4,284 / 4,284). The injection is in grid space; the target is continuous. A sub-micron continuous overhang can vanish on `GridSet::of`, so a `t_row_eligible` state can have zero failing far-`y` rows on the first scan. Spec §3 clause 4 would not count those as conversions. They are not why 7 and 8 fail.

Three arms, entry gate relaxed only for `0 < proxy_depth − T ≤ band`, final `published_depth > T` untouched: that is §1 as written.

---

## 3. It explains the verdict the specification already named. It does not change it into an instrument failure.

§3’s pre-declared FAIL is “either seed 7 or seed 8 does not convert,” reading: *the depth lip is not legalizable with the existing frozen-θ repair.* §5 already named the way this was allowed to die: *an overhang that reproducible may be one the repair cannot legally push.* The overhang *can* be pushed — that is `blockedOnBoundary = 0` and the 11-row checkpoint. What cannot be legally pushed, inside this repair’s competence, is the pair cascade the push creates. That is the same miss, diagnosed.

Instrument invalidity, as we wrote it: the control partition does not reproduce; an eligible digest bypasses the repair; too few unique eligible states; wiring (`eligibleWithTRow = 0` on a lip witness). None of those hold here. Seed 7 at 30 s: 4,284 eligible, 4,273 distinct above-target digests, 4,284 with a T-row, 0 conversions. Seed 8: 1,527 eligible, 1,510 with a T-row, `exploreBites` still 21. Eligible states arrive by the thousand. The T-row is wired. The existing repair then refuses a residual outside `ε_grid`. That is a mechanism result.

**“The T-row handed the repair a problem outside the repair’s competence” is the content of the miss, not a reason to void the instrument.** We signed the *existing* repair, froze the 4 µm guard, the 16 µm cap, and the `4n` row cap, and forbade a millimetre legalizer. Asking the existing repair whether a 2–3 µm depth lip is a grid-scale disagreement, and being told “no, it is an 8–16 µm packing cascade,” is the experiment we specified.

Two refutations of the table-as-clause-3 reading.

- These are **wall runs**, not the residual fork. They cannot close Gate 0. I am answering the mechanism, not stamping `GATE0_FAIL`.
- Seed 8’s census `published: 1` at 30 s is **not** a bite-22 conversion. `exploreBites` remains 21 and `depthMm` remains 179.007. Clause 3 is “publishes bite 22,” not “`attempt` returned `Some` once in compress.” Your prose (“neither converts”) is the right reading; the table’s `converted = 1` is not clause 3.

If Gate 0’s residual fork later prints the same partition — 7 and 8 eligible, T-row clears, pair-blocked, no bite-22 publication — I hold the spec as written. The autopsy does not convert that into an invalid instrument.

---

## 4. Nothing on this mechanism. The 8–16 µm pair cascade is nameable as a new specification’s subject only if someone independently funds a different operator; this miss does not license that spec, and the 37 conversions plus seed 4’s +8.28 mm do not re-aim this gate, because that would reopen the retired ten-second chase on a seed the 30-second control already closes.

The entry sweep is not licensed: eligible states arrive. A bounded-step or 16 µm-per-row repair would be a packing legalizer, which this repair exists to refuse. Seed 4 at 10 s wall is the 30-second control’s work done early; we already refused to reopen 10 s on that arithmetic. The 37 legal conversions prove the wiring and that *some* lips are grid-legalizable. They are not a quality clause, and seeds 2, 3 and 6 finishing worse on a wall clock are a warning, not a pairing.