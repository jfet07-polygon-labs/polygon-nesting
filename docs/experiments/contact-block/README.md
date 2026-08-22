# Active-contact block SE(2) — the last idea in the space, and why it does not pay

> Sol review 10 §3's third spend, the L. Research round: the matched-arm gate
> comes **before** any coordinator integration, and a clean negative with the
> decomposition is the deliverable. This is that negative, plus one retraction
> that had to be made along the way and is the more useful half of the round.

Branch `engine/topology-archive-search`, base `40852e6`. Feature
`contact-block-se2`, stacked on `se2-rigidity-certificate`, off by default and
reachable only through `POLYGON_NESTING_CONTACT_BLOCK` on the benchmark's
diagnostic door. Fixtures: mixed-61 exact-clearance, allowance `0.002`, the
twelve pinned from-request parents at 171.614–179.620 mm
(`drivers/parents12.json`, copied verbatim from
`docs/experiments/compression-schedule/evidence/parents.json`).

---

## 0. The verdict in one paragraph

The operator was built, it works, it is deterministic, it is sound, and **it
fails Sol's gate on both clauses**: the block ends shallower than the shipping
m34 continuation on **1 of 12** parents, and it buys **0.3%** as much depth per
unit of the cost both arms actually pay. The
decomposition is the third of Sol's three candidate explanations sharpened to a
mechanism: components are found on 99.8% of rounds and blocks are not
starved — **the joint step is refused by the canonical collision-envelope grid,
which the linearized program models only to first order at zero clearance.** The
median accepted step is **1% of the model's own vector** and the resulting
motion is sub-micron.

And the retraction: the first version of this operator validated its steps
against `validate_publication` — the *contract* half of the engine's acceptance
check, and the same half `se2_certificate`'s witness line search uses. At that
module's 0.025 mm trust radius the difference never shows. At this one's 0.5 mm
it shows on every parent. The wrong gate reported a **median 0.506 mm** that no
downstream operator could accept; the right gate reports **0.044 mm**. Both
numbers are in `evidence/`, the wrong one under a `-RETRACTED` name.

---

## 1. What was built

`crates/polygon-nesting-core/src/search/general_micro_legalization/contact_block.rs`,
a sibling of `se2_certificate` under the same parent module so it can reuse that
module's `Geometry`, `apply_se2`, the exact closest-approach witness pair and
the chord-error relaxation without any of it becoming crate-visible. The only
change to shipped code is visibility — `pub(super)` on the geometry helpers this
module reuses — plus one new `RoundedSum` accessor. No shipping arithmetic
moved, which §5 holds field by field.

The action, per Sol's five steps:

1. **Contact graph.** Per round, every pair within a band derived from the trust
   radius (`band_trust_multiple * trust`, default 2×) gets an edge carrying its
   own slack, `approach distance - contract`, on whichever gate is tighter.
2. **Component.** Breadth-first from the deepest piece, **tightest contact
   first**, to at most `max_block_pieces`. Deterministic tie-break on piece
   index, because two processes must walk the same component.
3. **Joint proposal.** Three variables per block piece and **none** per pinned
   piece — a five-piece block is a fifteen-variable program. `max delta` on the
   block's material depth rows, the rest of the rows holding. A row between a
   block piece and a pinned piece keeps the block piece's coefficients and drops
   the pinned one's; it is still a row, and dropping it would let the block walk
   into the layout.
4. **One action, validated exactly.** The model supplies the direction, the
   composite validator decides the length, and `scale = 0` is always available.
5. **Iterate.** Each round rebuilds the geometry and the graph from the layout
   the last round left — trust-region sequential convexification, with a
   contraction on refusal rather than an exit.

Three things in the repository look like this and are not it, and the module
head says why for each: m33 moves one piece and hopes; `global_legalize` is
translation-only; `se2_witness_proposal` solves the **whole layout** in one shot
at a radius small enough that the answer is the box. The restriction to a block
is what buys the radius and the re-linearization, and it is the only reason a
strictly smaller feasible set could beat its own relaxation.

