# PR7: the thin portfolio coordinator

Sol's review 3 asks for a coordinator with **two state objects** and a
**time-budgeted schedule**, and its finding 2 says why the engine cannot get
there with one:

> Mode 20's 206.869 candidate correctly loses to the 179.756 protected
> incumbent - but the documented from-scratch lineage begins from precisely this
> class of worse, structurally different constructor basin. If PR7 feeds only
> `GeneralRelaxedOutcome.result` forward, it destroys the only evidenced route
> to 164.

This is that coordinator, measured from the bare request at a ten-second wall
budget against the m0+coupled baseline.

## The headline

Nine paired rounds, three seeds, arms interleaved with the order rotating every
round, on a box shared with another benchmarking agent. Depth is the engine's
own `independentUsedLongAxisDepthMm`; the raw source depth agrees to the
reported digits.

| seed | m0+coupled baseline | coordinator (review schedule) | coordinator (focused) |
|---:|---:|---:|---:|
| 0 | 181.589 | **179.587** | **179.587** |
| 1 | **179.690** | 176.753 *(176.056 / 179.633 / 176.753)* | **176.056** *(3/3)* |
| 2 | 179.662 | **179.006** | **179.006** |

* **Paired delta, median over nine rounds: −2.002 mm.** Both coordinator arms.
* **9 of 9 rounds strictly better than the paired baseline.** Both arms.
* Against the stated bar - the baseline's **179.690 flatline at seed 1** - the
  focused arm publishes **176.056 mm in all three rounds**, a **3.634 mm
  margin**. The review's own schedule reaches the same layout in two rounds of
  three and stalls at 179.633 in the third; that spread is wall-clock
  nondeterminism, which the wall-budget mode does not pretend to avoid.
* The stretch goal (≤ 175 mm) is **not** reached. The best layout any arm
  published in ten seconds is 176.056 mm.
* Every published layout is dual-gate valid: publication goes through
  `general_relaxed::adopt_published_placements`, the same function the coupled
  separator's own mode slot publishes through, and the report carries the
  validator's verdict per run.

Time to depth, seed 1, median over three rounds (from the quality-frontier
trace, counters off so the clock is undistorted):

| threshold | baseline | review schedule | focused |
|---|---:|---:|---:|
| ≤ 185 mm | 0.702 s | 0.680 s | 0.671 s |
| ≤ 182 mm | 1.039 s | 1.135 s | 1.119 s |
| ≤ 180 mm | 1.807 s | 1.728 s | 1.734 s |
| ≤ 179.69 mm | 1.807 s | 1.728 s | 1.734 s |
| ≤ 179 mm | **never** | *(2 of 3 rounds)* | **6.800 s** |
| ≤ 178 mm | **never** | *(2 of 3 rounds)* | **7.526 s** |
| ≤ 177 mm | **never** | *(2 of 3 rounds)* | **7.526 s** |
| ≤ 175 mm | never | never | never |

The first 1.8 seconds are the same curve in all three arms, to within the
box's noise, because they *are* the same search: the coordinator's phase 0 is
the protected mode-0 run, unchanged. Everything below 179.69 mm is new.

## The two state objects

`crates/polygon-nesting-core/src/search/portfolio.rs`.

**`PublishedIncumbent`** - always dual-gate valid, best raw depth, and it moves
only through the existing adoption rule. `try_publish` hands a complete layout
to `adopt_published_placements`, which re-runs the composite exact validator
against the *real* request, requires complete cardinality, and requires a strict
raw-depth improvement. Adoption is detected by the fingerprint moving, because
that is the only way that function can have said yes. The coordinator has no
validity opinion of its own and cannot acquire one.

**`SearchArchive`** - basins keyed by placement fingerprint, each carrying raw
depth, birth time (seconds and work units), operator provenance, parent
fingerprint and its exact-validity verdict. Its retention rule:

1. Incomplete layouts are refused: they cannot be anyone's parent.
2. A fingerprint already present is a duplicate; the first arrival keeps its
   provenance.
3. Under capacity **everything** is admitted, including a basin deeper than
   every member. Depth is not an admission criterion, because the ledger's own
   eighteen-sample sweep measured Pearson(immediate, descended) = −0.212.
4. At capacity a member is evicted **only if some other layout is both
   dominated-by and similar-to it** - no deeper, and piece-assignment overlap at
   or above the similarity threshold. A full archive of mutually distinct basins
   refuses the newcomer rather than dropping a distinct member.

Structural distance is the fingerprint as the cheap first cut and
`assignment_overlap` - the fraction of pieces at an exactly identical pose - as
the better one. There is no tolerance in it, deliberately: a tolerance would be
a length, and a length would have to come from somewhere.

## The schedule, and where the time actually went

Phase deadlines are fractions of the budget, defaulting to the review's own
sketch. They are deadlines, not allocations: a phase that finishes early hands
the remainder on, and a phase whose deadline has passed is skipped and says so.

