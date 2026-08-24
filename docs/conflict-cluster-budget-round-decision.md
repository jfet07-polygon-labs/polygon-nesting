# Sparrow-informed conflict-cluster budget round — decision record

## Status

**Mechanism and experimental law selected by a 3/3 quorum. Implementation is
not yet licensed.** This document freezes what the round may change and how it
will be judged. It is deliberately a decision record rather than an
implementation specification: the independently authored disc construction,
mass equation, zero-mass fallback, and placebo permutation still require an
exact 3/3 pre-commit before code is written.

No treatment scout or implementation exists at this point.

## Frozen consultation corpus

- Paper: *An open-source heuristic to reboot 2D nesting research*, 29 pages,
  SHA-256 `8452eef76ad9fd77734a05bbe0e423f9cb30a5145b0640ebccc77512712c81df`.
- Sparrow source: clean commit
  `14f4868fcd7e97036700dbebaf193fb159180aa9`, all 36 Rust files under `src`,
  3,348 conventional lines.
- Current campaign authority: deterministic 30-second round at `804258a`, its
  committed evidence, current CutCloseRelocate source, and the constructor and
  scalar negative ledgers.

Sol, Grok, and ox-alpha each attest that they read the entire paper and all 36
source files. The common brief originally said 35; the three corrected passes
supersede that count.

## What Sparrow actually demonstrates at ten seconds

On the preserved same-machine mixed-61 run, Sparrow's LBF start is 214.027 mm,
about 31 mm worse than our 182.976 mm constructor. Sparrow reaches 150.796 mm
during eight seconds of repeated shrink-and-separate, then compresses to
150.165 mm. Its advantage is not bottom-left construction. It is the ability
to spend cheap separation work effectively across a long chain of legal
contractions.

This observation killed the proposed bottom-left revival rather than
supporting it. The local constructor family is indexed and closed in
[`experiments/persistent-vacancy-descent/constructed-basin/README.md`](experiments/persistent-vacancy-descent/constructed-basin/README.md).

## Prior negatives that bind this decision

The new mechanism may not be described as the first use of poles or a second
field in this repository. Prior engines already measured triangle-incenter
poles, whole-polygon coverage poles, a convex-hull shape-factor substitution,
exact-area finalist reranking, 50/25 sampling, aggressive GLS, boundary GLS,
and weighted reduction. The exact-area reranker alone observed 960 inversions,
changed 1,351 selected finalists, and regressed to 182.010 mm.

Therefore a multi-disc field in local sample ranking, acceptance, GLS, or
refinement is a closed scalar recombination. The only funded novelty is the
field's role in ex-ante allocation across conflict topology while the local
solver remains untouched.

## The selected mechanism

At the start of every worker outer iteration:

1. Freeze the authoritative positive-pair conflict graph and the entry
   colliding-piece set. Vertices are input pieces; a positive signed-gap pair
   row creates an edge. A piece with boundary violations but no positive pair
   row is a singleton component.
2. Let `Q` be the cardinality of the entry colliding-piece set.
3. Compute one independently authored multi-disc mass for each frozen
   component.
4. Allocate exactly `Q` integer **atomic relocate slots** across the components
   by stable largest remainder, with stable component-ID ties.
5. Consume each component's allocation through a counter-keyed stable member
   permutation/cycle frozen before any slot runs.

One slot is one complete attempted piece visit. If the piece remains
colliding, the current relocate completes its entire fixed sample pool and
adaptive coarse/fine coordinate descent. If the piece is already clear, the
current zero-energy skip consumes the slot. There is no mid-relocate or
per-component evaluation deadline. Skipped or unused slots are not transferred
after outcomes are visible. New and cross-component conflicts wait for the
next outer iteration.

The treatment and both partition controls have the same `Q`, graph/scheduler
skeleton, and quota accounting. Actual sample evaluations and charge deltas
may differ because the trajectories differ; they are emitted in full and
charged only at the existing worker-sweep barrier by the unchanged pacer.

