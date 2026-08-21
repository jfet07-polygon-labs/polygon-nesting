# The checkpoint was a report, not an interruption

`docs/experiments/replan/` shipped a slice that could be cut into batches, a
checkpoint at every batch boundary, and a policy - `m34cap` - that was said to
consume them. Sol review 9's P0 is one sentence about the call site:

> Ma il chiamante lo richiama immediatamente in `while !slice.finished`, fino
> alla fine del monolite. Il coordinatore non riottiene mai il controllo al
> checkpoint.

The slice recorded a checkpoint, set `finished = false`, and its caller asked it
again on the next line. **The coordinator never got a turn.** So `m34cap` could
change the checkpoint *report* and could not change a trajectory, a wall, a work
figure or a depth - and the previous round attributed a 3.089 mm move to it.

This round does three things, in this order, because the third depends on the
second and the second is only worth building once the first is admitted:

1. **retract the claim**, by reproducing Sol's replay rather than agreeing with
   it (§2);
2. **make the interruption real** - `advance_one_batch() -> Checkpoint|Finished`,
   a driver that consults a policy at every checkpoint, and a slice the
   coordinator can hold *alive* across other actions (§3, gated in §4-§6);
3. **spend it on the lever the density negative named**: `robust-plan` §13.1
   found that every cell of a twelve-cell confirmation-density sweep exited on
   `bound` dropping exactly 1.6160 mm, and concluded *"the lever that matters
   here is the bound, not the grid"* (§7-§10).

---

## The headline

<!--HEADLINE-->

---

# Part I - the retraction

## 1. What the previous round claimed, and what the code could do

`docs/experiments/replan/` §12.3 is titled *"The checkpoint's consumer, priced"*
and reads, in full:

> **It works, and it is priced.** The p50 falls from **32.64 s to 25.91 s** and
> the overruns from **4 of 6 to 2 of 6**; the cost is on seed 1, where the depth
> goes from 162.846 to 165.935 - **3.089 mm** - because the slices that were
> paying for that depth are the ones the cap stops.

Three things had to be true for that paragraph: that a checkpoint stops a slice,
that the coordinator decides whether to resume it, and that the driver which
produced the evidence could ask for the arm the evidence is labelled with. None
of the three was.

**The mechanism.** At that HEAD, `ScheduleSliceRun::advance` ran a batch,
recorded a checkpoint, and returned `Result<()>`. Its caller was:

```rust
while !slice.finished {
    slice.advance()?;
}
```

A slice that stopped at a batch boundary had `finished == false`, so the loop
called it again immediately. The batch budget therefore decided *where a row was
written in the report*, and nothing else. The unit test of the day asserted the
same shape from the other side - every checkpoint but the last has
`finished == false` and the last one is the slice's own end - which is a test of
**segmentation**, not of interruption.

**The driver.** `docs/experiments/replan/evidence/cap-30s.json` carries twelve
rows whose `spec` fields read `plan=30000,cells=...,v3=1,replan=1,m34cap=0` and
`...,m34cap=1`. The committed driver that is supposed to have produced them,
`drivers/trancheq.py`, builds its spec at line 44:

```python
extra = ('replan=0' if fraction == 'off'
         else f'replan=1,planfirst={fraction}')
```

There is no branch that can emit `m34cap`. A `fraction` of `capon` produces
`replan=1,planfirst=capon`. Source, driver and measured binary do not agree.
That is a provenance break independent of the semantic one, and it is why this
round's protocol is *build from the committed tree, gate the binary, then
measure* - and why `drivers/run-build.sh` refuses to build a dirty tree.

## 2. The replay, on a binary built from the committed base tree

`drivers/capreplay.py`, `evidence/capreplay-30M.json`.

A **work** budget, because that is the reproducible currency: two runs of one
binary at `work=30000000` are two runs of one trajectory unless something really
diverged. mixed-61, three seeds, `m34cap=0` against `m34cap=1`, on a binary built
from the committed base commit `ea7c843` - the commit this round starts from, so
the replay is of the code the claim was made about and not of the code that
replaces it.

<!--CAPREPLAY-->

Every column that could carry a trajectory is equal: the raw depth to seventeen
digits, the incumbent's placement fingerprint, the total work, the operator-call
count, and the slice's own per-step FNV digest. The whole document is equal once
the `scheduleSlice` block is dropped, and unequal only because of the checkpoint
list - which is the *report*, and the only thing the cap ever changed.

Seed 1 - the seed the retracted paragraph names - is **171.3619986855876 mm at
28,636,653 work units over 8 operator calls, on both arms.** That is the same
number Sol's own read-only replay reported.

