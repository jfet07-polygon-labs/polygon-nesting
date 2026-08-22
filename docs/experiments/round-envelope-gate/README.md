# The matched-arm gate: the two authorities agreed on every confirmation, and the promotion fails on the millimetre it was written for

Sol review 12 §3.2 set the clauses for promoting the certified round-envelope
kernel, and Grok review 7 §2 kept them unmodified. The previous round built the
kernel and cleared its soundness battery; it left the search-side clause —
*"equal-operator-wall: >=8/12 vittorie e >=1 mm mediano contro miter"* —
untouched and said so in its §7. This round runs it, and runs Grok's
reachability co-requirement and the anytime table beside it.

**Nothing was promoted and no default changed.** The kernel is still off at
compile time, off at run time when compiled, and armed only when a run asks for
it.

---

## 0. The three answers

| deliverable | question | answer |
|---|---|---|
| **1. the matched-arm gate** | promote the round-envelope kernel? | **DO-NOT-PROMOTE.** One clause fails — *≥1 mm median improvement at equal operator wall*, measured at **0.0007–0.0632 mm**. Every other clause passes, one of them by a factor of two. |
| **2. the reachability A/B** | does the `crot` tax flip sign under the round authority? | **Median yes at a work budget, no at a wall budget, and the flip is not robust.** On 9 seeds at `work=40M`: **+1.325 mm → −0.693 mm**. At `wall=10 s`: **+6.463 mm → +7.249 mm**, no flip. Per-seed range under the round authority is **[−14.9, +13.3] mm**. |
| **3. the anytime table** | does arming the kernel move the 3/10/30 s curve? | **It moves it and it does not close it.** At 10 s plan-mode on 9 seeds the armed arm is **−2.135 mm** median (6/9 better) with a per-seed range of **[−8.5, +9.9] mm**. The gap to Sparrow's 150.165 goes from 25.2 mm to 23.2 mm. |

### The headline, in one paragraph

On the twelve pinned parents, at four work budgets, the round-envelope kernel
and the canonical miter authority produced **bit-identical searches**: the same
published depth, the same placement fingerprint, the same schedule step digest
and the same confirmation counts, on **48 of 48 cells**. The armed arm's whole
advantage is therefore cost — it is **1.92x cheaper per confirmation** — and a
mode-34 slice does not spend its wall on confirmations, so that 1.92x buys
**6.9%** of slice wall, and 6.9% more work at these budgets buys **0.06 mm**.
Sol's rule asks for 1 mm. The kernel is sound, cheap, and on this population it
has nothing to sell.

The round-authority effects that *are* millimetre-sized all live on the
**from-request coordinator** path, not on the pinned-parent path — and they are a
basin lottery, not a gain: at `wall=10 s` on seed 1 the armed engine published a
layout HEAD's authority **cannot publish at all** (58 of its 61 poses off the
2.5° lattice), and it was **6.30 mm worse** than what the miter authority found
on the same seed at the same budget.

---

## 1. What had to be built first: `rek` cannot reach the gate's own arm

Sol's kill is written on **twelve pinned parents**, one mode-34 slice each. The
previous round's arming door is the `rek` **portfolio spec key**, and
`run_portfolio`'s own doc comment is *"Runs the portfolio from the request only:
no pinned parent, no warm start, no fixture anywhere"* — the RAII guard that
installs the kernel mode is constructed inside `run_portfolio` and nowhere else.
A pinned-parent mode-34 slice goes through
`improve_complete_layout_with_pinned_vacancy_parent` and never touches it.

So the gate as specified could not be run at all, and this round's only
production change is the door that lets it:
`POLYGON_NESTING_ROUND_ENVELOPE_KERNEL`, read by
`examples/general_request_benchmark.rs`, following the
`POLYGON_NESTING_UNPINNED_VACANCY_PARENT` precedent exactly — the positional
argument list is a pinned contract that replay drivers depend on, and the
meaning of a replayed command may not change.

**164 insertions, 0 deletions, one file, and that file is an example.** No line
of `polygon-nesting-core`'s library changed. The kernel, the wire point, the
material contract validator and `PolygonSet::offset` are all exactly as the
previous round committed them.

Four properties, each a test or a driver assertion rather than a claim:

* **it takes modes, not booleans.** `0`/`off`, `1`/`union`, `2`/`exclusive` —
  `KernelMode::parse`'s own vocabulary. `true`, `yes`, `on`, `Union`, `3` and
  the empty string are all refused, because a mode key that fell back to a
  boolean would silently pick an arm.
  (`the_kernel_environment_door_reads_modes_and_refuses_booleans`)
* **a binary without the feature refuses it.** Not ignores it. This is the `rek`
  rule and here the reason is stronger: an environment variable does not appear
  in the command line a driver logs, so a build that could not honour it would
  run the *miter* authority under a round label and leave no trace.
  (`the_kernel_environment_door_is_refused_without_the_feature`)
* **it refuses to coexist with a portfolio spec.** `run_portfolio` installs
  `settings.round_envelope_kernel` over whatever the process armed, so a
  coordinator run carrying the variable but no `rek` key would be a miter run
  under a round label. Refused rather than resolved, because either resolution
  would be a rule a reader has to know.
* **an armed run says so in its own document.** `roundEnvelopeKernel.mode`, and
  `matchedgate.py` *asserts* it on every cell rather than trusting that it set
  the variable on the right binary — 96 of 96 cells reported their own arm.
  Emitted only when armed, so an unarmed run's document is byte-identical,
  including in a build carrying the feature, which is what the pinned gates
  check.

The two arms of the gate are therefore **one binary and one environment
variable**. That is stronger than two binaries: the binary is common mode and
nothing else differs.

---

## 2. Deliverable 1 — the matched-arm gate

