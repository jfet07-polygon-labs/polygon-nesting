# Gate 0's STOP, verified independently

**Verdict: the STOP is CONFIRMED.** Gate 0 fails three of its six fatal cells —
**S1**, **C175**, **triangle-20** — and this round reproduced all three from the
previous round's own committed drivers, on a build made from nothing in a second
worktree. Round 1 was not run and `homotopy.rs` is still the stub it was.

That is not a discretionary choice. This round's instruction was conditional:
*"IF ITS REPORT SAYS STOP ON A FATAL GATE-0 CELL: do not build the schedule —
your whole task becomes verifying its failure evidence independently (re-run the
failing cell from its committed drivers on a fresh build) and reporting the
confirmed STOP."* The report said STOP; docs/overlap-ics-converged-spec.md
§"Round structure of record" makes S1, C175 and the triangle canary fatal; so
the strip homotopy, the eight-epoch schedule, the nine-seed 3/10/30 curve and
every Round-1 clause are **not** in this round.

---

## 1. What "independent" means here, and what it does not

The reproduction shares with the run it checks: the committed source, the
committed drivers, the pinned toolchain (`rustc 1.97.1 (8bab26f4f 2026-07-14)`),
and the box.

It shares nothing else. A **different worktree**, a `target/` directory that did
not exist when this round began, a separately compiled binary
(`3e53e1f194ca8e3d…` here against `68fa7cf0…` and `e7eebee9…` there), and a
different absolute path in every document.

What it therefore **cannot** claim: that the failures are not a property of this
box, this toolchain or this x86 target. Nothing here is a cross-platform claim,
and this campaign has never made one.

The path difference is the reason the comparison is `drivers/docdiff.py` and not
`lib.digest`. The evidence documents embed `request.path`, `poses.path` and
`binary`, so one hash over the whole document reports "different" for two runs
that agree on every number. `docdiff.py` walks both documents to their scalar
leaves and neutralises exactly three things, by name: the `wall` object, the
`executableSha256` field, and any leaf whose key ends in `path`, `binary` or
`root`. Everything else must be equal, and it prints the ones that are not.

---

## 2. The re-run, cell by cell

`ICS_ROOT` repointed at this worktree; otherwise `drivers/cells.py` unchanged.

| cell | class | verdict here | the number that decides it | vs the committed document |
|---|---|:--:|---|:--:|
| **S0** | fatal | **PASS** | 61 placements, `rawSourceDepthMm` **150.16451**, `phi.to_bits()` **0**, Exclusive accepts at `two_r = 5000`, contract accepts, **0** repair rows, giveback **0.0** | **every field identical** |
| **S1** | fatal | **FAIL** | Φ **433.4919406020829** → **0.0004482304876309415**; `max_g` **0.012634958179553735** mm against a 4 µm band; depth **150.1729903315535** against a locked **150.16547**; **0** publication attempts | **every field identical** |
| **C175** | fatal | **FAIL** | **0 of 3** seeds return a strict dual-valid non-constructor child; Φ 461.35/462.20/467.05 → **40.152/38.587/37.989**; `max_g` **2.1032/2.2435/2.1631** mm; **0** attempts on every seed | **every field identical** |
| **triangle-20** | fatal | **FAIL** | Φ **22.21647333933925** → **0.049891136703974694**; **0** active pair rows, 5 active edge rows, `max_g` **0.11764791331265201** mm; depth **70.60227107358449** against a locked **70.742**; **0** attempts | **every field identical** |
| **numeric soundness** (1,000) | fatal | **PASS** | 0 outside the 4 µm band (worst **0 µm**), 0 containment false-feasible on 60 containment states, 0 incremental mismatches, 501/501 force on the scored population | 219 of 220 fields identical; the one difference is `.wallSeconds` |
| **throughput** | fatal | **PASS** | cold Φ **36.81 µs** (≤200), row rebuild **1.244 µs** (≤20), **7.52 M** cell gaps/s (≥1 M), **1,061,224** proposals projected into 8 s (≥100 K) | wall-derived, so different by construction: 8–9 % faster here, same four verdicts |

