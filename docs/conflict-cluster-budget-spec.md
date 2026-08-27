# Conflict-cluster budget round — exact preimplementation specification

## Status and authority

This is the same-text ballot draft required by
[`conflict-cluster-budget-round-decision.md`](conflict-cluster-budget-round-decision.md).
It licenses no treatment scout and no solver implementation until Sol, Grok,
and ox-alpha have each confirmed this exact file digest.

The frozen code member is commit `a6e5d1b13b14b3b776d48d7f3298af5980fb762d`.
The only source-level correction made while drafting this text is the decision
record's authority predicate: an edge exists when
`PairRow.violation_mm > 0.0`. Literal positivity of
`Contact.signed_gap_mm` denotes separated material and is `AUTOFAIL`.

## 1. Independently authored source discs

The implementation derives discs only from the current local
`PieceSource.decomposition`. It does not triangulate again.

For each piece in input-index order and each existing `Decomposition::cells`
entry in cell order:

1. Let `o` be the existing `decomposition::centroid(cell_points)`.
2. For every stored directed edge `(a,b)` in cell order, compute, in this
   operation order:

   ```text
   dx  = b.x - a.x
   dy  = b.y - a.y
   len = libm::hypot(dx, dy)
   h   = (dx * (o.y - a.y) - dy * (o.x - a.x)) / len
   ```

3. Ignore an edge only when `len == 0.0`. The radius is the minimum `h` over
   the remaining edges.
4. Require a finite center, at least three nonzero edges, and a finite radius
   strictly greater than zero. A failure marks the field invalid; it is not a
   silently omitted cell.

The immutable representation is:

```text
SourceDisc {
    piece_input_index,
    cell_ordinal,
    center_source_mm: [f64; 2],
    radius_mm: f64,
}
```

Thus a convex ring has the one disc of its existing one-cell decomposition and
a nonconvex ring has one disc per cell of the existing deterministic ear clip.
There is no `K`, cap, sampling, simplification, or fitted parameter. At a
worker decision, transform only the center with the current
`pose_sin_cos`/`apply_pose`; the radius is invariant under pose and mirror.

These are centroid-centered inscribed discs. Circumscribing discs and an
unconditional source-area or `pi*r^2` baseline are outside this specification:
they would fund generic piece size rather than conflict mass.

## 2. Frozen graph and components

Freeze all of the following at entry to each worker `gauss_seidel`, before the
first slot executes:

- `S = { p | energy::incident_raw(state, p) > 0.0 }`;
- `Q = |S|`;
- the graph on `S`, with edge `(i,j)` exactly when the cached authoritative
  `state.pair_rows[pair_index(n,i,j)].violation_mm > 0.0`;
- its connected components, masses, integer quotas, member cycles, and the
  complete `Q`-entry schedule.

Build components by scanning cached pair rows in their existing pair-ID order.
A boundary-colliding member without a positive pair edge is a singleton.
Component ID is the minimum input piece index among its members. Members and
components are ascending by input index/component ID.

If `Q == 0`, there are no components and the schedule is empty. Otherwise every
member of `S` belongs to exactly one component. New pair or boundary conflicts
created during consumption do not enter the graph or receive new slots until
the next worker sweep. A scheduled entry member is still rechecked by the
unchanged `relocate` against all current rows when its slot arrives.

## 3. Arm-B multi-disc conflict mass

Every mass is in square millimetres. Every fold is a serial `f64` fold in the
orders below; unordered or parallel reductions, explicit reassociation, and
FMA substitution are forbidden.

For each authoritative positive pair edge `(i,j)` of a component, in pair-ID
order, visit discs of `i` and then discs of `j` in cell order. With transformed
centers `o_a`, `o_b`, radii `r_a`, `r_b`, and the existing contract quantity
`c_pair = contract.pair_clearance_mm()`:

```text
d        = libm::hypot(o_a.x - o_b.x, o_a.y - o_b.y)
delta_ab = max(r_a + r_b + c_pair - d, 0.0)
term_ab  = delta_ab * delta_ab
```

The clearance is part of the frozen law because it is part of the predicate
that creates a pair conflict; omitting it can erase a real clearance-only edge.
No cached pair violation magnitude and no GLS weight enters this pair term.

