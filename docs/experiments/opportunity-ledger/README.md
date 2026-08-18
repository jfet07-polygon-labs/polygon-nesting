# The opportunity-and-delayed-credit ledger, and the A/B/C at identical work

Sol's round-4 reply ends with one instruction: before spending anything else,
measure **what the saturated state still had available** and then run **A/B/C at
identical work** on the three saturated archives. This is that measurement.

It has two headline results and they both cut against the prediction that
prompted them.

* **The saturated run is not a fixpoint of the operator set, and not even
  nearly.** At exit the top-3 frontier alone carries **360 ordered, cut-derived
  crossover actions and has attempted exactly one**; over the whole archive it
  is 4,318 actions and three attempts. Not one phase exits on a deadline or on
  affordability - the schedule stops because it has run out of *keys it knows
  how to name*, at 23-27% of its budget.
* **Sol predicted B breaks the saturation and C is the 165 component. B breaks
  nothing at any seed; C is both the breaker and the largest single gain this
  coordinator has ever made.** One short mode-26 ladder plus one global
  legalizer rung takes seed 0 from 174.208 mm to **169.251 mm** and seed 1 from
  176.056 mm to **171.739 mm**, both exact-valid and contract-valid, and both
  independently re-confirmed on the *default-feature* binary in a separate
  process with zero repair applied.