### 2.1 The design, and the three readings

The control is `docs/experiments/contact-block/drivers/matched.py`'s `run_m34`,
and what is verbatim is everything that decides what gets measured: the
`past=1,rollback=0,work=W,lanes=1,pconfirm=0` spec string character for
character, the pinned positional tail through the shared `runlib.ARGS`,
`POLYGON_NESTING_PROFILE=1`, the `target = parent − drop` construction and its
`.17g` formatting, the same `searchProfile` reader with
`processWorkUnits = candidateQueries + 5 × exactPairTests`, and the same
`rawSourceDepthMm` scoring with the parent as the floor. The drop is **1.0 mm**,
the m26 audition's, so the lowest rung — `W = 3 341 379` — is the audition's own
pinned m34 cell and this table connects to its **1.104 mm**.

The arm is that same command with the variable set to `1` (union).

Depths are scored on the **raw source** basis, as the previous round's §7 item 3
required: an armed run's `used_long_axis_depth_mm` is on the round envelope
(bbox + `r`) and is smaller than a miter run's by the binding corner's
excursion, while `raw_source_long_axis_depth_mm` reads no envelope at all. Every
millimetre in this document is on the untouched basis.

**The engine is deterministic in the seed and the work cap**, so a cell's depth
is exact and needs no replicas; only the wall varies between two runs of one
cell. That gives three readings, and all three are reported because they can
disagree:

* **equal work** — cell against cell at the same `work=W`. Immune to this box's
  pollution entirely, and the reading to check first.
* **equal operator wall** — Sol's own axis. On a shared box the honest way there
  is not to *declare* two runs equal-wall but to measure each arm's own
  depth-against-wall curve and read both arms at the same point on it. The
  control's operator wall at each budget is the target and the arm's depth at
  that wall is interpolated from the arm's own ladder. Because the arm is the
  cheaper of the two, its ladder ends at a *shorter* wall than the control's, so
  it gets **a fifth rung at `W = 48 000 000` that the control does not run**,
  purely so the top of the ladder is a measurement and not a clamp.
* **equal operator wall, no interpolation** — the best depth the arm was
  *measured* reaching inside the control's wall. A step function read at a rung
  it actually ran, so it can only understate the arm, which is the direction a
  promotion gate should err in.

`operatorWallSeconds` is the benchmark's own `medianElapsedMs`: the measured
stream, which excludes process start-up and request loading. `processWallSeconds`
is recorded beside it and used for nothing.

### 2.2 The finding: the two authorities never disagreed

On **48 of 48** paired cells — twelve parents × four work budgets — the arm and
the control agree on **every** field that is not a clock:

| field | cells agreeing |
|---|---:|
| `rawSourceDepthMm` | 48 / 48 |
| `finalPlacementFingerprint` | 48 / 48 |
| the schedule's `stepDigest` | 48 / 48 |
| `confirmationsAttempted` / `Accepted` / `Refused` | 48 / 48 |
| `exactValid` and `contractValid` | 96 / 96 runs, all `true` |

The union authority admits what either half admits, so this says the **kernel
never released a move the miter was pinning** on this population — not once in the
**44 710** confirmations the control arm attempted across the ladder. The publication audit in §2.6 says the same thing
about the finished layouts, independently, through
`validate_and_measure_placements` itself.

That is the whole result. Everything below is the price of the thing that made
no difference.

One cell aborted on each arm — seed 3 at `W = 3 341 379`, the barren-probe
abort, `confirmationsAttempted = 0` — and it aborted identically on both, so it
is a paired tie rather than a missing measurement.

### 2.3 Per-seed, at equal work

