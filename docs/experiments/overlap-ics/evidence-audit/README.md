# The evidence audit — a second computation of everything round 2 is about to sign

The owner's mandate for this round, verbatim:

> *verificare bene il codice che mandi in valutazione, perché se ci sono dei bug
> che danno risultati meh i risultati usati come evidenza non sono granché.*

So this is not a review of whether `CutCloseRelocate` is a good algorithm. It is
an audit of the **machinery that produced the numbers** in
[`../cutclose-rerun/`](../cutclose-rerun/README.md), looking for the four defect
shapes this campaign's autopsies have already produced: a silent filter emptying
an operator, two sites carrying one convention differently, a counter that means
something other than its name, and a test that is green because it checks a
literal instead of a transition.

Every claim below is a **second computation by a different route**, not a
reading. Where a number is recomputed, the recomputation is in this directory
and is committed with it.

```
bash run-all.sh                      # the whole thing, ~6 minutes
bash run-all.sh <root> <work-dir>    # explicit paths
```

---

## What is here

| file | what it recomputes | needs |
|---|---|---|
| `counters.py` | 15 counter identities derived by hand from the increment sites, over all 27 committed wall cells plus `triangle20.json` and `cutclose-fast.json` | committed evidence only |
| `bites-consistency.py` | the bite schedule re-derived from the publication chain, over the same 27 cells | committed evidence only |
| `strike-effect.py` | the rerun README's §5 and §6 tables, recomputed from the two committed raw bite-row files | committed evidence only |
| `rust-vectors/` | a **detached** cargo package that links the shipped library and recomputes the strike helper, the clearance/pivot convention family, Algorithm 8, the eight-worker tournament and the first bite record, each from the spec text | the release library |
| `run-cells.sh` | nine 10.000 s wall cells, so the per-publication clock readings the committed reduction dropped exist somewhere | the release binary |
| `checkpoint-frame.py` | which clock `PublishedBite.wallSeconds` is on and which one the gate compares it against | stage 5's documents |
| `replay.py` | three committed fixed-work replays, re-run on this machine, two processes each | the release binary |
| `chain.py` | six publication-chain identities that need `bites` **and** `publications` together | full cell documents |
| `funnel-names.py` | what the funnel's rungs actually count | full cell documents |
| `driver-fix-vector.py` | the red/green for the two driver repairs made here | stage 5's documents |
| `show.py` | prints a named vector's `detail` out of any audit document | — |
| [`revalidation/`](revalidation/README.md) | a **second, independent** pass over the same evidence by a different auditor, written without importing anything above; see §5 | committed evidence, plus the raw cell documents where they survive |

`rust-vectors/` carries an empty `[workspace]` table so it is **outside** the
repository workspace. Nothing under `crates/` changes, and `cargo build` at the
root is byte-for-byte the build the gates measured.

---

## 1. What was verified clean

**314 counter identities, all green** (`counters.py`, over the 27 committed wall
cells). The ones that would have caught a lying counter:

* `focusedSamples == 25 · relocates` and `containerSamples == 50 · relocates`,
  exactly, on every cell — the 75-sample pool is not being short-circuited;
* `containerWinners + focusedWinners + stayPutWinners == relocates`, exactly —
  the origin partition is total and is counted once per relocate;
* `sampleEvaluations − focusedSamples − containerSamples` is non-negative and
  **even** on every cell — every coordinate-descent step evaluates its two
  candidates and neither is dropped or double-counted;
* `len(exactCheckpoints) == work.exactCheckpoints`, on every document that
  carries both;
* `repairDepthGivebackMm == publishedRawDepth − proxyRawDepth`, **bit for bit**,
  on every published checkpoint — the giveback is accounted against the
  published depth and cannot hide inside it;
* `publishedRawDepthMm <= targetDepthMm`, `repairMaxDisplacementMm <= 16 µm`,
  `repairRows <= 4n`, everywhere.

**The double-debit clause is clean** (`rust-vectors` S5a). One master iteration
of the real gate fixture was reconstructed **outside** the engine — eight worker
sweeps from the same entry state with the same keys, into eight fresh work
vectors — and the master's aggregate equals the sum of the eight deltas in all
nine relocate counters:

