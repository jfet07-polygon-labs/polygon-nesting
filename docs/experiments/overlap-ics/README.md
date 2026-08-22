# overlap-ICS — the vertical slice, and Gate 0

**Verdict: STOP.** Three of the six fatal cells fail: **S1**, **C175** and the
**triangle-20 canary**. The other three fatal cells — S0, numeric soundness,
throughput — pass, several of them by an order of magnitude.

Per docs/overlap-ics-converged-spec.md §"Round structure of record", a fatal
Gate-0 failure stops the round before any schedule or parallel work. That is
what this document reports. `homotopy.rs` remains the stub it was written as,
no epoch schedule was built, and nothing here is wired into a production route.

The spec of record is docs/overlap-ics-converged-spec.md, whose body is Sol
review 14 Round 2 §4 (the implementation spec) plus §3 (the two-tier test
discipline), with Grok review 9 Round 2 §4's amendments as arbitrated there.

---

## 1. The verdict table

| cell | class | verdict | the number that decides it |
|---|---|:--:|---|
| **S0** — the Sparrow pin, untouched | fatal | **PASS** | 61 placements, `rawSourceDepthMm` **150.16451**, `phi.to_bits()` **0**, Exclusive accepts at `two_r = 5000`, contract accepts, **0** repair rows, giveback **0.0** |
| **S1** — ±0.5 mm / ±2°, locked `W = 150.16547` | fatal | **FAIL** | Φ falls 433.492 → **0.00044819**, but `max_g` stalls at **0.012635 mm** against a **0.004 mm** attempt band, and the depth lands at **150.17299** — **7.53 µm outside** the locked strip. Zero publication attempts. |
| **C175** — constructor shocked by 0.10 (D₀−L) | fatal | **FAIL** | **0 of 3** seeds return a strict dual-valid non-constructor child. Φ 461/462/467 → 40.2/38.6/38.0, `max_g` **2.10 / 2.24 / 2.16 mm**. Zero publication attempts. |
| **triangle-20** — locked `W = 70.742` | fatal | **FAIL** | Φ 22.216 → **0.049891**, **0 active pair rows**, `max_g` **0.11765 mm** — entirely boundary. Zero publication attempts. |
| **numeric soundness** — 1,000-state corpus | fatal | **PASS** | 0 outside the 4 µm band (worst deficit **0 µm**), 0 containment false-feasible on **60** containment states, 0 incremental mismatches, force correlation **100 % / 100 %** on the scored population |
| **throughput** | fatal | **PASS** | cold Φ **40.5 µs** (≤200), row rebuild **1.358 µs** (≤20), **6.83 M** cell-gap evals/s (≥1 M), **987,861** projected proposals in 8 s (≥100 K) |
| S2 — ±2 mm / ±10° | diagnostic | fail | Φ 12,668 → 362.36, `max_g` **9.13 mm** |
| C168 — constructor squeezed to 168.484 | diagnostic | fail | Φ 1,932.9 → 468.83, `max_g` **7.38 mm** |
| random-T — uniform throw into 168.484, 8 jumps | diagnostic | fail | Φ 182,899 → 8,688.3, `max_g` **28.29 mm**; 8 jumps, 4 improved guided Φ |
| 10,000-state corpus | diagnostic | **pass** | 0 outside band, 0 containment false-feasible on **589** containment states, 0 incremental mismatches, 100 %/100 % on the scored population |

`evidence/gate0.json` is the battery document; `evidence/cell-*.json` are the
per-cell run documents the verdicts were read from.

---

## 2. What passed, and what that establishes

### 2.1 S0: the measure is right on a known-legal layout

The committed Sparrow pose set scores **exactly zero** — `phi.to_bits() == 0`,
not `|Φ| < 1e-6` — at `c_pair = 5.0`, reproduces **150.16451 mm** of raw source
depth to every printed digit, and is accepted by the request-scoped round kernel
at `two_r = 5000` **and** by the untouched material contract validator, with
zero repair rows and zero giveback.