| seed | parent | W | miter | union | diff | miter opwall | union opwall | ratio | confirmations | same fingerprint |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|:--:|
| 0 | 174.208 | 3341379 | 173.3799 | 173.3799 | +0.0000 | 3.15 s | 5.73 s | 1.822 | 328 | yes |
| 1 | 176.056 | 3341379 | 174.5760 | 174.5760 | +0.0000 | 7.29 s | 7.64 s | 1.048 | 327 | yes |
| 2 | 179.006 | 3341379 | 177.3430 | 177.3430 | +0.0000 | 4.61 s | 8.91 s | 1.933 | 229 | yes |
| 3 | 176.061 | 3341379 | 176.0610 | 176.0610 | +0.0000 | 7.32 s | 6.80 s | 0.930 | 0 | yes |
| 4 | 171.650 | 3341379 | 171.0818 | 171.0818 | +0.0000 | 4.41 s | 4.30 s | 0.974 | 235 | yes |
| 5 | 179.052 | 3341379 | 177.5628 | 177.5628 | +0.0000 | 3.44 s | 3.39 s | 0.987 | 233 | yes |
| 6 | 179.620 | 3341379 | 178.4030 | 178.4030 | +0.0000 | 3.43 s | 3.84 s | 1.119 | 246 | yes |
| 7 | 179.522 | 3341379 | 178.2640 | 178.2640 | +0.0000 | 3.43 s | 2.98 s | 0.870 | 286 | yes |
| 8 | 178.932 | 3341379 | 177.2800 | 177.2800 | +0.0000 | 3.61 s | 3.18 s | 0.879 | 324 | yes |
| 9 | 174.966 | 3341379 | 173.9738 | 173.9738 | +0.0000 | 2.73 s | 3.30 s | 1.209 | 325 | yes |
| 10 | 176.362 | 3341379 | 175.3937 | 175.3937 | +0.0000 | 2.35 s | 2.84 s | 1.209 | 351 | yes |
| 11 | 171.614 | 3341379 | 170.8680 | 170.8680 | +0.0000 | 3.89 s | 3.76 s | 0.966 | 287 | yes |
| 0 | 174.208 | 8000000 | 171.4190 | 171.4190 | +0.0000 | 7.64 s | 6.62 s | 0.866 | 796 | yes |
| 1 | 176.056 | 8000000 | 172.2413 | 172.2413 | +0.0000 | 8.16 s | 4.67 s | 0.572 | 753 | yes |
| 2 | 179.006 | 8000000 | 175.1848 | 175.1848 | +0.0000 | 9.40 s | 5.54 s | 0.590 | 686 | yes |
| 3 | 176.061 | 8000000 | 169.8820 | 169.8820 | +0.0000 | 13.64 s | 13.15 s | 0.964 | 19 | yes |
| 4 | 171.650 | 8000000 | 169.5970 | 169.5970 | +0.0000 | 9.00 s | 8.84 s | 0.982 | 404 | yes |
| 5 | 179.052 | 8000000 | 175.1960 | 175.1960 | +0.0000 | 5.16 s | 4.98 s | 0.965 | 627 | yes |
| 6 | 179.620 | 8000000 | 175.8770 | 175.8770 | +0.0000 | 4.80 s | 5.03 s | 1.048 | 670 | yes |
| 7 | 179.522 | 8000000 | 176.3220 | 176.3220 | +0.0000 | 4.18 s | 3.51 s | 0.840 | 765 | yes |
| 8 | 178.932 | 8000000 | 174.9350 | 174.9350 | +0.0000 | 5.55 s | 4.83 s | 0.870 | 682 | yes |
| 9 | 174.966 | 8000000 | 171.6240 | 171.6240 | +0.0000 | 4.68 s | 5.06 s | 1.081 | 705 | yes |
| 10 | 176.362 | 8000000 | 173.2510 | 173.2510 | +0.0000 | 3.30 s | 3.59 s | 1.088 | 809 | yes |
| 11 | 171.614 | 8000000 | 169.2340 | 169.2340 | +0.0000 | 7.65 s | 7.40 s | 0.968 | 500 | yes |
| 0 | 174.208 | 16000000 | 167.4220 | 167.4220 | +0.0000 | 7.76 s | 6.76 s | 0.871 | 1468 | yes |
| 1 | 176.056 | 16000000 | 168.3480 | 168.3480 | +0.0000 | 8.47 s | 7.56 s | 0.893 | 1401 | yes |
| 2 | 179.006 | 16000000 | 171.4000 | 171.4000 | +0.0000 | 7.50 s | 7.14 s | 0.952 | 1431 | yes |
| 3 | 176.061 | 16000000 | 168.5820 | 168.5820 | +0.0000 | 23.73 s | 23.35 s | 0.984 | 50 | yes |
| 4 | 171.650 | 16000000 | 166.7340 | 166.7340 | +0.0000 | 17.22 s | 16.96 s | 0.985 | 686 | yes |
| 5 | 179.052 | 16000000 | 170.9560 | 170.9560 | +0.0000 | 9.92 s | 9.47 s | 0.955 | 1178 | yes |
| 6 | 179.620 | 16000000 | 171.6300 | 171.6300 | +0.0000 | 7.62 s | 7.60 s | 0.997 | 1351 | yes |
| 7 | 179.522 | 16000000 | 172.4150 | 172.4150 | +0.0000 | 6.40 s | 5.49 s | 0.858 | 1493 | yes |
| 8 | 178.932 | 16000000 | 170.9630 | 170.9630 | +0.0000 | 8.83 s | 7.93 s | 0.899 | 1322 | yes |
| 9 | 174.966 | 16000000 | 167.5658 | 167.5658 | +0.0000 | 8.58 s | 8.77 s | 1.022 | 1298 | yes |
| 10 | 176.362 | 16000000 | 169.5790 | 169.5790 | +0.0000 | 5.64 s | 5.63 s | 0.999 | 1541 | yes |
| 11 | 171.614 | 16000000 | 164.7788 | 164.7788 | +0.0000 | 12.65 s | 12.15 s | 0.960 | 1015 | yes |
| 0 | 174.208 | 32000000 | 164.0080 | 164.0080 | +0.0000 | 27.13 s | 26.87 s | 0.990 | 1612 | yes |
| 1 | 176.056 | 32000000 | 164.1030 | 164.1030 | +0.0000 | 24.54 s | 23.78 s | 0.969 | 1884 | yes |
| 2 | 179.006 | 32000000 | 165.4850 | 165.4850 | +0.0000 | 21.40 s | 20.64 s | 0.964 | 2113 | yes |
| 3 | 176.061 | 32000000 | 168.5820 | 168.5820 | +0.0000 | 45.68 s | 45.37 s | 0.993 | 50 | yes |
| 4 | 171.650 | 32000000 | 164.1550 | 164.1550 | +0.0000 | 36.83 s | 36.41 s | 0.989 | 846 | yes |
| 5 | 179.052 | 32000000 | 165.6590 | 165.6590 | +0.0000 | 23.73 s | 23.11 s | 0.974 | 1881 | yes |
| 6 | 179.620 | 32000000 | 164.0870 | 164.0870 | +0.0000 | 16.65 s | 16.29 s | 0.978 | 2488 | yes |
| 7 | 179.522 | 32000000 | 164.7040 | 164.7040 | +0.0000 | 20.69 s | 19.49 s | 0.942 | 2127 | yes |
| 8 | 178.932 | 32000000 | 166.6660 | 166.6660 | +0.0000 | 25.42 s | 24.28 s | 0.955 | 1795 | yes |
| 9 | 174.966 | 32000000 | 164.1270 | 164.1270 | +0.0000 | 27.47 s | 27.52 s | 1.002 | 1538 | yes |
| 10 | 176.362 | 32000000 | 161.4908 | 161.4908 | +0.0000 | 16.66 s | 16.23 s | 0.974 | 2533 | yes |
| 11 | 171.614 | 32000000 | 164.7160 | 164.7160 | +0.0000 | 34.30 s | 33.80 s | 0.985 | 1022 | yes |