One quantity had to be invented to make a null legible and it turned out to
matter: **`headroomMm`**, the published depth minus the deepest piece the block
does *not* contain. The publication measure is a maximum over 61 pieces, so this
is the most any motion of this block could buy whatever the program says.

---

## 2. The retraction: the operator was validating the wrong gate

The first pass reported a median **0.506 mm** across the twelve parents, 12/12
moved, in 175 ms of operator time. `evidence/blockprobe-contract-gate-RETRACTED.json`.

It was wrong, and the round trip that was supposed to catch it did not, for a
reason worth recording separately.

### 2.1 What the operator did

The line search validated each candidate scale with
`validation::general_polygon::validate_publication`. That is the **contract**
gate: transformed source outlines against the pair clearance and the sheet edge.
The engine's acceptance authority for a layout is
`general_fast::validate_and_measure_placements`, which is that check **plus**
canonical-collision-grid admissibility — each placement's collision envelope
inside the sheet and pairwise disjoint on the grid. `general_relaxed.rs:6413`
runs exactly that on any parent handed to mode 34 and refuses the whole run when
it fails.

At a 0.5–2 mm trust radius a block step routinely opens the source outlines'
clearance while pushing two collision envelopes into grid overlap. Every one of
the twelve first-pass outputs was refused:

```
compression schedule parent validation: pieces c5135087-…-copy-2 and
e4b6ebdf-…-copy-4 overlap on the canonical collision grid
compression schedule parent validation: piece 25cc62e4-…-copy-2 violates the
canonical-grid sheet boundary
```

`evidence/compose-contract-gate-RETRACTED.json`, `blockThenM34` on all twelve.

### 2.2 Why the round trip missed it, which is the methodological half

`drivers/roundtrip.py` wrote the operator's output back out as a pinned-parent
fixture, replayed it through mode 34, and read `exactValid`. It got `False`. It
then compared against a control — the untouched parents through the same
replay — which also got `False`, and concluded the operator's output was judged
exactly as its parent was.

The two `False`s were different events. The control's was
`persistent vacancy mode 34 final bound must be below the parent depth`: an
artifact of the driver handing it a target above its own depth. The operator's
was `compression schedule parent validation: … overlap on the canonical
collision grid`: the layout refused outright. One boolean, two causes, and the
control made the wrong one look normal.

This is the lesson `next-generation-engine-plan.md` already paid for once —
*"a perturb-relax experiment must state, and a reviewer must check, what the
incumbent depth field was set to and what actually bounded the relaxation"*. It
was reintroduced here in a new shape. The driver now asserts on `failureReason`
against the exact prefix `general_relaxed.rs:6413` writes, which is the only
field that distinguishes the two. What actually caught it was the composition
test in §6, which noticed that `blockThenM34` equalled `blockOnly` to the last
digit on all twelve seeds — a coincidence no real search produces.

### 2.3 The correction, and the counter that keeps it visible

`evaluate_scale` now validates with `validate_and_measure_placements`. Beside
it, `BlockRound::contract_only_accepts` counts the steps the contract gate would
have taken and the composite gate refused — the first version of the operator
being wrong, per round, in the data rather than only in a comment.

| gate the line search asked | median Δ over 12 parents | outputs the engine accepts as a parent |
|---|---:|---:|
| contract only (`validate_publication`) — **retracted** | **0.506 mm** | **0 of 12** |
| composite (`validate_and_measure_placements`) | **0.044 mm** | **11 of 11 produced** |

`se2_certificate::se2_witness_proposal` has the identical structure and is
**not** retracted by this: its trust radius is 0.025 mm, twenty times smaller,
and `docs/experiments/sparse-rotation/` §3.2 measured its final-depth effect at
0 of 12 anyway. But the defect is latent there, and any future caller that
widens that radius inherits it. That is recorded in the module head.

---

## 3. The gate

Sol's, verbatim: from-request parents in the 171–179 band, equal-work matched
arms, ≥2/3 seeds moved and net mm/work improvement.