That closes the first of the three self-deceptions Grok review 9 §3 names: the
proxy is not lying about a layout that is genuinely legal. It also fixes the
`c_pair`/radius wiring in place — 5.0 mm pair clearance, `r = 2.500`,
`two_r = 5000`, allowance 0 — because a mis-wired contract would not have
produced a bit-zero Φ.

**The fixture was read by nothing else.** It is not a seed, not a parameter
source, and every constant in `search::overlap_ics` is derived from the request
(`c_pair = total_padding + 2·sag`, `ε_grid = 2⌈√2·1 µm⌉ = 4 µm`, the ladder from
`max(clearance/4, median diameter/128, 8 µm)` down to 0.25 µm) rather than
chosen against it.

### 2.2 Numeric soundness: the field is arithmetically sound

At 1,000 and again at 10,000 states:

| clause | 1,000 | 10,000 |
|---|---:|---:|
| proxy-feasible states | 40 | 416 |
| of those, exact-invalid | **0** | **0** |
| outside the 4 µm band | **0** | **0** |
| worst deficit | **0 µm** | **0 µm** |
| containment states detected independently | 60 | 589 |
| containment false-feasible | **0** | **0** |
| incremental rows ≠ cold rebuild | **0** | **0** |
| force: active improved (scored population) | **501/501** | **5001/5001** |
| force: total not worsened (scored population) | **501/501** | **5001/5001** |

The corpus is a **second implementation** on purpose (`corpus.rs`): it measures
whole transformed rings rather than the convex cells Φ uses, detects overlap by
ray-cast containment and segment crossing rather than by SAT, and measures
penetration as the deepest interior vertex's distance to the other boundary —
a different quantity from the minimum translation vector, so agreement means
something.

**One honest caveat, stated before anyone has to find it.** The corpus has three
families and the fatal force clauses are scored on one of them:

| family | share | what it is for | force: active improved |
|---|---:|---|---:|
| `compressed` | 50 % | Sol's own corpus definition: 1 %/3 %/10 %-residual affine compression plus predeclared SE(2) perturbations | **5001/5001 = 100 %** |
| `grazing` | 33 % | 0 % compression, µm-scale perturbations | 2447/2917 = 83.9 % |
| `containment` | 17 % | one piece translated inside another | 286/1433 = 20.0 % |
| **all three** | | | **7734/9351 = 82.71 %** |

The scored population is `compressed`, because that is the population Sol
review 14 §3 defines the clause over — *"on at least 10,000 deterministic states
produced from three constructor layouts by 1 %, 3 % and 10 %-residual affine
compression plus predeclared SE(2) perturbations"*. The other two families are
**this round's additions**, and they exist because the compression family alone
never produces a Φ-feasible state: scored on it alone, the false-feasibility
clause would read `proxyFeasible = 0` and pass vacuously, which is how a
soundness battery fakes itself. Both rates are in every corpus document
(`forceActiveImprovedRate` and `forceActiveImprovedRateAllFamilies`), so folding
them in is one division away.

Why the two added families miss, in mechanism rather than in excuse:

* **`grazing`.** Φ is a sum of squares; the independent score is a sum of
  violations. Near convergence, trading one 68 µm residual for several 10 µm
  ones lowers the first and raises the second. Every logged miss has that shape:
  `phi 0.004723 → 0.004478` while `active 0.027378 → 0.030113`, on a 15 µm step
  (`evidence/corpus-gate-1000.json`, `forceMisses`).
