# The m26 band audition — the never-gated short ladder at 171–179, and the
# control it turned out to be racing

Kimi review 1 §1 named one falsifiable gate and pre-committed its kill rule:

> **Gate falsificabile (≤1 round, harness esistente)**: i 12 parent from-request
> 171.614–179.620 di `contact-block/drivers/parents12.json`, il controllo
> verbatim di `contact-block/drivers/matched.py` (slice m34 seriale
> `past=1,work=W` dallo stesso parent). Braccio: un rung m26 corto (drop 1.0) →
> m31, cappato allo stesso W. **Kill**: se il braccio ladder non batte la
> mediana del controllo di ≥1mm **o** non scende sotto il controllo su ≥8/12
> parent, m26 è tagliato dalla banda 10s con evidenza — e si chiude per sempre
> anche la spesa rimasta aperta.
>
> **Aspettativa onesta**: pass/no-pass è ~50/50.

This round ran it. **The arm is CUT, at every control budget measured, including
one that gives the control ten times less work than the arm.** The retirement
entry for `shipped-surface.md`'s board is §8.

The verdict is *not* that Kimi's mechanism claim was wrong. It reproduces: §5
shows the ladder publishing 5.7 mm and 8.3 mm below its parent on the two seeds
where arm C published, from the same pinned parents, at from-request depths.
The verdict is that the phenomenon is real and the comparison is lost anyway —
by a factor of **8.4x in millimetres per coordinator work unit** — because the
thing the review nominated as the *follow-up* has already been built and is
what the control runs. §6.

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_f674db6b-1e0-2` |
| base commit | `1ca3315` (the three-way consultation) |
| branch | `engine/topology-archive-search` |
| request | `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`, sha256 `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3` |
| contract | exact clearance, from-request allowance `0.002` (the record-lineage `'' 0.0005` tail is used only by the four pinned gates) |
| gate binary | `jagua-experimental`, sha256 `82984c93a6fdcc144e1110ef8df5353b95b8c78fca404718a60a4e5e9d20992b` |
| measurement binary | full combo, sha256 `8d961552fe03a93ec751166f632def9e3e35043925fd65158c804a994519e23e` |
| parents | `drivers/parents12.json`, sha256 `298180e16bc1b8ff0e6588dc1e44237128c2e01a86b057b80c96e3a779657217`, copied byte for byte from `contact-block/drivers/parents12.json` |
| box | x86_64, 16 cores, engine pinned at 8 threads, load average 0.29 at start |

**This round changes no engine code.** `git diff` against `1ca3315` touches
`docs/experiments/m26-band-audition/` and nothing else, which is what Kimi's
"zero chirurgia al motore" requires and what makes the four pinned gates below a
statement about the committed tree rather than about a patch.

The full-combo measurement binary is
`jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator`.
Both arms run on it, because mode 34 does not exist without
`compression-schedule` and `lanes=`/`pconfirm=` are unknown spec keys without
`parallel-compression-schedule`. The two arms therefore differ by the mode
argument and by nothing else.

## 1. Step 1: the twelve parents replay, and what "replay exact-valid" means here

`drivers/replay.py` asks the contact-block round's question verbatim: mode 34
from the pinned parent with a target the parent already meets (`raw + 0.5`) and
a 200,000-unit cap. It reproduces
`contact-block/evidence/replaycontrol.json` cell for cell — including its
`engineExactValid: 0 of 12`, which is worth stating plainly because the field
name invites the wrong reading.

With a target *above* the parent's depth the mode refuses on `"persistent
vacancy mode 34 final bound must be below the parent depth"` and publishes
nothing, so `exactValid` reads `false` on all twelve. That `false` is *this run
published nothing*, not *this parent is invalid*. The parent verdict is upstream
of it: `general_relaxed.rs:6408` runs the authoritative publication gate
`validate_and_measure_placements` on the parent and `:6414` runs
`coupled_independent_source_depth`, both **before** the bound comparison at
`:6425`. A cell that fails on the bound message has passed the exact gate.