**Arms.** Block: the diagnostic door on the pinned parent — no search runs, so
the whole process counter is the operator's. Control: `workgate.py`'s arm
verbatim, one serial mode-34 slice from the same pinned parent at
`past=1,rollback=0,work=W,lanes=1,pconfirm=0`, scored on the raw source depth of
the best exact-valid publication with the parent as the floor. Same binary, same
request, same allowance.

**The cost axis.** Three, because the arms do not spend work in the same shape
and any one alone can be argued with: the portfolio's own
`candidateQueries + 5 * exactPairTests`; whole-layout composite validations
(the block's `validations` against the slice's `confirmationsAttempted`, which
are calls to the same function on the same 61 pieces); and wall. One thing worth
carrying forward: under the *retracted* contract-only gate the block's
`processWorkUnits` read **exactly zero**, because `validate_publication` does
not reach `search::kernel::exact`. An in-search block operator capped by `work=`
would have spent seconds without spending budget and a naive equal-work gate
would have called that a win.

**The curve.** Twelve parents, `evidence/matched.json`. Median over seeds; the
depth column is the drop from the parent, so bigger is better.

| arm | median Δ | seeds moved | median work units | median composite validations | median wall | mm / 1000 validations |
|---|---:|---:|---:|---:|---:|---:|
| block `trust=0.5,block=5,rounds=64` | **0.0438 mm** | 11/12 | 0.32 M | **1919** | 1.18 s | **0.0138** |
| block `trust=1.0,block=5,rounds=4096` | 0.0361 mm | 11/12 | 0.67 M | 3124 | 2.18 s | 0.0050 |
| m34 @ 0.5 M | 0.0000 mm | 3/12 | 9.09 M | 44 | 2.57 s | 0.000 |
| m34 @ 1.5 M | 0.1679 mm | 9/12 | 9.33 M | 131 | 2.83 s | 1.633 |
| m34 @ 3.34 M (design slice) | **1.1044 mm** | 11/12 | 9.99 M | 287 | 3.46 s | **4.400** |

**The verdict**, paired per seed against the design slice
(`evidence/verdict-cheap.json`):

| clause | requirement | measured | |
|---|---|---:|---|
| ≥2/3 seeds moved | block strictly shallower than the control on ≥8 of 12 | **1 of 12** | ✗ |
| net mm/work improvement | paired ratio of mm-per-composite-validation > 1 | **0.0030** | ✗ |
| | | **GATE FAIL** | |

The one seed the block wins is seed 3, by **0.0005 mm**, and it wins only
because the control found nothing at all there — 2810 steps and **0
confirmations** in 4251 ms of slice. On the other eleven the control is
0.45–1.66 mm ahead.

**On the block's best axis it still loses.** Wall carries the control's 2–3 s of
process startup, so `drivers/slicetime.py` prices both operators alone — the
block's own `elapsedMs` against the slice's `repairMs + confirmationMs`. At
almost exactly matched operator time (1180 ms against 1125 ms) the block buys
**0.021 mm/s** and the slice buys **0.967 mm/s**, a paired ratio of **0.025**,
and the block is ahead on **0 of 12** seeds. `evidence/slicetime.json`.

**More budget does not rescue it and cannot.** The block's round budget is never
exhausted: with `rounds=4096` it exits on the trust-region contraction floor
after 7–371 rounds, and `rounds=4096` is *worse* than `rounds=64`
(0.0361 against 0.0438) because the extra rounds are spent contracting. The
control moves the other way: 0.000 → 0.168 → 1.104 mm as its cap goes 0.5 M →
1.5 M → 3.34 M.

---

## 4. The decomposition — which of Sol's three it is

`drivers/why.py` over the raw per-round documents,
`evidence/why.json`. Sol names three candidates and they are distinguishable.