* **`containment`.** The minimum translation vector of a small piece deep inside
  a large one points the long way out — its magnitude is the *host's* extent, not
  the small piece's depth — so it is not a descent direction for a
  deepest-interior-vertex measure. This is the known weakness of MTV under deep
  penetration, and the spec checks containment with a different clause ("no
  containment false-feasible case"), which passes 589/589.

### 2.3 Throughput: the loop is fast enough by an order of magnitude

| kill | bar | measured | margin |
|---|---:|---:|---:|
| cold full mixed-61 Φ geometry | ≤ 200 µs | **40.51 µs** | 4.9× |
| one moved-piece row reconstruction | ≤ 20 µs | **1.358 µs** | 14.7× |
| convex-cell signed-gap evaluations | ≥ 1 M/s | **6.83 M/s** | 6.8× |
| complete piece proposals projected into 8 s | ≥ 100 K | **987,861** | 9.9× |

The proposal rate is reported with `rawPhiBeforeProposals = 565.38`,
`rawPhiAfterProposals = 118.14` and `acceptedMovesDuringProposals = 1626` beside
it, because a proposal on a piece with no incident energy returns before it
forms a gradient and would inflate the rate into a lie. Those three fields say
the loop was doing the work the currency is denominated in.

The broad phase is why: on mixed-61 a cold Φ probes all 1,830 pair rows and the
`f64` box proof zeroes **1,744** of them without one cell query, leaving 163
convex-cell gap queries (`evidence/cell-s0.json`, `outcome.work`).

---

## 3. What failed, and what the failures actually say

All three fatal failures have the **same shape**, and it is not the shape the
spec's authors were most worried about.

> Φ falls by two to four orders of magnitude, every *pair* row clears or nearly
> clears, and the trajectory then stalls **tens to hundreds of micrometres**
> outside the publication band — with the residual concentrated in **boundary**
> rows.

| cell | Φ in → out | active pair rows at the end | active edge rows | `max_g` | where the residual is |
|---|---|---:|---:|---:|---|
| triangle-20 @ 70.742 | 22.216 → **0.0499** | **0** | 5 | 0.11765 mm | 100 % boundary |
| S1 @ 150.16547 | 433.49 → **0.00045** | 2 | 2 | 0.012635 mm | 7.53 µm of depth + 11.7 µm of pair |
| C175 @ 176.2623 | 461.35 → **40.15** | 22 | 21 | 2.10 mm | mixed |

### 3.1 triangle-20 is the cleanest reading of the failure

The canary ends with **zero pair violations** — every one of the 190 pairs
clears the 5.0 mm contract — and 5 boundary rows violated by at most
**117.6 µm**, with the layout's own depth at **70.602 mm**, a full **140 µm
inside** the locked 70.742. The whole layout is legal except that a few pieces
poke out of the sheet edge by a tenth of a millimetre, and the strip has room.

A single global translation would legalize it. This solver cannot express one:
it moves one piece per proposal and accepts only a strict decrease in *that
piece's* incident guided Φ, so a layout that is collectively 0.12 mm too low is
jammed — moving any single piece up creates an overlap its own neighbour has to
absorb first, and the chain never starts.

That is a **structural** finding about the specified move set, not a numerical
one, and it is the single most useful thing this round produced.

Two supporting measurements:

* `L` for triangle-20 is **70.0 mm** (`max(area/usable width, max min-width) +
  2·edge = max(21.106, 60.0) + 10`). The canary's locked `W = 70.742` is
  **0.742 mm above the derived floor**, so the target is genuinely tight — the
  isoceles 70×60 triangle's minimum width over all rotations is exactly 60.0 mm
  and 60.742 mm of usable depth is what the strip has. This is a hard cell, and
  the campaign's own 10-second plan mark reaches it, so it is not an impossible
  one.
* With rotation frozen (`--rotation=off`,
  `evidence/probe-triangle20-rotation-frozen.json`) the same cell accepts
  **66,863** moves instead of **175** — the fixed translation/rotation coupling
  in the SE(2)-normalized direction is rejecting almost every step once the
  boundary weights grow — and still ends jammed on boundary rows at 0.18 mm.
  So the coupling is a real inefficiency and it is **not** the cause of the
  failure.

### 3.2 S1 misses by 12.6 µm, and the blocker is the strip, not the repair

S1 ends at `max_g = 12.635 µm` against a 4 µm attempt band, with a raw depth of
**150.17299** against a locked **150.16547** — 7.53 µm too deep. Widening the
attempt band to 16 µm as a probe (`evidence/probe-s1-widened-band.json`) still
produces **zero** checkpoints, because the second attempt gate is
`proxy_depth ≤ T` and the state is outside the strip. The repair machinery is
never reached, so this is a **search** failure and not a legalization one.

Raising the budget from 200 K to 2 M proposals changes nothing: `max_g` is
`0.012634958179553735` at both, with 32,549 stalled sweeps out of 32,786 and a
maximum guided penalty of 9,885. It is a hard stall, not a slow convergence.

### 3.3 The basin sweep: where the field does work

`evidence/basin-jump-guided.json` — the S0 pin perturbed by a ladder of
magnitudes, everything else identical:

| perturbation | entry Φ | final Φ | final `max_g` | republished | repair | giveback |
|---|---:|---:|---:|:--:|---:|---:|
| 0.005 mm / 0.02° | 0.003 | **0.0** | 0.0 | ✅ 150.16229 | 0 µm | 0.0 |
| 0.020 mm / 0.08° | 0.158 | **0.0** | 0.0 | ✅ 150.15664 | 5.0 µm | 0.0 |
| 0.050 mm / 0.20° | 1.567 | **0.0** | 0.0 | ✅ 150.16305 | 5.5 µm | 2.93 µm |
| 0.100 mm / 0.40° | 9.742 | **0.0** | 0.0 | ✅ 150.16223 | 0 µm | 0.0 |
| 0.250 mm / 1.00° | 90.190 | **0.0** | 0.0 | ✅ 150.16003 | 6.0 µm | 0.0 |
| **0.500 mm / 2.00°** (S1) | 433.49 | 0.00045 | 0.012635 | ❌ | — | — |
| 2.000 mm / 10.0° (S2) | 12,668 | 362.36 | 9.13 | ❌ | — | — |

**The whole vertical slice works end to end** — continuous descent to Φ = 0,
publication through `GridSet::of`, the Exclusive predicates at `r = 2.500`, the
bounded µm repair, the untouched contract validator, a protected incumbent — up
to a perturbation of **0.25 mm / 1.0°**, and it stops between there and
0.5 mm / 2.0°. Every republication is dual-valid, inside the locked strip, with
repair ≤ **6 µm** against a 16 µm cap and giveback ≤ **2.93 µm** against a
50 µm cap. Nothing in this table is a millimetre-scale legalization.

So the answer to Sol review 14 §6's named risk — *"the deepest-triangle witness
field may not be a useful navigation field"* — is: **it is a useful field with a
basin between 0.25 mm and 0.5 mm of SE(2) displacement on a critically packed
61-piece layout**, and the specified move set cannot cross that.

---

## 4. The one knob Gate 0 moved, and why

`DescentConfig::jump_commits_unconditionally` defaults to **`false`**.

Sol review 14 R2 §2's jump "chooses by guided Φ" among 16 low-discrepancy
relocations and "commits for a full epoch even if raw Φ temporarily worsens".
Read literally, staying put is not in the choice set and the best candidate
commits unconditionally. That reading was implemented first and measured
(`evidence/basin-jump-always.json` versus `evidence/basin-jump-guided.json`,
identical in every other respect):

| cell | commit unconditionally | commit on guided improvement |
|---|---:|---:|
| S1 (0.5 mm / 2°) final `max_g` | 2.552630 mm | **0.012635 mm** |
| S2 (2 mm / 10°) final raw Φ | 1,308.79 | **362.36** |

Two hundred times closer on the fatal cell. The spec calls the jump's type,
order and stall threshold **KNOBS** — *"not architectural disagreements"* — so
choosing between two readings on measured evidence is what Gate 0 is for. Both
settings stay reachable (`--jumpcommit=always|guided`) so the next round can
re-run the comparison in one command.

**Neither setting changes the verdict.** Under `always`, C175 is 0/3 with two
seeds diverging to Φ = 925 and Φ = 3,359; under `guided` it is 0/3 at Φ ≈ 38–40.

### Two defects the basin sweep found, both fixed in this round

1. **A converged state was counted as a stall.** A sweep that changes nothing
   has `raw_after == raw_before`, so a trajectory at Φ = 0 kept "stalling" and
   eventually relocated a piece out of a *feasible* layout: the 0.005 mm row
   converged to Φ = 0 and was driven back to **Φ = 142.58** by its own escape
   mechanism. Guided weights and topology jumps exist to escape a *violated*
   local minimum; at zero violation there is nothing to escape. Fixed in
   `mod.rs::run`.
2. **An unchanged state was re-checkpointed forever.** The attempt gate compares
   the *proxy* depth to the incumbent, and repair can give back more than the
   1 µm the gate asks for, so a converged state whose repaired depth is worse
   than its proxy depth passed the gate on every sweep and republished the
   identical layout. One basin row spent **3,266** exact checkpoints that way.
   Fixed with a last-attempt guard in `Engine::checkpoint`.

Neither is a design change; both are the kind of thing a falsifier battery is
supposed to surface before a schedule is built on top of them.

---

## 5. Reproduction

```sh
bash docs/experiments/overlap-ics/drivers/fast.sh                       # the FAST tier
python3 docs/experiments/overlap-ics/drivers/cells.py                   # Gate 0, all cells
python3 docs/experiments/overlap-ics/drivers/smoke.py 200000            # two-process smoke
python3 docs/experiments/overlap-ics/drivers/corpus_gate.py 10000       # the heavy corpus
python3 docs/experiments/overlap-ics/drivers/basin.py 200000 guided     # the basin sweep
python3 docs/experiments/overlap-ics/drivers/basin.py 200000 always     # the jump A/B

bash docs/experiments/overlap-ics/drivers/run-suites.sh                 # round boundary
python3 docs/experiments/overlap-ics/drivers/suitetotals.py
python3 docs/experiments/overlap-ics/drivers/gates.py base  BASE_BINARY
python3 docs/experiments/overlap-ics/drivers/gates.py meas  MEAS_BINARY
python3 docs/experiments/overlap-ics/drivers/gatecompare.py BASE_DIR MEAS_DIR
python3 docs/experiments/overlap-ics/drivers/determinism.py ICS_A ICS_B 200000
```

Do **not** pipe any of them into `tee` or `tail`: you would read the pipe's
status instead of the script's. Every exit status inside `fast.sh` is read
directly on the line after its command.

`fast.sh` exits **1** in this round. Its one red stage is the two-process
smoke's S1 *mechanism* clause. The smoke document separates that from the
invariants:

* `INVARIANTS_PASS: true` — no invalid publication, repair ≤ 16 µm, giveback
  ≤ 0.050 mm, every publication inside the locked `W`, and **both cells
  bit-identical across two processes** after stripping the `wall` field.
* `SMOKE_PASS: false` — S1 does not republish.

A FAST tier that went green while Gate 0 said STOP would be the lie the two-tier
discipline exists to prevent.

### The two-process comparison

Both cells compare the **entire** JSON document minus one named field, `wall`.
Every clock reading in the driver is inside that object precisely so the
comparison cannot be weakened by adding a field somewhere else. The covered
surface therefore includes everything Sol review 14 R2 §3 lists: every `x`, `y`
and `theta` bit (through `finalPoseDigest` / `perturbedPoseDigest`), raw and
guided Φ, the step digest, all ten work counters, exact attempts/refusals/
publications, repair displacement and giveback, and the placement fingerprint
and raw depth. Both cells: **bit-identical**.

---

## 6. What was built, and what was deliberately not

`crates/polygon-nesting-core/src/search/overlap_ics/`, behind
`overlap-ics = ["round-envelope-kernel", "fast-contract-validator"]`:

| file | what it owns |
|---|---|
| `state.rs` | continuous `f64` poses (θ in degrees, see below), SoA transformed geometry, cached pair/boundary rows, the protected exact incumbent |
| `decomposition.rs` | deterministic ear clip of source outer rings; **a convex piece is one cell**; holes are an explicit error |
| `contact.rs` | the signed convex gap: streamed allocation-free SAT MTV overlapping, closest material feature separated, triangle-cell maximum for nonconvex; witnesses and normals for torque |
| `broad_phase.rs` | `f64` AABBs; the box gap is a *proof* of clearance, never an estimate |
| `energy.rs` | raw squared-hinge Φ, guided integer weights, incremental rows, the fixed-order scalar fold |
| `descent.rs` | damped deterministic PGS, continuous θ from the first sweep, backtracking ladder, guided update after one stalled sweep, one topology jump after two guided stalls |
| `publish.rs` | continuous rings → `GridSet::of` → request-scoped Exclusive at `r = 2.500` allowance 0 → frozen-θ same-strip ≤4n-row ≤16 µm repair → untouched `validate_placements_against_contract` → `best_exact` |
| `diagnostics.rs` | the six-name work vector; exact-valid raw source depth as the only quality series |
| `corpus.rs` | the deterministic corpus and its independent score |
| `homotopy.rs` | **a stub**, by design |

`examples/overlap_ics_benchmark.rs`, `required-features = ["overlap-ics"]`, is
the only driver.

### Two implementation notes that are not free choices

**θ is carried in degrees.** `PolygonSet::transformed` and
`validation::general_polygon::placement_rotation` both derive their sine and
cosine from `rotation_deg.to_radians()`. If the search carried radians and
converted on the way out, the geometry Φ measures would differ in the last bits
from the geometry the two exact authorities judge, and S0 would not reproduce
its own depth. Degrees make the identity exact by construction while leaving the
coordinate continuous and unbounded — no catalogue, no window, no 2.5° step,
which is all the spec asks of it. Sol R2 §4 asks for `libm` trigonometry for
cross-version determinism; identity with the publication transform is the
stronger requirement, and the determinism contract already carries the libm
implementation in its environment tuple.

**The search-offset allowance is 0.002 mm, and it reaches exactly one
consumer.** The constructor's own envelope *is* the exact contract at allowance
zero, and a coincident envelope refuses its own legal layouts on exact contact —
the constructor fails outright with `--allowance=0`. The allowance reaches the
constructor and nothing else: Φ's clearance is `total_padding + 2·sag` read off
the material contract and `Contract` has no allowance field at all; the kernel
radius is `total_padding/2 + safety` with the allowance excluded by
construction; and `publish::publication_settings` forces it to zero before the
contract validator sees the settings.

### Chinese wall

`cargo tree -p polygon-nesting-core --features overlap-ics -e features` contains
no `jagua-rs`, and `fast.sh` fails the iteration if it ever does. No Sparrow or
jagua optimizer source was read. The pose fixture is the S0/S1/S2 correctness
pin and is read by no other cell.

---

## 7. The round-boundary battery

Run from the clean committed tree at `97c7ef5`, on binaries rebuilt from it.

### 7.1 The four pinned gates, on two binaries

`evidence/gates.json`, `drivers/gates.py` + `drivers/gatecompare.py`. The
`base` binary is `--features jagua-experimental` (this round's feature
**absent**); the `meas` binary is `--features jagua-experimental,overlap-ics`
(**compiled**, and unarmed — nothing outside the example can reach it).

| gate | pinned | base | meas | documents identical |
|---|---|:--:|:--:|:--:|
| g1 | 206.869 / `8a7737381238fa4d` | ✅ | ✅ | ✅ |
| g2 | 159.09233022733062 / `fa01012af1d559ae` | ✅ | ✅ | ✅ |
| g3 | 159.07876040364795 / `e28fba007f8031d4` | ✅ | ✅ | ✅ |
| g4 | 164.0375677990678 / `49f094d7e59a9008` | ✅ | ✅ | ✅ |

`BASE_ALL_PASS: true`, `MEAS_ALL_PASS: true`, and the stronger claim:
**`WHOLE_DOCUMENT_IDENTITY: true`** on all four. The document comparison strips
`gatelib.VOLATILE` — the round-envelope-gate protocol's own field list, copied
rather than re-derived — which removes the elapsed-derived summary statistics,
the binary hash and the worktree identity and nothing else. Compiling
`overlap-ics` in changes nothing a gate document can see.

### 7.2 The suites

`evidence/suites.json`, `drivers/run-suites.sh` + `drivers/suitetotals.py`. All
`--release`, every exit status read directly on the line after its command, no
pipelines.

| # | features | targets | passed | failed | ignored | exit |
|---|---|---:|---:|---:|---:|---:|
| 1 | `jagua-experimental` | 55 | 1293 | 0 | 2 | **0** |
| 2 | the protocol's full combo | 55 | 1357 | 0 | 2 | **0** |
| 3 | `jagua-experimental`, `--example general_request_benchmark` | 1 | 20 | 0 | 0 | **0** |
| 4 | `jagua-experimental,overlap-ics` | 55 | 1340 | 0 | 2 | **0** |
| 5 | `overlap-ics` alone, `--lib --tests` | 50 | 1150 | 0 | 0 | **0** |

Suite 4 is this round's feature. Suite 5 is it **alone** — the Chinese wall
checked as a build rather than only as a `cargo tree` grep — and it is scoped to
`--lib --tests` for a pre-existing reason the round-envelope-gate protocol
already documents: an unscoped `cargo test` builds
`general_request_benchmark`, which names `search::portfolio` and declares no
`required-features`, so any feature set without `jagua-experimental` fails to
compile it. `overlap_ics_benchmark` declares
`required-features = ["overlap-ics"]` precisely so it never does that to anyone
else.

Suite 5's first run tripped the campaign's known flake,
`free_material_multi_eviction_shrinks_retained_container_capacity` — an
allocator property, not a search one. Both runs are committed
(`suite-overlap-ics-run1-flaky.log` and `suite-overlap-ics.log`); the rerun is
clean.

### 7.3 Determinism

| comparison | cells | verdict |
|---|---|:--:|
| two processes, one binary | S0, S1 | **bit-identical** (`evidence/smoke-two-process.json`) |
| two independently built binaries | S0, S1, C175, triangle-20 | **bit-identical** (`evidence/determinism-two-binary.json`) |

The two-binary comparison strips `wall` and `executableSha256` — the second is
the thing being varied, so leaving it in would make the comparison trivially
false — and nothing else. The two binaries were built into different target
directories from the same commit and have different SHA-256s
(`evidence/binaries.txt`). Cross-platform `sin`/`cos` identity is not a claim,
here or anywhere in this campaign.

---

## 8. What the next round would have to change

Not a knob. The three failures agree on one thing: the specified move set —
**one piece, strict decrease in its own incident guided Φ** — cannot express the
collective motions the last hundred micrometres need. triangle-20 states it in
the clearest possible form: zero pair violations, 140 µm of unused strip, and a
layout that a single global translation would legalize.

The candidates, in the order the evidence ranks them, are all **outside** this
round's mandate and are recorded here rather than acted on:

1. a rigid whole-layout (or connected-component) translation move, which is what
   triangle-20's residual is asking for by name;
2. decoupling translation from rotation in the ladder — the frozen-rotation
   probe accepted 382× more moves — rather than stepping along one fixed
   SE(2)-normalized direction;
3. Grok review 9 R2 §1.1's named fallback, hull-as-one-cell SAT, which is
   already what this implementation does for the 52 convex mixed-61 pieces, so
   it is spent;
4. the strip homotopy itself: every cell here ran at a **locked** target, which
   is what a Gate-0 battery is for, and a schedule that bisects toward a
   reachable target would have turned these three failures into slow successes
   and made the round uninterpretable.

Under docs/overlap-ics-converged-spec.md the fatal set is fatal: *"Kill or
replace the contact model immediately if any false-feasible state lies outside
the band, or if force correlation/throughput misses. Do not proceed to schedule
work."* Neither of those missed. What missed is inflation — S1, C175, the
triangle canary — which the same section makes fatal and which stops the round
before `homotopy.rs`, workers, or a 3/10/30 driver.

## 9. Wall, and why it is not a claim here

The machine this ran on has a polluted wall. Every wall number in the evidence
is inside a `wall` object, is excluded from every determinism comparison, and is
reported for orientation only. The claims in this document are in **work units**
(piece proposals, pair row probes, convex cell gap queries) and in **depth**
(millimetres of raw source depth, micrometres of `max_g`). The constructor
measured **2.32 s** here against the spec's ~1.4 s budget; that is a fact about
the box, not about the constructor, and no gate in this round depends on it.

The one place a wall number does enter a verdict is C175's "within two solver
seconds" clause, which the spec states in seconds. All three seeds finished
their 200,000-proposal quota in **1.26–1.44 s** of solver wall, so the clause
was satisfied and the cell still failed on its mechanism — the deadline is not
what killed it.