After all pair terms, visit every member in input order and every boundary row
in fixed `LEFT, RIGHT, BOTTOM, TOP` order. For every finite authoritative
`EdgeRow.violation_mm = v_ie > 0.0`, add:

```text
term_ie = v_ie * v_ie
```

Boundary terms apply to every affected component, not only boundary-only
singletons. The authoritative term avoids inventing a second sheet half-plane
proxy and cannot miss a tip conflict that an inscribed disc does not cover.
No GLS weight enters it.

The component mass is the pair-term fold followed by the boundary-term fold.
A zero component remains zero when another component has positive mass. There
is no per-component area, member-count, or max-violation rescue.

If any source disc, transformed center, intermediate, or component mass is
negative or non-finite, mark the entire decision `invalidFallback`. Execution
remains total by assigning one slot to each entry member as described below,
but any `invalidFallback` on an admissible Gate-0 or battery input is
`AUTOFAIL`.

If all component masses are finite and their fixed-order total is exactly zero,
record `zeroSignalFallback` and also assign one slot to each entry member. This
is a legitimate no-signal result, not a piece-size proxy. It must be reported;
it may not be used selectively. Arms B and C have identical quotas under this
fallback because there is no mass association to shuffle.

## 4. Integer allocation

For a valid finite nonnegative mass vector with total `T > 0`, let `k` be the
component count. In component-ID order compute:

```text
ideal_c     = ((Q as f64) * mass_c) / T
base_c      = floor(ideal_c)
remainder_c = ideal_c - base_c
R           = Q - sum(base_c)
```

Assign one additional slot to each of the `R` largest remainders, breaking
ties by ascending component ID. Zero quotas are allowed. Require finite ideals
and remainders, `sum(base_c) <= Q`, `R < k`, and final `sum(q_c) == Q`.
Failure of one of those arithmetic identities is `invalidFallback` and uses
the one-slot-per-entry-member schedule.

The global invalid/zero-signal fallback is direct, not floating arithmetic:

```text
q_c = number of entry members in component c
```

Because `Q` is the number of entry members, its quotas sum exactly to `Q`.

## 5. Controls and counter keys

Arm D, `max-violation`, gives each component the maximum finite positive
`violation_mm` among its authoritative pair edges and all boundary rows of its
members, scanning pair rows first and then member/`L,R,B,T` rows. It uses the
same largest-remainder allocator, member cycles, and schedule builder. A
non-finite D value is invalid; the impossible normal case `Q > 0` with total D
weight zero uses the direct one-slot-per-entry-member fallback and is reported.

Arm C, `shuffled-mass`, first pays for the complete B mass vector. When B has a
valid positive total and `k > 1`, rotate the association between ordered
components and B masses by this nonzero offset:

```text
PLACEBO_TAG = 0x4343_4d41_5353_5031  // "CCMASSP1"
offset = 1 + counter_hash([
    seed, bite, iteration, PLACEBO_TAG
]) % (k - 1)
mass_C[t] = mass_B[(t + offset) % k]
```

For `k <= 1`, or B's invalid/zero-signal fallback, C is identity and reports
why. The worker ordinal is deliberately absent: all eight workers in one
tournament see the same component-to-mass association. The rotation preserves
the exact multiset and is guaranteed non-identity by position for `k > 1`.

For B, C, and D, independently permute each component's ascending member list:

```text
MEMBER_TAG = 0x4343_4d45_4d42_5231  // "CCMEMBR1"
root = counter_hash([
    seed, bite, iteration, worker, component_id, MEMBER_TAG
])

for i in (1 .. member_count).rev():
    j = counter_hash([root, i]) % (i + 1)
    swap(member[i], member[j])
```

The member cycle is `member_perm[t % member_count]`.

## 6. Atomic schedule and accounting

Build the complete schedule ex ante by round-robin component layers:

```text
for layer in 0 .. max(q_c):
    for component in ascending component-ID order:
        if layer < q_c:
            emit component.member_cycle[layer % member_count]
```

This emits exactly `Q` entries and gives every funded component an earlier
first visit before one component consumes a long block. Component-block
concatenation is not this round's scheduler.