| spec | rounds | components ≥2 | median block | `exact-rejected` | `no-depth-gain` | `moved` | full step survives | median scale |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `trust=0.5,block=5,rounds=64` | 481 | **99.8%** | 5 | **44.3%** | 21.6% | 33.9% | **24.8%** | **0.01** |
| `trust=1.0,block=5,rounds=4096` | 812 | 100% | 5 | **43.8%** | 27.2% | 28.9% | 28.6% | 0.01 |

* **"No components" is refuted.** A component of at least two pieces is found on
  99.8% of rounds, median size 5, median 5 contact edges, median 4–7 pieces
  inside the seed band. The contact graph is there and the walk finds it.
* **"Blocks rejected by exact" is the answer.** 44% of rounds cannot take any
  step at all; of the steps that are offered, the model's full-length vector
  survives the composite gate **one time in four**; and the median accepted
  scale is **0.01** — one percent of the model's own vector. The accepted
  motions are sub-micron: median `|dθ|` 3–7e-05°, median `|dx,dy|` 5–8e-05 mm,
  against a median model upper bound of 0.18–0.23 mm.
* **"Gains dominated" is the second-order effect.** 22–27% of rounds move the
  block a validated distance and the published depth does not fall, and 12–20%
  of the rounds that *do* move land exactly at their own `headroomMm`. Median
  headroom is 0.0036 mm.

The mechanism, stated once: **the binding constraint is the collision-envelope
grid, not the material contract, and the program models the envelope gate as a
first-order contact at zero clearance.** `EnvelopePair` rows carry
`contract_mm = 0.0` — envelopes only have to miss — so the linearization has no
margin to spend and systematically underestimates envelope penetration under a
joint rotation. The exact validator then cuts the step by two orders of
magnitude, and what is left is smaller than the 1 µm grid the publication
measure quantizes to.

That is a different statement from every prior negative in this space. It is not
"translation-only is insufficient" (this is SE(2)), not "one piece gets
dominated" (this is a component), and not "the schedule finds it anyway" (§6).
It is: the joint action's own linearization is written against the wrong gate,
and the right gate has no slack to linearize.

---

## 5. Flag-off, determinism, suites

**Flag-off bit reproduction.** The four pinned regression gates on both
binaries — `gate-base` (`jagua-experimental`) and `gate-cb`
(`+contact-block-se2`), no environment variable set. All four reproduce their
pinned depth and fingerprint on both: `206.869/8a7737381238fa4d`,
`159.09233022733062/fa01012af1d559ae`, `159.07876040364795/e28fba007f8031d4`,
`164.0375677990678/49f094d7e59a9008`. Whole-document comparison after stripping
the wall-clock fields, `drivers/flagoff.py`:

| gate | fields compared | fields differing |
|---|---:|---|
| g1 | 3265 | `executableSha256` |
| g2 | 3246 | `executableSha256` |
| g3 | 3246 | `executableSha256` |
| g4 | 3246 | `executableSha256` |

The one differing field is the binary's own hash, which must differ because the
binaries do. It is reported rather than stripped: a driver that filtered it
would be filtering the one difference it is certain of.
`evidence/flagoff.json`, `evidence/gates-base.json`, `evidence/gates-cb.json`.

**Determinism across two processes.** Twelve parents, two separate processes
each, same binary, same spec. The whole operator document — the round table,
every model bound, every validated delta and the moved placements — must be
byte-identical after the wall-clock fields are stripped. It is: **12 of 12**,
`ALL_IDENTICAL`. The placements are in the digest deliberately; a run that
produced the same depth from a different layout would pass a scalar comparison
and still be non-deterministic. `evidence/determinism.json`.

**Soundness, independently.** `drivers/roundtrip.py` writes each operator output
back out as a pinned-parent fixture and replays it through mode 34, so the
engine — not the operator — judges it. Of the twelve, eleven produce a proposal
at all (seed 4 produces none: every step it offers is refused). Of those eleven:
**0 refused as a parent**, **11 contract-valid**, and **11 whose depth the
engine's own independent re-derivation reproduces to the 1 µm grid**. Under the
retracted gate the same table read 12 proposals and **12 refused as parents**.
`evidence/roundtrip.json`.