```
sampleEvaluations  engineDelta 33354   eightWorkerSum 33354
relocates                     138                    138
focusedSamples               3450                   3450
containerSamples             6900                   6900
containerWinners                1                      1
focusedWinners                 28                     28
stayPutWinners                109                    109
containerCommits                1                      1
acceptedMoves                 132                    132
```

with `pieceProposals` advancing by `workers · n = 488` and `weightUpdates` by
exactly 1. The merge rule reproduces too: eight **distinct** guided totals, the
minimum at ordinal 1, and the engine's own fingerprint row agreeing on the
winner, the guided value and `contested`.

**The measurement path is one function, not two** (S3a). `publish::raw_depth_of`
(the published depth, computed on placements, matched by id) and
`state::raw_source_depth_mm` (the proxy depth, computed on the SoA geometry)
agree **bit for bit** on 400 random pose sets with random mirrors and unbounded
angles. `raw_source_depth_mm` is read from the **installed, post-repair**
placements (`publish.rs:422`, after the repair loop), and the giveback is the
difference against the pre-repair proxy — verified above.

**The pivot convention is one point** (S3b–S3d). `transformed_centroid` (used by
the coordinate descent's wiggle axis and by `split_and_close`) equals
`Geometry::centroids` (used by Φ's torque and by the repair's fallback normal)
bit for bit on all 61 pieces; `compose_proposal` about that point leaves it
fixed to 1e-9 over eight rotations; and `centroid_relative_extents` reproduces
`Geometry::piece_bounds` to 1e-9, so the container sampler's box is the box the
geometry actually has.

**The clearance family agrees across modules** (S2a–S2f, on two real contracts).
Every corner of `relocate::strip_sample_box` charges exactly zero boundary Φ,
1 µm outside any of its four walls charges exactly 1 µm on that side, a layout
whose raw depth is exactly `T` charges zero top Φ, and 7 µm past `T` charges
exactly 7 µm. **See finding F7 for what this could not test.**

**Algorithm 8 is the published schedule** (S4a). Over 12 real sweeps on the
gate fixture, every one of 21,960 pair-row weight updates and every edge-row
update equals an independent recomputation of
`v == 0 : w ← max(1, 0.95w)` / `v > 0 : w ← min(2²⁰, w·(1.2 + 0.8·v/v_max))`
from the pre-pass weights, bit for bit. `reset_weights` returns every row to the
floor and is the only writer of a weight outside `gls_update`; `rebuild_all` and
`measure_edges` were checked to carry weights through, which is what makes
"persist across a rollback, reset on a width change" true of the two call sites.

**The strike helper is the spec's** (S1a–S1c). Nine hand-derived transitions,
including the three knife edges (`raw == 0.98·min` is Marginal, not Substantial;
`raw == min` is not a minimum; NaN is not a minimum), plus **two million steps**
of a property test against a reference transition written from the frozen
sentence alone and shaped differently from the shipped one. Zero mismatches. The
repair's own red/green reproduces: on an alternating shelf sequence the repaired
counter reaches 200 where round 1's reaches 1.

**The trajectory machinery replays, across machines** (`replay.py`). Three
committed fixed-work replays (seeds 0, 1, 5 at their recorded 23-bite ordinals)
were re-run here in two processes each. All three: two processes bit-identical
after stripping the one `wall` object, **and** the committed
`replayDepthMm` reproduces bit for bit —
`179.16566573285345`, `179.17057349197626`, `181.51730509414207` — with matching
publication counts and matching per-publication ordinals. The whole-document
digests differ only by the absolute request path and the executable hash, both
of which are in the digest's input and differ between worktrees.

The `cutclose.py` FAST battery also reproduces exactly on this machine:
`relocates 590 / containerSamples 29500 / focusedSamples 14750 /
containerWinners 3 / containerCommits 3 / focusedWinners 84 /
stayPutWinners 503`, identical to the committed `cutclose-fast.json`.

**162 bite identities, all green** (`bites-consistency.py`) and **90 publication-
chain identities, all green** (`chain.py`, over 15 full cell documents),
including the two that matter most:

* every explore bite starts at the previous publication's **published** raw
  depth, bit for bit — not at its target and not at a pre-repair proxy depth
  (this is the exact-parent-drift clause, and it is the one a driver-level check
  cannot see, because the driver recomputes `previous × 0.999` from the same
  side of the seam);
* `improvedIncumbent` is true exactly on the publications that lowered the
  running best by more than 1 µm, recomputed from the depth series alone.

**The first bite record recomputes** (S6a–S6b). From the constructor's own poses:
`widthBefore 182.976`, `widthAfter 182.793024`, `delta −0.18297599999999647`,
`splitY 91.488`, `movedPieces 34` — all identical to the committed
`wall.json` `bites[0]`, with `movedPieces` recomputed two independent ways
(by pose delta and by the centroid-above-the-cut test). `split_and_close`
touches only `ty`, only on the far side, by exactly `delta`, on all 61 poses.

**No clock inside a sweep.** `Instant` appears in exactly one place under
`search/overlap_ics/` — `Pacer::Wall`, `mod.rs:1640/1665/1677`. Every caller of
`Pacer::elapsed_s` is at a phase boundary or after the eight-worker join. The
`FixedWork` variant has no `Instant` field and `Pacer::new` constructs one only
in the `Wall` arm, so a fixed-work trajectory cannot read a clock at all —
which is what makes the two-process bit-identity above a proof rather than a
coincidence.

**The relocate is not neutered** (S7b). One master iteration's largest committed
centroid displacement is **33.007 mm** against a `ladder_top_mm` of **1.25 mm** —
26×. The pre-named "PGS in a sampling costume" defect is not present.

**The rerun README's own tables reproduce.** All 18 rows of §5's and §6's
bite-22 tables recompute exactly from the two committed raw bite-row files, and
so does the "145 strikes and 164 disruptions against round 1's 88 and 122" line.

---

## 2. Findings

Severity is answered against one question: *does this distort the committed
evidence the round-2 quorum is reading?*

### F1 — EVIDENCE-DISTORTING (bounded). The anytime checkpoint filter is in the wrong clock frame, and cannot fire.

`drivers/wall.py`, pre-fix line 71:

```python
within = [row for row in publications
          if row.get('wallSeconds') is None or row['wallSeconds'] <= limit]
```

`limit` is `10.000`, measured from the decoded request. `wallSeconds` is
`Pacer::elapsed_s()` (`mod.rs:1663`), and the `Pacer` is constructed **inside**
`Engine::run_cutclose`, after the constructor has already spent its share. So
the left side is bounded above by `limit − constructorSeconds` **by
construction**, and the filter can never exclude anything on any cell.

Measured over nine fresh 10.000 s cells: minimum filter headroom **2.307 s**;
maximum request-relative publication age **9.9981 s** — **1.9 ms** of margin on
the one qualifying seed. `publicationsWithinBudget == publicationsTotal` on all 27
committed cells is therefore a tautology, not a check, and §0.1's "a publication
completed after 10.000 s cannot change that verdict" was never enforced.

The overrun it is supposed to catch is reachable: `Engine::separate` runs its
band test and publication attempt at the **top** of the loop and checks the
deadline afterwards (`mod.rs:780-820`), so the last master iteration of the
compress phase can publish after `total_s`.

**Direction, and why it is bounded.** A too-permissive filter can only *add*
qualifying seeds. The committed verdict is FAIL at 2/9; a late publication among
the two qualifying seeds would make it 1/9 or 0/9, still FAIL. It cannot turn
this round's FAIL into a PASS. It would matter to any future PASS round, and it
already distorts what `publicationsWithinBudget` claims to be.

**Fixed here** (driver, evidence-presentation). `wall.py` now brackets the
offset with the two bounds the document carries and excludes only publications
that are *certainly* late, reporting `publicationsUndecidedByFrame` for the
band it cannot settle. Before/after in `driver-fix-vector.py`.

### F2 — EVIDENCE-DISTORTING (bounded). The reduction drops every per-publication clock reading, so F1 is not re-derivable from committed evidence.

`wall.json`'s seed rows carry `constructorSeconds`, `searchSeconds` and
`totalSeconds` but no per-publication `wallSeconds`, so no reader can check
which publications landed inside the budget. This is Sol review 18's
general-fidelity risk 2 one level down: the `bites` array was restored, the
clock readings were not.

**Fixed here.** Each seed row now carries a `checkpointFrame` object with
`loopRelativeMaxSeconds`, `requestSecondsLowerMax`, `requestSecondsUpperMax`,
`publicationsExcludedAsLate`, `publicationsUndecidedByFrame` and
`bestStrictChildRequestSecondsLower`. Measured on the 3 s battery: the two
bounds differ by 0.3 ms, so the bracket is tight enough to decide the clause.

### F3 — correctness-but-not-evidence. The wall battery's licence gate is vacuously grantable.

`drivers/cutclose.py`, pre-fix:

```python
'CANARY_PASS': all(row['pass'] for row in results if row['stage'] == 'canary'),
```

`all()` over an empty selection is `True`. `python3 cutclose.py bites` — or any
invocation that does not name the canary — writes `CANARY_PASS: true` into
`cutclose-fast.json`, and `wall.py` reads exactly that field to decide whether
it is licensed to spend ninety wall seconds. Grok review 12 Round 2 §6.3.4 makes
the canary a **stop**; a stop satisfied by an empty selection is not one.

Red vector, measured: with `stagesRun = ['bites', 'merge']`, the old expression
returns `True` and the repaired one returns `False`.

Does **not** distort the committed evidence: the committed `cutclose-fast.json`
ran all four stages and all four passed, reproduced here.

**Fixed here** (driver). The four FAST stages re-run green on the repaired
driver and reproduce the committed numbers exactly.

### F4 — EVIDENCE-DISTORTING for the funnel autopsy. `exactAttempts` counts band entries, not exact-geometry calls.

`mod.rs:782`:

```rust
if totals.max_violation_mm <= band {
    band_reached = true;
    exact_attempts += 1;
    let outcome = self.attempt_publication();
```

The counter is incremented before the call. `Engine::attempt_publication`
returns early on an unchanged pose digest (`mod.rs:403`), and `publish::attempt`
returns `None` — no checkpoint row, no `work.exact_checkpoints` — whenever
`max_g > band`, `proxy > T`, or `proxy > incumbent − 1 µm`
(`publish.rs:265-273`).

Measured over the nine fresh 10 s cells of the committed audit run: **2,780**
band entries counted as `exactAttempts`, against **756** calls that actually
reached exact geometry. **2,024 (73 %) never asked the exact authorities
anything.** The funnel's `exactAttempted` rung reports a *third* number —
**461**, the count of *bites* that attempted, which is the overclaim already
recorded in the rerun README §9.

So the failure license's funnel
`bitesStarted → proxyBandReached → exactAttempted → dualValidPublished` has **no
rung** that answers "how many times were the exact authorities asked", the true
number is `work.exactCheckpoints`, and `wall.py`'s reduction drops `work`
entirely. The autopsy the failure license buys is being read off two numbers
that are 0.6× and 3.7× the one it wants.

Not verdict-changing. **Not fixed** — the counter is engine code.

### F5 — correctness-but-not-evidence. `stayPutWinners` names the winning *seed*, not a piece that stayed put.

`relocate.rs:850-859` reports `best.origin` — the origin of the pool member the
winner **descended from** — and then runs a fine coordinate descent from it
(`relocate.rs:817-829`), so a relocate that moved the piece a long way still
reports `StayPut` whenever the entry pose was the best of the 76 pool members.

Measured on one master iteration of the gate fixture: of 14 StayPut-origin
winners, **12 moved the piece**. On the committed 10 s seed-0 cell,
`stayPutWinners` is 79,539 of 80,551 relocates (98.7 %) while `acceptedMoves` is
29,253 (36 %) — roughly 28,000 "stay-put winners" moved.

The doc-comment is accurate ("... from the pose the piece already had"), and the
neutered-relocate tripwire is unaffected (F-clean above: 33.007 mm against a
1.25 mm `ladder_top`). But a reader who takes 98.7 % as "the operator almost
never moves anything" reaches the opposite of the truth, and that is exactly the
reading the tripwire exists to refute. `RejectionCensus::accepted_by_origin`
already carries the honest split; `schedule_json` does not emit it.

**Not fixed** — engine/driver surface, next round.

### F6 — correctness-but-not-evidence. `invalidPublications` is structurally zero, and no placements are emitted to re-validate.

`publish::attempt` writes `checkpoint.published_raw_depth_mm = Some(..)`
(`publish.rs:449`) only after `kernel_exclusive_valid` (line 421) and
`contract_valid` (line 440) are both true. The predicate the drivers compute —
`published.is_some() && !(kernel && contract)` — therefore has **no reachable
witness**. `everyPublicationDualValid: true` in the verdict is an invariant of
the emitter, not a measurement of the layouts.

That would be fine if something else could check the layouts. Nothing can:
`schedule_json` emits fingerprints and depths, and measured over the nine cells
of the committed audit run, **0 of 437 publications carry placements**. The gate's clause "every emitted
publication at every time passes Exclusive `r = 2.500` and the untouched
publication contract" is self-certified end to end.

The layouts really are dual-valid — the code path proves it, and the audit
verified the path. The finding is that the *number* carries no independent
information and should not be read as if it does.

**Not fixed.**

### F7 — correctness-but-not-evidence (latent). The clearance split is degenerate on the gate fixture, and one contract shape inverts it.

mixed-61 has `flatteningSagToleranceMm = 0` and `clearanceSafetyMarginMm = 0`, so
`physical_edge_clearance_mm() == depth_top_inset_mm() == sheet_edge_clearance_mm
== 5.0`. **The entire clearance-split family — this campaign's worst bug shape,
which produced Sol review 15 §A.1 and Grok review 10 Finding 1 — is untestable
on the gate cell.** triangle-20 and shapes-17 have `sag == safety == 0.25`, where
Φ's boundary (`edge + sag`) exactly equals the round kernel's
(`sheet_inset_mm() + expansion_mm() == edge + safety`). All three campaign
fixtures sit on the equality.

A request with `safety > sag` puts Φ's boundary **inside** the kernel's by
`safety − sag`. Measured on a probe at `sag = 0.10, safety = 0.40`: Φ charges
its rows at 5.1 mm while `boundary_admissible` demands 5.4 mm — a 0.300 mm
shortfall, **75× the 4 µm repair band**. Every publication attempt on such a
request would be refused at the boundary, and the funnel would read
`proxyBandReached` with `dualValidPublished = 0` for a reason no counter names.

Sites: `broad_phase.rs:51` and `relocate.rs:481` and `publish.rs:627` all read
`physical_edge_clearance_mm()`; `publish.rs:170` reads `sheet_inset_mm()` and
`publish.rs:292` reads `expansion_mm()`. The two families never meet in one
assertion anywhere in the tree.

Does not touch the committed evidence. Recorded as the next clearance-split
candidate. The probe is in `rust-vectors` S2e, labelled `asserted: false` so the
harness is not permanently red about a request nobody runs.

### F8 — cosmetic. `WorkVector::saturating_add` does not saturate.

`diagnostics.rs:102-123` uses `+=` throughout. The workspace release profile
sets `overflow-checks = true`, so an overflow would panic rather than saturate.
Unreachable at `u64` scale; the name is wrong, not the arithmetic.

### F9 — cosmetic (false-green shape). Two of the cut-close tripwire's three clauses are tautologies.

`drivers/cutclose.py:265-272`:

```python
expected_delta = row['widthAfterMm'] - row['widthBeforeMm']
expected_split = row['widthBeforeMm'] / 2.0 if row['phase'] == 'explore' else None
...
'deltaExact': row['deltaMm'] == expected_delta,
'splitIsMidDepth': row['splitYMm'] == expected_split,
```

`homotopy.rs:178-180` computes `delta_mm` as exactly that subtraction and
`split_y_mm` as exactly `centre_cut_mm(width) = width / 2.0`. Both driver
clauses re-run the engine's own expression on the engine's own inputs and cannot
fail whatever `split_and_close` does with the poses — which is the property the
tripwire is named for. This is the "false-green tests (literals checked,
transitions not)" shape from the campaign's autopsies.