| check | result |
|---|---|
| cleared `validate_and_measure_placements` at allowance `0.002` | **12 / 12** |
| re-measured depth matches the pin to < 5e-7 mm | **12 / 12** |
| `parentFingerprint` matches the pinned fingerprint | **12 / 12** |
| `contractValid` on the returned layout | 12 / 12 |

`evidence/replay.json`. The audition may run from all twelve.

## 2. What W is, and why the round measures a curve instead of one cell

Two things had to be settled before an arm could be capped "at the same W".

**(a) Mode 26 has no work cap, and the cap must not be one.** Kimi's gate is
explicitly "zero engine surgery". `POLYGON_NESTING_COMPRESSION_SCHEDULE`'s
`work=` key is mode 34's alone. So the arm's cap is *structural*, and it is
exact rather than approximate:

* `ladder_compression_bounds` (`general_relaxed.rs:5994`) sets
  `step_mm = max(span/8, parent*0.001)` and `steps = clamp(ceil(span/step), 1, 8)`.
  At a 174 mm parent the `parent*0.001 = 0.174208` floor dominates for **every**
  drop from 0.175 mm to 1.3937 mm (`8 x 0.174208`). **A drop of 0.3 mm and a
  drop of 1.0 mm produce
  the identical rung-1 bound**; they differ only in how many rungs follow, 2
  against 6. That is the plan's own reading of arm C — *"the mode's own bounds
  function turns a 0.3 mm drop at a 174 mm parent into a 0.174208 mm step"* and
  *"**rung 1** publishes 4.25 mm and 3.61 mm below its own requested bound"*
  (`next-generation-engine-plan.md:4380-4384`).
* One rung is the anatomy's own pinned equal-budget figure: 32,246,564 candidate
  queries + 5 × 233,445 exact pair tests = **33,413,789 work units**
  (`mode26-rung-anatomy/README.md` §3.4). At W = that number, *the drop-1.0
  ladder truncated at W* **is** *the drop-1.0 ladder's first rung*, because the
  meter passes W inside rung 1.
* So the arm asks for the largest drop `ladder_compression_bounds` still turns
  into exactly one rung, and the driver **checks `stepsPlanned == 1` off the
  emitted document** rather than trusting the derivation. It is 1 on 12 of 12
  (`evidence/audition-arm.json`), and the resulting bound equals the drop-1.0
  ladder's rung-1 bound exactly on 6 cells and by one ULP — 2.84e-14 mm, eleven
  orders below the 1 µm canonical grid — on the other 6.
* `m26:drop1.0` is the same ladder asked for the literal 1.0 mm drop and left
  **uncapped**, at 6 rungs on all twelve. It is the secondary reading: "and if
  the cap were six times wider".

**(b) The process work meter carries a floor that belongs to neither arm.** A
mode-34 process that refuses its mode outright and runs no search at all still
burns **6.84 M – 11.91 M work units** (median 8.97 M) — phase 0 constructing
before the deep operator is handed the pinned parent. Both arms pay it, so it is
common mode, and leaving it in compresses a 10x work difference into a 1.4x one.
Every table below reports **operator work units** = process meter − that floor,
measured per seed by the same replay of §1, alongside the raw process meter and
the wall.

**(c) The brief's two clauses about W are not simultaneously satisfiable, and
the round says so rather than picking the flattering one.** The task named "W in
the 15–35M band where m34 buys ~1.5–2.5 mm". In this harness those are different
budgets: at the schedule cap that puts the control in the 15–35 M *process*
band it buys 12.1 mm, and the cap at which it buys 1.5–2.5 mm is ~6.7 M. So the
control is run at **five** budgets spanning both readings — which is also
`matched.py`'s own declared method, *"each arm is run at several budgets so the
comparison is read off a curve rather than off one cell that happened to land
where the author wanted it"* — and the kill rule is reported at all five.