**Reproduction from the committed tree.** This crate does not build
bit-reproducibly — two binaries from identical source have different SHA-256s —
so a hash comparison cannot establish that the evidence came from what is
committed. `drivers/reproduce.py` establishes it the only way that works: it
rebuilds and re-derives, comparing each seed's `finalDepthMm` against
`evidence/blockprobe.json` **exactly**, not to a tolerance. **12 of 12
identical**, to the last digit of the `f64`. `evidence/reproduce.json`.

**Unit tests.** Four in the module: the block is seeded on the deeper piece and
the stacked pair is found as one component; the reported delta is the one the
exact validator signed and re-measuring the returned placements reproduces it;
two calls on the same input are the same document; and a block of one is refused
by the settings.

**Suites.** Three, with the exit status captured directly rather than through a
pipe — `cargo test … | tee log` reports `tee`'s status, which is how a red suite
gets written up as green. Suites 1 and 2 are the protocol's two unchanged; suite
3 is the combo plus `se2-rigidity-certificate,contact-block-se2`, which is the
only build in which this round's code exists at all — without it the protocol's
combo compiles none of it.

```
== suite 1: jagua-experimental              suite-jagua exit=0   1293 passed
== suite 2: the protocol's full combo       suite-combo exit=0   1356 passed
== suite 3: the combo plus this round       suite-block exit=0   1377 passed
SUITE_EXITS 0 0 0
```

Zero failures in any of the three; the 21-test difference between suites 2 and 3
is this round's module plus the certificate's.

`evidence/suite-jagua.log`, `evidence/suite-combo.log`,
`evidence/suite-block.log`. The campaign's known-flaky
`layout_scorer::tests::free_material_multi_eviction_shrinks_retained_container_capacity`
passed first time in all three; no rerun was needed.

---

## 6. Does it compose with the operator it loses to?

Sol's gate is head to head, and the answer to it is no. A reader will still ask
the second question, so it was measured: `drivers/compose.py`, three arms from
the same parent, the slice at the design cap in both, the composed arm starting
from the layout the block left (the fixture the engine has already accepted in
§5). `evidence/compose.json`.

| | median depth | seeds shallower | median process work |
|---|---:|---:|---:|
| `m34` alone | 176.061 mm | 3 of 11 | 9.99 M |
| `block` then `m34` | 176.0605 mm | **8 of 11** | 9.84 M + the block's ~0.3 M |

Median paired difference **−0.030 mm** in the composition's favour, at about 3%
more total work. It is a real effect and it is 0.030 mm — smaller than the
operator's own median contribution, and the spread runs from −0.395 mm (seed 8)
to **+0.659 mm** (seed 9, where the slice stalls completely on the block's
output and publishes it unchanged). This is not a gate pass by any reading, and
it is not offered as one; it is recorded because it is the only axis on which
the operator has a measurable positive sign, and because the seed-9 stall is a
warning about what a block move does to the operator that runs after it.

---

## 7. The operator's own parameter space is closed

Three parents spanning the band, five settings, `evidence/sweep.json`. Every
knob the operator has was moved and none of them changes the order of magnitude.

| spec | seed 0 | seed 4 | seed 6 |
|---|---:|---:|---:|
| `trust=0.25, block=5, iters=256` | 0.00047 | 0.00000 | 0.1071 |
| `trust=1.0,  block=5, iters=256` | 0.00052 | 0.00000 | 0.0908 |
| `trust=2.0,  block=5, iters=256` | 0.00159 | 0.00000 | 0.0982 |
| `trust=1.0,  block=3, iters=256` | 0.00165 | 0.00000 | 0.0389 |
| `trust=1.0,  block=14, iters=256` | 0.00047 | 0.00000 | 0.1121 |
| `trust=1.0,  block=5, iters=2000` | 0.00047 | 0.00000 | 0.0623 |

Three things worth saying about this table:

* **Seed 4 is 0.000 mm on every setting**, with 100% `exact-rejected`. There is a
  component, there is a model bound of 0.025–0.159 mm, and not one step of any
  length in any direction survives the composite gate.
* **Bigger blocks do not help.** `block=14` is indistinguishable from `block=5`,
  and on seed 6 the walk returns a component of 10 even when allowed 14 or 24 —
  the near-binding contact graph is simply that size.
* **Solving the model better makes it worse.** Eight times the iterations
  (`iters=2000`) takes seed 6 from 0.091 to 0.062 mm. The better the
  linearization is optimized, the further outside the true envelope constraint
  its optimum sits, and the harder the exact validator cuts it back. That is
  §4's mechanism showing up as a monotone trend rather than as an average.

---

## 8. What this closes, and what it does not

**Closed.** The active-contact block SE(2) action, as Sol specified it, on
from-request parents in the 171–179 band. It is cheap, sound, deterministic, its
outputs are acceptable parents, and it does not pay. Both reviewers agreed this
was the last idea in the current operator space; its honest failure completes
the map, and the map's last entry is *the canonical collision-envelope grid is
what pins the front, and no first-order model of it has slack to spend*.

**Not closed, and deliberately not attempted here.** A block program whose
envelope rows carry a real margin — that is, one that asks the envelopes to
separate by some `epsilon > 0` rather than merely to miss — would have slack to
linearize and might take steps the composite gate accepts. That is a different
program from the one Sol specified and it is not what this round was funded for;
it is written down here because §4's mechanism points straight at it and a
future reader should not have to re-derive it.

**Retracted from this round's own first pass.** The 0.506 mm median, the "12/12
seeds moved", and the round trip's "judged exactly as its parent is". Named,
kept in `evidence/*-RETRACTED.json`, and explained in §2.

---

## 9. Reproduce

Every number above is reproducible from the committed tree. `ROOT` in the
drivers points at this worktree.

```
D=docs/experiments/contact-block/drivers

bash $D/build.sh all                        # gate-base, gate-cb, meas, meas-base

python3 $D/gates.py base /var/lib/t3/tmp/cblock/bin/gate-base
python3 $D/gates.py cb   /var/lib/t3/tmp/cblock/bin/gate-cb
python3 $D/flagoff.py /var/lib/t3/tmp/cblock/gates/base \
    /var/lib/t3/tmp/cblock/gates/cb base cb <out>/flagoff.json

M=/var/lib/t3/tmp/cblock/bin/meas
P=$D/parents12.json
S='trust=0.5;iters=256;block=5;rounds=64;seeds=3;band=2'

python3 $D/blockprobe.py   <out>/probe12 $M $P "$S,trust=1.0;iters=256;block=5;rounds=4096;seeds=8;band=4"
python3 $D/why.py          <out>/probe12 <out>/probe12/blockprobe.json <out>/why.json
python3 $D/matched.py      <out>/matched $M $P "$S" 500000,1500000,3341379
python3 $D/verdict.py      <out>/matched/matched.json "block:${S//;/,}" m34:3341379 <out>/verdict.json
python3 $D/slicetime.py    <out>/matched <out>/matched/matched.json "block:${S//;/,}" 3341379
python3 $D/roundtrip.py    <out>/roundtrip $M $P "$S"
python3 $D/replaycontrol.py <out>/replaycontrol $M $P
python3 $D/compose.py      <out>/compose $M $P <out>/roundtrip 3341379
python3 $D/determinism.py  <out>/determinism $M $P "$S"

bash $D/run-suites.sh
python3 $D/reproduce.py $M docs/experiments/contact-block/evidence/blockprobe.json "$S"
bash $D/collect.sh
```

`run-all.sh` runs the whole corrected chain in one command; the individual
invocations above are what it does.

Binaries and their SHA-256 are in `evidence/binaries.txt`, and every evidence
document carries its own `binarySha256`. Those hashes are provenance, not proof:
see §5's reproduction note for why the depths, and not the hashes, are what
establishes that this evidence came from the committed tree.