The real property *is* covered — by the module's unit vectors and by this
audit's S6b, which asserts on the pose arrays that only `ty` moved, only on the
far side, by exactly `delta`, for all 61 pieces. **Not fixed**: the driver has
no pose data to check the real property with, because the engine does not emit
per-bite poses, so the honest repair is a relabel and belongs to whoever owns
the surface.

### F10 — reported, not a defect. The repair pauses the counter; it does not increment it.

A **strictly monotone** trickle — a new minimum inside the 2 % band on every
single iteration — leaves `since_improvement` exactly where it started, under
the repaired predicate as much as under round 1's. Measured: 1,000 consecutive
marginal minima, `since_improvement` still 0.

This is faithful to the cited source (`separator.rs:102-115` increments only on
a non-improvement) and is not a port defect. It is the boundary of what the
repair can do, and it means any claim of the form "the separation now strikes
out on the shelf" is a claim about a sequence that contains non-improving
iterations. The committed evidence shows such sequences occurring: **145 strikes
and 164 disruptions across the 27 rerun cells.**

### F11 — reported. Round 1 was not globally strike-free.

Recomputed from `round1-bites-red.json`: **88 strikes and 122 disruptions across
its 27 cells**, on 1,391 bites. The rerun README states this correctly in §5,
and its "no separation there ever struck out ... Algorithm 12 never ran"
sentence is scoped to seed 1's 22nd bite, where round 1 really is `0 / 0` and the
rerun is `6 / 2`. Both the §5 and §6 tables reproduce here row for row. Noted
only because the sentence appears in `mod.rs`'s `observe_raw` doc-comment
without its scope, where it reads as a global claim.