### 2.4 The pre-committed rule, clause by clause

| W | equal-work wins | equal-work median | equal-wall wins (interpolated) | equal-wall median | equal-wall wins (measured only) | equal-wall median (measured only) |
|---:|---:|---:|---:|---:|---:|---:|
| 3341379 | 0/12 | -0.0000 mm | 8/12 | +0.0632 mm | 1/12 | -0.0000 mm |
| 8000000 | 0/12 | -0.0000 mm | 7/12 | +0.0594 mm | 3/12 | -0.0000 mm |
| 16000000 | 0/12 | -0.0000 mm | 8/12 | +0.0115 mm | 0/12 | -0.0000 mm |
| 32000000 | 0/12 | -0.0000 mm | 6/12 | +0.0007 mm | 0/12 | -0.0000 mm |

**Read the columns against each other.** The equal-work column is `0/12` and
`0.0000 mm` at every budget because the two arms produce the same layout. The
equal-wall column is where the arm's cost advantage is cashed, and it reaches
`8/12` — **the win-count clause passes at two of the four budgets** — while the
median improvement it buys is between **0.0007 mm and 0.0632 mm**. Sol asked for
**1 mm**. The clause fails by a factor of sixteen at the most favourable budget.

The same ladder collected on the **pre-commit binary**, on a quieter box
(`evidence/gate-verdict-precommit.json`), is more generous to the arm and does
not change the verdict: `7/12` to `9/12` wins and medians of **0.0009 mm to
0.1996 mm** — still five times short.

| clause | required | measured | |
|---|---|---|---|
| ≥8/12 paired wins at equal operator wall | ≥8 | **8/12** at two budgets, 6–8/12 across four | ✅ at 2 of 4 budgets |
| ≥1 mm median improvement vs the miter control | ≥1 mm | **0.0632 mm** at best | ❌ |
| per-confirmation overhead ≤1.25x | ≤1.25 | **0.5216x** median, **0.6631x** worst | ✅ |
| every publication passes the untouched material contract validator | all | **192 of 192** layouts across every arm in this round | ✅ |
| **KILL** — any false accept anywhere | 0 | **0** | ✅ |
| **KILL** — new admissions clustering at the contact-block ~0.5 mm class against m34's 1.104 mm | — | **0 new admissions on all 96 matched-gate layouts**; the class question is vacuous on this population. One distinct new admission exists in the round's *coordinator* runs and it is **6.30 mm worse** than the control's own publication on that seed (§3.3) | ✅ / see §3.3 |

**VERDICT: DO-NOT-PROMOTE.** The failing clause is *≥1 mm median improvement vs
the miter control at equal operator wall*.

### 2.5 Cost

| quantity | value |
|---|---:|
| per-confirmation cost, arm / control, median over 47 cells | **0.5216x** |
| per-confirmation cost, worst cell | 0.6631x |
| per-confirmation cost, best cell | 0.4348x |
| whole-slice operator wall, arm / control, median of 3 paired-replica cells | **0.9313x** |
| the same, range | [0.8859, 0.9325] |

The arm is **1.92x cheaper per confirmation** and only **1.07x cheaper per
slice**, and the gap between those two numbers is the round's economic finding:
**a mode-34 slice does not spend its wall inside `validate_and_measure_placements`.**
Halving the confirmation buys about 7% of the slice, and 7% more work at these
budgets is worth about 0.06 mm. The previous round's *"0.470x with the contract
certificate armed"* reproduces here as 0.5216x on a whole slice's worth of real
confirmations, on a binary that also carries `fast-contract-validator` — so the
economy claim survives contact with the search, and it is the promotion that
does not.

The wall ratio is measured rather than taken from the ladder: five interleaved
replicas on three parents with the arm order alternating, at `W = 16 000 000`,
`evidence/wallratio.json`. All five replicas of every cell produced the same
depth, and the two arms produced the same depth as each other, so that document
is purely a clock.

### 2.6 The publication audit, and the `exclusive` arm

Every layout this round published — from the matched gate and from both
coordinator batteries — was written back out as a pose fixture and re-asked of
**both** authorities through `round_envelope_gate::wired_verdicts`, which calls
`validate_and_measure_placements`: the real wire point, not a re-implementation
of it. `drivers/publications.py` builds the corpus, the previous round's
`round_envelope_battery` is the authority, `drivers/pubaudit.py` reduces it.

| corpus | layouts | union accepts | miter accepts | new admissions | regressions |
|---|---:|---:|---:|---:|---:|
| the matched gate | 96 | 96 | 96 | **0** | 0 |
| reachability, `work=40M` | 12 | 12 | 12 | **0** | 0 |
| reachability, `wall=10 s` | 36 | 36 | 33 | **3** | 0 |
| anytime, plan mode | 36 | 36 | 36 | **0** | 0 |
| anytime, `wall=10 s` | 12 | 12 | 10 | **2** | 0 |
| **total** | **192** | **192** | **187** | **5** | **0** |

*Union accepts* implies the untouched material contract accepted, because the
wire point runs the contract on **both** of its branches and returns `Err` if the
contract refuses. So `192 / 192` is the promotion rule's fourth clause, measured
rather than asserted, and `0` regressions is the previous round's `union` claim
holding on this round's own output.

The five new admissions are **one distinct layout** counted five times — the same
seed, budget and arm reproduced across two batteries. §3.3 is what it is.