The sole licensed intervention is component membership, mass, allocation, and
the resulting component member schedule. Any treatment effect on sample or
pair ranking, relocate acceptance, GLS, coordinate descent/refinement, blocker
selection, constructor behavior, raw Phi, `max_g`, exact-clear
classification, or publication is `AUTOFAIL`.

## Gate 0 — mechanism existence and isolation

Every clause is mandatory:

1. Feature-off output, poses, fingerprints, and counters are byte-identical to
   the frozen member.
2. A synthetic multi-component corpus has an exact expected mass allocation
   that inverts the max-violation allocation.
3. Across all eligible shadow decisions from fixed seeds 0 through 8, at least
   one integer mass-allocation vector differs from the max-violation vector.
   The complete disagreement rate and Spearman series are reported, never
   gated on a selected subset of seeds.
4. Total atomic slot quotas and charge-accounting rules agree; authority,
   work, and charge identities reconcile. No claim of equal downstream
   sample-evaluation counts is made.
5. At fixed slots on a compute-ignore corpus, the added mass/allocation work
   reduces completed slots per second by no more than 5%.
6. Seed 0 is bit-identical across two processes at the same fixed slot budget.

Any miss stops the round before a quality battery. There is no engagement
threshold selected after observing treatment quality.

## Thirty-second primary battery

The four arms are frozen before execution:

| arm | behavior | role |
|---|---|---|
| A | current composed member | contemporaneous control |
| B | multi-disc mass partition | gate owner |
| C | deterministic shuffled-mass partition | same-cost placebo |
| D | max-violation partition | scalar-allocation control |

Arms B/C/D share the exact graph, allocation, scheduling, and total-slot
skeleton. No arm may become an alternative winner after results are visible.

PASS requires every clause:

1. B median raw-source depth is at most **163.00461 mm**, the inherited signed
   primary median law.
2. At least **7/9** B seeds are at or below **168.484 mm**.
3. Paired median `A - B` is at least **1.000 mm**.
4. Paired medians `C - B` and `D - B` are each at least **1.000 mm**. This
   reuses the standing material-effect bar; there is no new quality `kappa`.
5. Every publication is Exclusive `r=2.500`, contract-valid, and independently
   revalidated; authority, plan, and charge identities are green.

There is **no 30-second p95 clause**. Wall p95, maximum, and overshoots are
reported. Ten and sixty seconds are report-only for this mechanism.

A valid primary miss closes the mass-partition mechanism without retuning,
another arm, or a rescue run. If B passes against A but misses either placebo
attribution clause, the mass field is real but not causally material and is
closed; a simpler partition arm may only be considered under a new spec.

## Before implementation can begin

The next exact specification must freeze, and the same quorum must sign:

- the source-ring disc construction and fixed-order representation;
- the mass equation and its units, including boundary-only components;
- zero/non-finite mass behavior and largest-remainder arithmetic;
- the shuffled-mass permutation and max-violation control definitions;
- the component/member stable IDs and counter-stream keys;
- the fixed-slot Gate-0 corpus, ceilings, emitted census, and battery driver;
- a source-provenance audit proving that no Sparrow/Jagua code, dependency,
  data layout, constants, or formula was copied or linked.

Because all reviewers read Sparrow source, this is called an **independently
authored implementation boundary**, not a literal legal clean room.

## Quorum record

- **Sol:** `CONFIRM corrected exact text`; found and caused the atomic-slot
  correction.
- **Grok:** `CONFIRM corrected exact text`; final Grok 4.5 session `76553`,
  exit 0.
- **ox-alpha:** `CONFIRM corrected exact text`; retained full paper/source and
  campaign ledgers despite intermittent provider failures.
- **Owner:** full quorum retained; no 2/3 shortcut.

Individual records:

- [`sol-review-20-sparrow-informed-next-round.md`](sol-review-20-sparrow-informed-next-round.md)
- [`grok-review-15-sparrow-informed-next-round.md`](grok-review-15-sparrow-informed-next-round.md)
- [`ox-alpha-review-2-sparrow-informed-next-round.md`](ox-alpha-review-2-sparrow-informed-next-round.md)