### Known nits, re-confirmed

* **The disruption follower cap is vacuous.** `disrupt.rs:332`, `if moved.len()
  >= count`, sits inside `if !moved.contains(&follower)`; `moved` can hold at
  most `count` distinct pieces, so when the cap could fire the guard above it
  has already excluded every candidate. `followers_capped` is unreachable and
  reads 0 in all committed evidence for that reason, not because the cap held.
* **The pool-restore weight policy** (`PoolEntry::restore_weights` restores the
  pooled layout's own landscape rather than the tracker's current one) is a
  declared, documented difference from the source. Unchanged, and correct as
  documented: `rebuild_all` and `measure_edges` were verified to carry weights
  through, so the restore is the only writer in that path.
* **Per-iteration thread spawn.** `Engine::tournament` builds a fresh
  `std::thread::scope` per master iteration. Recorded risk, not a defect: the
  merge was verified deterministic and the joins are in ordinal order.

---

## 3. Honest caveats

* **Wall trajectories are load-bound and do not reproduce across machines.** The
  nine 10 s cells run here gave 1/9 qualifying (seed 2) against the committed
  2/9 (seeds 2 and 3). Both are FAIL. Nothing in this audit compares a wall
  trajectory across machines, and no finding rests on one. The **fixed-work**
  replay does reproduce, bit for bit, and that is the only cross-machine
  reproduction claim made here.