`GATE0_PASS: false`, `fatalFailures: ["S1", "C175", "triangle-20"]` — the same
three cells, by name, that the previous round reported.

### The document comparison, in full

| document | scalar leaves compared | path leaves ignored | leaves that differ |
|---|---:|---:|---:|
| `cell-s0.json` | 94 | 2 | **0** |
| `cell-s1.json` | 80 | 2 | **0** |
| `cell-triangle20.json` | 84 | 1 | **0** |
| `cell-c175-seed0.json` | 84 | 1 | **0** |
| `cell-c175-seed1.json` | 84 | 1 | **0** |
| `cell-c175-seed2.json` | 84 | 1 | **0** |
| `corpus-gate-1000.json` | 220 | 1 | **1** (`.wallSeconds`) |

Every `f64` in every failing cell agrees to its last bit — Φ in and out, `max_g`,
the depth, the ten work counters, the pose digests, the census. The only
non-path leaf that moved in the whole set is one wall reading that the corpus
driver writes at the top level of its own document rather than inside `wall`.

### The FAST tier

`drivers/fast.sh` on the fresh build: **EXIT=1**, and the red stage is the same
one.

| stage | exit |
|---|---:|
| default-build compile check (`--no-default-features --lib`) | 0 |
| `cargo tree --features overlap-ics` | 0 |
| dependency hygiene: `jagua-rs` **ABSENT** | 0 |
| module unit vectors | 0 |
| `validation_vectors::sat_penetration_matches_ts_oracle` | 0 |
| `canonical_grid_vectors` | 0 |
| `collision_builder_vectors` | 0 |
| release example build | 0 |
| 1,000-state contact corpus | 0 |
| **two-process fixed-work smoke** | **1** |

`INVARIANTS_PASS: true`, `SMOKE_PASS: false`. Both smoke cells are
**bit-identical across two processes** on this build too
(`3e324229f7b1832b…` for S0, `b9b7511333cba3af…` for S1), and every scalar in
`s0.pins` and `s1.measured` equals the committed one.

---

## 3. Three things the re-run adds, none of which rescues the round

A reproduction that only reproduces has verified determinism, not a verdict.
Three questions were left open by the previous round's evidence, and all three
are answered here from its own instrument, with no change to any crate source.

### 3.1 The exact authorities agree with Φ — asked directly

At the shipped 4 µm band all three fatal cells make **zero** publication
attempts, so the round kernel and the untouched contract validator never see
their states. The only witness against those states was Φ itself, and Φ is the
thing under suspicion.

`drivers/band-probe.sh` puts them in front of the authorities, using the one
committed knob built for it — `--band`, whose own doc comment says it exists "so
a failing cell can be asked *which half* failed". At a 0.2 mm band:

| cell | attempts | what the exact authorities said |
|---|---:|---|
| **triangle-20** | **3** | all three **refused**: `kernelExclusiveValid false`, `contractValid false`, `repairRows 0` |
| S1 | 0 | the band was never the only gate: `proxy_depth` **150.17299** > `T` 150.16547 |
| C175 seed 0 | 0 | same: `proxy_depth` **177.44027** > `T` 176.26227 |

The three triangle-20 attempts are at proposal ordinals 140, 160 and 200, at
proxy depths **70.66182**, **70.60020** and **70.59775** — all three *inside* the
locked 70.742, so the strip gate passed and the kernel is genuinely the thing
that refused. It refused on the **first** scan, before a single repair row ran
(`repairRows: 0`), with `refusal: "a failing row is outside the 4 µm band or has
no sheet slack; discarding the checkpoint"`.

So Φ is not inventing the residual: an authority that shares no code with it
refuses the same layouts. The triangle canary's failure is geometry, not a proxy
artefact. And no probe run enlarged a strip or moved a piece — `repairRows` is 0
in every attempt, so the "target never expanded by repair" invariant survives the
diagnosis as well as the gate.

### 3.2 The residual is not an artefact of the strip target — proved from the numbers