One budget is designated the kill-rule control by a rule fixed before it was
picked: **the budget whose median operator work is nearest the arm's**. That is
`m34:15000000`, at 3.911 M against the arm's 4.094 M — the control gets **4.5%
less work than the arm**. The designation changes nothing: the verdict is `CUT`
at all five.

## 3. The battery

Twelve parents, both arms from the same pinned parent at the same seed, drop
1.0 mm for the control's bound (`past=1`, so the bound is a start and not a
stop) and one rung for the arm, `POLYGON_NESTING_PROFILE=1` on both because the
x-axis is a counter. Scored on `rawSourceDepthMm` with the parent as the floor,
which is `matched.py`'s scoring, for both arms.

| arm | median delta | moved | median operator work | median wall | mm / M operator unit (aggregate) | mm / wall s (aggregate) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `m26:1rung` (the arm) | 0.2332 mm | 10/12 | 4.094 M | 3.60 s | **0.1547** | 0.2221 |
| `m26:drop1.0` (uncapped, 6 rungs) | 3.3784 mm | 10/12 | 45.486 M | 16.39 s | **0.0756** | 0.2143 |
| `m34:1670689` | 0.2534 mm | 10/12 | 0.390 M | 3.13 s | **0.5828** | 0.1058 |
| `m34:3341379` | 1.1044 mm | 11/12 | 0.869 M | 3.55 s | **1.0135** | 0.2839 |
| `m34:6682758` | 2.5904 mm | 11/12 | 1.706 M | 4.58 s | **1.1961** | 0.4776 |
| `m34:15000000` **← work-matched** | 7.0129 mm | 12/12 | 3.911 M | 7.59 s | **1.2991** | 0.7038 |
| `m34:33413789` | 12.1095 mm | 12/12 | 17.481 M | 26.85 s | **0.6233** | 0.4079 |

**The published statistic Kimi asked for — millimetres per coordinator work
unit, never per arm — is the bolded column, and it is the whole verdict.** At
matched work the control is **8.4x** the arm (1.2991 against 0.1547). At the
control's *worst* budget on that axis it is still **3.8x** the arm. **The arm
does not beat any control budget on the work axis at all.**

The one place the arm leads anything is the secondary wall axis against the
smallest control budget - 0.2221 mm/s against `m34:1670689`'s 0.1058 - and it is
worth naming rather than burying, because it is also worthless: that same budget
beats the arm on median depth (0.2534 against 0.2332) while spending a tenth of
its work, so the arm is leading on seconds-per-millimetre in a cell it is
simultaneously losing on both millimetres and work. At matched work the control
leads the wall axis 3.2x as well.

## 4. The kill rule, applied

Kimi's connective is `o` — the arm survives only if it clears **both** clauses.
The campaign brief transcribed it into English with `AND` between the two
negations, which is the weaker kill. `drivers/verdict.py` evaluates both and
reports both; the arm fails under either, and on four of five budgets it fails
both clauses outright.

| control | control median | arm median | arm − control | clause A (≥ +1 mm) | clause B (arm below on ≥ 8/12) | verdict |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `m34:1670689` | 0.2534 mm | 0.2332 mm | −0.0202 mm | FAIL | pass | **CUT** |
| `m34:3341379` | 1.1044 mm | 0.2332 mm | −0.8713 mm | FAIL | FAIL (5/12) | **CUT** |
| `m34:6682758` | 2.5904 mm | 0.2332 mm | −2.3573 mm | FAIL | FAIL (2/12) | **CUT** |
| `m34:15000000` **← chosen** | 7.0129 mm | 0.2332 mm | −6.7798 mm | FAIL | FAIL (0/12) | **CUT** |
| `m34:33413789` | 12.1095 mm | 0.2332 mm | −11.8763 mm | FAIL | FAIL (0/12) | **CUT** |

