# Intra-arm parallelism of mode 34: the idle lanes were real, the bottleneck was not

> Grok plan action 2 (`docs/grok-review-1-independent-v5.md` §2b item 2),
> executed at campaign HEAD `65f6fc9` on x86_64, 16 CPUs, box shared with one
> other measurement agent.

Grok's action 2 starts from a measured fact and a proposed remedy. The fact:
one mode-34 arm runs **one** lane while a mode-26 rung arm runs eight
(`docs/experiments/compression-schedule/README.md` §6.3). The remedy: one
clock, eight workers proposing and scoring moves under it, cadence and floor
preserved.

Both halves were built, and the fact checks out — **the m34 slice's own
occupancy is 0.99 lanes** (§2.1), which is the first number anyone has put on
§6.3's claim. But the remedy is aimed at the wrong 22% of the arm. At the
design slice, **73–78% of the m34 slice's wall is one whole-layout exact
confirmation at a time**, and inside that confirmation the cost is not the
collision-grid overlap loop the schedule's own anatomy blamed — it is the
exact-clearance contract's all-pairs boundary-distance loop, which is 97% of a
confirmation and which no round in this campaign had measured (§3).

So this round delivers two levers and prices them separately:

| lever | spec key | occupancy | slice wall, equal walk | quality per unit **work** | 10 s anytime, bare request |
|---|---|---|---|---|---|
| repair fan-out | `lanes=8` | 0.99 → 2.47 lanes | **0.912x** (9.6% *slower*) | **−0.867 mm**, 1 win / 11 losses | **−2.158 mm, 0 wins / 9** |
| parallel confirmation | `pconfirm=1` | 0.99 → 3.16 lanes | **2.623x** | **0.000 mm, 12/12 ties** | **+1.882 mm, 9 wins / 9** |
| both | | 0.99 → 5.17 lanes | 2.036x | −0.867 mm | −1.634 mm, 0 wins / 9 |

**The headline is the second row, and it is the one Grok did not ask for:**
a 2.62x median wall speedup of the m34 slice (1.43x of the whole process) whose
output document differs from the serial schedule's in **exactly one leaf** —
the diagnostic flag that says it was armed — and which is worth **+1.882 mm at
10 s and +3.359 mm at 30 s** on the bare-request anytime curve, 9 of 9 paired
wins at both. The lever Grok did ask for **fails** the equal-work gate the
action itself set, is below parity in wall, and is **−2.158 mm at 10 s with 0
wins in 9**.

Grok's hypothesis was "this brings 10 s toward the 40M-work quality (~166), not
to 150". **It lands at 172.288** — a quarter of the way, from the other lever.
§8 answers it in full.

Both are behind `parallel-compression-schedule`, off by default, stacked on
`compression-schedule`. All four pinned gates reproduce bit-for-bit on the
flag-off build **and on the armed build with an unarmed spec** (§7).

---

## 1. What was built

`parallel-compression-schedule` (`crates/polygon-nesting-core/Cargo.toml`),
stacked on `compression-schedule`, off by default. Two independent levers, each
with its own spec key, so they can be priced apart:

**`lanes=W` — the repair half.** On a step whose proxy tier is infeasible, `W`
workers each repair a private clone of the frontier at the *same* depth, from a
seed derived from `(base, step, worker)`, and a reduce in worker-index order
adopts the best. The clock stays where it was: a worker never calls
`step_down`, never confirms, never moves the monotone floor, and the schedule
object it is handed is a clone whose mutations are discarded. `rollback_after_steps`
stays 0, which is the compression-schedule round's measured verdict and not a
default this round revisited.

The workers are **persistent** across steps rather than rebuilt per step
(`general_relaxed.rs`, the `repair_workers` vector), because a worker's
surrogate and pair-NFP caches are most of what makes its second step cheaper
than its first; rebuilding per step would have priced the fan-out against a cold
cache the serial lane never pays for. That needed one new primitive,
`parallel::map_slice_mut_with_job_pool`, which is `map_slice_with_job_pool` for
items the closure mutates.