The strip `T` is an objective device. The round kernel does not model it; the
kernel's boundaries are the **sheet's**. So a residual that lived entirely in the
strip's own top row would be invisible to the exact authorities and the fatal
verdicts would be reading a constraint the product does not have.

`drivers/residual_split.py` bounds that row exactly, from numbers the documents
already print and with no new run. Two conventions meet in this engine and they
are not the same number:

* `state::raw_source_depth_mm` and the `proxy_depth <= T` publication gate use
  `Contract::sheet_edge_clearance_mm`, the settings field;
* `broad_phase::boundary_residuals` uses `Contract::edge_clearance_mm()`, which
  is that field **plus the flattening sag tolerance** `s`.

Writing `E = edge_clearance_mm()`, the deepest material point is
`y_max = depth − (E − s)` and the top row's threshold is `T − E`, so

    max top-row residual  =  depth − T + s

and it is a maximum over pieces, because `raw_source_depth_mm` maximises over the
same ring points the per-piece boxes are built from.

| cell | `T` | final depth | `s` | max top-row residual | `max_g` (edge) | a **sheet** row is violated |
|---|---:|---:|---:|---:|---:|:--:|
| triangle-20 | 70.742 | 70.60227 | 0.25 | **0.11027** | 0.11765 | **yes** |
| S1 | 150.16547 | 150.17299 | 0.0 | **0.00752** | 0.01263 | **yes** |
| C175 s0 | 176.26227 | 177.44027 | 0.0 | **1.17800** | 2.10319 | **yes** |
| C175 s1 | 176.26227 | 177.56853 | 0.0 | **1.30626** | 2.24348 | **yes** |
| C175 s2 | 176.26227 | 177.53988 | 0.0 | **1.27761** | 2.16307 | **yes** |

In every failing cell `max_g` **exceeds** the largest residual the strip's own
row can carry, so at least one violated boundary row is a left, right or bottom
row of the **sheet** — a row the kernel's `boundary_admissible` scan does see,
which is why §3.1's refusal happened. The fatal verdicts are not reading a
phantom constraint.

### 3.3 The stall is a fixed point, not a short quota

The fatal cells run at 200,000 piece proposals — `cells.py`'s work quota for two
solver seconds at the measured rate. If ten times that moved anything, the
verdict would be "the budget is short", which is a schedule question and not a
kill. `drivers/budget-probe.sh` asks it, changing only `--budget`:

| cell | quota | `max_g` (mm) | raw Φ | **accepted moves** | sweeps | stalled sweeps | max guided penalty |
|---|---:|---:|---:|---:|---:|---:|---:|
| S1 | 200 K | 0.012634958179553735 | 0.0004482304876309415 | **1,044** | 3,277 | 3,040 | 923 |
| S1 | **2 M** | **0.012634958179553735** | **0.0004482304876309415** | **1,044** | 32,786 | 32,549 | 9,885 |
| triangle-20 | 200 K | 0.11764791331265201 | 0.049891136703974694 | **175** | 9,996 | 9,976 | 2,545 |
| triangle-20 | **2 M** | **0.11764791331265201** | **0.049891136703974694** | **175** | 99,996 | 99,976 | 25,500 |

The accepted-move counter is **identical**, not merely similar. In 1.8 million
further proposals — 29,509 more sweeps on S1, 90,000 more on triangle-20 — the
solver accepted **not one move**, while the guided weights it raises to escape
grew by 10.7× and 10.0×. Both states are exact fixed points of the specified move
set under its own escape mechanism, and the guided local search provably cannot
dislodge them.

The previous round asserted this for S1 ("unchanged at 2 M proposals"). It holds
on the triangle canary too, and the identical accepted-move counters are a
stronger form of it than equal `max_g`: equal `max_g` is consistent with a
trajectory that keeps moving and keeps arriving back; equal accepted moves is
not.

### 3.4 The asymmetry that fell out of §3.2's derivation

