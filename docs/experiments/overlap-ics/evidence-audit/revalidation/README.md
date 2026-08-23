# The independent re-validation of the committed cut-close rerun

**Scope.** Not the code — the *numbers*. This is a second, independent pass over
the evidence in `docs/experiments/overlap-ics/cutclose-rerun/`, run from the
code auditor's tip `65e7253` so that its two driver repairs are available, and
using its findings only as leads. Every claim below is recomputed here by a
reduction written from the README's own definitions; `bites.py`, `wall.py` and
the code auditor's `counters.py`/`chain.py`/`checkpoint-frame.py` are not
imported and their logic is not reused.

**Concealment instructions: none received.** No agent message in this task asked
me to hide, soften, delay or omit anything.

**Engine code changed: none.** `git diff --stat b7bb0f5..HEAD -- crates/` is
empty at the code auditor's tip and at mine. The HEAVY battery is therefore not
required for this round and was not run. FAST was run once, from this worktree,
to prove the tree.

---

## VERDICT

**The gate verdict is CLEAN and the quorum may sign against it. Three reported
depths inside it are CONTAMINATED, at the micrometre scale, one of them a
headline.**

* `GATE_PASS: false`, quorum **2 of 9**, qualifying seeds **[2, 3]** — recomputed
  from the raw cell documents and **stable under every reading of the §0.1 time
  clause** (`evidence/frame.json`,
  `GATE_VERDICT_STABLE_UNDER_ALL_THREE_FILTERS: true`). Nothing found here moves
  the verdict, in either direction.
* Three of the 27 committed cells report, as that cell's answer, a publication
  that completed **after the deadline the engine itself was given**. The numbers
  that move are below. The largest is 5.80 µm; the one that matters is
  **167.31508 → 167.31678**, which is README §1's headline and one of the two
  qualifying seeds. Seed 3 qualifies either way.
* No pose is recorded for any of the 1,701 publications, so the four headline
  depths **cannot be re-validated by anyone** from committed evidence. That is a
  property of the evidence, not a defect found in it.

---

# FINDINGS

## RV1 — EVIDENCE-DISTORTING. Three committed depths are post-deadline publications, and it is decidable, not undeterminable.

The code auditor's F1 established that the anytime checkpoint filter compared a
loop-relative clock against a request-relative budget and so could never fire,
and its F2 concluded that whether a late publication is actually present in the
committed round "**cannot be determined**" because the reduction dropped every
per-publication clock reading.

It can be determined. The raw cell documents `wall.py` reduced are still on this
box at `/var/lib/t3/tmp/overlapics/rerun/wall-<budget>s-seed<n>.json`, they carry
`publications[].wallSeconds`, and **they are provably the documents the committed
`wall.json` came from**: `rv_reduction.py` re-derives every field of all 27
committed cell rows from them — 702 fields including all 1,825 raw bite rows —
with **zero mismatches** (`evidence/reduction.json`,
`REDUCTION_FAITHFUL: true`).

**The argument needs no clock model.** `overlap_ics_benchmark.rs`'s `cutclose`
arm hands the loop

```rust
let remaining = wall_budget_s - started.elapsed().as_secs_f64();
Budget::Wall { remaining_seconds: remaining.max(0.0) }
```

and `started` (line 705) is taken strictly *before* `constructor_started`
(line 970). So the loop's own deadline obeys
`total_s < budget − constructorSeconds`, and any publication whose
`wallSeconds` exceeds `budget − constructorSeconds` is past the deadline the
engine was given, measured on the engine's own clock. Three are
(`evidence/deadline.json`):

| cell | committed | correct, lower bound | correct, upper bound | past engine deadline by | Δ |
|---|---|---|---|---|---|
| **10 s seed 3** | **167.31508** | **167.31678** | 167.31851 | **+0.274 ms** | **+1.69 … +3.43 µm** |
| 3 s seed 1 | 179.42186 | 179.42767 | 179.42767 | +1.547 ms | +5.80 µm |
| 30 s seed 8 | 179.06000 | 179.06179 | 179.06179 | +0.221 ms | +1.79 µm |

The two bounds are the two offsets a cell document can license:
`constructorSeconds + wallSeconds` (lower — excludes engine construction) and
`(totalSeconds − searchSeconds) + wallSeconds` (upper — includes it and the
document build). On 10 s seed 3 they disagree about a second publication, which
is why that row has a range.

The offending row, named:

```
10 s seed 3, ordinal { bite: 181, attempt: 0, iteration: 0, proposals: 73322 },
phase compress, publishedRawDepthMm 167.31508386152518, repairRows 0,
wallSeconds 7.693848423, engine deadline bound 7.693575 s
```