**Where the retraction is recorded.** `evidence/cap-30s.json` now opens with a
`SUPERSEDED` block; `docs/experiments/replan/README.md` opens with a correction
and §12.3 is struck through in place rather than deleted, so the retraction can
be audited against what it retracts; the ledger chapter carries a *"Corrected
by"* note at its head.

**What survives.** The wall figures in that table are real measurements of real
processes - they are just two measurements of **one** trajectory, on a box that
the same round's §7 already records as not quiet. And the mechanism the round
built is sound: the struct, the field discipline, the step digest and the
concatenation gate are what this round builds on.

---

# Part II - the interruption, made real

## 3. `advance_one_batch`, and the three answers

The whole change is that a batch boundary is now a **question**:

```rust
fn advance_one_batch(&mut self) -> Result<SliceProgress, GeneralFastError>

enum SliceProgress {
    Checkpoint(ScheduleCheckpoint),   // a question for the caller
    Finished(ScheduleCheckpoint),     // a statement
}
```

and a driver - `drive_slice_batches` - that asks a policy at every one of them:

```rust
enum SliceControl { Continue, Stop, Suspend }
```

The three answers are three different things, and the difference is the round.

**`Continue`** is the previous round's only behaviour, and it is what
`run_slice_to_completion` returns. With `batch_work_units: None` the policy is
never consulted at all: there is exactly one call to `advance_one_batch`, it
returns `Finished`, and the slice is the atomic arm every pinned m34 number in
this repository was measured with.

**`Stop`** ends the slice at the checkpoint it is sitting on, with the exact-valid
incumbent that checkpoint holds. This is Sol review 8 §3 condition 3's anytime
contract - *return the last exact-valid incumbent between checkpoints* - and the
tail confirmation is deliberately **not** run: a caller that stopped on a wall
deadline does not have the milliseconds a whole-layout confirmation costs, and a
stop that ran it would publish a layout the equivalent uninterrupted run reaches
later, which would make "stop at K" and "never stop" two different walks.

**`Suspend`** hands the slice back **alive**. The caller owns a
`SuspendedScheduleSlice`, may run any number of other actions, and may resume it
later. This is the piece Grok review 4 §4 named as missing:

> Senza portare uno slice sospeso alla coda, m34 non può cedere a un'altra
> classe. [...] lo slice resta atomico verso il coordinatore e può ancora
> mangiare i 10s.

Nothing is torn down across a suspension, because nothing *can* be: every value
the next batch reads is already a field on `ScheduleSliceRun` - the frontier, the
deepest-confirmed slot, `search`, `repair_workers`, `state`, `score`, `schedule`,
the lane's rng, its guided weights, every surrogate and pair-NFP cache, and the
whole step account. That is the previous round's design decision, and it is what
makes this round two hundred lines rather than a rewrite. The suspended slice
lives on `Coordinator::suspended_slice`, whose lifetime is the coordinator's
`pieces`, so the type-checker enforces that a parked slice cannot outlive the
request it belongs to.

### 3.1 What a suspended call reports, and what it does not

A suspended call **publishes** - `exact_valid` is true and the placements are the
checkpoint's incumbent, because refusing to publish a layout the exact tier has
already accepted would make interruption cost depth it does not cost - and
carries **no `compression_schedule` report at all**. The report is written once,
by whichever call finishes the slice, so N batches across two coordinator actions
report **one slice** exactly as N batches inside one call do. A call with no
report says why: `scheduleSliceSuspended`.

The consequence for a reader is stated rather than hidden: **work totalled over
`operatorCalls` and work totalled over slice reports are two different sums**,
and only the second one is the slice's own meter.

### 3.2 The drain

A slice can still be parked when the budget runs out. `Coordinator::drain_suspended_slice`
ends it where it stands - no batch, no confirmation, nothing further spent - and
writes its report, so a run's class totals add up. It runs once, at the end of
`run_v3_schedule`, which is the only place a suspension can be created.

## 4. Gate (a): N batches reproduce the monolith, on five instruments

The previous round's gate compared the whole document and an FNV-1a digest over
every step row. Sol review 9 §P1 judged the second insufficient:

> FNV-1a è adeguato come checksum regressivo, non come certificato. [...] Due
> cammini differenti possono avere lo stesso payload senza alcuna collisione
> FNV. La frase "same digest walked the same walk" è falsa. [...] Aggiungerei,
> solo nei gate, fingerprint completi di state/tracker/RNG.

So the slice now computes three SHA-256 digests at the instant it ends, and all
three are `#[serde(skip)]` - readable by a unit test, absent from every document,
which is exactly *"solo nei gate"*:

| digest | over |
|---|---|
| `final_state_fingerprint` | the frontier's geometry: the clamp and every piece's canonical (index, angle, mirror, x, y) |
| `final_tracker_fingerprint` | every boundary's violations and raw loss, and every pair's raw loss, guided weight and normalisation scale |
| `final_lane_fingerprint` | the lane's rng position and every guided weight in its map |

`concatenated_batches_reproduce_the_monolithic_slice` now asserts all three
against the monolith at three batch sizes, on top of the step digest, the
aggregates and the published layout. From the request, the gate is
`drivers/equiv.py`:

<!--CONCAT-->

## 5. Gate (b): stop at K is exact-valid; resume-later is bit-identical

Two unit tests, and they are the two halves of the contract.

**`stopping_at_a_checkpoint_returns_the_exact_valid_incumbent`** stops at
checkpoint K for K = 1 and K = 2 and asserts that what comes back is the depth
the checkpoint named, that the exit is `interrupted`, that the slice really
stopped early, and - the part that makes "exact-valid" a claim rather than a flag
this code set - that the returned layout **re-validates against the real
request** through `validate_and_measure_placements`.

**`a_suspended_slice_resumes_onto_the_uninterrupted_run`** suspends every fourth
checkpoint and runs *a whole other operator* - a mode-22 alternation fixpoint on
the same pieces - between every pair of batches. It allocates, it drives the job
pool, it runs the same proxy tier, and it advances every process-global counter
the slice's own meter is not derived from. Then it resumes, and compares against
the run that was never stopped:

* the step digest, and all three gate fingerprints;
* the whole report, field for field, with the six wall-clock fields zeroed and
  `resumptions` - which is *meant* to differ - asserted separately;
* `steps_taken`, `work_units`, `sweeps_run`, both confirmation counts,
  `rollbacks`, `final_depth_mm`, `exit_cause`, `batches`;
* the published layout's depth and fingerprint.

**All equal.** `batches` in particular: the interleaved slice ran the same number
of batches as the one that was never stopped, which is the statement that the
interleave changed the *schedule of the work* and not the work.

A third test, `a_suspended_slice_can_be_ended_where_it_stands`, pins the drain:
`work_units` unchanged, `interrupted` set, the incumbent equal to the one the
suspension was already holding.

## 6. Gate (c): determinism, two processes, and the equivalence

<!--DETERMINISM-->

And the refactor itself, against the base binary with nothing armed:

<!--EQUIV-->

---

# Part III - the bound

## 7. Why the bound, and not the grid

`docs/experiments/robust-plan/` §13.1 is the hard negative this part exists to
answer. Its confirmation-density sweep was flat-to-negative at all twelve cells
in both budget modes, and the cause was a column it did not expect:

> Every cell of both sweeps exits on **`bound`**, and every cell's first slice
> drops **exactly 1.6160 mm**. [...] So the coordinator's slice is a **walk of a
> fixed length**. A quarter-grid clamp does not walk further; it walks *the same
> distance in four times as many steps* [...] **25.7x the work for zero extra
> depth**. [...] So the lever that matters here is **the bound, not the grid**,
> and this round does not touch it.

And it named the condition under which the record line's millimetre *was* bought:

> `record-line-cascade`'s millimetre was bought under the opposite condition. Its
> arms are `past=1,work=20000000` against `past=1,work=20000000,step=0.25` -
> `continue_past_bound` **on**, at a pinned work budget.

So `m34past=1` unlocks the bound inside the coordinator, and the budget it runs
under is the coordinator's own remaining budget for the action rather than a
pinned constant. Two writes, because the slice's loop needs both:
`continue_past_bound` takes the lower limit down to the sheet floor, and only a
work cap makes the tail unbounded.

Past the bound the walk has no natural end short of the sheet, so two things
bound it, and they are different in kind:

* **`m34pastshare`** - what fraction of the action's budget the slice may spend.
  A work cap is exactly the right instrument for *"you have spent enough"*: it
  fires at the top of a step, so the slice cannot overshoot it by a batch.
* **`m34pastbarren`** - how many consecutive batches the slice may run **without
  deepening its published incumbent**. This is the per-batch affordability rule,
  and it is the one thing the previous round's mechanism could not have
  expressed: a work cap is a number checked against a meter and cannot say *"you
  have stopped buying anything"*. A checkpoint carries `published_depth_mm`, so
  two checkpoints are a derivative.

## 8. The bound at ten seconds

<!--BOUND10-->

## 9. The bound at thirty seconds, and the overrun

<!--BOUND30-->

## 10. The density point, re-tested with the bound unlocked

<!--DENSITY-->

---

# Part IV - the table

## 11. The anytime table, and Sparrow

<!--ANYTIME-->