**The `exclusive` arm (`rek=2`) is not runnable at the shipping allowance.** Run
on the same twelve parents at the same cheapest rung, **12 of 12 exited 1**
before any search ran, with
`InvalidInput("pieces …-copy-1 and …-copy-2 overlap on the canonical collision
grid")` — the short-side-first constructor's own self-check refusing the layout
the constructor just built. The previous round predicted this from the corpus
side and reported six of twelve parents as not `exclusive`-valid at 0.002;
measured end to end through a real slice it is twelve of twelve, because the
run dies at construction before the parent is reached. `union` is the only arm a
promotion could be asked for, and this round measured that rather than inheriting
it.

---

## 3. Deliverable 2 — reachability: does the `crot` tax flip?

`continuous-rotation`'s README measures the blanket operator at **+3.721 mm**
(worse) at ten seconds on mixed-61, **0 of 9** paired rounds better, under the
miter authority — and Gate A measured that 57 of Sparrow's 61 poses are off the
2.5° lattice *and* off 1.0°, with `crot` expressing 46 of them. Grok review 7 §3
names reachability as a **co-requirement** of any 150@10s claim and refuses to
fund it as a sequel; Sol review 12 §3.3 endorses asking it with the tools that
already exist rather than opening a new family. So the question is exactly one
paired difference, computed twice:

```
tax_miter = crot(miter) − base(miter)
tax_round = crot(round) − base(round)
```

and the answer is whether `tax_round` is negative where `tax_miter` is positive.

Two budget modes, because they answer different objections. `work=40 000 000` is
`runlib.WORK_10S` — the ten-second equivalent the campaign denominates in — and
is **reproducible**, so its depths are immune to this box's load. `wall=10000` is
the budget the published −3.721 mm is on, is not reproducible, and is run three
rounds per seed with the arm order rotating.

**This is a diagnostic. Nothing here promotes anything**, and "it does not flip"
is as useful an answer as "it does".

### 3.1 The answer

| budget | crot tax under miter | crot tax under round | flipped? | round-armed off-2.5-degree poses |
|---|---:|---:|:--:|---|
| `wall=10000` | +6.4629 mm | +7.2490 mm | no | [0, 30, 37, 58] |
| `work=40000000` | +0.9690 mm | -1.2141 mm | **yes** | [0, 49] |

Extended to **nine seeds** at the reproducible budget
(`evidence/reach-work9.json`, `evidence/crot-flip9.json`):

| quantity | value |
|---|---:|
| `crot` tax under the **miter** authority | **+1.325 mm** median, 3/9 better |
| `crot` tax under the **round** authority | **−0.693 mm** median, 5/9 better |
| flipped? | **yes, on the median** |
| per-seed range of the round-authority tax | **[−14.923, +13.347] mm** |
| the round authority alone (`crot=0`), against the miter baseline | **−1.721 mm** median, 6/9 better, range [−5.557, +14.486] |

**So: the sign flips, and the flip is worth less than the noise it sits in.** The
median moves from +1.325 mm to −0.693 mm on nine paired seeds; the per-seed
differences span twenty-eight millimetres. At `wall=10 s` on nine paired rounds
the tax does not flip at all (+6.463 mm → +7.249 mm). A number this unstable does
not license re-arming a measured-negative operator, and it is not offered as
one — Grok review 7's own instruction was that this be *costed* alongside the
kernel, and the cost is that the operator is still not worth its wall.

### 3.2 Off-lattice poses in armed publications

Counted directly off each published layout's `rotationDeg`, against the 2.5°
lattice the default candidate stream can name:

| arm | off-2.5° poses in its published layouts (9 seeds, `work=40M`) |
|---|---|
| `base` (miter, `crot=0`) | 0, 14, 31, 57, 60 |
| `crot` (miter, `crot=1`) | 26, 30, 36, 38, 39, 46, 50, 55 |
| **`rek`** (round, `crot=0`) | **0, 23, 34, 37, 49** |
| `rekcrot` (round, `crot=1`) | 14, 33, 42, 43, 44, 49, 55, 57, 60 |

The round-armed arm publishes off-lattice poses in the majority of its runs — up
to 49 of 61 — **without** `crot`. That is the relaxed engine's own continuous
separator, not a rotation operator, and it means the "default lane cannot propose
those poses" framing is too strong for the deep phases: it cannot propose them,
but it can reach them by refinement. It also means the round authority's wins are
**not** attributable to off-lattice legality: seed 2's 10.7 mm win at `wall=10 s`
was published with **zero** off-lattice poses.

### 3.3 The one new admission

Exactly one distinct layout in this round's 192 publications is a **genuine new
admission** — accepted by the material contract and by the round kernel, refused
by HEAD's miter envelope:

| | |
|---|---|
| where | mixed-61 from request, `wall=10000`, seed 1, `rek=1` |
| raw source depth | **171.9528 mm** |
| poses off the 2.5° lattice | **58 of 61** |
| the miter's refusal | *"pieces 604bc424…-copy-4 and d06db288… overlap on the canonical collision grid"* |
| the contract | accepts |
| what the **control** published on the same seed and budget | **165.6558 mm**, with **0** off-lattice poses |

**The round authority admitted a topology HEAD cannot publish, and that topology
was 6.30 mm worse than the one the miter authority found from the same seed in
the same wall.** This is Gate A's case 3 occurring for the first time in a real
from-request publication rather than on an imported fixture, and it is also the
cleanest available statement of Grok review 7's warning: *legalising a pose does
not make a good one appear.*

---

## 4. Deliverable 3 — the anytime table

Three budgets, two arms, three seeds, **two processes per cell**, mixed-61 from a
bare request. `plan=<ms>` is the reproducible mode `calibrated-plan` shipped and
`replan` refined; both arms carry `replan=1`, so the `canonical` arm is the
shipped configuration exactly. `wall=10000` is run beside it because every
earlier millimetre in this campaign is on that arm.