Median phase wall over the nine review-schedule runs, and what each phase
bought:

| phase | review sketch | measured wall | operator calls | publications | Δmm published |
|---|---|---:|---:|---:|---:|
| `m0` protected | 0–1.9 s | 2.83 s | 0 | 0 | — |
| `basins` salted mode 20 | 1.9–4.0 s | 1.24 s | 19 | **0** | **0.000** |
| `descent` mode-22 quanta | 4.0–6.8 s | 3.71 s | 27 | 9 | 8.145 |
| `crossover` mode 23 | 6.8–7.4 s | 0.00 s | 4 | 2 | 5.760 |
| `compression` m31 → m22 | 7.4–9.6 s | 0.98 s | 12 | 0 | 0.000 |
| `drain` | 9.6–10 s | 0.00 s | 0 | 0 | 0.000 |

Per-call economics, over all nine runs of each arm:

| operator | calls | mean wall | exact-valid | published |
|---|---:|---:|---:|---:|
| `basins/mode20` | 19 | 0.613 s | 19 | **0** |
| `descent/mode22` | 27 | 1.168 s | 27 | 9 |
| `crossover/mode23` | 4 | 2.716 s | 4 | 2 |
| `compression/mode31` | 6 | 0.096 s | **0** | 0 |
| `compression/mode22` | 6 | 1.036 s | 6 | 0 |

Three things this table says.

**The constructor slice does not pay at this budget.** Nineteen salted mode-20
arms, every one exact-valid, every one refused by the adoption rule, and not one
of their descendants ever caught the incumbent inside ten seconds. They cost
0.613 s each - which is the `fast-constructor-profile` price, 4.2× cheaper than
the shipped evaluator, and still not cheap enough. The `focused` arm prices this
directly by setting the slice to zero: it is **never worse** and on seed 1 it is
**better and more consistent**, because the 1.24 s goes to the crossover phase
instead. That is a measured verdict on the review's 1.9–4.0 s allocation at this
budget, not on mode 20's mechanism.

**Mode 31 never legalised anything.** Six calls, zero exact-valid results, all
of them "global legalization did not reach a feasible fixpoint". The review
already said m31 is "production-worthy only as the legalizer for a
compressed/perturbed frontier"; on a clean m22 fixpoint it has nothing to
legalise, and this measures that.

**Mode 23 is the second most productive operator here.** Two publications in
four calls under the review's schedule, three in nine under the focused one, and
the largest single gains in the run. The review called it "conditional but
currently evidence-required"; it is now evidence-*producing*.

Archive occupancy over time, one review-schedule run per seed:

| run | final occupancy | provenance | evictions | duplicates | refused-full |
|---|---:|---|---:|---:|---:|
| seed 0 | 8 / 16 | 1 constructor, 1 m0, 2 mode20, 3 mode22, 1 mode23 | 0 | 3 | 0 |
| seed 1 | 8 / 16 | same shape | 0 | 3 | 0 |
| seed 2 | 8 / 16 | same shape | 0 | 4 | 0 |

The focused arm ends at 4–5 of 16. **The archive never filled on this stream, so
its eviction rule is exercised by unit tests and not by the measurement** - that
is a caveat, not a claim. The three coupled-separator arms are offered and all
three come back `Duplicate`, which is itself a small finding: the m0 result *is*
the boundary-projection arm, so the separator's three arms are one layout.

## Two schedule defects the trace found

Both were found by reading the coordinator's own operator-call ledger, and both
are recorded because a schedule that was right first time would not have needed
the instrument.

**Charging a constructor's pose prior as a descent.** The archive orders its
frontier partly by how often a basin has been descended from. Mode 20 does not
descend from its parent - it builds from scratch and reads the parent as a pose
prior - so charging it a descent pushed the incumbent to the back of a queue it
should have led. The first schedule spent its whole alternation phase on
194–214 mm constructor basins while the one parent whose quantum published
waited. `ParentRole` is the fix.

**Ordering the frontier by fairness before quality.** The review's phrase is
"m22 work quanta across the **best** structurally distinct archive states", and
the word is load-bearing. Sorting by descent count first is fairer and measured
worse. `evidence/curve-schedule-v1-fairness-first.json` is that schedule's nine
rounds: −2.002 mm median as well, but its best layout is 179.006 mm rather than
176.056 mm, and seed 1 gets 179.545–179.633 instead of 176.056.

## One experiment built, measured and declined

**Iterated deepening of the alternation quantum.** When the frontier is a
fixpoint at the current quantum size, the obvious move is to double the quantum
and go round again rather than end the phase early with budget left. It is
built, it is `descent_iterated_deepening`, and it is **off**: it keeps the phase
busy by spending the crossover phase's budget, and the crossover phase is the
second most productive operator in this schedule. Seed 1 goes from 176.056 to
179.633 under the review's schedule and from 176.056 to 176.753 under the
focused one. `evidence/curve-iterated-deepening-probe.json`.