The row that closes the question is the first one. At a schedule cap of
1,670,689 the control spends **0.390 M operator work units against the arm's
4.094 M — one tenth** — and its median is still 0.0202 mm *better* than the
arm's. There is no budget in the measured range at which the ladder is the
better use of the coordinator's work.

Per parent, against the work-matched control:

| seed | parent raw | arm published | arm delta | control published | control delta | arm below control? |
| ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 0 | 174.2081 | 172.5135 | 1.6946 | 167.9060 | 6.3021 | no |
| 1 | 176.0560 | 173.3800 | 2.6760 | 168.9450 | 7.1110 | no |
| 2 | 179.0060 | 179.0060 | 0.0000 | 171.9110 | 7.0950 | no |
| 3 | 176.0610 | 175.4490 | 0.6120 | 168.5820 | 7.4790 | no |
| 4 | 171.6495 | 171.6465 | 0.0030 | 166.7340 | 4.9155 | no |
| 5 | 179.0518 | 179.0518 | 0.0000 | 171.6050 | 7.4468 | no |
| 6 | 179.6200 | 179.5681 | 0.0519 | 172.1935 | 7.4265 | no |
| 7 | 179.5223 | 179.4818 | 0.0405 | 172.8980 | 6.6243 | no |
| 8 | 178.9320 | 177.1900 | 1.7420 | 171.4320 | 7.5000 | no |
| 9 | 174.9656 | 171.6848 | 3.2808 | 168.0348 | 6.9308 | no |
| 10 | 176.3622 | 176.0952 | 0.2671 | 169.9475 | 6.4147 | no |
| 11 | 171.6141 | 171.4149 | 0.1992 | 165.4188 | 6.1953 | no |

**0 of 12.** The uncapped 6-rung ladder does no better where it counts: its
median 3.3784 mm at 45.486 M operator work loses to `m34:33413789`'s 12.1095 mm
at 17.481 M — **3.6x the depth for 2.6x less work**.

## 5. Arm C reproduces. That is not the same as arm C winning

The plan's arm C published on 2 of 3 seeds from a saturated archive at
174–179 mm: **−4.9571 mm on seed 0 and −4.3170 mm on seed 1**, *"the largest
single gain this coordinator has ever made"* (`next-generation-engine-plan.md:4362-4372`).
Run from the pinned parents at the same three seeds, the uncapped ladder:

| seed | parent | arm C (plan, in-coordinator) | `m26:drop1.0` here (pinned parent) |
|---:|---:|---:|---:|
| 0 | 174.2081 | −4.9571 (169.251) | **−5.7266 (168.4815)** |
| 1 | 176.0560 | −4.3170 (171.739) | **−8.2890 (167.7670)** |
| 2 | 179.0060 | 0.0000 | 0.0000 |

Same shape, same two-of-three, larger magnitudes. **Kimi's mechanism claim is
confirmed on the band, not refuted.** Grok review 5's archival of arm C as an
artefact of its saturated precondition is the thing this round falsifies: the
gain survives being lifted out of the coordinator onto a pinned parent.

It loses anyway, because on those same three seeds the control at 33.4 M takes
**−10.2001 / −11.9530 / −14.7020 mm** for 2.6x less operator work. The ladder's
largest measured gain anywhere in the battery - −8.2890 mm on seed 1 - is
**3.664 mm behind what the control took from that same parent**, and it cost 2.6x
the work to get there.

## 6. Why: the follow-up Kimi reserved for a pass had already shipped, and it is the control

Kimi §1 closes: *"e anche se passa, a 10s serve il porting del rung (4.7–13.8s →
slice 0.5–1.0s, il design di `mode26-rung-anatomy` §3)"*. That port exists. It
is `compression_schedule.rs`, whose module documentation opens *"This is the
port the mode-26 rung anatomy designed from measurement (see
`docs/experiments/mode26-rung-anatomy/README.md` §2-3)"*, and it is reached as
**mode 34** — which is what `matched.py`'s control arm runs and therefore what
this audition raced the ladder against.