169.251 mm is **4.957 mm below the previous best-from-request layout on this
request** (174.208 mm, coordinator v2).

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_8545aefe-80d-1` |
| base commit | `57ad992` (coordinator v2, inner certificate, lane levers, m26 anatomy, Sol review 4) |
| request | `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`, sha256 `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3` |
| contract | true 5.0/5.0 exact clearance; search-offset allowance **`0.002`** |
| base gate binary (pristine `57ad992`) | sha256 `1176fcf8b3ab0f1a63dc9a98f2b0a9c7a3fd563af18bc34a0cece13b1749f220` |
| worktree gate binary (`jagua-experimental`) | sha256 `a4d57e614a445d2d5feca75d601c22d05b9677492f1407b14df776f778741c49` |
| ledger binary (`jagua-experimental,portfolio-ledger`) | sha256 `36345b02efaab8076377b463c3a07e8fe965a7c0f367ad63b2fa625220d2c13f` |
| box | x86_64, 16 cores, engine pinned at 8 threads, **shared with another measurement agent** |

The allowance is `0.002`, which is coordinator v2's, **not** the four pinned
gates' `0.0005`. It has to be: this stage measures what the coordinator-v2
saturated state still had available, and a state reached under a different
search envelope is a different state. Every depth here is therefore comparable
to coordinator v2's 174.208 / 176.056 / 179.006 and **is not** comparable to
the 159.079 / 164.038 record lineage.

## What was instrumented

One feature, `portfolio-ledger`, off by default and off in every shipping path.
It compiles two things:

* the **ledger**, computed once after the last phase, reading the archive. It
  never feeds a schedule decision.
* the **probe**, one extra phase that runs *after the drain*, so the base
  trajectory of an A run, a B run, a C run and a D run is bit-identical and the
  arms are paired on the same saturated archive by construction.

Three small things are always on, because they are stores rather than
computations and because a saturated run is unreadable without them:
`PhaseReport::exit_cause`, `ArchivedBasin::secondary_parent_fingerprint` (mode
23 has two parents and the archive recorded one, so *every* crossover was a
genealogical dead end on the record), and `OperatorCallReport::result_fingerprint`.

**The four pinned gates reproduce the pristine binary as whole documents** -
3,263 / 3,244 / 3,244 / 3,244 fields, **0 differences**, with only wall-clock
and build-identity fields removed (`evidence/gates-docdiff.json` lists exactly
which). See *Regression* below.

# Part 1 - the ledger

Three seeds, `work=120,000,000` (three times coordinator v2's own 40M
ten-second anchor, so no phase is stopped by the budget), mixed-61, from the
bare request. Work-budget mode is deterministic, so one run per seed is the
whole measurement; each was re-run in a second process and compared as a whole
document.

| seed | raw depth | dualGateValid | work spent | of budget | wall | cross-process differences |
|---:|---:|---|---:|---:|---:|---|
| 0 | **174.20812003998896** | true | 32,393,757 | 27.0% | 16.7 s | **0** of 8,844 fields |
| 1 | **176.05599999999998** | true | 31,957,935 | 26.6% | 16.3 s | **0** of 8,233 fields |
| 2 | **179.006** | true | 27,938,867 | 23.3% | 15.7 s | **0** of 8,794 fields |

Those are coordinator v2's three depths to the digit. The instrument did not
move the schedule.

## 3. The exact exit cause of every phase

Identical on all three seeds:

| phase | exit cause | meaning |
|---|---|---|
| `m0` | `completed` | protected, never budget-checked |
| `descent` | **`keysExhausted`** | every `22:cycles:epochs:fingerprint` key in the selection had been used |
| `crossover` | **`completed`** | it made its `crossover_attempts = 3` calls and the *counter* ran out, not the pairs |
| `compression` | **`noResidue`** | the compressing descent came back exact-valid, so mode 31 was never handed anything |
| `diversify` | **`patience`** | one barren draw-and-descend iteration |
| `drain` | `completed` | nothing left unoffered |

**Not one phase on any seed exits on `deadline` or on `affordability`.** The
run stops at 23-27% of its budget for reasons that are all internal to the
schedule's own action naming. That is the precise sense in which "joint
fixpoint at eleven seconds" is, as Sol said, too strong - and it is now a
measured field rather than an inference.

## 1. Every direct crossover action still untried

Mode 23 is directional and cut-parameterised, and the schedule uses one
direction and one cut. Making that concrete: for an ordered pair `(A, B)` the
cut only ever partitions **A's occupied short-axis positions**, so the
continuum of cut fractions collapses to at most one action per *gap* between two
consecutive occupied positions. The ledger enumerates one cut per gap, placed at
the gap's midpoint, and carries the gap width and how many pieces at the band's
lower edge actually differ between the parents - which is Sol's "derive the cuts
from where the parents differ, not from a constant grid".

| | seed 0 | seed 1 | seed 2 |
|---|---:|---:|---:|
| archive members | 9 | 9 | 8 |
| ordered pairs over the whole archive | 72 | 72 | 56 |
| **actions over the whole archive** | **4,318** | **4,316** | **3,357** |
| untried | 4,315 | 4,313 | 3,354 |
| untried and non-degenerate | 4,315 | 4,313 | 3,345 |
| actions over the crossover phase's own top-3 frontier | **360** | **360** | **360** |
| of those, attempted | **1** | **1** | **1** |

The top-3 frontier's 360 is exact and it decomposes as 3 unordered pairs x 2
directions x 60 interface bands, and **all 60 bands per ordered pair produce a
distinct hybrid**: on all three seeds, `bands whose lower edge holds no
differing piece: 0 of 360`. Two structurally distinct 61-piece layouts differ at
every occupied short-axis position, so every gap is a real action.

| interface band gap, mm | min | p50 | p95 | max |
|---|---:|---:|---:|---:|
| seed 0 | 0.399 | 24.385 | 99.566 | 135.361 |
| seed 1 | 0.072 | 19.776 | 103.920 | 133.621 |
| seed 2 | 0.150 | 22.287 | 102.254 | 185.167 |

### Only one of the three crossovers the schedule made is still on the frontier

The crossover phase re-selects its frontier after every attempt, so the pairs it
attempted are not the pairs the final frontier holds. Final ranks of the parents
of the three mode-23 calls:

| | call 1 | call 2 | call 3 |
|---|---|---|---|
| seed 0 | A rank 2, B rank 5 | A rank 2, B rank 4 **(published)** | A rank 1, B rank 2 |
| seed 1 | A rank 2, B rank 3 **(published)** | A rank 1, B rank 2 | A rank 1, B rank 3 |
| seed 2 | A rank 0, B rank 1 | A rank 0, B rank 3 | A rank 1, B rank 3 |

**On seeds 0 and 1 the final rank-0 state was never a crossover parent at all**,
because it was born in the *compression* phase, after the crossover phase had
ended. The schedule's best state and its recombination operator never met. On
seed 1 the ledger's next untried action is the `rank0 -> rank1` **midpoint**
cut - the schedule's own action, on the schedule's own two best states, that the
schedule's phase ordering makes unreachable.

The next untried action in the ledger's canonical order (pair order, then
forward before reciprocal, then cuts by distance from 0.5):

| seed | direction | pair | cut fraction | band gap | pieces from A / B | is the 0.5 band |
|---|---|---|---:|---:|---|---|
| 0 | forward | rank0 -> rank1 | 0.495798035 | 4.606 mm | 30 / 31 | no |
| 1 | forward | rank0 -> rank1 | 0.494714163 | 26.465 mm | 30 / 31 | **yes** |
| 2 | forward | rank0 -> rank1 | 0.495566704 | 0.578 mm | 28 / 33 | no |

## 2. Archive states excluded by top-K and by the similarity rule

seed 0 (9 members; `dF`/`xF` = reachable by the descent / crossover selection):

| rank | operator | raw | dF | xF | excluded by | actions received | descents | descendant publications | best descendant | generations to incumbent |
|---:|---|---:|---|---|---|---:|---:|---:|---:|---:|
| 0 | mode22 | 174.208 | y | y | - | 1 | 0 | 1 | 174.208 | 0 |
| 1 | mode23 | 176.309 | n | y | - | 2 | 2 | 2 | 174.208 | 1 |
| 2 | mode22 | 179.587 | n | y | - | 4 | 4 | 3 | 174.208 | 2 |
| 3 | mode23 | 179.608 | n | n | **topK** | **0** | 0 | 0 | - | - |
| 4 | mode23 | 179.639 | n | n | **topK** | 1 | 1 | 2 | 174.208 | 2 |
| 5 | m0 | 181.589 | n | n | **topK** | 2 | 2 | 3 | 174.208 | **3** |
| 6 | constructor | 182.976 | n | n | **topK** | **0** | 0 | 0 | - | - |
| 7 | mode22 | 206.447 | n | n | **topK** | **0** | 0 | 0 | - | - |
| 8 | mode20 | 216.574 | n | n | **topK** | 1 | 1 | **0** | - | - |

| | seed 0 | seed 1 | seed 2 |
|---|---:|---:|---:|
| excluded by **top-K** | 6 | 6 | 5 |
| excluded by the **bit-exact-pose similarity rule** | **0** | **0** | **0** |
| members receiving **no action at all** | 3 | 4 | 4 |

**The similarity rule excludes nobody.** Sol's §4 lists the bit-exact pose
comparison as a reason members never receive an action; on these three archives
it never fires, and the whole exclusion is top-K. The rule is still as fragile
as §4 says - this measurement does not defend it - but it is not what is costing
actions here, and the eviction rule is likewise inert (occupancy 8-9 of 16, zero
evictions, zero `RefusedArchiveFullAllDistinct` on all three seeds).

## 4. Genealogical credit, including the m20 feeder

The archive now records both crossover parents, so the genealogy is a DAG rather
than a chain that stops at every recombination. Seed 0's incumbent lineage, in
birth order:

```
m0     181.5890  born at  8,777,493 units
mode22 179.5869  born at 10,792,266
mode23 179.6386  born at 18,921,527   <- deeper than its own parent, and an ancestor anyway
mode23 176.3094  born at 23,988,191
mode22 174.2081  born at 31,427,729
```

The 179.6386 state is the case the archive exists for: it is *worse* than the
179.5869 it descended from, it is excluded from the frontier by top-K, and it is
the second parent of the crossover that published 176.309. Under the old record
it was not anyone's ancestor at all.

**The m20 feeder's verdict, on deferred credit and not on immediate
publication:** on all three seeds the mode-20 basin receives exactly **1**
action - the quantum the diversify phase spends on it in the same iteration -
and has **0 descendant publications**. The phase-0 constructor basin (182.976,
the same fingerprint `a791c397f3` on all three seeds, since it is
seed-independent) receives **0** actions on all three. Sol is right that
immediate publication is the wrong measure for a feeder; on this request, at
these seeds, the deferred measure returns zero too.

## 5. Cost and yield per action class

seed 0 / 1 / 2, work units and seconds from the run's own calls:

| phase | operator | calls | published | work total | p50 | p95 | s p50 | Δraw mm | **Δraw / M eval** |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| compression | mode22 | 1/1/1 | 1/1/0 | 1.91M / 1.58M / 1.94M | = | = | 1.02/0.69/0.76 | 2.101 / 0.697 / 0 | **1.1017 / 0.4407 / 0** |
| crossover | mode23 | 3/3/3 | 1/1/0 | 16.04M / 15.31M / 12.27M | 5.44M/5.08M/4.01M | 5.53M/5.32M/4.35M | 2.34/2.29/2.08 | 3.277 / 2.880 / 0 | 0.2043 / 0.1881 / 0 |
| descent | mode22 | 2/2/2 | 1/1/1 | 4.70M / 4.79M / 3.58M | 2.01M/2.26M/1.64M | 2.68M/2.53M/1.94M | 0.94/0.77/0.73 | 2.002 / 0.057 / 0.656 | 0.4264 / 0.0119 / 0.1832 |
| diversify | mode20 | 1/1/1 | 0/0/0 | **260 / 335 / 320** | = | = | **3.09/3.07/3.12** | 0 | 0 |
| diversify | mode22 | 1/1/1 | 0/0/0 | 0.96M / 0.64M / 1.19M | = | = | 1.03/1.16/1.62 | 0 | 0 |

Two things fall out of this table.

**The compressing micro-descent is the most efficient operator in the schedule
by a factor of five** - 1.10 mm per million evaluations on seed 0 against
mode 23's 0.20 - and the schedule gives it one call.

**The work budget prices a mode-20 arm at essentially zero.** 260-335 units
against 3.07-3.12 seconds: a constructor arm costs about 1/6,000 of an m22
quantum in the budget's currency and about 3x it on the clock. PR7's caveat -
the deep operators' Clipper counters are behind `search-profiling` - is not a
rounding error, it is four orders of magnitude, and it is the single most
important qualification on Part 2 below.

# Part 2 - A/B/C at identical work, plus a control

Every arm runs the identical base schedule at `work=120,000,000` and then gets a
probe phase with the same allowance, measured from the same saturated state.
That pairing is verifiable in the evidence rather than asserted: across all four
arms on a seed, the work spent *before* the probe is a single value
(32,393,757 / 31,957,935 / 27,938,867) and the entry depth is a single value
(174.20812003998896 / 176.05599999999998 / 179.006).

**The allowance is 21,000,000 work units**, which is the smallest round budget
above every arm's own measured spend. That it did not truncate anything is
checked rather than assumed: re-running all twelve arms at a 400,000,000
allowance gives **0 differing fields over 12 arms x 7 fields**
(`evidence/allowance-check.json`), so the allowance is non-binding and "equal
work" here is equal *allowance* with the actual spend reported alongside.

* **A** - the next derived crossover action the ledger names, executed with the
  minimal directional/derived-cut extension.
* **B** - one mode-20 ticket (single restart, fresh slot, clamp per coordinator
  v2's `constructor_clamp_mm`), then a direct crossover with the incumbent, then
  a short mode-22.
* **C** - one short mode-26 ladder (drop 0.3 mm, the anatomy's shortest sampled
  drop), then the coordinator's own global legalizer tier on what it leaves.
* **D** - *the control, not one of the three*: the **same target depth**, the
  **same parent**, asked of the schedule's own mode-22 alternation, **no clamp**.
  Without it, C's result is "the arm that got 21M more units found something",
  which is not a statement about the clamp.

## The table

| seed | arm | entry raw | exit raw | **Δ raw** | dualGateValid | probe work | probe s | publications |
|---:|---|---:|---:|---:|---|---:|---:|---:|
| 0 | A | 174.2081 | 174.2081 | 0.0000 | true | 6,036,325 | 2.36 | 0 |
| 0 | B | 174.2081 | 174.2081 | 0.0000 | true | 3,795,975 | 6.43 | 0 |
| 0 | **C** | 174.2081 | **169.2510** | **4.9571** | true | 14,822,849 | 4.72 | 2 |
| 0 | D | 174.2081 | 171.5878 | 2.6203 | true | 3,079,958 | 1.37 | 1 |
| 1 | A | 176.0560 | 176.0560 | 0.0000 | true | 5,784,955 | 2.35 | 0 |
| 1 | B | 176.0560 | 176.0560 | 0.0000 | true | 3,476,577 | 6.22 | 0 |
| 1 | **C** | 176.0560 | **171.7390** | **4.3170** | true | 20,998,305 | 6.46 | 2 |
| 1 | D | 176.0560 | 176.0560 | 0.0000 | true | 2,082,057 | 0.94 | 0 |
| 2 | **A** | 179.0060 | **178.2857** | **0.7203** | true | 4,856,537 | 2.34 | 1 |
| 2 | B | 179.0060 | 179.0060 | 0.0000 | true | 2,373,258 | 6.20 | 0 |
| 2 | C | 179.0060 | 179.0060 | 0.0000 | true | 5,682,616 | 1.85 | 0 |
| 2 | D | 179.0060 | 179.0060 | 0.0000 | true | 1,936,358 | 0.96 | 0 |

Paired per seed, Δ raw depth of the best exact-valid publication:

| seed | A | B | **C** | D (control) |
|---:|---:|---:|---:|---:|
| 0 | 0.0000 | 0.0000 | **4.9571** | 2.6203 |
| 1 | 0.0000 | 0.0000 | **4.3170** | 0.0000 |
| 2 | **0.7203** | 0.0000 | 0.0000 | 0.0000 |
| **publishes** | **1 of 3** | **0 of 3** | **2 of 3** | 1 of 3 |

## What each arm actually did

**A** executes the derived action and produces a legal hybrid every time -
176.817 / 178.722 / 178.286 - and on two of three seeds that hybrid is worse
than the incumbent it was drawn from, so the adoption rule refuses it. On
seed 2, the one seed where the incumbent is *not* itself a crossover descendant,
it publishes 0.720 mm. The derived cut is not a no-op: seed 0's action is at
cut 0.4958 in a 4.606 mm band, 30 pieces from A and 31 from B, and it is a
different legal layout from anything the run had. It is simply not, at these
two seeds, a *better* one.

**B publishes nothing anywhere, and the intermediate depths say why.** The
ticket lands at 216.5-227.5 mm, the direct crossover with the incumbent at
192.0-200.9, and the short mode-22 at 187.3-194.6. The chain is monotonically
improving and it starts 42-53 mm behind. Sol's `m20 -> m22 -> crossover -> m22`
deferred-credit chain is exactly what this arm runs, one generation of it, and
one generation does not close a 40 mm gap. The ledger's Part 1 finding is the
same finding measured the other way: the m20 basins already in the archive have
0 descendant publications.

**C is the result.** Its mode-26 call is a 2-rung ladder (the mode's own
`ladder_compression_bounds` turns a 0.3 mm drop at a 174 mm parent into a
0.174208 mm step and two rungs) running 2-3 arms - against the anatomy's 35
rungs and 171 arms - and on seeds 0 and 1 **rung 1 publishes 4.25 mm and 3.61 mm
below its own requested bound**:

| seed | ladder | rungs planned/run | arms | published step | m26 raw | then m31 | final |
|---:|---|---|---:|---|---:|---:|---:|
| 0 | 174.2081 -> 173.9081 | 2 / 2 | 2 | step 1 | 169.655 | bound 169.2550 | **169.251** |
| 1 | 176.0560 -> 175.7560 | 2 / 2 | 3 | step 1 | 172.143 | bound 171.7430 | **171.739** |
| 2 | 179.0060 -> 178.7060 | 2 / 2 | 2 | **none** | 179.006 (its own parent) | refused | 179.006 |

The clamp is doing the work the mechanism was designed to do: it removes the
depth-ward room, and the separator then compresses far past the rung it was
asked for. On seed 2 the same ladder returns its own parent and the m31 tier
refuses with "global legalization did not reach a feasible fixpoint: 2 violating
pairs" - the 0/3 case, reported as such.

**The coordinator-level m31 tier lands exactly on the bound it is given.**
0.404 mm on seed 0 and 0.404 mm on seed 1, and in both cases the published
layout's snapped depth is its requested bound to the digit. Its contribution
here is therefore a property of the rung it was asked for (`COMPRESSION_RUNG_MM
= 0.4`), not a measurement of what it can do. That is a lead, not a result.

**D, the control, is why C's number means something - and it is a finding of its
own.** On seed 0, asking the schedule's *own* mode-22 for the same target,
without any clamp, publishes 2.620 mm for 3.08M units and 1.37 s. The schedule
never asks for that: its compression phase asks mode 22 for `depth + 0.8`, a
*looser* target than the incumbent it already holds, gets an exact-valid answer
and exits `noResidue`. **2.620 mm was sitting one sign change away from the
schedule's own most efficient operator.** On seeds 1 and 2 the same call returns
a duplicate, so it is not a general free lunch - but the C-minus-D difference,
2.337 mm on seed 0 and the whole 4.317 mm on seed 1, is attributable to the
clamp and not to the extra budget.

## Independent confirmation of every publication

Each published layout was written out as a pinned-parent fixture and replayed
through **mode 27** - the micro-legalization probe, the one mode meant to be
pointed at states that may not validate - in a separate process, from the
**default-feature gate binary**, which contains neither the ledger nor the probe
phase:

| layout | exactValid | contractValid | rawSourceDepthMm | fingerprint unchanged | violating pairs before | pieces moved |
|---|---|---|---:|---|---:|---:|
| seed 0 arm C | true | true | 169.251 | yes | 0 | 0 |
| seed 1 arm C | true | true | 171.739 | yes | 0 | 0 |
| seed 2 arm A | true | true | 178.2857218718321 | yes | 0 | 0 |
| seed 0 arm D | true | true | 171.58783248647316 | yes | 0 | 0 |

Zero repair applied, zero violating pairs, fingerprint unchanged: a different
build, a different code path and a different process agree that these are legal
layouts at those depths under the request's own 5.0/5.0 contract.
`evidence/confirmations.json` carries all twelve rows - the four confirmations
and the eight arms that published nothing, recorded rather than skipped, because
`0 of 3` is a result.

# Regression

| gate | pinned value | fingerprint | fields compared | differences vs pristine `57ad992` |
|---|---:|---|---:|---:|
| mode 20 `independentDepthMm` | 206.869 | `87dd48f28fac99ca` | 3,263 | **0** |
| mode 22 raw | 159.09233022733062 | `fa01012af1d559ae` | 3,244 | **0** |
| mode 22 raw | 159.07876040364795 | `e28fba007f8031d4` | 3,244 | **0** |
| mode 22 raw | 164.0375677990678 | `49f094d7e59a9008` | 3,244 | **0** |

All four are `exactValid` and `contractValid`. The comparison is the whole
document with wall-clock and build-identity fields removed;
`evidence/gates-docdiff.json` lists exactly which fields those are.

Release suite, `cargo test --release --features jagua-experimental`:
**1,238 passed, 0 failed, 2 ignored**, including seven new
`portfolio-ledger` unit tests. Full log at `evidence/suite.log`.

One of those seven is worth naming because it pins a bug this stage wrote and
then found: the `attempted` key of a crossover action has to be built from the
two parents *in the order they were handed to the operator*, never from their
ranks. The frontier reorders between attempts, so a rank-built key reports an
attempted action as untried - which is exactly the class of error this whole
ledger exists to avoid making.

# Honest limits

* **One request, three seeds.** Nothing here says anything about shapes-17,
  triangle-20 or a fourth request. Coordinator v2's own generality finding - that
  the constructor slice's verdict is a property of the request - applies with
  full force to arm B's 0/3.
* **The allowance is `0.002`, not `0.0005`.** These depths are comparable to
  coordinator v2's and not to the record lineage's.
* **"Equal work" is not "equal wall", and the gap is worst exactly where it
  matters.** A mode-20 arm costs 260-335 work units and 3.0-3.2 seconds. At
  equal work arm B is the *cheapest* arm; on the clock it is the second most
  expensive. If the arms had been paired on wall time instead, B would have been
  given roughly a third of the calls it got. That does not change its 0/3 - it
  published nothing at any point in its chain - but it does mean the *cost* half
  of B's verdict here is unreliable in both directions.
* **No wall-clock claim is made.** The box is shared: seed 0 arm A's probe took
  2.36 s, 2.44 s and 5.62 s across three runs with a bit-identical
  6,036,325-unit spend - a 2.4x spread on the clock and zero on the meter.
  Every quality comparison here is in work units, which is why the work-budget
  mode exists.
* **The m26 anatomy's 0/171 is not contradicted, it is bounded.** That round
  sampled ladders at 159 mm and 164 mm parents and measured zero publishing
  arms; this one samples 174-179 mm parents and measures two publishing ladders
  in three. The cost-sample-versus-yield trap the anatomy named is real and it
  points the other way here: **the anatomy's sample is not a prediction for the
  174-179 band**, which is precisely the census Sol asked for as the gate before
  the L-sized m26 port. On this evidence that gate passes.
* **Arm C's mode-26 call runs its own internal repair tiers**, including the
  tier-4 global program. The coordinator-level mode 31 that follows it is a
  *second*, outer legalizer rung, and its 0.404 mm is the rung it was asked for.
  The split between what the internal tiers did and what the outer rung did is
  not separated here.
* **Nothing here is a schedule change.** No default was moved. The probe is one
  action from a saturated state, not a proposal for where it belongs in a budget.
* **A single generation of arm B is not Sol's chain.** He asks for `m20 -> short
  m22 -> crossover -> m22` judged on deferred credit over *several* descendants.
  This arm runs one generation of it once. Its 0/3 is evidence about one
  generation at one budget, and the Part 1 genealogy (0 descendant publications
  from any archived m20 basin over the whole run) is the stronger version of the
  same finding.

# Reproducing

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,portfolio-ledger    # ledger + probe
cargo build --release --example general_request_benchmark \
    --features jagua-experimental                     # gates

python3 drivers/ledger.py 120000000 0,1,2 <ledger-binary> --twice
python3 drivers/summarize.py evidence/ledger-120M-mixed61.json

python3 drivers/abc.py equalwork  21000000 0,1,2 A,B,C,D <ledger-binary>
python3 drivers/abc.py unbounded 400000000 0,1,2 A,B,C,D <ledger-binary>
python3 drivers/abcsummary.py evidence/abc-equalwork-mixed61.json
python3 drivers/allowancecheck.py evidence/abc-unbounded-mixed61.json \
                                  evidence/abc-equalwork-mixed61.json
python3 drivers/collect.py evidence/abc-equalwork-mixed61.json \
                           evidence/confirmations.json <gate-binary>

python3 drivers/gates.py pristine <pristine-binary> /var/lib/t3/tmp/ledger/gates
python3 drivers/gates.py final    <gate-binary>     /var/lib/t3/tmp/ledger/gates
python3 drivers/docdiff.py /var/lib/t3/tmp/ledger/gates pristine final

python3 drivers/confirm.py /var/lib/t3/tmp/ledger/abc/equalwork-seed0-C.json \
    /var/lib/t3/tmp/ledger/confirm/seed0-C <gate-binary>
```

`drivers/runlib.py` carries the pinned CLI tail, the salt sets and the work
anchors; point `ROOT` at your worktree. The probe is armed by two keys in the
portfolio spec, `probe=A|B|C|D` and `probeWork=<units>`; absent, every existing
invocation is byte-identical.