Every entry calls the current `relocate` exactly once. It either completes the
whole current fixed sample pool and adaptive coarse/fine coordinate descent or
takes the current zero-energy early return. A skip still consumes its slot.
There is no transfer, mid-relocate deadline, outcome-visible rescheduling, or
slot-ordinal addition to `RelocateKey`; repeated members deliberately reuse the
current piece stream inside the same outer iteration.

Add a separate diagnostic `partition_slots`, incremented by `Q` for each armed
worker pass. Reconcile:

```text
sum(component quotas) == Q
schedule length        == Q
executed slots          == Q
full relocate slots + zero-energy slots == Q
```

The historical trajectory/work currency remains exactly as it is:
`Descent.proposals += n` and `WorkVector.piece_proposals += n` per worker
sweep. `partition_slots` does not enter `WorkTerms` or the pacer. Downstream
sample evaluations and charges may differ after trajectories diverge; emit
them fully and charge them only at the unchanged worker barrier.

## 7. Integration boundary

Add Cargo feature `conflict-cluster-budget = []`, disabled by default and used
only with `overlap-ics`. The feature exposes runtime arms `Off`, `Mass`,
`ShuffledMass`, `MaxViolation`, plus read-only `Shadow` and `ComputeIgnore`
diagnostics. Without the feature, the current `gauss_seidel` order path is
compiled unchanged. With it compiled, runtime `Off` still calls the current
`colliding_permutation` path and emits no new output field.

The only behavioral seam is the frozen order producer at the top of
`Descent::gauss_seidel`. The immutable source-disc cache is constructed once
outside `Descent` so tournament cloning does not copy it eight times. The cache
may be shared read-only with workers. The current relocate loop consumes the
resulting vector unchanged.

No treatment code may change or read into sample/pair ranking, acceptance,
coordinate descent/refinement, blocker selection, tournament ranking or merge,
master-only GLS, raw Phi, `max_g`, exact-clear classification, constructor,
publication, or barrier pacer charging. Any such change is `AUTOFAIL`.

No new dependency is allowed. The implementation must contain no Sparrow or
Jagua import, link, code, formula, data layout, name, or numeric constant. Its
provenance is the current local decomposition, contract, counter stream, and
the equations in this document.

## 8. Gate 0

Every clause is mandatory and runs before any quality cell.

### G0.1 — off identity

Compare the frozen `a6e5d1b` binary with the new feature build in runtime arm A
on the four fixed-work cells already enumerated by
`economics-round/integration/armgate.py`. Require recursive document equality
after removing only `wall`, `executableSha256`, and `buildFeatures`. No new
field may appear in A. Poses, fingerprints, decisions, and all work counters
must be bit-identical. The feature-disabled build must also pass its unchanged
test corpus.

### G0.2 — exact vectors

Unit vectors must cover:

- a unit-square cell: center `(0.5,0.5)`, radius `0.5`;
- the same disc under mirror, rotation, and translation, with unchanged radius;
- pair clearance: with two components, `Q=4`, one frozen pair has radii
  `(2,2)`, center distance `4`, `c_pair=1`, B mass `1`, D weight `2`; the
  other has radii `(1,1)`, distance `1.5`, B mass `2.25`, D weight `1`;
  therefore `q_B=[1,3]` and `q_D=[3,1]`;
- a positive boundary row `v=3`, contributing exactly `9` to its component;
- largest-remainder tie: component IDs `[0,3,7]`, `Q=5`, weights `[6,3,1]`
  produce `[3,2,0]`;
- mixed zeros: `Q=5`, weights `[0,4,0]` produce `[0,5,0]`;
- global zero-signal component member counts `[3,1,1]` produce `[3,1,1]`;
- the nonzero placebo rotation, member permutations, round-robin schedule,
  invalid arithmetic fallback, and every accounting identity.

The inversion witness is a pure frozen-row/field vector and is not represented
as a test of `measure_pair`; that distinction must be printed in the evidence.

### G0.3 — shadow engagement

