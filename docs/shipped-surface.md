# The shipped surface: every flag and key, and what it is for

This is the map a session starts from. The ten-second campaign on
`engine/topology-archive-search` built one shipping stack and about thirty
levers around it, and roughly two thirds of those levers are **retired with a
signed negative** rather than merely unset. A reader who cannot tell those two
states apart will re-open a question three rounds have answered, which is the
failure Grok review 5 §"Prossime 3 spese" and Sol review 10 §"Prossime tre
spese" both close with.

So every row below carries a **verdict** and an **evidence pointer**, and the
verdicts are three:

| verdict | what it means |
|---|---|
| **ships-on** | armed in the shipping configuration. A caller who changes nothing gets it. |
| **ships-off-available** | off by default, and there is a *measured reason a caller might arm it*. It answers a question the default answers the other way. |
| **retired-with-negative** | off by default, and a measured battery says arming it is worse. Re-opening it needs a new mechanism, not another sweep. |

A fourth state exists and is called out where it applies: **instrument** - code
that exists to measure and is never quoted as production behaviour.

> **Scope.** This describes the campaign branch, not `main`. Everything here is
> reachable only through the benchmark example's portfolio spec (argument 48) or
> a Cargo feature; §4 says what a production route actually sees.

---

## 1. The shipping stack

The stack is **v3 + `plancal` + `m34pconfirm` + `fcv` + the interruption
refactor**, and the sentence that matters about it is:

> **Every one of those is off at the Cargo level, and the three that are
> "default on" are default on *inside v3*, which is itself default off.**

`PortfolioSettings::new` sets `coordinator_v3: false`
([`portfolio.rs`][v3-default]), and the test
`the_shipping_defaults_are_v3_plus_three_and_v3_is_off` is what holds that. So
the phrase "the shipping defaults" in this campaign's documents always means
*"the defaults a spec that says `v3=1` gets"*, and never *"what a caller who
links this crate gets"*. The second is the pre-campaign engine.

| component | verdict | armed how | evidence |
|---|---|---|---|
| `coordinator_v3` | **ships-off-available** | `v3=1` | `docs/experiments/coordinator-v3/`, `coordinator-v4/`. At 10 s v3 vs v2 is **0.000 mm on 6 of 9**; the 5 mm arrive at 30 s (`coordinator-v3/README.md:186-188,249-262`) |
| `plancal=<path>` | **ships-off-available**, and it is the recommended way to *run* the plan mode | `plan=<ms>,plancal=<file>` | `robust-plan/README.md:41` - one plan, one depth, **one whole document per seed, 60 of 60 under load**, and −2.792 / −1.869 / 0.000 mm against bare `plan` |
| `m34pconfirm` | **ships-on inside v3** | default `true`, opt out with `m34pconfirm=0` | `calibrated-plan/README.md:27` - **3.11x / 3.17x** per accepted confirmation on mixed-61, semantics-preserving in the work currency |
| `fcv` (clearance certificate) | **ships-on inside v3** | default `true`, opt out with `fcv=0` | `fast-contract-validator/` §13.2. Verdict-preserving on every input; all four pinned gates reproduce **in both states** |
| the interruption refactor (`advance_one_batch`) | **ships-on as structure, off as policy** | always compiled under `compression-schedule`; no policy installed unless a key asks | `real-interruption/` §4-§6. N batches reproduce the monolith on five instruments; stop-at-K is exact-valid and resume-later is bit-identical |

[v3-default]: ../crates/polygon-nesting-core/src/search/portfolio.rs

### 1.1 What the stack is worth, honestly

`plan=10000` on mixed-61 is **175.388** at p95 8.28 s and reproduces 3 of 3; the
wall arm is **168.484** at 10.30 s and reproduces 0 of 3
(`real-interruption/README.md:507-508`). The plan mode is **6.904 mm worse and
reproducible**, and that trade is the mode's whole purpose - not a defect to be
optimised away. `calibrated-plan` §9 decomposes the 6.904 into a bias constant
(3.741), the work counters (1.882) and the quantisation floor (1.281).