So the gate was never "a new action against the shipping schedule". It was
**mode 26 against its own port**, and the port wins at every budget by the
factor the anatomy predicted it would: the rung pays 33.4 M work units to move
one bound by 0.159 mm and rebuilds a whole clamped-sheet pipeline to do it,
while the schedule lowers a clamp under one live lane a canonical grid unit at a
time. The audition's finding is that the port did not merely match its parent
mechanism — it dominates it on the band the parent mechanism was best at.

This also settles Kimi's §4 branch without ambiguity: there is no porting price
to state, because there is no porting left to do, and the arm did not pass.

## 7. Three measurements this round corrects, all of them against itself

### 7.1 The 85.4% abort does not transfer to this band — and the arm loses anyway

Kimi priced the anatomy's dominant failure mode *inside* the arm, which was the
right instinct. Counted explicitly off `abortedByRollbackDisagreement`
(`general_relaxed.rs:8354`, a default-build field):

| ladder | rungs run | rung arms run | aborted on the rollback disagreement | produced no state at all | produced an exact-valid state |
| --- | ---: | ---: | ---: | ---: | ---: |
| `m26:1rung` | 12 | 14 | **1 (7.1%)** | 1 | 6 |
| `m26:drop1.0` | 72 | 113 | **23 (20.4%)** | 22 | 57 |

Against the anatomy's **85.4% of 171 arms** and its **0 of 171 producing an
exact-valid state** at 159 mm and 164 mm parents. The 0–6 f32-ulp disagreement
of `mode26-rung-anatomy` §1.5 is a deep-frontier phenomenon; at 171–179 mm it
fires on one arm in fourteen. A rung here is also **8x cheaper** than the
anatomy's: 4.094 M operator work units against 33.4 M, because it publishes on
its first arm instead of grinding through 4.9.

**The honest consequence is that the arm's loss cannot be blamed on the abort.**
Fixing the ULP rule the anatomy named would recover a fifth of the 6-rung
ladder's arms at best, against a deficit of 8.4x. The mechanism is not being
held back by its known bug; it is simply a more expensive way to buy depth.

### 7.2 A sub-grid difference in the rung's bound is worth 6.5 mm of outcome

The first pass of this audition derived the single-rung target from the parents'
`rawDepthMm`. The ladder measures its parent with
`coupled_independent_source_depth`, which is the grid-snapped
`independentDepthMm`; on seed 4 the two differ by 0.00047 mm, and four of twelve
cells silently planned two rungs. `evidence/audition-first-pass-RETRACTED.json`
is that pass, kept because of what falls out of comparing it with the corrected
one.

On **seed 10 both passes ran exactly one rung**, with bounds
176.18586152213675 and 176.185638 — **2.235e-4 mm apart, a ninth of the
search allowance and 22% of one canonical grid unit**. The two publications are
169.59481435720784 and 176.0951602474156: **6.5003 mm apart**.

That is the review's own §0 thesis measured on its own mechanism — *"l'endpoint
è dominato dalla fortuna della traiettoria greedy"* — and it is the reason this
round refuses to read a verdict off any single m26 cell. It also means the
retirement is robust in the direction that matters: the accidental first pass
was the *luckier* one (median 0.4990 mm against 0.2332 mm) and it is still
6.5 mm short of the work-matched control's median.

### 7.3 `matched.py`'s process meter needs its floor named

`matched.py` calls `processWorkUnits` *"the portfolio's own meter, measured on
the process rather than declared"* and warns that an operator invisible to the
meter would let a naive equal-work gate call a loss a win. The complementary
hazard is the one this round hit: a **9 M-unit constant** that both arms pay
before either operator starts. Read raw, the arm's 13.00 M against the control's
12.39 M looks like a fair fight; net of the floor it is 4.094 M against 3.911 M,
and the control is the one being starved. Any future matched-arm gate on this
harness should subtract the refused-replay floor, which costs one extra 2.5 s
process per seed to measure.