On mixed-61's exact-clearance contract `s = 0`, and the two conventions coincide.
On triangle-20 `s = 0.25 mm`, and they do not: **Φ's strip boundary is one sag
tolerance stricter than the publication depth gate it is descending toward.** A
triangle-20 layout can satisfy `proxy_depth ≤ T` with 140 µm to spare and still
be charged up to 0.25 mm of top-row violation by Φ.

That is exactly what the canary's final state does — `publishedDepthSlackMm`
**+0.13973** and `maxTopRowResidualMm` **0.11027** at the same instant — and it
reconciles the previous round's "140 µm of unused strip" with a top row that is
simultaneously violated. Neither the README nor the code comments on either side
of the seam mention the mismatch, and it is recorded here as a fact about the
instrument.

**It does not change any verdict.** At the observed final state the maximum is
already carried by a sheet row (0.11765 > 0.11027), so relaxing the top row by
one sag tolerance would leave `max_g` where it is — 29× the 4 µm band. It is
raised because a locked-strip cell that is a quarter of a millimetre harder than
its own publication gate is worth knowing about before the next round designs a
schedule around locked strips.

---

## 4. What was verified, what was reproduced, and what was neither

**Verified** — a claim checked against something other than a second run of the
same code: the S0 pin (two independent authorities accept it); the exact
authorities' refusal of triangle-20's states (§3.1); the presence of a violated
sheet boundary row in all three failing cells (§3.2); the stall as a fixed point
of the move set at ten times the quota (§3.3); the publication gates in
`publish::attempt` read from source and matched against the observed counters —
band, `proxy_depth ≤ T`, and `proxy_depth ≤ incumbent − 1 µm`, which together
explain every "0 publication attempts" without residue.

**Reproduced** — bit-for-bit, which establishes determinism and the absence of
build- or directory-dependence, but is not independent evidence about geometry:
every number in §2.

**Neither.** The previous round's central structural sentence — *"a single global
translation would legalize it"* — is **consistent with** everything measured
here and is **not demonstrated by** it. What the evidence shows is that
triangle-20 ends with zero pair violations and its whole residual in boundary
rows; that at least one of those rows is a sheet row; and that
`homotopy::compressed` moves centroids along the long axis only and toward the
floor, a mechanism that manufactures bottom-edge violations. The documents do not
decompose the five active rows by side, so "one translation suffices" is a
reading, not a measurement. If two of the five rows were on opposite sides, no
single translation exists. Deciding it needs a per-edge census the instrument
does not currently emit, and adding one is next round's business, not a
verification round's.

Also not done, and named so nobody has to infer it: no 10,000-state corpus (it is
diagnostic and passed there), no S2/C168/random-T diagnostics, no basin sweep, no
jump A/B, no rotation-frozen probe. All of those are the previous round's
evidence and none of them carries a fatal verdict.

---

## 5. The round-boundary battery

Run from the clean committed tree, on binaries built from it in this worktree.

| binary | features | sha256 |
|---|---|---|
| `base` | `jagua-experimental` (overlap-ics **absent**) | `61befdc544b4135a…` |
| `meas` | `jagua-experimental,overlap-ics` (**compiled**, unarmed) | `8ab2d882731a4d36…` |
| `ics-a` | `overlap-ics`, worktree `target/` | `3e53e1f194ca8e3d…` |
| `ics-b` | `overlap-ics`, freshly deleted separate `CARGO_TARGET_DIR` | `3e53e1f194ca8e3d…` |

### 5.1 The four pinned gates, on both binaries

`BASE_ALL_PASS: true`, `MEAS_ALL_PASS: true`, **`WHOLE_DOCUMENT_IDENTITY: true`**.

| gate | pinned | base | meas | documents identical |
|---|---|:--:|:--:|:--:|
| g1 | 206.869 / `8a7737381238fa4d` | ✅ | ✅ | ✅ |
| g2 | 159.09233022733062 / `fa01012af1d559ae` | ✅ | ✅ | ✅ |
| g3 | 159.07876040364795 / `e28fba007f8031d4` | ✅ | ✅ | ✅ |
| g4 | 164.0375677990678 / `49f094d7e59a9008` | ✅ | ✅ | ✅ |