---

## 2. Cargo features

All of these are `default = []`. The **combo** every battery in this campaign
measures on is:

```
jagua-experimental,compression-schedule,parallel-compression-schedule,
continuous-rotation,sparse-rotation,fast-contract-validator
```

and the **gate** binary is `jagua-experimental` alone. A feature being in the
combo means *it is compiled*, not that its operator is armed.

| feature | verdict | note |
|---|---|---|
| `jagua-experimental` | **ships-off-available** | gates `search::portfolio` itself - without it there is no coordinator to configure |
| `compression-schedule` | **ships-off-available** | mode 34, the depth clock. Default build is byte-identical; an *armed* lane is not, which is why it needs a matched-arm gate |
| `parallel-compression-schedule` | **ships-off-available** | two independent levers: `pconfirm` (promoted, §1) and `lanes` (retired, §3) |
| `fast-contract-validator` | **ships-off-available**; armed by default once compiled | sound broad phase, verdict-preserving |
| `continuous-rotation` | **retired-with-negative** as a blanket operator | `continuous-rotation/README.md:20-21`: **+3.721 mm worse at 10 s, 0 of 9 better**. Still compiled because `sparse-rotation` is stacked on it |
| `sparse-rotation` | **retired-with-negative** | `sparse-rotation/README.md` §4.1: **−0.290 mm at 10 s on 4 of 6 seeds** (an unresolvable null at that spread) and **+1.483 mm worse at 30 s on 5 of 6** |
| `se2-rigidity-certificate` | **instrument** | never runs inside a request; one CLI branch behind `POLYGON_NESTING_SE2_CERTIFICATE` |
| `portfolio-ledger` | **instrument** | `O(members^2 * pieces)` at exit; reads the archive, never feeds a schedule decision |
| `search-profiling`, `relaxed-lane-census`, `constructor-census`, `rotation-tax-census`, `shadow-rescore`, `quality-trace`, `profiling-allocator` | **instrument** | measurement builds. Their milliseconds are a decomposition and never a wall-clock claim |
| `skip-pile-dump` | **instrument** | writes the frontiers `due_for_confirmation`'s feasibility clause suppressed, as poses, when `POLYGON_NESTING_SKIP_PILE_DUMP` names a path. Disarmed even when compiled; all four pinned gates reproduce as whole documents with it compiled. Implies `compression-schedule`. `skip-pile-diagnostic/` |
| `overlap-ics` | **Gate0-stopped** (retired-with-negative as a 10 s line; the geometry core survives as an instrument) | the overlap-tolerant continuous engine (converged spec `overlap-ics-converged-spec.md`). Built, autopsied twice, four implementation defects found and fixed with red-to-green evidence (clearance split at seven sites, the three-way-neutered jump, three latent defects, the rotation-pivot mismatch). Final battery: **five of six fatal cells green** (S0 bit-for-bit; S1 publishes inside its lock with no jump needed; triangle-20 repair 0.0 µm; soundness 1k/10k all zeros; throughput ~1M proposals/8 s) — **C175, the pre-committed separator, 0/3 on a clean machine**: the corrected descent reaches a reproducible attraction band ~1.6 mm infeasible from the 10 % shock (2.88 M proposals, zero publication attempts), and both refuters confirm the STOP under their own pre-committed readings. Falsified: the Round-1 solver MEMBER (one-piece strict-decrease PGS + GLS + one jump), NOT the mathematical family — a chain/two-endpoint member would be a new owner-funded project, not a retrofit. `homotopy.rs` deliberately a stub. **Round 2, `CutCloseRelocate`** (the quorum spec, both consultants signing the same member after reading Sparrow at `14f4868f`): the stub becomes a real 0.1 % split-and-close, the descent becomes a 25+50-sample global relocate with all-rows GLS and an eight-worker Algorithm-10 tournament, and the pre-committed gate is **FAILED at 0 of 9 seeds under 168.484 at 10.000 s** (best 169.00246). Not a defect: 1,269 publications, **zero invalid**, regression floor green in every clause, both pre-named defects hunted and closed on the round's own evidence. **5 of 9 seeds go under the bar at 30 s** (163.69-165.06), which makes it throughput rather than basin - and Sol R2 §6 pre-committed that eight workers already being seated voids the scaling follow-up that diagnosis would otherwise license. **Both implementation reviews then returned (A) on the same line** (Sol 18 §P0, Grok 13 flag 3): the inner strike predicate reset the 200-counter on ANY new raw-Φ minimum where the frozen spec and `separator.rs:102-115` require 2 %, so Algorithm 12 was starved on every stuck 22nd bite. **The one licensed repair and the identical rerun: still FAIL, at 2 of 9** (best 167.31508), floor green in every clause, the default-build gate binary byte-identical, and the green vector holding hard - the named red cell goes 5,319 iterations / 0 strikes / 0 disruptions / unpublished to 3,059 / 6 / 2 / published, 7 of 9 under the bar at 30 s, best-ever 161.05499. **The member closes on a now-faithful FAIL** | `overlap-ics/` (+ `gate0-verification/`, `gate0-rerun/`, `gate0-pivot-rerun/`, **`cutclose-round1/`**, **`cutclose-rerun/`**), `sol-review-14..18`, `grok-review-9..13`, `overlap-ics-converged-spec.md`, **`cutclose-relocate-spec.md`** |
| `round-envelope-kernel` | **ships-off-available** as a correctness surface; **retired-with-negative** as a 10 s lever | the exact integer disc authority (spec key `rek`: 1=union, 2=exclusive; example env door `POLYGON_NESTING_ROUND_ENVELOPE_KERNEL`). Zero false accepts outside the √2 µm band, Sparrow differential 4/4, envelope half 8x cheaper. The matched gate is a signed negative: 48/48 bit-identical searches, equal work 0/12 at 0.0000 mm, and the skip-pile census prices the released region at 0.80% worth zero depth. `rek=2` aborts a bare-request run (the miter's own 1 µm contact leak); `Exclusive` = canonical round oracle, `Union` = backward-compatible hybrid. Not promotable as-is: process-global arming needs a request-scoped policy first (sol-review-13). `round-envelope-kernel/`, `round-envelope-gate/`, `skip-pile-diagnostic/` |
| `import-gate-shadow` | **instrument** | Gate A's three-verdict census machinery (`import_gate.rs`, example `sparrow_import_gate`): scores a pose fixture under contract / composite-miter / composite-round / composite-square with per-pair `r*` bisection. Never on a search path; named by nothing in `src/` outside itself. `gate-a-sparrow-import/` |
| `fast-constructor-profile` / `-confirm` / `-reject` | **ships-off-available**, unpriced at 10 s | not bit-identical as documents (quota quantities move), so the pinned gates must be run against the default build |
| `relaxed-scan-shape-reuse`, `relaxed-cached-pose-bounds`, `relaxed-row-buffer-reuse` | **ships-off-available** | bit-identical as whole documents; unpriced against the 10 s contract |
| `relaxed-scan-order-proxy`, `canonical-pair-order`, `fast-proxy-hypot`, `fused-pair-query` | **retired-with-negative or measurement-only** | `fast-proxy-hypot` is explicitly rejected by corpus (one in six results differs in the last place); `fused-pair-query` measured at parity, so the default stays split |

---

## 3. The retired board

These are the levers the campaign armed, measured, and put down. **Each row is
a signed negative on a named instrument**, not an absence of evidence. Grok
review 5 §"Prossime 3 spese" item 1 is the instruction this section discharges:
*"ritirare dalla board race, `cur2=1`, `m34past`/`yield`/`grid1`/`confirm1`,
`lanes=8`, `adopt`, `crot`"*.

| lever | key | the negative | where |
|---|---|---|---|
| multi-basin race | `race=<arms:keep:rungs>` | **0 moves in 21 auditions.** mixed-61 equal-work cells **+2.366 / +2.934 mm**; 30 s equal-work row **+1.879 mm**. The cost diagnosis (m20 priced 71,500x low) is sound; the *selection* diagnosis is not, and Sol review 9 §P0 lists four defects that would have to be fixed before it could be re-opened at all | `basin-race/`, `work-currency/` §4.2, `sol-review-9-m34cap-provenance.md` §P0 |
| scheduler currency v2, charging | `cur2=1` | **median 0.000 mm.** Under a wall budget `debit_self_metered` returns zero by construction, so the currency cannot reprice anything; on seeds 0 and 1 the armed arm is identical to the unarmed one **to seventeen digits across three rounds** | `work-currency/README.md:579-595` |
| the bound unlocked | `m34past=1` | **+0.331 mm at 10 s** at full share, −0.264 / −0.221 at quarter and half share. The lever *works* - the first slice walks past 1.6160 mm and the exit stops being `bound` - and the run is worse for it. At 30 s it buys 2.000 mm at **9 of 9 overrun** and wallMax 47.96 s | `real-interruption/` §8, §9 |
| the interleave | `m34yield=2` | **+0.361 mm at 10 s** | `real-interruption/` §8 |
| first-slice density | `m34grid1`, `m34confirm1` | 12 cells x 2 budget modes. **Every cell exits on `bound` and drops exactly 1.6160 mm**; depth per thousand slice-units falls **25.7x** monotonically in both knobs. Re-tested with the bound unlocked in `real-interruption/` §10 and it is worse there too (**+3.570 / +3.618 mm**) | `robust-plan/` §13-§15, `real-interruption/` §10 |
| repair fan-out | `m34lanes=8` | **−2.158 mm at 10 s, 0 wins in 9**, and 9.6% *slower* per action | `parallel-compression-schedule/README.md:26-27` |
| witness adoption | `se2w=<...>:adopt=1` | 2 of 12 "descendant" is `final(adopt) < final(publish)` and proves only that the trajectory changed; the arms are not equal-work (10,150,405 vs 10,433,031 units on seed 1); and the adoption writes `confirmed_state` before the composite gate has passed | `sol-review-9-m34cap-provenance.md` §P0 (third), `basin-race/evidence/witnessab-12parents.json` |
| blanket rotation | `crot=1` | **+3.721 mm at 10 s, 0 of 9 better**; +7.071 mm at 30 s. The mechanism does what the certificate predicted - 46 of 61 pieces off-catalogue, 56% of removed proxy loss from rotation - and loses anyway | `continuous-rotation/README.md:20-46` |
| sparse rotation arming | `sparserot=1`, `roteq`, `rotbit` | the tax is genuinely gone (3.30x cheaper per rung, 1.064x a base slice) **and the gap is not**: null at 10 s, −1.483 mm at 30 s on 5 of 6 seeds | `sparse-rotation/` §4.1, §8.2 |
| max-of-k phase-0 probe | `planprobe=<k>` | saturates against its own clamp on **5 of 9 cells**, **19 of 30 over target on an unstressed box**, 43 of 60 under load. `robust-plan`'s recommendation is explicit: *"Do not arm `planprobe`"* | `robust-plan/` §8-§10, `README.md:60` |
| the in-run re-plan, as a *quality* mechanism | `replan=1` | **2.808 mm on one seed, 0.252 mm of median.** It adds a second clock reading, which makes the load axis worse (4 / 2 / 3 distinct depths against `plan`'s 2 / 3 / 1) | `next-generation-engine-plan.md:6908-6913`, `replan/` §11.1 |
| `m34cap` as a stop | `m34cap=1` | **RETRACTED.** It could not stop the slice at the HEAD its evidence was taken on: `advance` recorded a checkpoint and left `finished=false`, and the caller looped to the end of the monolith. `m34cap=0` and `m34cap=1` produce identical depth, fingerprint, work, call count and step digests | `real-interruption/` §2, `replan/README.md:3-42`, `sol-review-9-m34cap-provenance.md` §P0 |
| the m26 short ladder in the 10 s band | mode `26` (`m26:1rung`, and uncapped `m26:drop1.0`) | **CUT — strict kill 5/5 control budgets, weak kill 4/5, the designated work-matched control under both; 0 of 12 parents below the control at matched median work.** 12 from-request parents 171.614–179.620. Median **0.2332 mm against 7.0129 mm**, **0.1547 against 1.2991 mm per M coordinator work unit — 8.4x**; the control at one tenth the arm's work still has the better median (0.2534 vs 0.2332). The uncapped 6-rung ladder: 3.3784 mm at 45.5 M units against the control's 12.1095 mm at 17.5 M. **Arm C reproduces** (−5.73 / −8.29 mm at seeds 0/1) — the mechanism is real and the comparison is lost anyway, because the control *is* mode 26's own shipped port (`compression_schedule.rs`). Re-opening needs a new mechanism — not another sweep, and not the ULP fix (a fifth of a 20% abort rate against an 8.4x deficit) | `m26-band-audition/` (§8, Errata), `mode26-rung-anatomy/` §3 |

### 3.1 The three keys this consolidation added

They are in neither list above because they are new, and two of the three are
**ships-off-available** rather than retirements. `docs/experiments/consolidation/`
is the round.

| lever | key | verdict | what it is worth |
|---|---|---|---|
| the lane-local debit | `lanedebit=1` | **ships-off-available**, and it is the one a deployment should consider | a work or plan budget runs with `profiling::set_enabled(false)`, taking the meter's two counters from their own flag. Measured at identical work with **identical documents on 9 of 9 cells**: the same work in **84.9%** of the seconds at 24.9 M units, **82.5%** at 120 M. End to end on a calibrated plan: either **−1.108 mm** at ten seconds (with its own calibration file) or **the same depth and the same document at p50 7.31 s → 6.27 s** (with the incumbent's) |
| the wall stop, all classes | `m34wallstopall=1` | **ships-off-available** as a 30 s dial, exactly like `m34wallstop` | on a forced overrun, worst overrun **+26.63 s → +0.99 s**, 6/6 exact-valid. On the calibrated 30 s battery: **+0.000 mm** of median depth, worst overrun **+12.38 s → +1.31 s**, and the *count* of crossings does **not** reach 0 of 9 |
| the wall reserve | `m34wallreserve=<multiple>` | **retired-with-negative** | **worse** than the plain admission rule: +1.87 s against +0.99 s worst overrun. It prices classes by mean seconds, mode 34's mean is the largest, so near the deadline it refuses the one class that can stop itself mid-action and buys an uninterruptible one instead |

Two corrections this round applied to the board above rather than to itself:

* `calibrated-plan` §9's *"the work counters… there is no version of this mode
  that avoids it"* is **struck**. The +1.882 mm reproduced to four decimals and
  then split: the counting is +0.000 mm median on all three seeds and the
  *timing* is the whole of it. One flag armed both, which is why the round that
  measured it could not have attributed it.
* `work-currency` §6's specified fix - lift `surrogate_evaluations` onto the
  relaxed lane - would **not** have worked. The meter's exact half is 27% of it
  (2.93 M of 10.79 M units on a measured run) and lives in `kernel::exact`,
  which has no lane.

### 3.2 The one row that is not a retirement

`m34wallstop` is **not** on the board above, and the distinction is worth being
exact about because it is easy to file it there:

> **`m34wallstop` is a dial of the thirty-second contract, not a ten-second
> quality lever.**

At 10 s it is **0.000 mm** and changes nothing, because the plan arm is already
0 of 9 over target there (`real-interruption/README.md:432-438`). At 30 s it
holds `base`'s depth **exactly** and takes wallMax from **36.42 s to 31.98 s**,
with 3 of 9 still crossing. That is a real property with a real price - the
depth becomes a function of the box, because a wall stop reads a clock - and it
is why the key ships off and the §11 anytime table reports both ends of the
trade.

`docs/experiments/consolidation/` §6-§9 extends it to the classes that own no
checkpoint and measures what that closes: the overrun's *size*, not its count.
See §3.1.

---

## 4. What a production route actually sees

None of the above. The napi and CLI routes do not construct a
`PortfolioSettings` with `coordinator_v3 = true`, none of the six campaign
features is in `default`, and every key in this document is parsed by
`general_request_benchmark`'s spec argument, which is a benchmark example.

That is stated in nearly every round's honest-caveats section
(`real-interruption/README.md:629-632` is the most recent form) and it is the
single most load-bearing fact about this map: **the campaign has been measuring
a configuration, not shipping one.** Promoting any of it to a production route
is a separate decision with its own gate, and this document is not that
decision.

---

## 5. Reading the evidence safely

Three traps this campaign has already fallen into, so a future session does not
have to re-discover them:

1. **A quiet-box number is not a property of the algorithm.** `calibrated-plan`
   §8.2's *"one plan, one depth, one document per seed, 60 of 60"* is true on an
   unloaded box and false under contention, where the same arm produced 2 / 3 / 1
   distinct depths (`replan/` §11.1). §8.2 now carries that qualification in
   place; `plancal` is the fix and its own 60/60 **is** measured under load.
2. **A key that nothing emits fails silently under an armed label.** The
   `m34cap` retraction began as an evidence file whose `spec` field named a key
   its own committed driver could not generate. Both the interruption keys and
   this round's keys carry a round-trip test for exactly that reason.
3. **A digest is not a certificate.** `replan/README.md:126`'s *"same digest
   walked the same walk"* is false: FNV-1a over clamp, counts and aggregate loss
   contains no placement fingerprint, no RNG state and no winning lane, so two
   different walks can agree without any collision (`sol-review-9` §P1). The
   sentence is struck in place and `real-interruption` §4's three SHA-256
   fingerprints are the repair.
4. **A wall-budget arm cannot carry a millimetre between sessions.** The same
   counter tax has been published at 2.700 / 1.527 / 1.882 mm
   (`calibrated-plan` §9), 7.553 / 10.400 / 4.006 mm (`work-currency` §6) and
   1.177 / 10.400 / 1.882 mm (`consolidation` §2.1) on the same three seeds.
   The *medians* agree; the per-seed numbers are the arm's own spread. When a
   claim can be made at a fixed **work** budget instead - where the documents
   are provably identical and only the seconds move - it should be
   (`consolidation` §3).

---

## 6. Where the next spend is, per the two strategic verdicts

Both reviewers converged, from different directions, on the same shape:

* **Grok review 5**: the lever ledger is closed. 150 @ 10 s on this box needs an
  idea outside `{m20, m22, m23, m26, m31, m33, m34 + rotation + overlay +
  race}`. The wall-reference 168.5 *is* the honest ceiling Grok review 1 put at
  ~166.
* **Sol review 10**: *"il wall engineering può probabilmente portare 175 → circa
  169; 169 → 150 richiede una nuova azione di ricerca, non un altro scheduler"*,
  and the new action it names is an **active-contact block SE(2)** - a joint
  `Δx, Δy, Δθ` over a connected component of near-binding contacts, applied as
  one block, with a matched-arm gate *before* integration.

The owner's decision after both: consolidate the shipped surface first (this
document, `docs/experiments/consolidation/`), then start the active-contact
block SE(2) research with a matched-arm gate before integration. **No
intermediate governor round.**