| budget | arm | seed medians (mm) | median | reproduced 2/2 | coordinator seconds (max) |
|---|---|---|---:|---:|---:|
| `plan=10000` | canonical | 175.136 / 171.362 / 176.162 | **175.136** | 3/3 | 8.24 s |
| `plan=10000` | rek | 171.202 / 173.347 / 167.645 | **171.202** | 3/3 | 9.01 s |
| `plan=3000` | canonical | 181.589 / 179.690 / 179.662 | **179.690** | 3/3 | 2.32 s |
| `plan=3000` | rek | 179.621 / 180.696 / 179.730 | **179.730** | 3/3 | 2.40 s |
| `plan=30000` | canonical | 164.188 / 162.846 / 164.171 | **164.171** | 3/3 | 36.49 s |
| `plan=30000` | rek | 164.136 / 166.347 / 161.083 | **164.136** | 3/3 | 27.21 s |
| `wall=10000` | canonical | 168.484 / 165.656 / 174.280 | **168.484** | 3/3 | 10.26 s |
| `wall=10000` | rek | 169.214 / 171.953 / 163.830 | **169.214** | 1/3 | 11.98 s |

**The `canonical` column reproduces the shipped baseline exactly.** `replan`'s
own README records mixed-61 seed medians of `181.589 / 179.690 / 179.662` at 3 s
and `175.136 / 171.362 / 176.162` at 10 s; this round measures the same six
numbers on its own binary. That is what makes the `rek` column readable as a
difference.

Extended to **nine seeds** at `plan=10000` (`evidence/anytime9.json`):

| arm | seed medians | median | better | range |
|---|---|---:|---:|---|
| `canonical` | 175.388 / 171.362 / 176.162 / 175.114 / 173.491 / 177.878 / 169.772 / 175.648 / 175.635 | **175.388** | — | — |
| `rek` | 169.165 / 173.347 / 167.645 / 173.398 / 175.183 / 173.716 / 179.633 / 168.509 / 173.500 | **173.398** | 6/9 | **[−8.517, +9.861] mm** |

Median **−2.135 mm**, and a per-seed range of nearly twenty millimetres. Both
arms reproduce byte-for-byte across two processes (18/18 cells in the three-seed
table), so that spread is not measurement noise — it is the coordinator landing
in a different basin because the acceptance authority changed. **A median that
small on a spread that large is a lottery ticket, not a curve.**

### 4.1 Against Sparrow

Sparrow on this same x86_64 box, seed 0, 8 workers: **157.971 mm at three seconds
and 150.165 mm at ten**, both exact-valid, from
`docs/experiments/sparrow-mixed61/`. Those were taken on a quiet box; the column
beside them was not.

| budget | Sparrow | canonical (`plan`) | **`rek` (`plan`)** | canonical (`wall`) | `rek` (`wall`) | best gap |
|---|---:|---:|---:|---:|---:|---:|
| 3 s | 157.971 | 179.690 | 179.730 | — | — | **21.7 mm** |
| 10 s | 150.165 | 175.388 | **173.398** (9 seeds) | 168.484 | 169.214 | **18.3 mm** |
| 30 s | not published | 164.171 | 164.136 | — | — | — |

**The gap is not closed and this round does not claim to close it.** At ten
seconds the best arm here is the non-reproducible `wall` control at 168.484 mm,
18.3 mm above Sparrow; the kernel-armed reproducible arm is 23.2 mm above it.
Arming the kernel moves the reproducible ten-second median by −2.1 mm, which is
about a tenth of the gap and is inside the arm's own per-seed spread.

### 4.2 The Sparrow re-import, through the full armed publication path

| allowance | expansion | contract | miter | round | **union** | kernel pair failures |
|---:|---:|:--:|:--:|:--:|:--:|---|
| 0.002 mm | 2.502 mm | accepts | refuses | refuses | **refuses** | 2 [[0, 1], [42, 44]] |
| 0.0005 mm | 2.5005 mm | accepts | refuses | refuses | **refuses** | 1 [[0, 1]] |
| 0.0 mm | 2.5 mm | accepts | refuses | accepts | **accepts** | 0 [] |

**The armed authority publishes the Sparrow layout only at zero search-offset
allowance.** At the shipping `0.002 mm` it does not, and the reason is not the
join: the collision expansion is `total_padding/2 + margin + allowance`, so
`0.002 mm` of allowance asks for a **2.502 mm** disc where the contract asks for
**2.500 mm**, and at that radius the kernel itself refuses pose pairs `[0,1]` and
`[42,44]` — Gate A's own two radius-caused pairs. The material contract accepts
at every allowance, and the miter refuses at every allowance.

One nuance worth naming, because it is a design choice and not a measurement:
the union is taken **per layout**, not per row. At `0.002` the kernel refuses two
pairs and the miter refuses one *boundary* — the miter's message is
`"violates the canonical-grid sheet boundary"`, and the kernel's boundary count
is zero. A per-row hybrid would have admitted this layout at the shipping
allowance; the per-layout union does not. That is deliberate — a per-row
disjunction is a third authority contained in neither half, and nothing in Sol
12 §3.2 or Grok 7 §2 licenses one — but a reader comparing this row to Gate A
should know the two are not the same question.

**This is legality, and legality only.** Whether a search can reach that layout
in ten seconds is what §2 and §3 measure, and they measure it separately for the
reason Grok review 7 §3 gives.

---

## 5. Reproduction, gates, suites, determinism

```sh
bash docs/experiments/round-envelope-gate/drivers/collect.sh all
bash docs/experiments/round-envelope-gate/drivers/run-suites.sh
```

Do **not** pipe either into `tee` or `tail`: you will read the pipe's status
instead of the script's. Every exit status inside them is read directly on the
line after the command.