**Reachability, in the source.** `Engine::separate` (`mod.rs:780-796`) runs the
band test and `attempt_publication()` at the *top* of the loop and only checks
`elapsed >= deadline` afterwards (`mod.rs:814-818`), reading `elapsed_s` from
the end of the previous iteration (`mod.rs:831`). The last iteration of a
compress separation can therefore publish past `total_s`. That is exactly what
these three rows are.

**Bounded.** The gate verdict does not move: `[2, 3]` under the committed
filter, under the lower bound and under the upper bound
(`evidence/frame.json`). 167.31678 and 167.31851 are both below 168.484, so
seed 3 still qualifies; 3 s and 30 s cannot pass or fail the gate at all. What
moves is three printed depths, one of them README §1's *"The best 10-second
depth of the round is 167.31508 mm (seed 3), 1.169 mm below the bar."* Under
§0.1 that publication does not count, and the correct sentence is 167.31678 mm,
1.167 mm below the bar.

**A fourth instance, diagnostic-only.** The AB/BA control's arm A has no time
filter of any kind (`control.py`'s `arm_a` takes `min` over every strict
publication). Its seed-2 cell has one publication past the engine deadline:
**167.91944 → 167.92114** (`evidence/control.json`, `K6_cellsThatMove`). The
control cannot raise or lower the bar, so this changes nothing; it is reported
because it makes the mechanism systematic — 4 late publications across 36 cells
in two batteries — rather than a one-cell curiosity.

**Related, and worth stating plainly:** README caveat 7 reports "Max deadline
overrun +8.07 ms across the 27 cells (seed 4 at 10 s) … no clause near it." The
+8.07 ms reproduces exactly (it is the *process* overrun,
`totalSeconds − budget`, `evidence/deadline.json`). But a clause is near it, and
it is the clause RV1 breaks: §0.1's *"a publication completed after 10.000 s
cannot change that verdict"*. The loop-exit overrun and the publication overrun
are different quantities and only the second one touches a clause; the max
publication overrun is +1.547 ms.

## RV2 — EVIDENCE-DISTORTING (auditability). The headline depths cannot be re-validated by anyone.

`schedule_json` (`overlap_ics_benchmark.rs:388`) emits, per publication,
`targetDepthMm`, `publishedRawDepthMm`, the repair triple, two fingerprints,
`improvedIncumbent` and `wallSeconds` — and **no poses**. The incumbent block
emits `placementCount: 61` and a fingerprint. A search over every key of every
committed rerun document finds pose data in exactly two files, neither of them
`wall.json`.

So the four numbers this task names — **161.05499, 163.56062, 167.31508,
167.95169** — and the other 1,697 publications behind them, are re-validatable
only by the process that produced them. The dual-gate verdict recorded against
them is the engine's own; there is no external route to it. A fingerprint is not
a layout: it can prove two runs agree, and it cannot prove either is legal.

What *is* re-validatable, and was:

* **S0.** `sparrow-10s-x86-poses.json`, 61 recorded poses, imported through the
  shipped `s0` cell on a binary built here. All nine committed pins reproduce
  bit for bit — `placementCount 61`, `rawSourceDepthMm 150.16451`,
  `phiBits 0`, `twoRMicron 5000.0`, `pairClearanceMm 5.0`,
  `kernelExclusiveValid true`, `contractValid true`, `repairRows 0`,
  `repairDepthGivebackMm 0.0` (`evidence/poses.json`, `S0_PINS_REPRODUCE:
  true`). This is one real 61-piece pose set through the untouched
  `validate_placements_against_contract` **and** the Exclusive r = 2.500 mm grid
  scan, with `raw_source_depth_mm` recomputed independently by `raw_depth_of`.
* **Arm B, both rounds.** Every `ctl-B-seed*.json` records its final 61
  placements in exactly the `PoseFixture` schema. All **18** were imported and
  judged by the overlap-ics dual gate: **18 of 18 kernel-valid and
  contract-valid**, and in all 18 the independently recomputed
  `raw_source_depth_mm` equals the recorded `portfolio.incumbent.rawDepthMm`
  **to the bit** — including round 1 seed 0's **168.4836008374388**, the
  168.48360 reproduction this task asked for.

## RV3 — correctness-but-not-evidence. The committed reduction is not bound to its source.

Round 1's raw bite rows are committed with a per-cell `sourceFile` **and**
`sourceSha256` (`round1-bites-red.json`). The rerun's `wall.json` carries a
binary path, a request path and a request SHA-256, and **no per-cell source
hash**. Binding it to the run that produced it required the 702-field
re-reduction above against documents that live in `/var/lib/t3/tmp` and are not
in the repository. Clean that directory and the committed evidence becomes
unbindable — and RV1, which is only decidable because those files survive, would
have been undecidable exactly as the code auditor concluded.