## Determinism: the work-budget mode

A wall-clock schedule branches on a clock, so two of its runs are two different
searches on a shared box - which the seed-1 spread above shows directly. The
work-budget mode branches only on the engine's own counters: one work unit is
one proxy candidate query, and an exact Clipper pair test is charged 5, a ratio
read off the quality-frontier trace's own scope ledger (1.108 µs against
0.224 µs).

| gate | budget | result |
|---|---|---|
| in-process `runs=2` replay | 40M units | pass - the benchmark's own fail-closed replay check |
| two independent processes, whole documents | 40M units | **0 differences**, depth 176.056, 33,286,633 units spent |
| in-process `runs=2` replay | 20M units (binding) | pass |
| two independent processes, whole documents | 20M units (binding) | **0 differences**, depth 176.056, 20,360,238 units spent |

The binding run is the interesting one: at 20M the `basins` phase is skipped
(mode 0 alone spends 9.63M, past the 8M deadline), `descent` gets 4.12M instead
of 17.04M, and `drain` is skipped - a *different* schedule, taken identically by
both processes.

One honest limit: the deep operators' own Clipper counters live behind
`search-profiling`, which is off, so the `basins` phase charges only 2,310 work
units for 2.44 s of real work. A work budget therefore under-prices constructor
arms relative to relaxed ones. The phase is still bounded - by its slot count -
so the schedule terminates, but the work currency is not yet a faithful proxy
for wall time across *all* operators, and no number here claims it is.

## The coordinator's own overhead

Paired A/B on the phase both arms share. Arm `outside` is the plain engine run;
arm `inside` is the same search as the coordinator's phase 0 with a zero budget,
so every later phase is entered, finds no room, and is skipped. The difference
is the coordinator's fixed cost on work it did not add: five archive offers,
each re-measuring a raw depth and re-running the composite validator on a
61-piece layout, plus the incumbent's own validation.

Ten interleaved rounds per sample, arm order alternating, statistic the
per-round paired ratio of the benchmark's own measured stream:

| sample | paired median | spread | rounds below parity | same engine depth |
|---|---:|---|---:|:--:|
| 1 | 1.020 | 0.701–1.131 | 2/10 | yes, 179.690 both arms |
| 2 | 1.049 | 0.950–1.106 | 3/10 | yes, 179.690 both arms |

So roughly **2–5%**, on a box whose same-work times ranged 2.5–6.4 s during
sample 1 because another agent was benchmarking; the 0.701 round is that
contention and not a coordinator speedup. Both samples are reported for the
reason the ledger's fused-pair-query entry gives: one sample of this would have
been written up as a number it is not.

## Protected legacy

All four pinned regression gates reproduce the pristine `0cf1163` binary as
**whole documents** on the default-features worktree binary - every counter,
every restart row, every diagnostic field, wall-clock and build-identity fields
removed:

| gate | value | fingerprint |
|---|---:|---|
| mode 20 `independentDepthMm` | 206.869 | `8a7737381238fa4d` |
| mode 22 raw | 159.09233022733062 | `fa01012af1d559ae` |
| mode 22 raw | 159.07876040364795 | `e28fba007f8031d4` |
| mode 22 raw | 164.0375677990678 | `49f094d7e59a9008` |

`drivers/docdiff.py` is the comparison; `evidence/gates-pristine.json` and
`evidence/gates-worktree.json` are the two runs.

## Reproducing

```
python3 drivers/gates.py pristine <pristine-binary>
python3 drivers/gates.py worktree <worktree-binary>
python3 drivers/docdiff.py <dir-a> <dir-b> g1 g2 g3 g4
python3 drivers/curve.py 3 <measure-binary> 0 1 2
python3 drivers/summarize.py
python3 drivers/determinism.py <measure-binary> 20000000 1
python3 drivers/overhead.py 10 <measure-binary> 1
python3 drivers/collect.py
```

`drivers/lib.py` carries the pinned CLI tail; point `ROOT` at your worktree. The
measurement binary is

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,fast-constructor-profile,quality-trace
```

and the coordinator is armed by the trailing positional argument 48, a
`key=value` spec:

```
... 0 '' '' '' 0.002 'wall=10000,slots=4,states=3,cycles=1,epochs=4,cells=13:15:17:19'
```

Absent or empty, every existing invocation is byte-identical.

## What this does not measure

* One request. Mixed-61, exact clearance, one box. The schedule's shape is
  general code; its *verdicts* - that the constructor slice does not pay, that
  m31 has nothing to legalise, that m23 does pay - are measurements of this
  stream.
* The archive's eviction rule, which never fired here.
* Any budget other than ten seconds. The 18.3-second unbounded probe reaches the
  same 176.056 mm, so the schedule's own ceiling on this stream is close to what
  ten seconds already buys; that is one probe and not a curve.
* Anything about 160 or 150. The review is explicit that orchestration alone
  cannot reach them, and nothing here contradicts it.