* **F1's overrun was not produced.** Twenty-one 10.000 s cells were run across
  three batteries and none published after the budget in the request-relative
  frame; the closest was **9.9981 s** against 10.000 s. The finding is that the guard is inoperative and
  the margin is 7 ms, not that a late publication is present in the committed
  evidence. Whether one is present there **cannot be determined**, because the
  committed reduction dropped the readings (F2).
* **The clearance split cannot be exercised on the gate fixture at all** (F7).
  The S2 vectors run on a synthetic triangle-20-shaped contract to cover it, and
  on a probe contract to show the inversion. Neither is the gate cell.
* **`sheet_slack` is private**, so S2's third site is reached through the public
  statement of the same rule (`strip_sample_box` and `boundary_residuals` naming
  the same four walls, plus the arithmetic identity `top = target −
  depth_top_inset` all three share). A direct assertion on `sheet_slack` would
  need a `pub(crate)` change to the engine, which this round is not licensed to
  make.
* **S4's Algorithm-8 vector checks the schedule, not the call site count.** It
  recomputes each pass's weights from the pre-pass weights and the post-sweep
  violations; "one pass per master iteration" is asserted separately, in S5c,
  as `weightUpdates == 1` for one reconstructed iteration.
* **The audit found no defect that changes the committed verdict.** The gate
  reads FAIL at 2/9 and every finding above either cannot move it or moves it in
  the FAIL direction. That is a statement about this round's numbers, not a
  general clearance of the machinery — F4, F5 and F6 all describe numbers that
  would be misread if a future round were closer to the bar.