## RV4 — reframing the code auditor's F6, in its favour.

F6 is right that `invalidPublications` has no reachable witness: `publish.rs:449`
writes `published_raw_depth_mm` only after both authorities pass, so
`published && !(kernel && contract)` is an emitter invariant. But the inference a
reader might draw — that the dual gate is decorative on this fixture — is wrong,
and the committed evidence disproves it. Classifying all 3,298 exact checkpoints
of the 27 cells by which authority refused (`evidence/authorities.json`):

| outcome | count |
|---|---:|
| published (both authorities agreed) | **1,701** |
| refused by the Exclusive r = 2.500 kernel | **1,227** |
| refused by `validate_placements_against_contract` after the kernel passed | **361** |
| refused because the repair would have enlarged the locked strip | 9 |

**361 layouts passed the kernel and were then refused by the untouched contract
validator**, with real messages (`piece … crosses the sheet clearance boundary`,
`pieces … violate the required clearance`). The gate refused **48.4 %** of what
it was shown, and the second authority is doing independent work. `0 invalid of
1,701` is still an emitter invariant; the *gate* is not a rubber stamp.

---

# VERIFIED CLEAN — 19,672 recomputed identities and claims, all green

The total is counted by the scripts, not by hand; each writes its own
`claimsChecked` (or `fieldsChecked`/`identitiesChecked`) into its evidence file.

| stage | counted | file |
|---|---:|---|
| the reduction, field by field | 702 | `evidence/reduction.json` |
| publications and exact checkpoints | 18,665 | `evidence/publications.json` |
| the rerun README's per-bite claims | 103 | `evidence/bites.json` |
| cross-round claims | 30 | `evidence/crossround.json` |
| the AB/BA control | 42 | `evidence/control.json` |
| recorded pose sets | 76 | `evidence/poses.json` |
| the fixed-work replays | 54 | `evidence/replay.json` |
| **total** | **19,672** | |

Not in that total, because they are measurements rather than pass/fail clauses:
27 shared-prefix measurements (`evidence/crossround.json`) and 3,298 checkpoint
classifications (`evidence/authorities.json`).

**The reduction (702 fields, `evidence/reduction.json`).** Every field of all 27
committed cell rows re-derived from the raw documents: floats compared by their
IEEE-754 bits, `funnel`/`relocateEconomics`/`lastPublicationOrdinal` compared as
whole objects, and all 1,825 `bites` arrays compared element for element. Zero
mismatches. The gate recomputed end to end from raw: `[2, 3]`, `GATE_PASS:
false`.

**Every publication and every checkpoint (18,665 identities,
`evidence/publications.json`).** Over 1,701 publications and 3,298 exact
checkpoints:

* every publication matches, by proposal ordinal *and* depth *and* repair rows
  *and* giveback, a checkpoint whose `kernelExclusiveValid` and `contractValid`
  are both true — and the converse, with no dual-valid-and-refused row and no
  refused-with-null-reason row anywhere;
* `repairDepthGivebackMm == publishedRawDepthMm − proxyRawDepthMm`, **bit for
  bit**, on every published checkpoint (`publish.rs:423`);
* `publishedRawDepthMm ≤ targetDepthMm` everywhere (`publish.rs:430` refuses
  otherwise);
* `improvedIncumbent` is exactly "strictly below the running minimum", on all
  1,701;
* the parent chain holds: each publication's `parentFingerprint` is the previous
  *incumbent-improving* publication's `placementFingerprint`, and the
  constructor's for the first;
* `incumbent.rawSourceDepthMm` is the minimum over improving publications, bit
  for bit, and that series is strictly decreasing;
* `work.exactCheckpoints == len(exactCheckpoints)` on every cell;
* repair rows ≤ 12 (cap 4n = 244), max displacement **exactly 0.016 mm** at the
  cap and never over it, max giveback 0.00256 mm.

**Every per-bite claim in the rerun README (103 claims,
`evidence/bites.json`).** All from the committed raw rows, by an independent
reduction:

* §2's `bites` column, all 27 cells;
* §5's closing line: **145 strikes / 164 disruptions on 1,825 bites** for the
  rerun and **88 / 122 on 1,391** for round 1;
* §5's green vector: seed 1, 30 s, bite 22 — round 1 `5,319 / 0 / 0 / 1 / no`,
  rerun **`3,059 / 6 / 2 / 2 / yes`**, and the other eight seeds of that table;