## 12. Thirty seconds on three fixtures

<!--ANYTIME30-->

---

## 13. Honest caveats

* **A wall stop cannot be deterministic, and this one is not.** `m34wallstop`
  reads a clock at a checkpoint, so two processes agree on the layout only while
  they cross the deadline between the same two checkpoints. That is the trade,
  it is stated where the field is defined, and it is why the key ships off. A
  caller who needs one document per seed leaves it off and accepts the overrun;
  a caller who needs ten seconds arms it and accepts the spread. The §11 table
  reports both ends of that trade rather than one.

* **The barren rule applies to every batch of a past-bound slice, not only to
  the batches past the bound.** An exact reading would need the checkpoint to
  carry "am I past the bound", which is a field on the serialised checkpoint and
  a change to a document the previous round pinned. The simplification is safe
  in one direction only: with `m34past` off, no checkpoint policy is installed at
  all, so no pinned number in this repository is measured by a coordinator that
  has ever consulted the rule. It is *not* safe as a reading of the sweep, and
  §8's table therefore reports the barren stops separately from the work-cap
  stops rather than folding them together.

* **The barren counter does not survive a suspension.** It lives in the
  checkpoint policy's closure, and `resume_suspended_slice` builds a fresh one.
  So a slice that is suspended and resumed gets its barren run reset. This
  matters only when `m34past` and `m34yield` are armed together, which is the
  `pastyield` arm and nothing that ships.

* **The interleave's batch size is a fraction of the *remaining budget*, not of
  the slice's expected cost.** With `m34yield` armed and no `m34batch`, the batch
  is `remaining_to(deadline) / 8`, and on the measured band a bounded slice costs
  far less than an eighth of what is left - so a nine-rung slice yields once or
  twice rather than eight times. The mechanism is not limited by that; the
  default is. A caller who wants a finer interleave names `m34batch` and gets it.

* **A suspended call publishes but carries no slice report, so two sums over the
  document disagree.** Work totalled over `operatorCalls` and work totalled over
  slice reports are different numbers when a slice was suspended, and only the
  second is the slice's own meter. §3.1 says so; every driver here reads the
  second.

* **The self-metered charge of an interleaved slice is slightly conservative.**
  `settle_operator_charge` charges `max(global, self)` per call, and a resumed
  call's self-meter is the *whole* slice's while its global delta is only the
  resumed part's. So an interleaved slice is charged marginally more than the
  same slice run atomically. It is an over-charge and not an under-charge, which
  is the safe direction for a budget, and it is another reason the interleave is
  a key rather than a default.

* **`m34past` moves the depth in the wrong direction at the budget the user
  priority names, and §8 is that result.** The lever works - the bound is
  genuinely unlocked, the first slice walks further than 1.6160 mm, and the exit
  stops being `bound` - and the run is worse for it on this fixture at this
  budget. `robust-plan` §13.1's guess that *"the other classes spend it better
  than a denser slice would"* is the thing this round measured, and it survived.

* **Three fixtures, three seeds and a handful of rounds are not a distribution.**
  Every table here says its `n`. The per-seed table is printed beside the median
  of seed medians for exactly this reason: a median of three is not three
  agreeing runs.

* **The box was shared with this round's own gate runs for part of the window.**
  Every driver records `os.getloadavg()` before and after every process and every
  evidence file carries the min/median/max; the load line is printed above each
  wall table rather than at the end of the document.

* **Nothing here is wired into a production route.** `m34wallstop`, `m34yield`,
  `m34past` and its two dials are spec keys on the benchmark example, the
  coordinator that reads them is still `coordinator_v3`, and `coordinator_v3` is
  still off by default.

## 14. Reproducing this

```
bash drivers/run-build.sh      # refuses a dirty tree; writes evidence/binaries.txt
bash drivers/run-gates.sh      # the four pinned gates, the equivalences, determinism
bash drivers/run-measure.sh    # the calibration pass, then every battery
bash drivers/run-suites.sh     # the two suites, exits captured directly
python3 drivers/tables.py docs/experiments/real-interruption/evidence
```

The levers, as one line each:

```
'plan=10000,plancal=<file>,cells=...,v3=1'                  # the incumbent
'plan=10000,plancal=<file>,m34wallstop=1,cells=...,v3=1'    # the wall stop
'plan=10000,plancal=<file>,m34past=1,cells=...,v3=1'        # the bound unlocked
'plan=10000,plancal=<file>,m34past=1,m34pastshare=0.5,...'  # at half the budget
'plan=10000,plancal=<file>,m34yield=2,cells=...,v3=1'       # the interleave
'work=<units>,cells=...,v3=1'                               # replay any of them
```