---

## 4. The two repairs made here

Both are in `docs/experiments/overlap-ics/drivers/`. Neither touches
`crates/`, and the engine binary is unchanged.

| | before | after | vector |
|---|---|---|---|
| `wall.py` checkpoint filter | `wallSeconds <= 10.000`, headroom 2.307 s, cannot fire | brackets the offset, excludes only certainly-late publications, reports the undecided band | `driver-fix-vector.py` `D1` |
| `cutclose.py` `CANARY_PASS` | `True` when the canary did not run | requires a canary row | `driver-fix-vector.py` `D2` |

Both were re-run afterwards: `wall.py 3` completes green on nine seeds and now
emits `checkpointFrame` per cell; `cutclose.py` completes all four stages green
and reproduces the committed `cutclose-fast.json` numbers exactly.

---

## 5. The independent re-validation, and what it settles

A second auditor re-ran this audit's subject — the committed numbers, not the
code — from this document's own tip, using the findings above only as leads and
importing none of the scripts above. Its work is in
[`revalidation/`](revalidation/README.md); its scripts carry an `rv_` prefix so
the two sets never collide.

**It settles F2, against F2.** This chapter concluded that whether a
post-budget publication is actually present in the committed round "cannot be
determined", because the reduction dropped every per-publication clock reading.
It can be determined: the raw cell documents `wall.py` reduced still exist at
`/var/lib/t3/tmp/overlapics/rerun/`, they carry `publications[].wallSeconds`,
and `rv_reduction.py` binds them to the committed `wall.json` by re-deriving all
702 of its cell-row fields with zero mismatches. On that binding,