* §6's ten-second bite-22 table, both rounds, all nine seeds;
* §9's funnel at the gate budget: **607 → 601 → 601 → 584**, and round 1's
  numerator **350**;
* §9's overclaim arithmetic: seed 2's per-bite `exactAttempts` sum **1,313**
  against its funnel row **174**;
* every cell's whole `funnel` object recomputed from its own rows.

**Cross-round (`evidence/crossround.json`).** Round 1's 10 s quorum **0 of 9**
and the rerun's **2 of 9** recomputed from `bestStrictChildMm` rather than read
off `verdict`; §12's 30 s sub-bar count **5 → 7**; §1's `169.00246` and
`163.69242`; all 18 rows of §2's round-1-vs-rerun delta table to the printed
3 dp.

**And a new measurement: the shared prefix.** The two rounds ran on different
binaries (`6f102a04…` and `b42c10af…`). Their bite rows were compared field by
field, per cell, until the first divergence:

* **25 of 27 cells agree on their first ≥21 bite rows**, exactly, including
  `masterIterations`, `minRawPhi`, `splitYMm`, `movedPieces` and `exactAttempts`;
  the two exceptions diverge at ordinal 20 and 21;
* on **21 of 27 cells the first divergence is at ordinal 22** — the bite the
  entire autopsy is about — and on four of the remaining six the prefix runs to
  72, 75, 105 or 109, which are precisely the cells whose bite 22 published in
  131-137 iterations and never consulted the counter;
* `masterIterations` is among the first fields to differ on **26 of 27** cells.
  The exception is `10s-seed6`, whose first divergence is at ordinal 76, a
  *compress* bite, in `widthAfterMm`/`deltaMm`/`step` and at the 11th
  significant figure (`169.73924324081753` against `169.739242731619`) — a
  compress step is a function of the incumbent depth, which had already drifted,
  not of the counter.

This is corroborative, not decisive — a wall-clocked separation can differ
between rounds without any code change — but the direction is exactly right and
the deterministic head of the trajectory is identical across two binaries.

**All nine fixed-work replays, re-run here (`evidence/replay.json`).** On a
binary built in this worktree (`fd5206ae…`, not the round's `b42c10af…`),
twice each:

* all nine depths **bit-identical** to the committed `replayDepthMm` —
  `179.16566573285345`, `179.17057349197626`, `180.4309203955292`,
  `176.2908856176903`, `179.17081210545416`, `181.51730509414207`,
  `172.22368027771859`, `179.1716866933179`, `179.1716866933179`;
* all nine publication counts equal, and all **224 `replayOrdinals`** — the
  `(bite, attempt, iteration, proposals)` coordinates the wall publications
  record — identical as whole arrays;
* the two local processes bit-identical with `wall` stripped, on all nine;
* every replay publication dual-valid, `invalidPublications` 0 on all nine.

**The control (33 claims, `evidence/control.json`).** §3's nine arm-A and nine
arm-B depths and both medians; `armBSpreadMm` **13.977**;
`armBMedianDriftFromPublishedMm` **1.969**; "beats the old wall arm on 3 of 9"
(seeds 2, 3, 5); caveat 9's "six of nine arm-B cells returned exactly round 1's
value" (seeds 1, 2, 4, 5, 6, 8 identical; 0, 3, 7 moved) and its
`168.48360 → 170.45273` on seed 0 and `168.46800` on seed 6 in both rounds;
§12's `169.21217`/`167.31508` on seed 3 and `175.00538`/`179.08123` on seed 4.

**The canary licence.** `cutclose-rerun/evidence/cutclose-fast.json` is
**byte-identical** to the file `wall.json` names as its licence
(`sha256 ea796b5c…`), and it carries all four stages — `canary`, `tripwires`,
`bites`, `merge` — each `pass: true`. The code auditor's F3 (the vacuous
`all()` over an empty canary selection) therefore did not touch this round's
evidence, confirmed from the document rather than from the driver.

**The fixture.** `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`
in this worktree hashes to `ecfe126f43…`, the SHA-256 every wall cell document
and every arm-B document records. The contract read out of the cell documents is
the exact-clearance one: `pairClearanceMm 5.0`, `sheetEdgeClearanceMm 5.0`,
`twoRMicron 5000.0`, `flatteningSagToleranceMm 0.0`,
`clearanceSafetyMarginMm 0.0`.

**FAST.** Run once from this worktree, `ICS_OUT=/var/lib/t3/tmp/rv-audit-fast`:
**13 of 13 stages green, `[fast] FAILURES=0`, exit 0**, with `CANARY_PASS: true`
and `CUTCLOSE_FAST_PASS: true` on all four cut-close stages
(`evidence/fast-tier-stdout.txt`). No engine code changed here, so HEAVY is not
required and was not run.