## 8. The retirement entry for `shipped-surface.md`'s board

Written here, not there; the merge applies it. `m26` is not currently on that
board — it appears once, in the operator set at `shipped-surface.md:216`.
Proposed row for §3, *The retired board*:

| lever | key | the negative | where |
|---|---|---|---|
| the m26 short ladder in the 10 s band | mode `26` (`m26:1rung`, and uncapped `m26:drop1.0`) | **CUT at five of five control budgets, 0 of 12 parents below the control at matched work.** 12 from-request parents 171.614–179.620. Work-matched (control gets 4.5% *less* work): median **0.2332 mm against 7.0129 mm**, and **0.1547 against 1.2991 mm per M coordinator work unit — 8.4x**. At one tenth the arm's work the control's median is still better (0.2534 vs 0.2332). The uncapped 6-rung ladder is 3.3784 mm at 45.5 M operator units against the control's 12.1095 mm at 17.5 M. **Arm C reproduces** (−5.73 / −8.29 mm at seeds 0/1) — the mechanism is real and the comparison is lost anyway, because the control *is* mode 26's own shipped port | `m26-band-audition/`, `next-generation-engine-plan.md:4362-4372`, `mode26-rung-anatomy/` §3 |

Two clauses of that row are worth keeping verbatim in any summary, because they
are the ones that close the spend Kimi said would close: **the phenomenon
reproduced**, and **it lost to its own port by 8.4x per work unit**. Re-opening
mode 26 for the 10 s band needs a new mechanism, not another sweep, and in
particular not the ULP fix — §7.1 prices that at a fifth of a 20% abort rate
against an 8.4x deficit.

The two consultation claims this round settles as measurements rather than
opinions:

* **Grok review 5**, archiving arm C as an artefact of its saturated
  precondition: **falsified**. §5.
* **Kimi review 1 §1**, that the band has *"né un positivo gated né un negativo
  gated"*: **discharged**. It now has a gated negative.

## 9. Regression, determinism, suites

**The four pinned gates**, on the rebuilt `jagua-experimental` binary
(`82984c93…`) from the committed tree — `evidence/gates-base.json`:

| gate | pinned | reproduced |
|---|---|---|
| g1 mode 20 | 206.869 / `8a7737381238fa4d` | yes |
| g2 mode 22 | 159.09233022733062 / `fa01012af1d559ae` | yes |
| g3 mode 22 | 159.07876040364795 / `e28fba007f8031d4` | yes |
| g4 mode 22 | 164.0375677990678 / `49f094d7e59a9008` | yes |

`ALL_PASS: true`. This round changes no engine code, so the gates are a check
that the measurement was taken on the tree it claims and not a check on a patch.

They were run **twice on the same binary, before and after the audition
commit**, and the four documents compared field by field with the clocks
stripped — `evidence/gates-base-post-commit.json`,
`evidence/gates-pre-post-docdiff.json`, `drivers/docdiff.py`:

| gate | scalar fields compared | differences |
|---|---:|---|
| g1 | 3265 | `engineCommit`, `engineWorktreeDirty` |
| g2 | 3246 | `engineCommit`, `engineWorktreeDirty` |
| g3 | 3246 | `engineCommit`, `engineWorktreeDirty` |
| g4 | 3246 | `engineCommit`, `engineWorktreeDirty` |

`ONLY_BUILD_IDENTITY_DIFFERS: true`. Every search-visible field of all four
documents is identical; the two that move are the commit id and the dirty flag,
which is the round becoming a commit. The `engineCommit` recorded in the `post`
documents is `cde147c`, the audition commit itself — the second commit in this
worktree adds only this cross-check, because a document cannot record the hash
of the commit that contains it.