**`pconfirm=1` — the confirmation half.** One whole-layout exact confirmation
spread over the job pool, in the two places its milliseconds actually are: the
collision-grid rebuild-and-overlap phases in
`search::general_fast::validate_and_measure_placements`, and — the one that
matters — the `n(n-1)/2` clearance loop in
`validation::general_polygon::validate_publication`. Both reduce on the
lowest-indexed failure, which is the pair the serial nest short-circuits on, so
the verdict and its message are the serial ones.

Both levers are also reachable from the coordinator's own m34 slice, spec-keyed
and off by default: `m34lanes=` and `m34pconfirm=` in the portfolio spec
(`PortfolioSettings::compression_schedule_lanes` /
`compression_schedule_parallel_confirm`). An unarmed spec builds
`CompressionScheduleSettings::default()` field for field, so the merged
coordinator is unchanged.

---

## 2. The measurement that reordered the plan

### 2.1 The m34 slice runs at 0.99 lanes — §6.3's claim, with a number

`compression-schedule` README §6.3 asserts "the schedule is one lane; the
mode-26 pipeline is eight" and explicitly declines to make a wall claim. This
round measures it. Process CPU-seconds over process wall-seconds is the average
number of cores an arm kept busy; the identical mode-0 preamble is measured in
the same shape and subtracted, because the preamble *is* eight-lane and
conflating the two is what makes the whole-process number (2.71 lanes)
uninformative.

Equal-walk shape (1.5 mm drop, `past=0`, no work cap → exactly 1,500 steps),
three pinned parents, unprofiled build, GNU `time`:

| arm | slice CPU-s | slice wall-s | **slice occupancy** |
|---|---:|---:|---:|
| preamble (mode 0) | 8.88 / 9.39 / 8.95 | 1.96 / 2.06 / 1.99 | 4.53 / 4.56 / 4.50 |
| `serial` | 2.04 / 1.91 / 2.20 | 2.06 / 1.97 / 2.15 | **0.99 / 0.97 / 1.02** |
| `lanes8` | 5.68 / 3.95 / 11.52 | 2.30 / 2.06 / 3.14 | 2.47 / 1.92 / 3.67 |
| `pconfirm` | 2.96 / 2.61 / 2.59 | 0.94 / 0.71 / 1.40 | 3.16 / 3.70 / 1.84 |
| `both` | 6.50 / 4.13 / 11.90 | 1.31 / 0.71 / 2.30 | **4.97 / 5.80 / 5.17** |

The serial slice is one lane to within measurement noise on all three parents.
The claim was right.

### 2.2 But the idle lanes are not where the wall is

The schedule reports its own `repairMs` and `confirmationMs`. At the design
slice (`sched10-noroll`, `work=3341379`, the compression-schedule round's most
work-efficient arm), on its own three parents:

| seed | steps | steps with **zero** sweeps | `repairMs` | `confirmationMs` | confirmation share |
|---:|---:|---:|---:|---:|---:|
| 0 | 1,550 | 1,192 (76.9%) | 549.8 | 1,644.9 | **74.9%** |
| 1 | 1,524 | 1,208 (79.3%) | 471.9 | 1,613.5 | **77.4%** |
| 2 | 1,565 | 860 (55.0%) | 1,519.5 | 1,069.7 | 41.3% |

Two facts, both fatal to an eight-times expectation:

1. **55–79% of steps repair nothing at all.** A one-micron step usually leaves
   the layout proxy-feasible, the sweep loop breaks immediately, and there is
   nothing to hand eight workers. The fan-out fires on 411 of 1,500 steps in the
   equal-walk shape. Work is concentrated too: the top decile of steps by query
   count carries 38–72% of all candidate queries.
2. **The confirmation is the larger half**, and it is strictly serial: one
   61-piece rebuild pass and one `n(n-1)/2` pair loop, on one thread.

Amdahl on row (1)+(2), taken before anything was built: perfect eight-way
parallelism of the repair half alone predicts 1.25x / 1.25x / 2.06x. The
measured `lanes8` result (§5.2) is worse than that prediction, and §5.2 says
why.

