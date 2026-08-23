# The profile census, and the executor go/no-go (2026-08-23)

Wave 1 of the economics round: [`../../../../economics-round-spec.md`](../../../../economics-round-spec.md).
**No quality-semantic edit of any kind.** Everything here is a measurement, a
diagnostic, or a driver repair, and the last section lists what each one is.

---

## The verdict, first

> **DO NOT BUILD the persistent executor.**

The spec's clause, quoted rather than chosen:

> **Persistent executor, behind a measured gate**: profile easy + bite-22 hard
> states, workers 1/2/4/8, identical fixed work (prep, dispatch/join, sweeps,
> merge+GLS, exact/repair separately). **Build iff prep+dispatch ≥ 10 % of
> hard-state wall.**

Measured, on the frozen eight workers, at the 179 shelf, over 200 master
iterations, in two processes, on six seeds:

| | prep + dispatch, share of barrier-to-barrier |
|---|---|
| **the bar** | **10.000 %** |
| largest reading anywhere in the verdict set | **5.082 %** |
| smallest | 4.088 % |
| median | 4.709 % |

`BUILD_PERSISTENT_EXECUTOR: false`. The gate is rendered on the reading **most
favourable to building** — the largest prep+dispatch share over every seed at
the shelf density and both of its processes — so a NO-GO cannot be an artefact
of one slow process or one unlucky seed. The bar is missed by a factor of 1.97.