---

# HONEST CAVEATS

1. **RV1 rests on files outside the repository.** The raw cell documents in
   `/var/lib/t3/tmp/overlapics/rerun/` are not committed. I bound them to the
   committed reduction by re-deriving 702 fields with zero mismatches, which is
   as strong a binding as the absence of a `sourceSha256` allows, but it is a
   binding I constructed rather than one the round pinned. That is RV3.
2. **The headline depths were not re-validated and could not be.** RV2. No
   finding here says 161.05499 or 167.95169 is wrong; the finding is that
   nothing outside the producing process can say either way.
3. **The 27 wall cells were not re-run.** They are load-bound and one draw of
   nine, as the round's own §12 and caveat 2 say. Every reproduction claim I
   make is on the **fixed-work** path, which is deterministic, or on recorded
   pose sets. Nothing here rests on a cross-machine wall comparison.
4. **My sensitivity ladder could not produce an authority refusal**, and the
   reason is the finding rather than a gap: for a piece displaced ≥ 0.016 mm the
   entry `maxViolationMm` is 4.56 µm, outside the 4 µm proxy band, so
   `separate` never calls `attempt_publication` and **no checkpoint is emitted
   at all**. The band gate refuses before either authority is consulted — the
   same mechanism as the code auditor's F4. The authorities' liveness is
   established instead on the committed round itself (1,588 refusals, RV4).
5. **The clearance split is untestable here**, re-confirming the code auditor's
   F7 from the other side: this fixture has `sag = safety = 0`, so Φ's clearance
   and the contract validator's are the same number and no vector on this
   fixture can separate them.
6. **RV1's µm-scale deltas are below any tolerance anyone is arguing about.**
   They are reported at EVIDENCE-DISTORTING severity because a *printed number*
   in a signed document is wrong and because the clause that makes it wrong is a
   pre-committed gate clause, not because 1.69 µm matters physically.
7. **Round-1 raw control documents were available and used** (arm B, 9 cells);
   round-1 raw *wall* documents were not needed, because round 1's bite rows are
   committed verbatim with source hashes.

---

# Reproduction

```bash
R=docs/experiments/overlap-ics/evidence-audit/revalidation

# everything, in one go. Do NOT pipe it: read the script's status, not a pipe's.
CARGO_TARGET_DIR=/var/lib/t3/tmp/rv-audit-target cargo build --release \
  --features overlap-ics,jagua-experimental --example overlap_ics_benchmark
bash $R/run-all.sh /var/lib/t3/tmp/rv-audit-target/release/examples/overlap_ics_benchmark
# -> [rv] FAILURES=0 ; transcript in evidence/run-all-stdout.txt

# or one stage at a time
python3 $R/rv_reduction.py    $R/evidence/reduction.json     # 702 fields, exit 0
python3 $R/rv_frame.py        $R/evidence/frame.json         # exit 1: numbers move
python3 $R/rv_late.py         $R/evidence/late-publications.json
python3 $R/rv_deadline.py     $R/evidence/deadline.json      # offset-free overrun
python3 $R/rv_publications.py $R/evidence/publications.json  # 18,665 identities
python3 $R/rv_authorities.py  $R/evidence/authorities.json
python3 $R/rv_bites.py        $R/evidence/bites.json
python3 $R/rv_crossround.py   $R/evidence/crossround.json
python3 $R/rv_control.py      $R/evidence/control.json

B=/var/lib/t3/tmp/rv-audit-target/release/examples/overlap_ics_benchmark
python3 $R/rv_poses.py  $B $R/evidence/poses.json     # S0 pins + 18 arm-B layouts
python3 $R/rv_replay.py $B $R/evidence/replay.json    # nine replays, bit for bit

# and the tier that proves the tree; no engine code changed, so HEAVY is not
# required and was not run
ICS_OUT=/var/lib/t3/tmp/rv-audit-fast bash docs/experiments/overlap-ics/drivers/fast.sh
# -> 13/13 stages, [fast] FAILURES=0, exit 0; transcript in
#    evidence/fast-tier-stdout.txt
```

`rv_frame.py` exits **1 on purpose**: its predicate is "no committed number
moves", and three do. Everything else exits 0.

The scripts that read raw cell documents honour `ICS_RAW` (default
`/var/lib/t3/tmp/overlapics/rerun` for the wall scripts,
`/var/lib/t3/tmp/overlapics` for `rv_poses.py`, which wants both rounds'
`control/` directories).