g2, g3 and g4 also report `exactValid: true` and `contractValid: true` on the
pinned population. The document digests here are not comparable with the previous
round's — `gatelib.VOLATILE` strips the binary hash and the worktree identity but
not the absolute paths inside the documents — so what is claimed is the protocol's
own claim: base and meas identical, both hitting the pins.

### 5.2 The suites

All `--release`, every exit status read directly on the line after its command,
no pipelines. `SUITES_PASS: true`; no suite tripped the campaign's known
allocator flake, so no rerun clause fired.

| # | features | targets | passed | failed | ignored | exit |
|---|---|---:|---:|---:|---:|---:|
| 1 | `jagua-experimental` | 55 | 1293 | 0 | 2 | **0** |
| 2 | the protocol's full combo | 55 | 1357 | 0 | 2 | **0** |
| 3 | `jagua-experimental`, `--example general_request_benchmark` | 1 | 20 | 0 | 0 | **0** |
| 4 | `jagua-experimental,overlap-ics` | 55 | 1340 | 0 | 2 | **0** |
| 5 | `overlap-ics` alone, `--lib --tests` | 50 | 1150 | 0 | 0 | **0** |

The same five totals as the previous round, which is the expected result: this
round changed no crate source at all.

`run-suites.sh` writes its logs into `../evidence/`, which is the previous
round's committed record. This round copied its five logs into
`evidence/suite-*.log` here and restored theirs with `git checkout`, so both
rounds' logs exist and neither overwrote the other.

### 5.3 Determinism, in both forms

| comparison | cells | verdict |
|---|---|:--:|
| two processes, one binary | S0, S1 | **bit-identical** (`INVARIANTS_PASS: true`, `SMOKE_PASS: false`) |
| two builds, separate target directories | S0, S1, C175, triangle-20 | **bit-identical**, `TWO_BINARY_IDENTICAL: true` |

**The same honest deflation the previous round recorded still applies, and this
round sharpens it.** `ics-a` and `ics-b` came out byte-identical again, so the
two-binary cell reduces to a second two-process cell. But the two binaries built
in *different worktrees* are **not** identical (`3e53e1f1…` here against
`e7eebee9…` there) from the same source and toolchain — so the release build of
this example is reproducible across target directories and **not** across
worktree paths. That is a fact about what is baked into the binary, and the
practical consequence is that on this toolchain a genuinely non-degenerate
"two independently built binaries" cell needs two directories, not two builds.

For the same reason **none of this round's document digests is comparable with
the previous round's**: every one of them is taken over a document that contains
its own absolute paths. Cross-round agreement is established field by field in
§2, which is the comparison that survives the move; the digests here establish
agreement *within* this round only.

---

## 6. Reproduction

```sh
bash docs/experiments/overlap-ics/gate0-verification/drivers/verify.sh
bash docs/experiments/overlap-ics/gate0-verification/drivers/band-probe.sh
bash docs/experiments/overlap-ics/gate0-verification/drivers/budget-probe.sh
python3 docs/experiments/overlap-ics/gate0-verification/drivers/residual_split.py \
    docs/experiments/overlap-ics/evidence/cell-triangle20.json \
    docs/experiments/overlap-ics/evidence/cell-s1.json \
    docs/experiments/overlap-ics/evidence/cell-c175-seed0.json
```

`verify.sh` runs the previous round's `fast.sh` and `cells.py` unchanged, then
compares six documents leaf by leaf. It exits non-zero if any comparison fails;
`fast.sh`'s own exit of 1 is expected and is reported rather than counted, because
that red stage **is** the STOP.

Do not pipe any of them into `tee` or `tail`.

## 7. Chinese wall

`cargo tree -p polygon-nesting-core --features overlap-ics -e features` contains
no `jagua-rs`, checked by `fast.sh` on this build. No Sparrow or jagua optimizer
source was read in this round. The Sparrow pose fixture was read only by the S0,
S1 and band-probe cells, all of which take it as the correctness pin; it was
never a seed and never a parameter source, and this round changed no constant in
`search::overlap_ics` at all — the module is byte-for-byte the previous round's.