**Determinism, two processes, three cells, both arms** —
`evidence/determinism.json`, **6 of 6 identical**:

| cell | arm | published | process work | documents identical |
|---|---|---:|---:|---|
| seed 0 | `m26:1rung` | 172.5135461640932 | 12,883,716 | yes |
| seed 0 | `m34:15000000` | 167.906 | 11,759,830 | yes |
| seed 1 | `m26:1rung` | 173.380 | 21,788,716 | yes |
| seed 1 | `m34:15000000` | 168.945 | 12,891,700 | yes |
| seed 9 | `m26:1rung` | 171.68476662453673 | 15,890,631 | yes |
| seed 9 | `m34:15000000` | 168.03476662453676 | 11,104,048 | yes |

The comparison is the whole document with the wall-clock fields stripped —
placements included, because a run that reached the same depth by a different
layout would pass a scalar check and still be non-deterministic. Three
`searchProfile` fields had to be added to the strip list by name
(`milliseconds`, `leafMilliseconds`, `leafSharePercent`); the shared
`gatelib.strip_times` misses them because they end in neither `Ms` nor
`Seconds`, and the first determinism run reported 0 of 6 on them alone while
every depth, fingerprint and counter already agreed. `calls`, `phase` and the
whole `counters` block stay in the digest, so this is a determinism check on the
work meter and not only on the answer.

The published depths above also reproduce the battery's cells in a different
process on a different day, which is a third replay of the same numbers.

**Suites** — see §10.

## 9.1 What this round did **not** test

Named here rather than left for a reader to find.

* **A longer ladder.** The brief pre-committed drop 1.0, which is 6 rungs of the
  8 `LADDER_COMPRESSION_STEPS` allows. A drop between 1.2195 and 1.3937 mm
  (`7 x` and `8 x` the 0.174208 mm floor) would buy the other two. The round did not run it, and the reason it was not treated as a gap is
  a number this round *did* measure: the ladder's yield per work unit **falls**
  as rungs are added — 0.1547 mm/M-unit at one rung, 0.0756 at six. Adding rungs
  moves the arm away from the control on the axis the kill rule is written in,
  not toward it. At *equal* work a longer ladder is not available at all, since
  six rungs already cost 2.6x the control's matched budget.
* **The ULP fix.** `mode26-rung-anatomy` §1.5 names the one-f64-ulp rule that
  aborts arms on a 0-6 f32-ulp disagreement. This round measures the abort rate
  at 7.1% / 20.4% in this band (§7.1) rather than repairing it. Repairing it
  cannot close an 8.4x deficit, which is why it is priced rather than tried.
* **Other requests.** mixed-61 only, as the gate specifies. `shapes-17` and
  `triangle-20` are in `runlib.REQUESTS` and untouched.
* **Wall as a primary axis.** The secondary column is reported and the arm loses
  on it too (0.2221 against 0.7038 mm/s), but this is a work-budget round by
  protocol and the wall numbers carry ordinary box noise.
* **The seed spread.** One seed per parent, the parent's own. The 6.5 mm
  sensitivity of §7.2 says a per-parent m26 cell has a wide distribution that
  twelve cells sample once each. That is an argument for reading the *aggregate*
  per-work statistic, which is what §3 bolds, and it cuts against the arm in
  every reading: even the accidental luckier pass loses by 6.5 mm of median.

## 10. Suites

Run by `drivers/run-suites.sh`, each exit code captured directly from the
command rather than through a pipe, because a pipe reports the exit of the last
stage and a suite that failed behind `| tee` reads as a pass.