* **three of the 27 committed cells report a publication that completed after
  the deadline the engine itself was given**, and in all three that publication
  is the one whose depth the README prints as the cell's answer:
  10 s seed 3 **167.31508 → 167.31678** (+0.274 ms past the deadline), 3 s
  seed 1 179.42186 → 179.42767 (+1.547 ms), 30 s seed 8 179.06000 → 179.06179
  (+0.221 ms), plus a fourth in the diagnostic control arm A;
* **the gate verdict does not move.** The qualifying set is `[2, 3]` under the
  committed filter, under the repair's lower bound and under its upper bound;
  seed 3 stays below the bar on every reading. `GATE_PASS: false`, quorum 2 of 3.

So F1 was right about the mechanism and the size of the hole, and wrong to
record the overrun as unproduced; the correction is µm-scale and one of the µm
is under a headline.

**It reframes F6 in this audit's favour.** `invalidPublications` really has no
reachable witness — but classifying all 3,298 exact checkpoints of the round by
*which* authority refused shows the gate is not decorative: 1,227 refused by the
Exclusive r = 2.500 kernel, **361 refused by the untouched
`validate_placements_against_contract` after the kernel had already passed**, 9
by the immutable target, 1,701 published. The second authority does independent
work on 361 real layouts.

**It confirms, from the documents rather than the drivers, that F3 did not touch
this round:** the committed `cutclose-fast.json` is byte-identical to the file
`wall.json` names as its licence and carries all four stages, canary included.

**What it adds that this chapter did not have.** All nine fixed-work replays
re-run bit for bit on a third binary, including all 224 `replayOrdinals`; the
S0 pose fixture and all 18 recorded arm-B layouts pushed back through both
authorities with their depths recomputed independently (including round 1's
`168.4836008374388`); every per-bite claim of the rerun README recomputed by a
reduction written from the README's text; and the measured shared prefix between
the two rounds' bite rows — 25 of 27 cells identical for ≥21 bites, first
divergence at ordinal 22 on 21 of 27.

**What it could not do, and neither could this chapter.** No pose is recorded
for any of the 1,701 publications. `161.05499`, `163.56062`, `167.31508` and
`167.95169` are re-validatable only by the process that produced them.