### 5.1 The four pinned gates

Run on binaries rebuilt from the committed tree, twice — with the feature
absent, which is the protocol's gate, and with it compiled but unarmed:

| gate | pinned | feature ABSENT | feature COMPILED, unarmed |
|---|---|:--:|:--:|
| g1 | 206.869 / `8a7737381238fa4d` | ✅ | ✅ |
| g2 | 159.09233022733062 / `fa01012af1d559ae` | ✅ | ✅ |
| g3 | 159.07876040364795 / `e28fba007f8031d4` | ✅ | ✅ |
| g4 | 164.0375677990678 / `49f094d7e59a9008` | ✅ | ✅ |

`ALL_PASS: true` on both, and stronger: **the two binaries' whole-document
digests are identical on all four gates**, so the feature being compiled changes
nothing a document can see. `evidence/gates-base.json`,
`evidence/gates-meas.json`.

A **third** run closes the protocol's provenance loop after the evidence was
committed — §5.6.

### 5.2 Suites

All `--release`, every exit status read directly rather than through a pipe.

| # | features | result | exit |
|---|---|---|---:|
| 1 | `jagua-experimental` | 1293 passed / 0 failed / 2 ignored | **0** |
| 2 | the protocol's full combo | 1357 / 0 / 2 | **0** |
| 3 | `jagua-experimental`, `--example general_request_benchmark` | 20 / 0 / 0 | **0** |
| 4 | `jagua-experimental,round-envelope-kernel` | 1307 / 0 / 2 | **0** |
| 5 | (supplementary) `jagua-experimental,round-envelope-kernel`, the example | 20 / 0 / 0 | **0** |
| 6 | (supplementary) the measurement binary's own feature set, the example | 22 / 0 / 0 | **0** |

This round adds no new cargo feature — it adds a second arming door to the
previous round's `round-envelope-kernel` — so suite 4 is that feature's suite.
Suites 3 and 5 are the ones that actually cover the change, and they cover
*different halves* of it: suite 3 compiles the
`#[cfg(not(feature = "round-envelope-kernel"))]` refusal, suite 5 compiles the
arming. The campaign's known flake,
`free_material_multi_eviction_shrinks_retained_container_capacity`, did not fire
on any suite.

### 5.3 Determinism, two processes

`evidence/determinism.json`: six cases — three mode-34 slices (miter, union, and
union on a second parent) and three coordinator runs (plan canonical, plan
armed, work armed with `crot`) — each run in **two separate processes**, compared
as whole documents with the wall-clock fields stripped **by name**.

**The first pass failed and is committed as it failed**
(`evidence/determinism-firstpass-unstripped.json`). The three mode-34 cases were
byte-identical; the three coordinator cases were not. The two documents were then
diffed field by field: **32 differences, in exactly fourteen field names, every
one of them a clock or a quantity computed by dividing by one** —
`startedSeconds`, `seconds`, `atSeconds`, `horizonSeconds`, `remainingSeconds`,
`queueSeconds`, `probeSeconds`, `probeEffectiveSeconds`,
`probeRateUnitsPerSecond`, `queueRateUnitsPerSecond`, and
`planCalibration.rawUnits`, which is a float rather than a counter because it is
the work the plan probe observed inside a wall-bounded window.

The plan the coordinator *derives* from that probe does not move, because the
rung ladder quantises it: `plan.units` is `24891457` in both processes at
`plan=10000` on seed 0, and `workUnits`, `placements`, the incumbent fingerprint
and `rawDepthMm` are all identical. This is the protocol's own warning about
`gatelib.strip_times` playing out one level deeper, and it is recorded rather
than quietly absorbed.

Because a strip list is a licence to ignore fields, the second pass also compares
**fourteen verdict paths directly**, outside the digest: the placements, both
depth conventions, the incumbent's depth / fingerprint / dual-gate flag /
published work, the coordinator's work units and derived plan units, the
population's raw depth / fingerprint / `exactValid` / `contractValid`, and the
`roundEnvelopeKernel` block. **6 of 6 cases identical, 0 verdict-field
differences, exit 0.**

### 5.4 The ladder was collected twice, on two binaries

The four-rung ladder in `evidence/matched.json` was collected once before the
instrument was committed and once after. The binary changed between them because
the **source** did: the door's parse was split into a testable function on the
way in and its two tests were added, so "the refactor cannot have changed
anything" would have been an argument rather than a measurement.

> **Correction, made after §5.6 measured it.** An earlier draft of this
> paragraph said the commit changes the binary *whether or not* it changes
> behaviour, "because the benchmark embeds `engineCommit`,
> `engineWorktreeDirty` and `relevantSourceTreeSha256`". That is wrong: all
> three are read at **run** time, by shelling out to `git`
> (`general_request_benchmark.rs:1349` and `relevant_source_tree_sha256()`),
> and none of them is compiled in. The binary is a function of the source
> alone, which §5.6 then confirmed by rebuilding it. The re-collection was
> still the right call — the source *had* changed — but the reason given for it
> was not.

`evidence/ladder-cross-binary.json`: **96 of 96 cells identical** on every
non-clock field — depth, fingerprint, step digest, confirmation counts, schedule
work units, validity flags. Operator wall moved by a median of 1.009x, worst cell
2.52x, which is this round's own `cargo test` suites running beside the second
collection.

Both collections are committed and both produce **DO-NOT-PROMOTE** with the same
failing clause; the pre-commit one, on the quieter box, is the more generous of
the two and is in `evidence/gate-verdict-precommit.json`.

### 5.5 Provenance

`evidence/binaries.txt` carries the three binaries' SHA-256, the toolchain, the
commit and the box's load at build time. The order actually executed was: build
from the committed tree → gate both binaries → measure. The one exception is
stated in §5.4 and is why the ladder was collected twice.

