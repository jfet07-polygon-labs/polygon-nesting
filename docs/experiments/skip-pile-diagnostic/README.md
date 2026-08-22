# The skip pile: the released region exists, and everything in it is worse than what the run already had

Grok review 8 item 2. The round-envelope gate found that
`schedule_confirmationsRefused = 0` on all 108 runs of its twelve-parent matched
gate — the miter confirmation never refused anything, because almost nothing
reached it. The filter is one level up:

```rust
// compression_schedule.rs, due_for_confirmation
if !proxy_feasible { self.confirmations_skipped_infeasible += 1; return false; }
```

`proxy_feasible` is the **relaxed surrogate's** verdict, and the surrogate's
collision geometry is the production **miter** offset. Across the miter ladder
that clause suppressed **149 762** frontiers, cell-for-cell identical on the
union arm.

**The question.** What fraction of those skipped frontiers is *disc-legal*
(the certified round kernel accepts) yet *miter-illegal* — is there a released
region hiding behind the proxy — and if there is, is the released material in the
sub-micron class (~1 µm, the canonical grid's own step) or the join-tax class
(~0.5 mm, what Gate A measured on a miter-refused pair)?

**Nothing here changes shipped behaviour.** The instrument is a new cargo
feature, `skip-pile-dump`, compiled out by default; the four pinned gates are
what proves the default build did not move.

---

## The answer in one paragraph

Six cells of the gate's own ladder were reproduced digest-for-digest with the
dump armed, and **every one of their 13 867 distinct suppressed frontiers was
scored** — a census, not a sample. At the shipping radius **111 of 13 867
(0.80 %)** are released: the material contract accepts, the disc kernel accepts,
HEAD's miter authority refuses. The released material is squarely in the
**join-tax class** — a median **0.380 mm** of pair excursion and **0.928 mm** of
boundary excursion, with **zero of 111 rows** inside a ≤10 µm bound. And then the
number that decides it: **none of those 111 layouts is deeper than the layout its
own cell had already published.** The best released frontier in the three cells
that have any is **0.053 mm, 0.080 mm and 0.438 mm *worse*** than that cell's own
answer. The region is real, it is millimetre-sized in geometry, and it is empty
of anything worth publishing.

| | |
|---|---|
| **the one number** | **0.80 %** released (111 / 13 867), **join-tax class**, **0.000 mm** of depth |
| **verdict on option (b)** | **killed on this population**, by the pre-committed rule and by a stronger one it did not anticipate |

---

## 0. Pre-committed interpretation

> **This section was written and committed before a single frontier was
> scored**, in commit `f989c21`, which carries the instrument and no evidence.
> It is reproduced here **unaltered**; §5 reads the answer against it.

The proposal-geometry surgery Grok's review calls **option (b)** — moving the
schedule's proposal geometry off the miter proxy so that disc-legal frontiers
stop being suppressed — is worth funding only if the region it would open is
both **non-empty** and **large enough to matter**. Three outcomes, and what each
one means, fixed in advance:

| outcome | reading | consequence for option (b) |
|---|---|---|
| **~0 released rows** (0, or a handful that are all sub-micron-class) | the released region does not exist behind the proxy on this population: every frontier the proxy suppressed is one the disc refuses too | **killed a priori for these parents.** The proxy is not costing reachability; it is doing the job it was written for, and a surgery on it buys nothing to search |
| **a material fraction released, sub-micron class** | the region exists but the material it frees is at the canonical grid's own step | not funded. A micrometre of clearance is not a millimetre of depth, and the campaign's own currency is millimetres |
| **a material fraction released, 0.5 mm class** | the region exists and is join-tax sized | **sized and reported** — rows, millimetres, per seed — as a live candidate, without being promoted here |

"Material fraction" is fixed at **≥1 % of sampled skipped frontiers**, which on
the sample below is ≥18 rows. Below that, an existence proof; at or above it, a
rate.

**Grok's own prior** is that the pile is mostly *bulk overlap* — frontiers where
the clamp has just been lowered and the repair has not yet run them apart, so
every authority refuses them and the proxy is simply right. That prior is
reported as a number either way: the `contract=False, miter=False, kernel=False`
row of the joint table.

**What would falsify the instrument rather than answer the question.** Any
row where the miter accepts and the disc refuses (`kernel refuses ∧ miter
accepts`) on a layout the contract accepts is a **P0** against the kernel and is
reported as one, not as a data point. The previous round's soundness battery says
there should be none.

---

## 1. The instrument

One cargo feature, `skip-pile-dump`, and three edits behind it:

| file | what |
|---|---|
| `search/compression_schedule.rs` | one `cfg`-gated accessor, `confirmations_skipped_infeasible()` |
| `search/general_relaxed.rs` | one `cfg`-gated block **after** the confirmation branch of a schedule step |
| `search/skip_pile_dump.rs` | the sink: JSONL, deduplicated by the engine's own placement fingerprint, capped, with a tally sidecar |

Four properties, each a mechanism rather than a claim:

* **compiled out by default.** The module does not exist without the feature and
  the call site is behind the same `cfg`. §6.1 is the measurement.
* **disarmed by default when compiled.** The sink opens only when
  `POLYGON_NESTING_SKIP_PILE_DUMP` names a path. A compiled-but-unarmed binary
  reproduces all four gates *as whole documents* — §6.1 again.
* **it reads, it does not decide.** The block runs after
  `due_for_confirmation` has already returned; nothing is fed back, no field is
  published. Its only cost is wall.
* **and wall is not in the trajectory here**, because a mode-34 slice under
  `past=1,rollback=0,work=W,lanes=1,pconfirm=0` is capped in *work units*. That
  is an argument, so §2 replaces it with a measurement.

The scoring stage is a second example, `skip_pile_score`, which needs
`round-envelope-kernel` and `import-gate-shadow` and **not** `skip-pile-dump`:
the writing binary and the reading binary are different programs.

### 1.1 The tally sidecar, and why the first pass needed one

The sink deduplicates by placement fingerprint, so a cell's line count is
legitimately smaller than its skip count — and a first pass could not tell that
apart from a dump that had silently lost records. Two cells came out 22 and 15
lines short and the driver could only report the discrepancy, not explain it.
The sink now writes `<path>.tally.json` with `written + duplicates + overCap`,
and the driver asserts that those three sum to the schedule's own
`confirmationsSkippedInfeasible` **exactly**. They do, on all six cells, with
`overCap = 0` everywhere: the cap never bound and the dump is the whole pile.

## 2. The reproduction: six cells, digest for digest

Six cells of the round-envelope gate's miter ladder, chosen to span both the
size of the pile and the depth of the ladder, re-run with the dump armed and
checked against `round-envelope-gate/evidence/matched.json` on thirteen pinned
fields — the step digest, all four confirmation counters, the step count, the
work units, the exit cause, the final and published depths, the fingerprint and
both validity flags.

| cell | parent | steps | confirmations | refused | **skips** | distinct dumped | step digest | published |
|---|---:|---:|---:|---:|---:|---:|---|---:|
| seed10@3341379 | 176.362 | 1499 | 351 | 0 | 99 | 99 | `15208230584541081695` | 175.394 |
| seed0@3341379 | 174.208 | 1550 | 328 | 0 | 242 | 242 | `18417416165943714955` | 173.380 |
| seed2@3341379 | 179.006 | 1565 | 229 | 0 | 653 | 653 | `3524879778271727066` | 177.343 |
| seed3@3341379 | 176.061 | 2810 | 0 | 0 | 2807 | 2807 | `13882956371924736073` | 176.061 |
| seed4@16000000 | 171.650 | 6906 | 686 | 0 | 4159 | 4137 | `1225229695193028453` | 166.734 |
| seed0@32000000 | 174.208 | 12395 | 1612 | 0 | 5944 | 5929 | `9052932834971917816` | 164.008 |

**6 of 6 reproduced, 0 differing fields**, on a binary whose SHA-256 is not the
gate's (`16ac9d90…` against `9e6ad285…`) because this round's source is not that
round's. `refused = 0` on every cell is the finding this round exists to explain,
reproduced here rather than cited.

**13 904 skips, 13 867 distinct** — 37 repeated fingerprints, all in the two deep
cells. Every distinct one was dumped and every distinct one was scored, so §3 is
a **census of these six cells' entire skip pile** and the only sampling anywhere
in this round is which *cells* were run.

## 3. The joint table

Three authorities on every one of the 13 867 frontiers, at two radii. Both
composites run the material contract on the same placements, so *kernel accept ∧
miter refuse* is a released **layout** and not a weaker authority's opinion.

**Three properties hold on all 27 734 readings, and each one would catch a
different way of asking the wrong question:**

| checked | why it matters | result |
|---|---|---|
| the miter verdict read **twice**, through `import_gate::authority_verdict` and through `round_envelope_gate::wired_verdicts` | two committed instruments, two code paths; if they disagreed, one of them is not asking what it says | **27 734 / 27 734 agree** |
| `union == (kernel ∨ miter)` | the wire point's own hybrid must be the disjunction it claims to be | **0 exceptions** |
| no composite accepts where the contract refuses | this is what makes a released row a *publishable* layout rather than an envelope opinion | **0 exceptions** |

### 3.1 At the shipping radius, expansion **2.502 mm** (allowance 0.002)

| contract | miter | kernel | records | share | what it is |
|:--:|:--:|:--:|---:|---:|---|
| ✗ | ✗ | ✗ | **8 129** | **58.62 %** | bulk overlap — Grok's prior |
| ✓ | ✓ | ✓ | **4 439** | **32.01 %** | legal under every authority, never asked |
| ✓ | ✗ | ✗ | 1 187 | 8.56 % | the 2 µm search-offset allowance, and only that |
| ✓ | ✗ | ✓ | **111** | **0.80 %** | **released** |
| ✓ | ✓ | ✗ | 1 | 0.007 % | a one-grid-step artefact — §3.4 |
| all other combinations | | | **0** | | |

### 3.2 At the contract radius, expansion **2.500 mm** (allowance 0.0)

| contract | miter | kernel | records | share |
|:--:|:--:|:--:|---:|---:|
| ✗ | ✗ | ✗ | 8 129 | 58.62 % |
| ✓ | ✓ | ✓ | 5 626 | 40.57 % |
| ✓ | ✗ | ✓ | **112** | **0.81 %** |
| ✓ | ✓ | ✗ | **0** | — |
| all other combinations | | | 0 | |

The two tables differ by exactly **1 187 rows**, which move from *all three
accept* to *contract only*. That is the whole of what `search_offset_allowance =
0.002` buys and costs: 8.56 % of this pile is material the contract admits and
both envelopes refuse at 2.502 mm and both admit at 2.500 mm. It is a deliberate
margin, not a defect, and it is named here because it is larger than the
released region by a factor of eleven.

### 3.3 Split by the proxy's own reason

`feasible()` is `boundary_violations == 0 && collision_pairs.is_empty()`, so a
skip is either an *overlap* skip or a *boundary-only* skip — a frontier whose
pieces do not overlap but have not yet been compressed above the clamp the step
just lowered to. They are not the same population:

| proxy reason | records | all three refuse | all three accept | released |
|---|---:|---:|---:|---:|
| **overlap** | 5 956 (43.0 %) | **5 908 — 99.2 %** | 1 | 46 |
| **boundary-only** | 7 911 (57.0 %) | 2 221 — 28.1 % | **4 438 — 56.1 %** | 65 |

**Grok's prior is exactly right about the overlap half and wrong about the other
one.** Where the proxy sees an overlap it is almost never wrong: 99.2 % of those
frontiers are refused by the contract, by the miter and by the disc alike. Where
it sees only a boundary violation it is refusing a legal layout more often than
not — the frontier simply has not reached the clamp yet.

### 3.4 The one row where the disc refused and the miter admitted

Not a P0. `seed2@3341379` seq 375, placements 1 and 2, at 2.502 mm only:

| quantity | value |
|---|---:|
| demanded `2r` | 5.004 mm |
| the kernel's exact critical `2r` | 5.003 mm |
| shortfall | **−1.000 µm** |
| **material clearance, untouched source rings** | **5.0039882 mm** |
| material shortfall against `2r` | **−11.8 nanometres** |
| miter envelope intersection area | **0.000 mm²** |

The material really is short of `2r`, by twelve nanometres, and the disc is the
authority that notices. The miter admits it because Clipper's offset output is
re-quantized to the 1 µm canonical grid and a sliver that thin rounds away — an
intersection area of exactly zero is that rounding, measured. Twelve nanometres
is **120x** inside the round-envelope battery's own
`CANONICALIZATION_BUDGET_MM = 0.0014143`, and the row does not exist at all at
the contract radius. This is the kernel being conservative on the grid, which is
the direction a soundness argument needs it to err in.

## 4. What the released region is made of, and what it is worth

### 4.1 Its class: join-tax, unambiguously

Every released layout was censused pair by pair and boundary by boundary,
`census` against `miter_census` over the **full** scan.

| population | rows | median | min | max | grid-class (≤10 µm) | intermediate | join-tax (≥0.1 mm) |
|---|---:|---:|---:|---:|---:|---:|---:|
| released **pairs** | 46 | **0.380 mm** | 0.034 mm | 2.432 mm | **0** | 2 | **44** |
| released **boundaries** | 65 | **0.928 mm** | 0.055 mm | 0.928 mm | **0** | 7 | **58** |

The excursion is `kernel critical radius − r`: how much room the disc has where
the miter has none. The grid-class bound is deliberately generous — **ten** grid
steps, against a canonicalisation budget of 1.4143 µm — and **not one released
row of 111 falls inside it.** The answer to "the ~1 µm class or the ~0.5 mm
class" is the 0.5 mm class, and it is not close.

The 111 released layouts split by *why the miter refused them* exactly as the
proxy split them: **65 boundary refusals**, every one of them a boundary-only
proxy skip, and **46 pair-overlap refusals**, every one of them an overlap proxy
skip. The two halves never mix.

### 4.2 Its depth: zero

This is the reading the verdict rests on, and it needs no counterfactual. For
each cell, take the deepest layout in its **entire** skip pile under each
authority and compare it with what that cell actually published. Both numbers are
`raw_source_long_axis_depth_mm` of a finished layout, so the comparison is
apples to apples.

| cell | published | deepest **released** | released beats published by | deepest **all-three-legal** | that beats published by |
|---|---:|---:|---:|---:|---:|
| seed10@3341379 | 175.394 | — (0 released) | — | 175.470 | **−0.077 mm** |
| seed0@3341379 | 173.380 | 173.818 | **−0.438 mm** | 173.651 | **−0.271 mm** |
| seed2@3341379 | 177.343 | — (0 released) | — | 177.483 | **−0.140 mm** |
| seed3@3341379 | 176.061 | — (0 released) | — | — (none legal) | — |
| seed4@16000000 | 166.734 | 166.814 | **−0.080 mm** | 166.692 | **+0.042 mm** |
| seed0@32000000 | 164.008 | 164.061 | **−0.053 mm** | 164.008 | **0.000 mm** |

**Not one of the 111 released layouts is deeper than the layout its own cell had
already published.** The released region is real and it is millimetre-sized in
geometry; the *best* member of it in each cell is between **0.053 mm and
0.438 mm worse** than the run's own answer, and the rest are worse than that.
Arming the disc as the proposal authority on these six cells would have released
111 layouts and published **none** of them.

The column beside it prices the much larger 32 % row for free. Counted exactly —
suppressed frontiers that **all three** authorities accept *and* whose own depth
beats what their cell published:

| cell | such frontiers | best gain |
|---|---:|---:|
| seed4@16000000 | **2** | **0.042 mm** |
| the other five | **0** | — |

**Two frontiers out of 13 867.** They are `seed4@16000000` seq 3198 (step 5961)
and seq 3199 (step 5967), both at raw source depth **166.692 mm** against an
incumbent of **166.734 mm** — and the cell went on to publish exactly
**166.734 mm**, so the incumbent never improved again after step 5961. Both were
suppressed with **0 collision pairs and 1 boundary violation**, against clamps of
166.735 mm and 166.729 mm, while the layouts' own depth was 166.692 mm. **The
proxy's boundary test and the composite's are not the same test**, and these two
rows are what that costs on this population: 0.042 mm, in one cell of six.

Note what that row is *not*: both layouts are accepted by HEAD's own miter
authority. Publishing them needs no kernel and no proposal-geometry surgery —
only a schedule that asked.

### 4.3 The weaker reading, and why it is the weaker one

Against the *incumbent at the step of the skip*, 104 of the 111 released rows
would have improved it, at a median of 0.068 mm. That number is in the evidence
(`publicationValueByJointRow`) and it is **not** the one quoted above, for two
reasons: it is a counterfactual — publishing the first improvement moves the
incumbent the next one is judged against — and it mixes two depth conventions
by up to one grid step, because the slice's own `published_depth_mm` is measured
on grid-snapped bounds and the scorer's on untouched source rings. §4.2 has
neither problem.

## 5. The pre-committed rule, read against the answer

§0 above is byte-identical to `f989c21`'s copy of it — the check is one `git
show` and a string compare — including the one part of it that went stale: *"≥1 %
of sampled skipped frontiers, which on the sample below is ≥18 rows"* was written
when the plan was to score 1 800 records. The **fraction** is the clause; 18 was
an illustration of it on a sample that became a census. On 13 867 records the
same 1 % is 139 rows.

| clause, as committed in `f989c21` | measured | reading |
|---|---|---|
| ~0 released rows → killed a priori | **111 rows, 0.80 %** | **does not apply.** The region is not empty |
| material fraction (≥1 %), sub-micron class → not funded | 0.80 %, and **0 of 111 rows** inside the ≤10 µm bucket | **does not apply** on either half |
| material fraction (≥1 %), 0.5 mm class → size it | class **yes**, fraction **0.80 % — below the 1 % line** | the letter of the rule says *existence proof, not a rate* |

**The pre-commitment did not anticipate the answer, and it is worth saying so
plainly.** It was written on the assumption that a released region large enough
to matter would be a region worth publishing from, so it fixed a threshold on
*size* and a threshold on *class* and nothing on *value*. The measurement
satisfies the class test outright, misses the size test by a fifth, and then
fails a test the rule never wrote down: **the released layouts are all worse than
what the run already had.** Option (b) is killed on this population — not because
the region behind the proxy is empty, but because it is empty of improvements.

The third clause asked for the region to be **sized — rows, millimetres, per
seed — if the class test passed**, and the class test did pass, so it is
discharged rather than skipped:

| | |
|---|---|
| **rows** | 111 at 2.502 mm, 112 at 2.500 mm, of 13 867 |
| **millimetres, geometry** | 46 pair excursions, median 0.380 mm; 65 boundary excursions, median 0.928 mm |
| **millimetres, depth** | **0.000** — the released set contains nothing deeper than what its cells published |
| **per seed** | seed 0 at 3.3 M: 3 rows, best 0.438 mm worse. seed 4 at 16 M: 25 rows, best 0.080 mm worse. seed 0 at 32 M: 83 rows, best 0.053 mm worse. seeds 2, 3 and 10: **0 rows** |

What the pre-commitment did get right is that it was written first. Had the
threshold been set afterwards, 0.80 % would have been argued either way.

## 6. Reproduction, gates, suites, determinism

```sh
bash docs/experiments/skip-pile-diagnostic/drivers/collect.sh all
bash docs/experiments/skip-pile-diagnostic/drivers/run-suites.sh
```

Do **not** pipe either into `tee` or `tail`: you will read the pipe's status
instead of the script's. Every exit status inside them is read directly on the
line after the command.

### 6.1 The four pinned gates

| gate | pinned | feature ABSENT | feature COMPILED, unarmed |
|---|---|:--:|:--:|
| g1 | 206.869 / `8a7737381238fa4d` | ✅ | ✅ |
| g2 | 159.09233022733062 / `fa01012af1d559ae` | ✅ | ✅ |
| g3 | 159.07876040364795 / `e28fba007f8031d4` | ✅ | ✅ |
| g4 | 164.0375677990678 / `49f094d7e59a9008` | ✅ | ✅ |

`ALL_PASS: true` on both binaries, and stronger: **all eight whole-document
digests are identical to each other and to the ones the pre-instrument binary
produced** — `17cf86ef3880b374`, `822ccc256623e1ee`, `12b402bda35e0b89`,
`27ce9cfea5df93e9`. Compiling the hook changes nothing a document can see.

### 6.2 A correction the gates forced: the binary is a function of the source *text*

The round-envelope gate's §5.4 established that git metadata is read at run time
and not compiled in, so the same source builds the same binary. True — but the
converse does not follow, and this round measured that.

Adding **one line inside a `#[cfg(feature = "skip-pile-dump")]` block** — code
that cannot be compiled without the feature — moved the feature-absent binary
from `87d41243…` to `69b42abf…`. Suspecting the obvious, a **pure comment** was
then inserted at an unrelated point in the same file and the binary moved again,
to `62d99caa…`. Both were confirmed against a fresh `CARGO_TARGET_DIR`, so
neither is build noise.

The mechanism is line numbers: a release build still bakes `Location { file,
line, col }` into every panic path, so anything inserted above other code in
`general_relaxed.rs` — **27 679 lines** — rewrites thousands of them. **A changed
binary hash is therefore not evidence that behaviour changed**; the
whole-document gate digest is what distinguishes the two, and in §6.1 it does,
across a source change that moved the hash and eight digests that did not move
at all.

### 6.3 Determinism, two processes, three artefacts

`evidence/determinism.json`, `ALL_IDENTICAL: true`:

| artefact | cases | compared on | result |
|---|---|---|---|
| the armed run's document | 2 cells × 2 processes | whole document, wall-clock fields stripped by name, plus 8 verdict paths compared directly outside the digest | identical |
| **the dump the armed run wrote** | the same | SHA-256 of the JSONL | identical |
| the scored document | 1 plan × 2 processes | raw SHA-256 **and** stripped digest **and** 6 verdict paths | identical |

The stripped set is `round-envelope-gate/drivers/determinism.py`'s, verbatim —
including `milliseconds`, `leafMilliseconds` and `leafSharePercent`, which the
protocol names, and the mode-34 schedule's own six millisecond fields. The
scored document carries no clock at all, which is why it is compared unstripped
as well and why the raw hashes match.

### 6.4 Suites

All `--release`, every exit status read directly rather than through a pipe.

| # | features | passed / failed / ignored | exit | log |
|---|---|---|---:|---|
| 1 | `jagua-experimental` | 1293 / 0 / 2 | **0** | `suite-jagua.log` |
| 2 | the protocol's full combo | 1357 / 0 / 2 | **0** | `suite-combo.log` |
| 3 | `jagua-experimental`, `--example general_request_benchmark` | 20 / 0 / 0 | **0** | `suite-example.log` |
| 4 | `jagua-experimental,round-envelope-kernel,skip-pile-dump` | 1343 / 0 / 2 | **0** | `suite-kernel-dump.log` |
| 5 | `jagua-experimental,skip-pile-dump` — the feature without the kernel beside it | 1329 / 0 / 2 | **0** | `suite-dump.log` |
| 6 | the measurement binary's own feature set | 1372 / 0 / 2 | **0** | `suite-meas.log` |
| 7 | the scorer's feature set, `--example skip_pile_score` | 0 / 0 / 0 (it has no tests; this compiles it) | **0** | `suite-scorer.log` |

Suite 4 is the protocol's `jagua-experimental,round-envelope-kernel` **plus this
round's new feature**, which is separate. Suite 5 exists because
`skip-pile-dump` and `round-envelope-kernel` are independent and a suite that
only ever compiled them together could not say so. `skip-pile-dump` implies
`compression-schedule`, which is why suites 4 and 5 carry more tests than the
kernel round's 1307.

**The campaign's known flake fired, and its pattern is worth recording rather
than waving through.** `free_material_multi_eviction_shrinks_retained_container_capacity`
asserts `cache.entries.capacity() < entries_capacity_before` after an eviction —
an allocator property, not a search one, which is why the campaign calls it a
flake and the protocol says to rerun once and keep both logs.

This round ran the whole suite script **twice** (once mid-round, once on the
clean committed tree) and the observation is the same both times:

* it fired on the **first pass of suite 5**, both times — 2 of 2;
* it fired on **no other suite**, either time — 0 of 12;
* the **rerun passed both times**, on the same binary and the same feature set,
  which is what keeps "flake" the right word rather than "failure".

Suite 5 is `jagua-experimental,skip-pile-dump`, the only set here that compiles
`compression-schedule` *without* `round-envelope-kernel`, so it runs a different
test population in a different order and reaches this assertion with a different
allocation history. That is an observation and not a diagnosis; nothing in this
round touches `layout_scorer`. Both logs are committed
(`suite-dump-run1-flaky.log` beside `suite-dump.log`).

### 6.5 The closing gate: a fresh build of the clean committed tree

The protocol's last requirement, run as a stage (`collect.sh final`) rather than
by hand. With `git status --porcelain` printing nothing, a **fresh**
`CARGO_TARGET_DIR` was built from the committed tree and all three binaries came
back **byte-identical** to the ones every number above was measured on:

| binary | features | measured on | fresh rebuild |
|---|---|---|---|
| gate | `jagua-experimental` | `69b42abf…` | **`69b42abf…`** |
| measurement | the combo + kernel + dump | `16ac9d90…` | **`16ac9d90…`** |
| scorer | kernel + import-gate-shadow | `a1c84416…` | **`a1c84416…`** |

`ALL_PASS: true` on the fresh gate binary and on the fresh measurement binary.
Counting every gate run this round made — two binaries at the measurement stage
and two at the closing stage, four gates each — that is **16 of 16 passing with
the same four whole-document digests**: `17cf86ef3880b374`, `822ccc256623e1ee`,
`12b402bda35e0b89`, `27ce9cfea5df93e9`.

`evidence/final-worktree-status.txt` is **zero bytes**, which is the check
passing. `evidence/binaries-final.txt`, `evidence/gates-final.json`,
`evidence/gates-finalmeas.json`.

One more thing the stage happened to prove, because it was run twice on two
different commits (`fa7e557` and `62b971f`) whose **Rust** source is identical
and whose docs are not: the three binaries came back byte-identical both times.
That is the round-envelope gate's §5.4 — git metadata is read at run time, not
compiled in — holding on this round's own tree, and §6.2 above is the other half
of the same fact: the binary is a function of the source *text*, so identical
text gives identical binaries and a changed comment gives a changed hash without
a changed behaviour.

> **Correction, from the stage's own first run.** It `tee`-d `git status
> --porcelain` straight into the evidence directory, which creates the output
> file before `git status` finishes walking the tree — so the check reported its
> own output file as untracked and could never print nothing, which is the one
> thing it exists to print. It now writes outside the repository and copies the
> file in afterwards, and the committed file is empty.

## 7. Caveats, stated rather than left to be found

* **Six cells, one request, one platform.** mixed-61 at the exact-clearance
  5.0/5.0 contract on x86_64. Within those six cells the scoring is a **census**
  — every distinct suppressed frontier — but the six cells are a sample of the
  gate's forty-eight. They were fixed in the pre-commit
  (`git show f989c21:…/collect.sh` line 32, the same commit as §0) to span both
  the size of the pile (99 to 5 944 skips) and the depth of the ladder (3.3 M to
  32 M work), so which cells were run is not a function of what they said.
  Nothing generalises from six cells to forty-eight without measuring the other
  forty-two.
* **The scoring budget was raised after the pilot, and the direction matters.**
  The plan's `sample` began at 300 per cell; a timing pilot showed the whole
  pile was affordable and it was raised to cover all of it. Widening a sample to
  a census can only make the estimate less selective, and the released fraction
  is reported over the census.
* **The control arm only.** `POLYGON_NESTING_ROUND_ENVELOPE_KERNEL` is unset
  throughout. The gate's own evidence records the union arm's skip counts as
  cell-for-cell identical to the control's on all 48 matched cells, so dumping
  one is dumping both — but that is an inheritance, not a measurement this round
  made.
* **37 of 13 904 skips were repeats** and are deduplicated by placement
  fingerprint. The tally sidecar accounts for all 13 904; nothing was lost.
* **"Released" is a statement about a layout, not about a search.** These are
  frontier states the schedule declined to *confirm*. Whether a schedule with a
  different proposal geometry would reach the same states at all is a different
  question and this round does not answer it.
* **§4.2 compares against what each cell published**, which is a bound the run
  itself achieved. A cell run to a deeper budget would publish deeper and the
  comparison would be harsher, not kinder.
* **The 0.042 mm in seed4@16000000 is two layouts in one cell**, and it is an
  existence proof that the 32 % row is not entirely worthless, not a rate.
* **The 1 187-row allowance effect is measured at two radii only**, 2.502 and
  2.500. Nothing here says where between them it turns over.
* **The dump costs wall, and the cost is not separable from the box.** The six
  armed cells' operator walls against the gate's committed ones are
  `[0.893, 1.026]x` — the two deepest at 17.614 s / 17.223 s and 27.833 s /
  27.130 s — so the hook's price is inside the variation between two differently
  loaded boxes and this round does not claim to have measured it. It does not
  need to: **nothing here is a wall claim**, and the trajectory identity is the
  step digest, which is exact.