---

## 3. A correction to `compression-schedule` README §6.1

§6.1 explains the 4.83 ms confirmation like this:

> a confirmation the validator **accepts** asks all `61 * 60 / 2 = 1,830`
> pairs, and at that round's own 1,904.8 ns per `exactOverlapTest` that is
> 3.485 ms, plus 61 collision-polygon builds at 4,149.3 ns = 0.253 ms, plus
> 0.049 ms of depth — 3.79 ms before anything else.

**The total is right and the attribution is wrong.** Profiled here, mode-34
minus the identical mode-0 preamble, equal-walk shape, seed 0:

| phase (schedule's own, preamble subtracted) | `serial` ms | calls | per confirmation | `pconfirm` ms | per confirmation |
|---|---:|---:|---:|---:|---:|
| `publicationValidate` | 1,583.9 | 317 | **5.028 ms** | **343.8** | **1.091 ms** |
| `exactOverlapTest` | **32.9** | 30,985 | **98.4 calls, 0.104 ms** | 83.9 | 0.266 ms |
| `collisionPolygonBuild` | ~0 | 0 | 0 | ~0 | 0 |

The 5.028 ms per accepted confirmation reproduces §6.1's 4.83 ms. But the
collision-grid overlap loop is **0.104 ms of it — 2.1%**, not 3.485 ms, and the
collision-polygon builds are not attributed to the schedule at all. §6.1's own
§6.3 already contains the reason and does not connect it: `Counter::ExactPairTests`
and the `exactOverlapTest` span are both entered *past* the broad-phase bounds
reject, so a 1,830-pair confirmation reaches that code about **98** times, not
1,830. Multiplying 1,830 by a per-*narrow-phase* cost double-counts by roughly
18x — the same factor §6.3 names for the self-meter's over-charge, applied by
mistake in the opposite direction.

The right-hand columns are the same phase span under `pconfirm=1`: **5.028 ms →
1.091 ms, 4.61x**, measured by the profiler rather than by the schedule's own
`confirmationMs`, which is a second and independent instrument agreeing with
§5.1. (`exactOverlapTest`'s *total* rises, 32.9 → 83.9 ms, because that figure
is summed over threads and each thread now pays its own span overhead; its wall
falls with everything else.)

Where the 4.92 ms actually is: `validate_placements_against_contract` →
`validation::general_polygon::validate_publication`, whose own `n(n-1)/2` loop
calls `minimum_boundary_distance` on every pair. That walks every edge of one
material set against every edge of the other, has no bounds reject, and had no
phase span, which is why no round in this campaign had seen it. It is
essentially all of the remaining 4.79 ms.

**Consequence for the plan.** Grok's action 2 says "the deepest-confirmed slot
and exact confirmation cadence preserved (4.83 ms per accepted confirmation)".
Read as "leave the confirmation alone", that leaves 73–78% of the slice
untouched and caps the action at ~1.3x. Read as "preserve the cadence", the
4.83 ms is itself a target — and it is the one that paid.

---

## 4. Equal **work**: measurement (b)

Grok's measurement (b): equal-WORK matched arms on the twelve 171–179 mm pinned
parents, paired; the parallel schedule must not lose quality per unit of work.

The twelve parents are the compression-schedule round's. Seeds 0–2 are that
round's surviving fixtures; seeds 3–11 were regenerated here from the bare
request at `work=120000000` and **reproduce the committed raw depth, incumbent
fingerprint and work-unit spend to the digit on 9 of 9**
(`evidence/parents.json`), which is an independent confirmation of that round's
parent band from a different binary and worktree.

Every arm is the same armed binary at `past=1,rollback=0,work=3341379`,
differing only in `lanes` and `pconfirm`. Statistic: raw source depth of the
best exact-valid publication, parent as the floor for every arm.

| arm | publishes | median Δ (mm) | mean Δ | median schedule work | median steps |
|---|---:|---:|---:|---:|---:|
| `serial` (control) | **11 / 12** | **1.104** | 1.072 | 3,353,550 | 1,568 |
| `lanes4` | 9 / 12 | 0.479 | 0.493 | 3,347,170 | — |
| `lanes8` | 8 / 12 | 0.068 | 0.259 | 3,354,746 | 618 |
| `pconfirm` | **11 / 12** | **1.104** | 1.072 | 3,353,550 | 1,568 |
| `both` | 8 / 12 | 0.068 | 0.259 | 3,354,746 | 618 |

The control reproduces the compression-schedule round's `sched10-noroll` arm
exactly — median **1.104 mm**, 11 of 12 publishing — which is the check that
`lanes=1` is the shipped serial schedule and not a re-implementation of it.

Paired per cell, arm minus control:

| pairing | median advantage (mm) | wins | ties | losses |
|---|---:|---:|---:|---:|
| `lanes4` − `serial` | −0.546 | 1 | 0 | **11** |
| `lanes8` − `serial` | **−0.867** | 1 | 0 | **11** |
| `pconfirm` − `serial` | **+0.000** | 0 | **12** | 0 |
| `both` − `serial` | −0.867 | 1 | 0 | 11 |

**The repair fan-out fails this gate and the mechanism is not subtle.** The
fan-out charges every worker it dispatches — that is enforced by a test
(`the_fan_out_charges_every_worker_it_dispatched`) and is the honesty invariant
of the lever, because a best-of-eight repair that only paid for the branch it
kept would buy depth with unmetered work. At a fixed cap it therefore walks
**618 steps against the serial schedule's 1,568**, and the schedule's value is
the walk. Median discarded queries per arm: 1,998,928 against 276,832 kept.

The confirmation lever passes it exactly, 12 ties out of 12, because it changes
no trajectory at all.

---

## 5. Equal **walk**: measurement (c), the honest multiplier

Grok's measurement (c): paired interleaved ≥10 rounds of one slice on 3
parents, "the honest multiplier — occupancy 8x will NOT give wall 8x".

Equal **walk**, not equal work: `past=0`, no work cap, 1.5 mm drop, so the bound
alone sets the step count and every arm takes exactly 1,500 steps (verified —
`stepsTaken` is the single value 1,500 for all four arms across all 120 runs).
That is the arm in which the idle lanes are free. 10 rounds × 3 parents × 4
arms, interleaved, arm order reversed on odd rounds, n = 30 paired observations
per arm.

### 5.1 The multiplier

| arm | median slice ms | within-arm spread | **serial / arm, slice** | min–max | rounds above parity | serial / arm, process |
|---|---:|---:|---:|---|---:|---:|
| `serial` | 2,023.8 | 8.2% | 1.000 | — | — | 1.000 |
| `lanes8` | 2,191.9 | 31.9% | **0.912** | 0.758–0.951 | **0 / 30** | 0.953 |
| `pconfirm` | 773.3 | 88.2% | **2.623** | 1.587–3.079 | **30 / 30** | **1.431** |
| `both` | 995.6 | 161.8% | 2.036 | 0.913–2.900 | 28 / 30 | 1.334 |

The box was shared, and the within-arm spreads say so — 88% and 162% for the
two fast arms, against 8.2% for the control. They are reported next to the
deltas rather than smoothed away. The claim survives them: `pconfirm`'s
**worst** round of 30 is still a 1.587x speedup, and it is above parity in
30 of 30 paired rounds.

### 5.2 What `lanes8` costs, and why it is below parity

At equal walk the fan-out is **9.6% slower**, not free. Three losses, all
measured:

1. **The barrier's payload.** Each of eight workers gets a private clone of the
   frontier *and* of the `PairTracker`, whose `pairs` vector is
   `61 * 60 / 2 = 1,830` entries. Eight of those per fanned-out step, 411
   fanned-out steps.
2. **The winner is not eight times better, so the walk gets longer.** The fan-out
   ran 2,158 adopted sweeps against the serial lane's 1,822 for the same 1,500
   steps: a step whose eight branches all miss feasibility still costs its
   sweeps.
3. **Only the repair half was parallel here.** Repair went 440 ms → 727 ms while
   candidate queries went 352,650 → 3,540,584 — 10.0x the work for 1.65x the
   wall, so the fan-out really does achieve about **6.1x** effective throughput
   inside the repair phase. Pinned to one CPU the same arm takes 3,126 ms of
   repair against 621 ms unpinned (**5.03x**), which is the direct proof that
   the job pool is reached. But repair is only 22% of the slice, so 6x of 22%
   cannot outrun the clone cost plus the longer walk.

### 5.3 What `lanes8` buys

Paired quality at equal walk, over the same 30 observations:

| pairing | median (mm, positive = deeper) | wins | ties | losses |
|---|---:|---:|---:|---:|
| `lanes8` deeper than `serial` | **+0.028** | **30** | 0 | 0 |
| `pconfirm` deeper than `serial` | 0.000 | 0 | **30** | 0 |
| `both` deeper than `serial` | +0.028 | 30 | 0 | 0 |

Per parent the gain is 0.238 / 0.028 / 0.021 mm. It is real and it is
consistent — 30 of 30 — and it is small. The reduce picks a worker other than
worker 0 on 135 of 411 fanned-out steps (lane wins
`[276, 35, 23, 23, 16, 9, 14, 15]` on seed 0), so the fan-out genuinely finds
repairs the reference worker does not; it just does not find many that matter.

**So the honest multiplier for Grok's lever is 0.91x wall for +0.028 mm.** For
the lever the measurement pointed at instead, it is **2.62x wall for exactly
nothing lost**.

---

## 6. Determinism — the hard gate

Grok's action 2 makes this a gate, not a nicety: "if the parallel schedule
cannot be made deterministic in work mode, deliver the obstacle analysis with
file:line instead of a nondeterministic feature."

**It is deterministic, and the gate is met as worded.**

The argument is structural rather than empirical: every worker's entire input is
`(frontier, weights, depth, step, worker ordinal)` and nothing it reads depends
on which thread ran it; `map_slice_mut_with_job_pool` and
`map_slice_with_job_pool` return results in *input* order whatever order the
workers finish in; the reduce is a total order whose final tiebreak is the
worker ordinal, with `total_cmp` in the middle term; the counters are summed in
that same order; and every parallel validator reduces on the lowest-indexed
failure. The measurements:

**In-process, across pool widths.**
`parallel_compression_schedule_reproduces_across_pool_widths` runs the same
eight-worker schedule on a one-thread pool and on an eight-thread pool and
requires the entire report — every step row, the lane-win histogram, the work
units — to be equal, excluding only `repairMs` and `confirmationMs`. Two pool
widths differ in exactly the thing a nondeterministic fan-out would leak
through: which worker's result arrives first.

**Across processes, in work-budget mode.** Four arms × three parents × three
processes, `work=3341379`, whole-document digest:

```
ALL_REPRODUCIBLE  true      12 / 12 cells, 1 distinct digest each, 3 processes each
```

**Two more repairs to the digest, and the controls that forced them.** The
`se2-rigidity` round repaired `doc_digest` once, for statistics computed from
`elapsedMs`. It was still incomplete, and this round found it twice — both times
because a battery contained a control that *cannot* legitimately differ.

*First:* the determinism gate's opening run reported 3 distinct digests for
**every** cell, including the `serial` shipped schedule, which is deterministic
by construction. The leaf diff said why: the only fields that moved were
`repairMs` and `confirmationMs`, the compression-schedule round's own
wall-clock decomposition, never added to `VOLATILE`. **Without the serial
control this would have been recorded as the parallel schedule failing its own
gate.**

*Second:* the HEAD-parity check (§7.2) reported mismatches on all three seeds
between HEAD and a build that computes the same search. 43–59 leaves differed
and **every one was a clock**: `birthSeconds`, `startedSeconds`,
`publishedSeconds`, `/portfolio/publications/N/seconds`, and — the interesting
one — `/portfolio/archive/occupancyOverTime/N/0`, where the timestamp is
**array element 0** and a filter that drops by leaf *name* structurally cannot
reach it. That is a limit of the instrument worth naming rather than working
around: a document whose only volatile field were positional would defeat
`doc_digest` silently.

`drivers/lib.py` now excludes all of them, each with the measurement that
justified it. Every digest verdict in this README was then recomputed from the
stored run documents (`drivers/redigest.py`) and is reported next to its **leaf
count**, so a match is a match on leaves and not on a hash.

**How far `pconfirm` is semantics-preserving.** Leaf-by-leaf against the serial
arm's document, on all three parents:

| arm | differing leaves vs `serial` |
|---|---:|
| `pconfirm` | **1** — `compressionSchedule/parallel/parallelConfirm`, the flag that says it was armed |
| `lanes8` | 12,662 / 11,929 / 21,270 |

One leaf, and it is the diagnostic. Every placement, every step row, every
counter, every exact pair test count is identical.

**The one thing that is not identical, stated plainly.** A *refused*
confirmation charges more `exactPairTests` in the parallel validator than in the
serial one: the serial nest stops at the first violating pair and the parallel
one lets every row finish its own scan. On this corpus that is unobservable —
the schedule refuses 0 of 884 confirmations measured, because
`due_for_confirmation` only offers layouts the proxy tier already calls feasible
— but it is a real difference on the failure path and it is why `pconfirm` is a
flag rather than a default.

---

## 7. Gates and suites

### 7.1 The four pinned gates, on four binaries

Drivers copied from `docs/experiments/se2-rigidity/drivers/` (the repaired
`doc_digest`, repaired twice more here — §6), `ROOT` repointed at this worktree,
binaries rebuilt immediately before gating, exit codes captured directly.

| binary | features | g1 | g2 | g3 | g4 | digest | differing leaves vs HEAD |
|---|---|---|---|---|---|---|---|
| `base-jagua` | HEAD `65f6fc9`, `jagua-experimental` | pass | pass | pass | pass | reference | — |
| `mine-jagua` | this worktree, `jagua-experimental` | pass | pass | pass | pass | **identical** | **0 / 0 / 0 / 0** |
| `mine-csched` | + `compression-schedule` | pass | pass | pass | pass | **identical** | **0 / 0 / 0 / 0** |
| `mine-parallel` | + `parallel-compression-schedule` | pass | pass | pass | pass | **identical** | **0 / 0 / 0 / 0** |

All four pinned values reproduce (206.869 / 8a7737381238fa4d,
159.09233022733062 / fa01012af1d559ae, 159.07876040364795 / e28fba007f8031d4,
164.0375677990678 / 49f094d7e59a9008), and the four binaries agree as whole
documents — zero differing leaves, not merely equal hashes. The armed build is
in that list on purpose: the gates are modes 20 and 22 and never reach mode 34,
so an armed build with an unarmed spec must be the shipped engine, and it is.

### 7.2 The path the gates do not cover

The gates never schedule mode 34, and this round touched the coordinator
(two `PortfolioSettings` fields and the settings construction in
`execute_v3_action`). So the parity is also measured on the path that does:
the v3 coordinator from the bare request, HEAD against the armed build with an
unarmed spec.

The budget is **work**, not wall, for the reason the wall batteries above make
obvious — a wall-budgeted coordinator run is not reproducible across processes
even on one binary, so a wall comparison could not separate "the build differs"
from "the box was busy". At `work=40000000,v3=1`, three seeds:

| seed | HEAD raw depth | armed, unarmed spec | digests match | differing leaves |
|---:|---:|---:|---|---:|
| 0 | 169.891 | 169.891 | yes | **0** |
| 1 | 171.3619986855876 | 171.3619986855876 | yes | **0** |
| 2 | 170.155 | 170.155 | yes | **0** |

### 7.3 Suites

Re-run against the final tree — the first pass predated the contract
parallelisation and the coordinator plumbing, and a suite result from a tree
that no longer exists is not a suite result. Exit codes captured directly.

| features | passed | failed |
|---|---:|---:|
| `jagua-experimental` | 1,257 | 0 |
| `jagua-experimental,compression-schedule` | 1,279 | 0 |
| `+parallel-compression-schedule` | **1,282** | 0 |

The three added tests are the determinism gate across pool widths, the
semantics-preservation of `pconfirm`, and the fan-out's work-charging invariant.
No flakes; `free_material_multi_eviction` did not need its rerun.

---

## 8. Measurement (d): the anytime battery, and Grok's hypothesis

Run because §5's multiplier is real for one of the two levers. All four arms are
carried anyway, including the one §4 and §5 say should lose, because an arm
predicted to lose and then measured losing is worth more than an arm quietly
dropped.

mixed-61 from the **bare request**, v3 coordinator (`v3=1` — Grok F1: a `v3=0`
spec never enters the loop mode 34 lives in), `sched`/`barren`/`divq` at their
shipping defaults, 3 seeds × 3 rounds paired and interleaved, arm order reversed
on odd rounds, n = 9 paired observations per budget per arm. The slice is armed
through the coordinator's own spec keys, off by default.

| budget | `serial` | `pconfirm` | `lanes8` | `both` |
|---|---:|---:|---:|---:|
| 3 s | 179.5869 | 179.5869 | 179.5869 | 179.5869 |
| 10 s | 173.5751 | **172.2875** | 175.7327 | 175.2090 |
| 30 s | 166.8080 | **163.9270** | 169.8010 | 168.8259 |

Paired, arm minus control (positive = the arm published a *deeper* — better —
layout):

| budget | `pconfirm` | `lanes8` | `both` |
|---|---|---|---|
| 3 s | +0.000, **0 W / 9 T / 0 L** | +0.000, 0/9/0 | +0.000, 0/9/0 |
| 10 s | **+1.882**, **9 W / 0 T / 0 L** | −2.158, 0/0/9 | −1.634, 0/0/9 |
| 30 s | **+3.359**, **9 W / 0 T / 0 L** | −2.322, 3/0/6 | −0.198, 4/0/5 |

**The 3 s row is the control and it is exact.** At 3 s the coordinator never
schedules a mode-34 action (`medianScheduleActions` = 0 for every arm), and
every arm ties to the digit on all nine pairs. A lever that moved anything where
its operator does not run would show up here, and none does.

**The mechanism is visible in the driver's own counters, not inferred.** The
faster slice does not make a slice better; it makes room for another one:

| budget | arm | m34 calls (median) | m34 seconds (median) |
|---|---|---:|---:|
| 10 s | `serial` | 1 | 1.95 |
| 10 s | `pconfirm` | **2** | 2.88 |
| 10 s | `lanes8` | 1 | 2.30 |
| 30 s | `serial` | 3 | 7.47 |
| 30 s | `pconfirm` | **4** | 7.05 |
| 30 s | `lanes8` | **2** | 4.27 |

`pconfirm` buys one extra scheduled action at both budgets. `lanes8` *loses*
one at 30 s, which is exactly its 0.912x slice: a slower operator fits fewer
times into the same wall, and it is why it is 9 of 9 below the control at 10 s.

### Grok's hypothesis, answered as asked

> "this brings 10s toward the 40M-work quality (~166), not to 150. Report where
> it lands."

**It lands at 172.288 at 10 s.** That is +1.882 mm on the paired median against
the 173.575 control — real, 9 of 9, and *not* the hypothesis. Reaching ~166
needs about 7.6 mm and this delivers a quarter of it; 150.165 is not in view and
this round makes no claim in that direction.

Two things the hypothesis got right and one it got wrong:

* Right: 8x occupancy does not give 8x wall. Measured 2.62x on the slice, 1.43x
  on the process, from a 0.99 → 3.16 lane occupancy.
* Right: this does not reach 150.
* Wrong: the lever it named. `lanes=8` — intra-arm parallelism of the *repair*,
  which is what action 2 specifies — is **−2.158 mm at 10 s, 0 wins in 9**. The
  gain comes entirely from the confirmation, which action 2 listed under
  "preserved" rather than under "parallelised".

At **30 s** the picture is better than the hypothesis: 163.927 is past the
40M-work band (165.8–171.4) and inside the 120M-work band that
`coordinator-v4` measured at 162.161 / 163.927 / 164.004. That is the
wall-versus-work gap Grok's §2a estimated at 4–6 mm being closed at 30 s, and
partly closed (1.9 of it) at 10 s.

**Caveat on spread.** The within-arm spread across seeds is 4.8–6.2 mm at 10 s
and 30 s, far larger than every effect reported here. That is why the statistic
is the *paired* per-seed-per-round delta and not the difference of medians, and
why the win/tie/loss counts are printed next to it: 9 of 9 and 0 of 9 are the
claim, the medians are the size.

---

## 9. Honest limits, and what this does *not* claim

* **The lever Grok specified does not pay, and should not be armed.** `lanes=8`
  is 0.912x wall at equal walk, −0.867 mm at equal work, and −2.158 mm at 10 s
  with 0 wins in 9 from the bare request. Its 30/30 +0.028 mm at equal walk is
  real but is not worth 9.6% of the slice. The premise ("m34 is one lane, m26
  uses eight") was correct; the inference ("so occupying the lanes is the
  lever") was not, because the idle lanes are idle during the 22% of the slice
  that is repair, not during the 78% that is confirmation. It ships behind the
  flag at `lanes=1` — the serial default — and this README is the reason it
  should stay there.
* **`pconfirm`'s 2.62x is a slice number, not a request number.** The whole
  process moves 1.43x, because the mode-0 preamble is unchanged and is about
  half the process.
* **The within-arm spread is large** (88% for `pconfirm`, 162% for `both`) and
  the box was shared throughout. The speedup claim rests on 30/30 rounds above
  parity and a worst round of 1.587x, not on the median alone.
* **`minimum_boundary_distance` was not made faster, only spread.** The real
  finding of §3 is that the publication contract costs 4.8 ms per layout and is
  `O(n^2 * edges^2)` with no broad phase. Parallelising it is a 5x; fixing it
  (a bounds reject, the same one the collision tier already has) is plausibly a
  much larger constant, on *every* publication in the engine rather than only
  mode 34's. That is the next action this round would recommend, and it is not
  the action this round was given.
* **Nothing here touches `rollback_after_steps`,** which stays 0.
* **No wall claim is made from process wall alone.** Every multiplier is a
  paired per-round ratio with its spread reported.

---

## Files

* `drivers/anatomy.py` — the slice's wall decomposition and its lane occupancy,
  under GNU `time`, with the mode-0 preamble measured in the same shape.
* `drivers/workgate.py` — measurement (b), equal-work matched arms on twelve
  pinned parents.
* `drivers/wall.py` — measurement (c), paired interleaved equal-walk A/B.
* `drivers/anytime.py` — measurement (d), 3/10/30 s from the bare request.
* `drivers/determinism.py` — the cross-process work-mode gate.
* `drivers/gates.py`, `drivers/lib.py` — the four pinned gates; `lib.py` carries
  this round's two `VOLATILE` repairs (§6).
* `drivers/headparity.py` — HEAD against the armed build with an unarmed spec,
  on the coordinator path the gates do not reach.
* `drivers/redigest.py` — recomputes every digest verdict from the stored run
  documents with the final repaired digest, and reports leaf counts beside them.
* `drivers/collect.py` — assembles `evidence/`.
* `drivers/parents.py`, `drivers/runlib.py` — copied from
  `compression-schedule/drivers/`, `ROOT` repointed.
* `evidence/parents.json` — the twelve parents and their reproduction check.
* `evidence/occupancy.json`, `evidence/anatomy-*.json`, `evidence/workgate.json`,
  `evidence/wall.json`, `evidence/anytime.json`, `evidence/determinism.json`,
  `evidence/gates.json`, `evidence/headparity.json`,
  `evidence/phase-attribution.json`, `evidence/taskset.json`.