### 5.6 The closing gate: a fresh build of the clean committed tree

After the evidence commit, with `git status --porcelain` printing **nothing**, a
`cargo build --release --features jagua-experimental` into a **fresh**
`CARGO_TARGET_DIR` produced

```
971918823ca7aade8af0130dc0944285776ce94ad28170f56ad81462d0b41838
```

which is **byte-identical** to the gate-stage binary in `evidence/binaries.txt`,
and its four per-gate whole-document digests are identical to that run's as
well. `ALL_PASS: true`. `evidence/gates-final.json`,
`evidence/binaries-final.txt`.

That is the protocol's closing requirement — all four gates on a fresh build with
the feature off — and it is also what corrected §5.4: the same source at two
different commits builds the same binary, so the git metadata in the report is
runtime, not compiled in.

---

## 6. Caveats, stated rather than left to be found

* **`n = 12` parents and one request.** mixed-61 at the exact-clearance 5.0/5.0
  contract, the population Sol's kill names, and no wider. The reachability and
  anytime diagnostics are `n = 9` seeds on the same one request.
* **This box is shared.** Load average ran between 1.6 and 7.5 during the round,
  with other agents' work on it throughout and — during the second ladder
  collection and the two nine-seed extensions — this round's own suites. Every
  wall comparison here is **paired and interleaved** for that reason, and the
  equal-work reading, which needs no clock at all, is the one to check first.
  The wall-ratio document was collected at a load of about 2.
* **The equal-wall reading is an interpolation** between measured rungs. The
  no-interpolation column beside it can only understate the arm, and it reads
  `0/12` and `0.0000 mm` at three of four budgets — so the arm's equal-wall wins
  are a property of the model, not of a rung it ran.
* **The ladder is coarse**: four rungs (five for the arm) from 3.3 M to 48 M
  work. A finer ladder would move the interpolated equal-wall column; it cannot
  move the equal-work column, which is exact.
* **Zero disagreements is a statement about this corpus**, not a theorem. The
  union authority admitted nothing the miter refused *on these parents at these
  budgets*; the previous round's soundness battery is what bounds the general
  case, and this round did not re-run it.
* **The 5 new admissions are one layout counted five times.** One distinct
  new admission in 192 publications is an existence proof, not a rate.
* **The `crot` flip is a median on nine paired seeds with a ±14 mm per-seed
  spread**, and it does not flip at all under a wall budget. It is reported
  because it was asked for, not because it is stable.
* **The anytime `wall` arm is not reproducible by construction** — 1 of 3
  `rek` cells reproduced — and its numbers are quoted only where the campaign's
  own history is quoted on that arm. The `plan` arm reproduced 18 of 18.
* **The Sparrow re-import is legality only**, on one layout, and it is the
  previous round's measurement re-run on this round's binary rather than a new
  one.
* **Nothing here was re-priced under parallel confirmation.** Every slice ran
  `lanes=1,pconfirm=0`, and the kernel's loops are serial.
* **`exclusive` was measured only at the cheapest rung** — it exits 1 at process
  start, so a deeper rung would fail identically, but that is an inference.
* **The publication audit ran at one allowance (0.002)**, the allowance the runs
  were made at. A publication is legal or not at the allowance it was made under.
* **No claim here is about a platform other than x86_64.**

## Errata — from the post-round dual review (Sol 13 + Grok 8, 2026-08-22)

DO-NOT-PROMOTE stands under both reviews, and is **stronger** than §4's table:
neither reviewer found an evidence contradiction, but three interpretations
are corrected.

1. **The equal-wall interpolation is withdrawn as an observation.** Wall and
   work are not monotone on 2/12 seeds (load, not speed — a 16 M cell can
   finish before an 8 M cell), and best-so-far depth is a staircase, not a
   linear response; the interpolant books a 5.86 mm "win" on seed 1 for an arm
   whose same-work cell is bit-identical to the control. The honest reading is
   the published no-interpolation column — **0–3/12 wins, and equal work
   0/12 at 0.0000 mm everywhere**. The 8/12-at-two-budgets tick was a false
   pass of the model, and both quality clauses fail without it.
2. **The per-row-union Sparrow claim is false.** The republish message is the
   first short-circuit of `rebuild_one`, not a census; Gate A's census at the
   same radius shows 37 pair + 4 boundary miter failures with the two
   radius-caused pairs refused by BOTH halves (`roundRefusesMiterAcceptsPairs`
   is empty). A per-row OR would still refuse Sparrow at allowance 0.002; the
   refusal is the 2.502 expansion (the allowance/radius tax), and per-layout
   union is the right design — a per-row disjunction is a third authority
   corresponding to no envelope, and both reviewers reject building it.
3. **"The two authorities never disagreed" and "the residual is 100%
   reachability" are narrowed.** Union does not record component verdicts
   (kernel-refuse/miter-accept disagreements are invisible by construction —
   and known to exist from the exclusive-parent failures), and union returns
   the round metrics even on miter-admitted rows, so the from-request anytime
   A/B moves acceptance, cost, and an internal metric basis at once — the
   −2.135 mm median cannot be attributed to released legality. The licensed
   conclusion is narrower and sharper: **on these 12 parents, in mode 34, no
   round-valid/miter-invalid state ever reached the confirmation call**
   (44,710 attempted, 44,710 accepted, 0 refused), because
   `due_for_confirmation` skips 149,762 frontiers on the miter-geometry
   surrogate one level up, identically in both arms. The one unmeasured
   population — and the single remaining falsifier both reviewers converge on
   — is that skip pile: what fraction is contract-valid ∧ round-valid ∧
   miter-proxy-infeasible, at which radius, and with how much immediate depth.