| suite | command | exit | passed | failed | ignored |
|---|---|---:|---:|---:|---:|
| `suite-jagua` | `cargo test --release --features jagua-experimental` | **0** | 1293 | 0 | 2 |
| `suite-combo` | `cargo test --release --features jagua-experimental,compression-schedule,parallel-compression-schedule,continuous-rotation,sparse-rotation,fast-contract-validator` | **0** | 1357 | 0 | 2 |
| `suite-example` | `cargo test --release --features jagua-experimental --example general_request_benchmark` | **0** | 19 | 0 | 0 |

The third is the one the consolidation round proved the other two miss: `cargo
test` does not build or run an example's own `#[test]` functions unless the
example is named, and this repository's benchmark harness carries 19 of them.

The known flaky
`search::layout_scorer::tests::free_material_multi_eviction_shrinks_retained_container_capacity`
**passed on its first run in both library suites** (`suite-jagua.log:1042`,
`suite-combo.log:936`), so the protocol's re-run clause was not exercised and
there is no second reading to report. Logs are `evidence/suite-*.log`; the
captured exit codes are `evidence/suites-exits.log`.

## Files

* `drivers/build.sh` — the two binaries, from the committed tree.
* `drivers/gatelib.py`, `drivers/gates.py` — the four pinned gates, `ROOT`
  repointed at this worktree and nothing else changed.
* `drivers/runlib.py` — the shared request table, pinned positional tail and
  `0.002` allowance, `ROOT`/`BIN`/`OUT` repointed.
* `drivers/parents12.json` — the twelve pinned parents, byte for byte from
  `contact-block/drivers/`.
* `drivers/replay.py` — step 1, the parents' exact-gate replay, and the
  harness-floor measurement §2(b) needs.
* `drivers/audition.py` — the battery: the m34 control at N budgets, the m26
  single rung, the uncapped m26 drop-1.0 ladder.
* `drivers/determinism.py` — two processes, both arms, three cells.
* `drivers/verdict.py` — the pre-committed kill rule, both readings of its
  connective, at every control budget.
* `drivers/tables.py` — every table in §3, §4, §7.1 and §2(b), rendered from
  `evidence/verdict.json`. A number here that it does not print is a number this
  document made up.
* `drivers/run-suites.sh` — the three suites.
* `drivers/docdiff.py` — the four gate documents compared field by field
  across two runs, so a moved digest says *what* moved.
* `evidence/gates-base.json`, `evidence/gates-base-post-commit.json`,
  `evidence/gates-pre-post-docdiff.json`, `evidence/replay.json`,
  `evidence/audition-arm.json`, `evidence/audition-control-curve.json`,
  `evidence/audition-control-workmatched.json`,
  `evidence/audition-first-pass-RETRACTED.json`,
  `evidence/determinism.json`, `evidence/verdict.json`,
  `evidence/binaries.txt`, `evidence/suite-*.log`.

Reproduce:

```
bash drivers/build.sh all
python3 drivers/gates.py base-pre  /var/lib/t3/tmp/m26band/bin/gate-base
python3 drivers/replay.py     OUT/replay   BIN drivers/parents12.json
# the ladder pass carries the uncapped drop-1.0 arm; `arm2` is the corrected
# single rung of §7.2 and is the one the verdict reads.
python3 drivers/audition.py   OUT/ladder   BIN drivers/parents12.json 33413789 m26rung,m26drop1
python3 drivers/audition.py   OUT/arm2     BIN drivers/parents12.json 33413789 m26rung
python3 drivers/audition.py   OUT/control  BIN drivers/parents12.json \
  1670689,3341379,6682758,33413789 m34
python3 drivers/audition.py   OUT/control2 BIN drivers/parents12.json 15000000 m34
python3 drivers/determinism.py OUT/det     BIN drivers/parents12.json 15000000 0,1,9
python3 drivers/verdict.py OUT/verdict.json OUT/replay/replay.json \
  OUT/arm2/audition.json OUT/ladder/audition.json \
  OUT/control/audition.json OUT/control2/audition.json
python3 drivers/tables.py OUT/verdict.json
bash drivers/run-suites.sh
```