Run the current schedule plus read-only B/C/D shadow on bare mixed-61, orders
1, strike control, fixed work, seeds `0..=8`, `bites=1`, `attempts=1`,
`iterations_per_separation=400`, `compress_bites=0`, and `workers=8`.
Record every reached eligible decision (`component_count >= 2`) with no
subsampling. The a priori ceilings are 28,800 worker decisions and 1,756,800
theoretical piece slots; an early completed separation emits only the reached
decisions.

PASS requires at least one eligible decision whose integer B quota vector
differs from D. Report the complete disagreement rate, every Spearman value
(midranks; `null` for zero variance), component counts, zero-signal fallbacks,
and all mass/allocation bits. There is no selected-seed or correlation gate.

### G0.4 — compute-ignore cost

Use the existing mixed-61 C175 shocked state, seed 0, one worker. Every timed
sweep resets to the identical entry snapshot and completes its exact `Q`.
The control constructs and executes the current Off order. Compute-ignore
constructs the source field, graph, B masses, allocation, and schedule, then
discards that schedule and executes the same Off order. Therefore slot order,
pose digest, work vector, and downstream trajectory must match within every
pair.

Discard 32 warm-up reset/sweeps, then measure 256 reset/sweeps. Run at least
five alternating AB/BA pairs. For pair `p`:

```text
rate_arm_p = completed_atomic_slots_p / measured_seconds_p
ratio_p    = rate_compute_ignore_p / rate_off_p
```

PASS exactly when `median(ratio_p) >= 0.95`. Wall ratios are report-only; no
reciprocal median is substituted. The treatment timing includes its source
field construction. Clone/reset work common to both arms is included in both.

### G0.5 — determinism, authority, and accounting

Run arm B, seed 0, twice in fresh processes at the same fixed slot budget with
fingerprints enabled. After removing only wall, require identical documents,
master fingerprints, poses, mass/allocation/schedule digests, and counters.

For every B/C/D Gate-0 run require the four slot identities in section 6,
`partition_slots` reconciliation, unchanged legacy `n` accounting, zero
invalid fallbacks, exact authority and independent revalidation, and unchanged
barrier charging. Do not require equal sample-evaluation counts across quality
arms.

Gate-0 aggregate telemetry includes at least:

```text
partitionDecisions, eligibleDecisions, entryCollidingPieces,
componentCount, positivePairEdges, partitionSlots, executedSlots,
fullRelocateSlots, zeroEnergySlots, pairDiscTerms, positiveBoundaryRows,
zeroSignalFallbackDecisions, invalidFallbackDecisions,
graphDigestSha256, allocationDigestSha256, scheduleDigestSha256
```

The shadow evidence additionally records decision key, ordered component
IDs/members, exact `f64` B/D mass bits, B/C/D quotas, disagreement, and
Spearman value. Digests serialize integer fields little-endian and `f64` via
`to_bits().to_le_bytes()` in the fixed orders in this document.

Any Gate-0 miss stops the round. There is no quality scout, threshold repair,
or alternate formula.

## 9. Primary battery after Gate 0

Only a complete Gate-0 PASS licenses the already frozen four-arm quality
battery. Use bare mixed-61, orders 1, workers 8, strike control, currency U0,
seeds `0..=8`, fresh process per `(arm,seed)`, and independent revalidation.

Clone the numeric rates and factor from
`deterministic-30s-round/evidence/budget/plan-f100.icscal.json` without
recalibration:

```text
explore units/s  = 2759025.975468987
compress units/s = 1408465.9444235826
factor           = 1.00
30-second search budget = 27.67205079595 s-equivalent work
```

Only binary key, source hash, and provenance change before any quality run.
Apply every 30-second clause in the decision record literally. In particular,
there is no 30-second p95 clause. Ten- and sixty-second batteries use the same
arms and rates and are report-only.

A valid miss closes the mechanism. There is no retune, fifth arm, rescue run,
or promotion of C/D to winner.

## 10. Ballot

The signed form is:

```text
I read the complete file identified by its SHA-256, checked it against the
paper, Sparrow source, frozen campaign evidence, and current implementation,
and CONFIRM it without reservation or hidden amendment.
```

Implementation begins only at three identical `CONFIRM` votes on one digest.