**Nothing about the 5/9 clause changes.** The spec is explicit: "If not built,
the 5/9 clause does NOT drop (no 4/9 — Grok's refusal, unanimous in the end)."

**What Wave 2 does with this.** The executor agent's half of the parallel wave
has no funded work. The three implementation constraints the spec wrote for it
(local Rayon pool of 8, persistent slots, `clone_from`, ordinal merge; the
forbidden list; the parked-thread fallback) are not exercised, and the promotion
gate (bit-identity over ≥1,024 batches, ≥1.15× shelf p50, ≥1.10× geomean,
≤5 % any-fixture regression, ≤10 % RSS) is not run, because there is nothing to
promote. The meter agent's half is unaffected.

**Why the answer is this lopsided.** The critical-path sweep is **94.4–95.5 %**
of a master iteration at the shelf. There is no room under it for a 10 % tax to
hide, and the executor would be recovering at most five percent of a wall that
is spent almost entirely inside `worker_sweep`. The measurement below also says
where the five percent is: about **two thirds is thread creation and join** and
**one third is slot preparation** (median 3.085 % and 1.432 % of the iteration),
and only the second of those is what `clone_from` into persistent slots would
remove outright.

**And in absolute terms it is 314 µs.** Preparing eight slots and dispatching
eight threads costs **309–331 µs per master iteration** on every shelf arm, at
every seed. That number is close to constant; the sweep it is divided by is not.
That is the whole shape of the result, and §1.1 is about nothing else.

---

## 1. The ladder

`evidence/spawntax.json`. Nine seeds × workers 1/2/4/8 × two processes = 72
processes, each one the constructor, 21 published 0.1 % bites at the **frozen
eight workers**, then 200 master iterations on the 22nd bite at the arm's own
worker count. Binary `d9ef083e…` (`--features overlap-ics,ics-profile`),
request `ecfe126f…`, x86_64, 16 cores.

Shares of barrier-to-barrier at the shelf, median over the seeds that reached
it:

| workers | prep | dispatch | **prep+dispatch** | tax, µs/iteration | critical sweep | merge+GLS | sweep parallelism |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0.060 % | 0.001 % | **0.061 %** | 3 | 99.8 % | 0.08 % | 1.00× |
| 2 | 0.342 % | 2.496 % | **2.847 %** | 186 | 96.8 % | 0.34 % | 1.83× |
| 4 | 0.736 % | 2.584 % | **3.235 %** | 220 | 96.4 % | 0.33 % | 3.45× |
| **8** | **1.432 %** | **3.085 %** | **4.589 %** | **314** | **94.9 %** | 0.31 % | **6.63×** |

Three things worth reading off it:

* **At one worker the tax is 0.061 % — three microseconds — and dispatch is
  0.001 %.** No thread is created on that path: `tournament` takes the
  `workers == 1` branch. The ladder's bottom rung is a zero by construction,
  and that is what makes the other three rungs a measurement of the tax rather
  than of the box.
* **The tax is real and it grows with the worker count**, from 186 µs at two
  workers to 314 µs at eight — sub-linear in the thread count, and dominated by
  dispatch rather than by preparation at every rung above one. It is not noise.
  It is simply not 10 % of anything the trajectory spends its wall on.
* **The eight workers are worth having**: 6.63× of sweep work lands on a
  critical path one sweep long. The tournament is not paying eight times for
  one sweep's progress.

Per-seed, at eight workers, at the shelf. The first four columns are the first
process; `p+d max` is the larger of the two processes, and is what the verdict
reads:

| seed | prep | dispatch | prep+dispatch | critical sweep | **p+d max** |
|---:|---:|---:|---:|---:|---:|
| 0 | 1.658 % | 3.164 % | 4.822 % | 94.62 % | **5.082 %** |
| 1 | 1.204 % | 2.885 % | 4.088 % | 95.50 % | 4.088 % |
| 3 | 1.501 % | 3.561 % | 5.062 % | 94.45 % | 5.062 % |
| 4 | 1.363 % | 2.800 % | 4.163 % | 95.38 % | 4.255 % |
| 7 | 1.349 % | 3.006 % | 4.355 % | 95.24 % | 4.355 % |
| 8 | 1.630 % | 3.440 % | 5.070 % | 94.40 % | 5.070 % |

### 1.1 The share is a ratio, and only one end of it moves

Three arms at eight workers are **not** at the shelf density, and one of them
reads over the bar — in this run seed 5 reads **12.547 %** and seed 2 reads
**7.668 %** (8.212 % on its other process). That has to be looked at properly
rather than waved past.

All figures in the table below are the first process, so the numerator and the
denominator come from the same measurement.

| seed | at shelf | iterations | **tax µs/it** | **sweep µs/it** | share |
|---:|:--:|---:|---:|---:|---:|
| 0 | yes | 200 | 310.8 | 6 098.0 | 4.822 % |
| 1 | yes | 200 | 320.2 | 7 479.4 | 4.088 % |
| 3 | yes | 200 | 315.2 | 5 881.2 | 5.062 % |
| 4 | yes | 200 | 308.8 | 7 074.8 | 4.163 % |
| 7 | yes | 200 | 313.6 | 6 858.3 | 4.355 % |
| 8 | yes | 200 | 330.6 | 6 155.8 | 5.070 % |
| 6 | no | 113 | 284.9 | 5 117.5 | 5.241 % |
| 2 | no | 24 | 261.6 | 3 063.4 | 7.668 % |
| 5 | no | 7 | 433.2 | 2 902.2 | 12.547 % |

**The numerator is flat and the denominator is not.** Seed 5's dispatch is not
expensive; seed 5's *sweep* is cheap — 2.90 ms against the shelf's 5.88–7.48 ms
— because at a stalled width of 181.517 the colliding set is small and a sweep
relocates far less. The same 300-odd microseconds of thread creation is a
larger fraction of a smaller iteration. Its 7-iteration window is also unstable:
across six census runs on this box that one arm read **5.721 %, 6.910 %,
9.161 %, 9.210 %, 10.685 % and 12.547 %** — a factor of 2.2 — while the six
200-iteration shelf arms stayed inside 3.5–5.3 % on every one of those runs.

So the answer to "what if the bar were applied to every arm regardless of
density" is: on some runs it would cross, on a 7-iteration window at a state the
clause does not name, in the direction a shorter window always moves a
fixed-cost share. The spec names *"bite-22 hard states"* precisely because that
is where the trajectory's wall actually goes, and the honest reading is the one
the numerator gives — **the tax is ~314 µs per master iteration, and at the
hard state a master iteration is 5.9–7.5 ms.**

There is a real consequence in this for whoever revisits the executor: **a
persistent executor would help most exactly where the trajectory spends least
time.** At the cheap widths a master iteration is 3 ms and the tax is 8–12 %;
at the shelf, where the seeds that matter spend hundreds of iterations, it is
4–5 %. Buying the cheap end is not what the round is for.

`atShelfDensity` is a field of `evidence/spawntax.json`, computed from
`prefixAllPublished` and a 200-iteration probe that did not publish, so the
verdict set can be recomputed by anyone who disagrees with it.

### Which seeds are at the shelf, and who decided

The driver does not carry a list. An arm counts toward the verdict when
`prefixAllPublished` is true **and** its probe spent all 200 iterations without
publishing — both fields of `evidence/spawntax.json`. That gives
`[0, 1, 3, 4, 7, 8]`. Seeds 2 and 5 stall above 180 under fixed work; seed 6
reaches the shelf and then publishes at iteration 113; seed 3 reaches it at
eight workers and publishes before the cap at one, two and four. The spec's
regime map predicted exactly this: fast cascade `{2, 3; 6 near}`, strike-starved
shelf `{0, 1, 4, 5}`, different basin `{7, 8}`. No clause here requires seeds 7
or 8: dropping them leaves the maximum at 5.082 % and the verdict unchanged.

**Seed 0's prefix lands on `179.16566573285345`** — bit for bit the depth the
evidence audit's committed fixed-work replay reproduces on three machines. If
it had not, nothing above would be a measurement of the shelf, and the driver
exits non-zero on that clause.

### Load, and the honest error bar

The census ran six times on this box, at one-minute load averages between 2.27
and 4.23 on 16 cores. Its verdict maximum was **4.359 %, 5.082 %, 5.151 %,
5.154 %, 5.257 % and 5.310 %** — a 0.95-point band with no clean ordering
against load, which is what a run-to-run spread looks like rather than a load
effect. The committed run is the fifth; its number is not the largest of the
six, and the largest of the six (5.310 %) is still 4.7 points under the bar.
The two-process spread within a single shelf arm is 0.082–0.328 percentage
points. Every one of these readings is under half the bar.

---

## 2. The probe-on-cheap-bites defect, measured — and a finding for the pacer

The spec's pre-named defect (3): *"calibrating on bites 1-21 overstates iters/s
~1.5×; the probe is 400 iterations AT the 179 shelf."*

It is real, and the estimate was good. One master iteration at the shelf costs
this much more than one in the cheap prefix, at eight workers:

| seed | 0 | 1 | 3 | 4 | 7 | 8 |
|---|---:|---:|---:|---:|---:|---:|
| shelf ÷ prefix, per iteration | 1.600× | 1.774× | 1.494× | 1.973× | 1.864× | 1.536× |

**But the same comparison in the member's own work unit is flat:**

| seed | 0 | 1 | 3 | 4 | 7 | 8 |
|---|---:|---:|---:|---:|---:|---:|
| shelf ÷ prefix, sample evaluations per second | 1.040× | 1.021× | 0.966× | 1.074× | 1.109× | 1.066× |

The shelf iteration is not slower per unit of work. It does *more work per
iteration*, because the colliding set at 179 is large and the colliding set two
bites from the constructor is small.

**This is a first-class input to Wave 3 and it is worth stating plainly: the
defect is a property of the denominator, not of the machine.** A plan
denominated in master iterations per second inherits a 1.49–1.97× calibration
error from where it was probed. A plan denominated in `sampleEvaluations` — the
first term of the spec's `U` — carries an 11 % one. That is an argument for the
currency the spec already chose, arrived at from the other end, and it is the
strongest reason in this document to keep `U` denominated in work rather than
in batches. It is **not** a licence to skip probing at the shelf: `B` multiplies
`master_batches`, and that term still has to be measured where the batches are
expensive.

---

## 3. The instrumentation did not move the trajectory

`evidence/identity.json`, 16 vectors, all green, four fixed-work cells × three
binaries × two processes each.

| | question | result |
|---|---|---|
| **D1** | every field the pre-Wave-1 binary emitted, reproduced bit for bit | 4/4 green |
| **D2** | two processes of the new default build agree bit for bit | 4/4 green |
| **D3** | the `ics-profile` build takes the default build's trajectory | 4/4 green |
| **D4** | two processes of the `ics-profile` build agree on everything but time | 4/4 green |

The cells are `A` (8 bites, 8 workers, seed 0 — the FAST K=8 shape), `B` (21
bites, seed 0 — the shelf's parent), `C` (21 bites, seed 5 — the watch seed)
and `D` (8 bites, **1 worker** — the no-thread path). All four are fixed-work,
so no clock is read inside any trajectory and none of this is load-dependent.

**D1 is a left-subset comparison**: new fields are allowed, and every field that
existed before must be bit-identical. That is the only honest shape for a round
that adds evidence. Floats are compared on their `repr`, not with a tolerance.

**D3 is what makes the feature gate a fact.** The census is measured on a build
the gates are not; D3 says that build takes the same trajectory, so its
nanoseconds describe the shipped one. The counters — `iterations`,
`bandEntries`, `exactCalls`, `sampleEvaluations`, `repairRows`,
`disruptionMoves` — are deliberately **not** in D3's ignore set: they are
populated in both builds and if they ever disagreed the two builds would be
taking different trajectories.

### And the work is charged once

The spec ranks *"persistent-slot leakage / double-debit ('stable but false' work
accounting — the worst class this round has)"* first among this round's
pre-named defects. `PhaseProfile` charges each of the currency's five terms to
the bite that spent it, by taking the delta of `work` around the tournament,
around the publication attempt and around the disruption — so if any of it were
being charged twice, or to nobody, the per-bite deltas would stop summing to the
trajectory's own totals.

Two batteries check exactly that, and both are green:

* `cutclose.py`'s tripwire 5, in **FAST**: per-bite `sampleEvaluations`,
  `repairRows`, `disruptionMoves` and `masterBatches` against `work`'s own
  totals and against `sweeps`;
* every arm of this census, on the 21-bite prefix, which unlike the FAST cell
  actually repairs — `allPerBiteCurrencyReconciles: true` over all 72 processes.

Measured on the 21-bite seed-0 cell: `sum(profile.sampleEvaluations)` **1 089 047**
= `work.sampleEvaluations`; `sum(profile.exactCalls)` **32** =
`work.exactCheckpoints`; `sum(profile.repairRows)` **19** = `work.repairRows`;
`sum(profile.iterations)` **100** = `sweeps`. Exactly, on every term.

Beyond this battery, the whole evidence audit (`../../evidence-audit/run-all.sh`)
re-runs green on this tree, exit 0, including its 314 counter identities, its
`rust-vectors`, its nine wall cells, its 90 publication-chain identities and its
three fixed-work replays — `ALL_TWO_PROCESS_BIT_IDENTICAL: true` and
`ALL_REPRODUCE_COMMITTED: true`, so `179.16566573285345`,
`179.17057349197626` and `181.51730509414207` still reproduce bit for bit. The
FAST tier is green, `FAILURES=0`, and `cutclose.py` reproduces the committed
`cutclose-fast.json` relocate economics exactly (`relocates 590`,
`containerSamples 29500`, `focusedSamples 14750`, `containerWinners 3`,
`containerCommits 3`, `focusedWinners 84`, `stayPutWinners 503`).

---

## 4. The audit's instrumentation items

### F4 — `exactAttempts` counted band entries, and nothing counted calls

The counter is split. `exactAttempts` keeps its name **and its value** so every
committed document and every audit script still reduces to what it always did;
`exactBandEntries` is the same number under the name of what it counts; and
`exactCheckpointCalls` is the number the funnel never had — the delta of
`work.exact_checkpoints`, which `publish::attempt` increments only after the
band test, the target test and the incumbent test have all passed.

The funnel gained four rungs: `bitesWithBandEntry` (`exactAttempted`'s value
under an honest name), `exactBandEntries`, `exactCheckpointCalls`,
`workExactCheckpoints`, and the identity `exactCheckpointCallsReconcile`.

The reconciliation is now a **FAST stage** (`cutclose.py`, tripwire 5, the
spec's FAST-union "actual-attempt reconciliation"): per-bite calls sum to the
funnel's, which equals the work vector's own total; no bite made more calls than
entries; a bite with an entry reached the band; and the per-bite currency adds
back up (above). Measured on a 21-bite fixed-work cell: **36 band entries, 32
calls** — four band entries that never asked the exact authorities anything,
which is the shape F4 measured at 2,780 vs 756 on the nine 10 s cells.

Honest limit: on the FAST cell itself the split is currently degenerate (10
entries, 10 calls) and its `repairRows`/`disruptionMoves` clauses are vacuously
satisfied, because that cell publishes every bite on its first separation. The
identity is what the stage exists to hold; the interesting ratio lives on the
9-seed wall, and the census's own prefix is where the repair clause has teeth.

### RV2 — the headline depths are externally re-validatable now

> *"No pose is recorded for any of the 1,701 publications. `161.05499`,
> `163.56062`, `167.31508` and `167.95169` are re-validatable only by the
> process that produced them."*

Every publication now carries `poses` (the engine's post-repair continuous
poses) and `placements` (the same layout in the request's own coordinates, in
the shape a pose fixture is read in), and the incumbent carries `placements`.
Under `--revalidate=1` the binary re-derives both from the emitted array and
prints whether they matched. Measured on the 21-bite seed-0 cell: **21 of 21
publications, `fingerprintMatches` true and `depthMatchesBitwise` true**,
including `179.16566573285345`. `--revalidate` is off by default because the
recomputation sits between the loop's last clock read and `totalSeconds`; the
poses themselves are emitted unconditionally, because a re-validation nobody
else can run is not one.

### RV3 — every reduction names the bytes it reduced

`lib.run` maintains a manifest of every cell document it spawned — path, sha256,
size, exit, binary — and every reduction emits it as `cellSources`
(`wall.py`, `control.py`, `cells.py`, `cutclose.py`, `smoke.py`,
`determinism.py`, `corpus_gate.py`, `basin.py`, `bites.py`). The per-cell rows
that matter most also carry `sourceSha256` directly: `wall.py`'s seed rows,
`control.py`'s two arms, `cutclose.py`'s canary and tripwire stages, the
two-process comparisons in `cutclose.py`, `smoke.py` and `determinism.py`, and
every arm of this census.

The manifest catches the case a per-row sha cannot: a cell that was spawned and
then dropped from the reduction is still listed.

---

## 5. The two driver repairs

`evidence/frame-vector.json`, 27 committed raw cells, no wall second spent.

### `control.py` had no time filter at all — and it mattered on three cells

The audit's caveat on that file is "min over all publications, no frame": arm A
reported a plain minimum over every publication the cell emitted, so a
publication that landed after the arm's own budget could become the number the
control is read on. It now goes through the same `lib.within_budget` as
`wall.py`.

Red vector, recomputed on the committed raw cells — **the repair changes
exactly three numbers, and they are exactly the three the audit's revalidation
chapter identified**:

| cell | unfiltered (old) | filtered (repaired) |
|---|---:|---:|
| 3 s seed 1 | 179.42186276548130 | 179.42766659910880 |
| 10 s seed 3 | 167.31508386152518 | 167.31677530997393 |
| 30 s seed 8 | 179.05999714298510 | 179.06179163953075 |

Nothing else on any of the 27 cells moves.

### `wall.py`'s frame moved into `lib`, and did not change

Two copies of a repair drift, so the audit's `wall.py` block now lives in
`lib.within_budget`. `frame_vector.py` re-runs the pre-refactor expression —
copied verbatim, and not tidied, because a witness that has been improved is not
one — against the shared helper over all 27 committed raw cells and requires the
three partitions to hold the **same row objects**, not merely the same counts.
`F1_refactorIsANoOp: true`, 27/27.

### And the bracket got narrower, not wider

`evidence/bracket.json`. RV2's poses are built between the loop's last clock
read and `totalSeconds`, and the audit's upper bound for a publication's age —
`(totalSeconds − searchSeconds) + wallSeconds` — contains that build. Emitting
the poses widened it from the audit's 0.3 ms to **3.835 ms**, which would have
been a real evidence regression bought with an evidence improvement.

So the driver now emits `loopEntrySeconds`: the offset itself, read one
statement before the `Pacer` exists. Measured on a fresh 3 s cell, 23
publications, none late and none undecided:

| | bracket width |
|---|---:|
| old expression, on today's document | 3.731 ms |
| the audit's own bracket | 0.300 ms |
| **`loopEntrySeconds`** | **0.113 ms** |

`constructorSeconds` remains the conservative **lower** bound and the verdict
stays on that side; what `loopEntrySeconds` cannot see is the call prologue and
`Pacer::new`, tens of nanoseconds. Documents written before this round do not
carry the field and fall back to the old expression, which is why the 27
committed cells reduce identically.

---

## 6. The icscal file

`evidence/mixed61-w8-seed0.icscal.json`, schema `icscal/v1`. **Schema and writer
only**: no reader exists in this round and no `Pacer` changed. The spec's five
keys are all there — request hash, currency version, binary/feature key,
workers, executor implementation — plus per-phase safe units per second with the
measured rate and the discount printed beside it rather than baked into it.

Two things it deliberately does *not* claim:

* the currency is `U0-sample-evaluations`, not the spec's `U`. `U` needs B/E/R/D
  from timing-only microbenchmarks on all three fixtures, which Wave 1 did not
  run, and `WorkPlan::validate` will accept `U1-weighted-vector` only when
  someone writes one. Calling this file's rate `U` would be exactly the "stable
  but false work accounting" the spec ranks as this round's worst defect class;
* the rate is taken **from the shelf**, never from the prefix — trajectory bite
  22, 200 master iterations, `sampleEvaluations` charged to that bite by
  `PhaseProfile`'s own per-bite counters, with no apportionment anywhere.

`PhaseProfile` carries all five of `U`'s terms per bite — `sample_evaluations`,
`master_batches` (`iterations`), `actual_publication_attempt_calls`
(`exact_calls`), `repair_rows`, `disruption_moves` — and all five are
**counters**, so they are populated in every build, feature or no feature. Wave
3's meter agent can measure B/E/R/D against them without a profiling build, and
the identity in §3 says they sum back to the trajectory's own work vector on
every term.

---

## 7. How to re-run it

```
cargo build -p polygon-nesting-core --release --features overlap-ics \
      --example overlap_ics_benchmark
cargo build -p polygon-nesting-core --release --features overlap-ics,ics-profile \
      --example overlap_ics_benchmark --target-dir target/profile-build

python3 spawntax.py     <work-dir>            # the census and the verdict  (~5 min)
python3 identity.py     <base-binary> <dir>   # the trajectory identity     (~2 min)
python3 frame_vector.py [raw-cell-dir]        # the two driver repairs      (seconds)
python3 bracket.py      <work-dir>            # the checkpoint bracket      (seconds)
```

`identity.py` needs a copy of the example built at this round's base commit, and
it has to be passed in: a script that builds its own "before" can only ever
compare a tree to itself. `frame_vector.py` reads
`/var/lib/t3/tmp/overlapics/rerun/` and exits 2 rather than reporting a green it
did not measure.

Every one of the four exits non-zero on its own failure, and every exit status
in every driver here is read on the line after the command, never through a
pipe.

---

## 8. What this round changed, and what it is

| | what | kind |
|---|---|---|
| `profile.rs` | `PhaseProfile`, `ics_time!` | new module, feature-gated timers, five currency counters |
| `icscal.rs` | `icscal/v1` schema + writer | new module, no reader, no caller on any trajectory |
| `mod.rs` | phase timing in `tournament`/`separate`; `Slot`; the exact split; poses on `PublishedBite` | measurement + diagnostics |
| `overlap_ics_benchmark.rs` | the `spawntax` cell; poses/placements; funnel rungs; `loopEntrySeconds`; `--revalidate` | driver surface |
| `Cargo.toml` | the `ics-profile` feature | off by default, off in `overlap-ics` |
| `lib.py` | `checkpoint_frame` / `within_budget` / `source_sha256` / `MANIFEST` | driver |
| `control.py` | the missing time filter | driver repair |
| `wall.py`, `cutclose.py`, `smoke.py`, `determinism.py`, `corpus_gate.py`, `cells.py`, `basin.py`, `bites.py` | `cellSources`, `sourceSha256`, the reconciliation tripwire | driver |

**Nothing frozen was touched.** The operator, GLS, the bites, the 80/20 share,
`workers = 8`, the constructor, publication, `observe_raw`'s classifier and the
`1_630_000` / `815_000` quanta are all exactly as the spec froze them; the
quanta are not referenced anywhere in this wave, because the strike experiment
is not Wave 1's.

The one thing worth flagging to whoever owns Wave 3: **`mod.rs` was edited
here**, for the phase timers, the exact-counter split and the publication poses.
Sol's workflow gives `mod.rs` to the integration agent in wave 3 and says
neither wave-2 agent may own it. Wave 1's mandate is the profile census, which
cannot be built without touching the file the master iteration lives in, so the
edit is deliberate — but wave 3 should rebase onto it rather than around it.

---

## 9. What is not done

* **No persistent executor**, by the gate's own verdict. The spec's promotion
  criteria for it are unexercised.
* **No pacer, and no icscal reader.** The spec sequences both after the strike
  experiment and the executor freeze.
* **No B/E/R/D.** The currency written here is `U0`, and it says so in its own
  `currencyVersion` field.
* **No strike experiment.** That is the other funded change and it is not Wave
  1's.
* **The denominator has a half-percent bias against building, and it is left
  in.** `barrierToBarrierNs` accumulates every turn of the separation loop,
  including the one turn that broke out on the work cap without running a
  tournament, while `iterations` counts only turns that ran one. So the shares
  are divided by a hair more wall than the 200 tournaments strictly cost, which
  understates prep+dispatch. On a 200-iteration probe the extra turn is a single
  `energy::fold` — about 0.0001 % of the window — and correcting it moves
  nothing. It is named because a decomposition that is not exact should say
  which way it is not exact.
* **The census is one machine.** Every duration here is x86_64, 16 cores, this
  box. The spec's own honest caveat about wall trajectories applies to the
  nanoseconds too; what reproduces across machines is the fixed-work trajectory,
  and that is what `identity.py` and the audit's replay check.
* **The census's four probes at one, two and four workers diverge from the
  eight-worker probe after their first iteration.** The entry state is identical
  by construction — the prefix is always eight workers — but the winner of a
  two-worker tournament is not the winner of an eight-worker one, so the four
  rungs measure the same machinery at the same density on four slightly
  different 200-iteration walks. Normalising by `sampleEvaluations` instead of
  by iteration is the alternative reading, and both are in the document.
* **`cells.py` and `bites.py` carry `cellSources` but not per-row
  `sourceSha256`.** Their row builders are per-cell functions with divergent
  shapes and the manifest already binds every document they spawned; a per-row
  sha there is a tidy-up for whoever next edits those files.
* **F5, F6, F7, F8 and F9 are untouched.** The audit records them as
  correctness-but-not-evidence or latent, and none is in this round's budget.
  F6 is one step better than it was — the layouts are emitted now, so
  `invalidPublications` is no longer the only thing standing behind the
  publication contract — but the counter itself still has no reachable witness.
