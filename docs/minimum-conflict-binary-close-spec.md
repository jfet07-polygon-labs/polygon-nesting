# Minimum-Conflict Binary Close — exact round specification

## 0. Status and authority

This is a preimplementation specification. No treatment implementation, Gate-0
treatment cell, or quality scout may precede three identical confirmations of
one SHA-256 digest of this file.

The frozen behavioral control is commit
`918d6ff2041a652fbebbd91b9a8fba4d0cb1ad81`, built with `overlap-ics` and
without the feature introduced below. Commit `11a1a45` contains the reviewed
conflict-cluster implementation; `918d6ff` closes it on its first and only
Gate 0. That mechanism remains closed. Nothing here licenses a retry, retune,
cheaper rewrite, alternate threshold, or quality run for it.

The consultation corpus remains:

- Sparrow paper, 29 pages, SHA-256
  `8452eef76ad9fd77734a05bbe0e423f9cb30a5145b0640ebccc77512712c81df`;
- Sparrow source commit `14f4868fcd7e97036700dbebaf193fb159180aa9`,
  all 36 Rust files under `src`;
- the complete campaign ledger through `918d6ff`, including the constructor,
  operator, economics, deterministic 30-second, and conflict-cluster rounds.

Sol, Grok, and ox-alpha read that corpus before proposing independently. After
cross-ranking the three proposals and correcting one mathematical objection,
all three selected the mechanism specified here. Sparrow does not implement
this mechanism. The inspiration is its long legal-to-infeasible shrink and
separate lifecycle, not its source, formulas, data structures, constants, or
collision proxy.

## 1. Hypothesis and historical boundary

The current explore bite cuts at `D / 2` and translates every piece whose
source centroid lies above the cut by the same negative
`delta = T - D`, where `T = D * (1 - 0.001)`. On the fixed-work shelf, that
geometric half-plane choice creates a 22nd-bite contact topology which often
survives hundreds of master iterations. The hypothesis is that the separator
is being handed an unnecessarily expensive seam, not that it needs another
per-iteration score or work allocator.

The treatment chooses, once at explore-bite injection, which pieces receive
that same `delta`. It minimizes the authoritative raw entry energy over all
binary subsets. It does not change the target, shrink size, piece angles,
mirrors, horizontal coordinates, or any downstream operator.

This is not a revival of:

- bottom-left, constructor order, salt, settle, ruin, or another constructor;
- quarter/centre/three-quarter or random linear cuts, which select a geometric
  prefix rather than an arbitrary binary subset;
- m23 derived-cut crossover, guillotine/group-drop, or exact-valid monotone
  compaction;
- active-contact blocks, component moves, terminal coupled projection, or
  one-/two-endpoint PGS;
- a retained population, basin race, archive revival, or persistent lane;
- scalar-pole reranking, GLS retuning, conflict-cluster allocation, strike,
  currency, executor, or pacer work.

Those records bind this round. Renaming any of them is `AUTOFAIL`.

## 2. The binary energy

An explore bite begins from the installed exact incumbent at raw source depth
`D`. Let:

```text
T     = D * (1 - 0.001)
delta = T - D                    # strictly negative
p_i(0) = the installed pose of piece i
p_i(1) = p_i(0), except ty_mm := ty_mm + delta
```

`x_i = 0` keeps piece `i`; `x_i = 1` translates it. `tx_mm`, `theta_deg`, and
`mirrored` are copied bit-for-bit in both states. The two transformed source
geometries are built by the existing overlap-ICS geometry path.

For every unordered input pair `(i,j)`, cold-measure all four authoritative
raw pair costs at target `T`:

```text
c_ij(a,b) = square(PairRow.violation_mm for p_i(a), p_j(b)), a,b in {0,1}
```

`PairRow.violation_mm` is the existing positive-clearance violation produced
by the signed-gap field. No disc field, pole area, broad-phase decision,
weighted loss, surrogate, or exact validator supplies a coefficient.

For each piece and state, cold-measure the existing four boundary rows at
target `T` and define `u_i(a)` as their raw squared-violation sum in the
existing edge order. The complete binary energy is:

```text
E(x) = sum_i u_i(x_i) + sum_{i<j} c_ij(x_i,x_j)
```

Every operand, violation, square, term, and sum must be finite and
non-negative. Before graph construction, every pair must satisfy exactly:

```text
c_ij(0,0) == 0
c_ij(1,1) == 0
c_ij(0,0) + c_ij(1,1) <= c_ij(0,1) + c_ij(1,0)
```

The mathematical reason is fixed in advance: the parent is pair-legal, a
common translation preserves relative geometry, and raw pair costs are
non-negative. The implementation still cold-measures and checks the literals;
it does not assume floating-point translation invariance. A non-finite term,
negative term, nonzero diagonal, or submodularity miss is a visible invalid
decision and Gate-0 failure. There is no epsilon, clamp, QPBO, partial label,
centre fallback, or alternate energy.

## 3. The exact deterministic cut

Use labels `0 = source side` and `1 = sink side`. Build one directed
source/sink graph in stable input-piece order:

- `source -> i` has capacity `u_i(1)`;
- `i -> sink` has capacity `u_i(0)`;
- `i -> j` has capacity `c_ij(0,1)`;
- `j -> i` has capacity `c_ij(1,0)`.

Zero-capacity edges remain represented in the canonical digest even if the
max-flow implementation skips storing them. With the checked zero diagonal,
the capacity of every cut is exactly `E(x)`.

Implement an independently authored deterministic integer-free max-flow over
finite non-negative `f64` capacities. Node IDs, edge insertion, BFS/DFS scans,
and residual traversal are stable by input piece ID and the order above. The
chosen label is the final residual source-reachability partition. This is the
tie rule; no lexicographic re-solve or outcome-visible preference is added.
Trivial all-zero and all-one subsets are permitted.

After choosing `x`, rebuild the complete overlap-ICS state cold at `T`. In the
same canonical piece/pair/edge addition order, require bit identity between:

1. the selected cut capacity recomputed from the frozen term table;
2. `E(x)` recomputed from that table; and
3. the installed state's authoritative raw Phi.

The installed pair and boundary row bits must equal the selected table entries.
Any mismatch is an invalid decision and Gate-0 failure, never a repair.

## 4. Integration boundary

Add Cargo feature `minimum-conflict-binary-close = []`, disabled by default and
reachable only with `overlap-ics`. Its runtime arms are:

- `Centre`: the current `homotopy::explore_bite` path;
- `MinCut`: the treatment above;
- `ComputeIgnore`: construct and solve the treatment completely, then discard
  its labels and execute the current centre bite.

Without the feature, the current explore path compiles unchanged. With the
feature compiled, absent/runtime-`Centre` still calls that path and emits no new
output field. `ComputeIgnore` must have the same poses, rows, fingerprints,
consumed worker orders, publications, work vector, pacer charges, and whole
document as `Centre` after removing only timing, executable/build identity, and
the named diagnostic record.

The intervention applies only to explore-bite injection. Compression retains
the existing time-decayed step, uniform cut, and `split_and_close` bit-for-bit.
The following are frozen without treatment reads or writes:

- constructor and exact-parent installation;
- relocate sample pool, coordinate descent, acceptance, and order;
- GLS values, update timing, weights, and reset policy;
- tournament workers, keys, ranking, ordinal merge, and barrier;
- patience/strike, least-infeasible pool, disruption, and attempt policy;
- raw Phi, `max_g`, exact-clear band, repair, both publication authorities,
  independent revalidation, and incumbent arbitration;
- work currency, calibrated plan, phase split, and pacer charging.

Any treatment-dependent branch at those seams is `AUTOFAIL`.

## 5. Required telemetry and digests

For every attempted treatment/compute-ignore bite emit:

- decision key `(request seed, explore bite ordinal)` and `D`, `T`, `delta` bits;
- parent proxy-pair legality and complete parent pose fingerprint;
- for every pair in stable order: IDs and all four `c_ij` bit patterns;
- for every piece: both unary boundary sums and their four-row bit patterns;
- finiteness, non-negativity, zero-diagonal, and submodularity verdicts;
- graph edge list, residual source-reachability labels, centre labels, their
  Hamming disagreement, and moved-piece counts;
- canonical term-table, graph, label-vector, installed-pose, and installed-row
  SHA-256 digests;
- selected cut capacity, table energy, cold raw Phi, and their bit identities;
- downstream work, authority, publication, and revalidation records unchanged.

Digests serialize domain tags, lengths, integer IDs, booleans, and `f64` via
`to_bits().to_le_bytes()`; vectors carry explicit lengths and stable delimiters.

## 6. Gate 0 — mandatory and pre-quality

Gate 0 runs once on a quiet box (`load1 < 1.0`) against one frozen candidate
binary. The binary SHA-256 must remain unchanged. Any clause miss stops this
round before quality. No retry, selected seed, threshold repair, alternate
formula, fallback arm, or scout is licensed.

### G0.1 — feature/runtime isolation

Build an external frozen `918d6ff` release example with `overlap-ics`, and the
candidate with `overlap-ics,minimum-conflict-binary-close`. On the same four
fixed-work identity cells A/B/C/D used by the conflict-cluster Gate 0, compare
the frozen binary with candidate runtime `Centre`. After removing only wall,
executable SHA, and build-feature identity, require exact recursive document
identity on all four cells.

### G0.2 — arithmetic, graph, and failure vectors

Before real-corpus treatment, require printed exact vectors and unit tests for:

1. pose state 0/1 bits and a common-translation pair;
2. a hand-computable three-to-five-node asymmetric graph with boundary unaries,
   a unique nontrivial expected label vector, and exhaustive `2^n` enumeration
   agreeing with max-flow;
3. equality of directed cut capacity and `E(x)` for every labeling of that
   vector;
4. zero diagonal and submodularity acceptance;
5. visible rejection of non-finite, negative, nonzero-diagonal, and
   nonsubmodular inputs;
6. cold/incremental selected rows and raw-Phi bit identity;
7. stable tie behavior, including trivial all-zero/all-one optima.

The full `overlap-ics` default and feature test corpora must both pass.

### G0.3 — frozen prefix and next-bite causal inversion

Use bare mixed-61, order 1, strike control, seeds `0..=8`, and fresh processes.
For every seed, reproduce the existing fixed-work prefix shape:

```text
prefix: workers=8, requested explore bites=21, attempts/bite=1,
        iterations/separation=400, compress bites=0
probe:  workers=8, requested explore bites=1, attempts/bite=1,
        iterations/separation=400, compress bites=0
```

Run contemporaneous `Centre` and mandatory `MinCut` probes from bit-identical
copies of the exact incumbent left by the control prefix. Include all nine
seeds. `prefixAllPublished` is evidence, not a selection knob: a cell is a true
22nd-bite existence cell exactly when the prefix published all 21 requested
bites. Seeds whose prefix stopped earlier remain in the regression floor but
cannot satisfy the existence clause.

PASS requires all of:

1. constructor, prefix incumbent pose/depth/fingerprint, target, and work agree
   between arms for every seed;
2. every `Centre` probe publication is also a `MinCut` probe publication;
3. at least one true 22nd-bite cell where `Centre` fails publishes under
   `MinCut` within the same cap;
4. every treatment decision is valid under sections 2–5, and at least one true
   22nd-bite treatment label vector differs from the centre label vector;
5. every publication is strict, dual-valid, independently revalidated, and has
   zero invalid-publication count;
6. complete work, iteration, strike, attempt, band, authority, and digest
   records are emitted for all cells, without seed subsampling.

There is no raw-Phi percentage gate and no final-depth comparison beyond the
literal publication inversion above.

### G0.4 — compute-ignore cost and identity

Use seed 0 and the same fixed 21-bite prefix plus one 400-iteration probe.
`Centre` and `ComputeIgnore` each execute the complete fixed trajectory. Run
five fresh-process pairs in the frozen order `AB, BA, AB, BA, AB`, beginning
the battery in a quiet box. Constructor time is reported but excluded; the
timed interval is prefix plus probe search.

For pair `p`:

```text
rate_arm_p = completed legacy piece proposals / search seconds
ratio_p    = rate_compute_ignore_p / rate_centre_p
```

PASS exactly when the median of the five `ratio_p` values is at least `0.95`.
No reciprocal median is substituted. Within every pair require identical
poses, rows, consumed worker-order digest, complete work vector, completed
piece proposals, publication/revalidation record, and pacer charges. Treatment
construction and max-flow are inside its timed interval.

### G0.5 — determinism, authority, and provenance

Run the seed-0 `MinCut` prefix/probe twice in fresh processes with fingerprints
enabled. After removing only wall, require identical complete documents,
labels, term tables, graph/residual/pose/row digests, counters, publications,
and revalidations. Require zero invalid decisions and zero invalid
publications in every Gate-0 treatment cell.

Audit the feature diff for source provenance. No Sparrow/Jagua source,
dependency, data structure, capacity formula, constant, or layout is copied or
linked. The implementation may cite the paper only for the already shipped
shrink-and-separate lifecycle.

## 7. Independent implementation review

After implementation and before executing Gate 0, Sol, Grok, and ox-alpha each
review the same frozen worktree against this exact specification. Every
reported blocker must be corrected and the corrected source re-reviewed.
Gate 0 is licensed only by three `REVIEW PASS` verdicts on one source commit.

## 8. Quality battery, only after complete Gate-0 PASS

The quality arms are frozen:

| arm | explore injection | role |
| --- | --- | --- |
| A | `Centre` | contemporaneous control |
| B | mandatory `MinCut` | gate owner |

Use bare mixed-61, order 1, workers 8, strike control, currency U0, seeds
`0..=8`, fresh process per `(arm,seed)`, and independent revalidation. Clone
the deterministic 30-second plan without recalibration:

```text
explore units/s  = 2759025.975468987
compress units/s = 1408465.9444235826
factor           = 1.00
30-second search budget = 27.67205079595 s-equivalent work
```

Only binary key, source hash, and provenance change before any quality run.
The 30-second primary PASS requires every standing clause:

1. B median raw-source depth at most `163.00461 mm`;
2. at least `7/9` B seeds at or below `168.484 mm`;
3. paired median `A depth - B depth` at least `1.000 mm`;
4. every publication in both arms Exclusive `r=2.500`, contract-valid, and
   independently revalidated, with zero invalid publications;
5. plan, charge, authority, and executable identities green in every cell.

There is no 30-second p95 clause. Wall p95, maximum, and overshoots are
report-only. A valid primary miss closes Minimum-Conflict Binary Close without
retuning, alternate labels, another min-cut rule, or rescue run.

Only after primary PASS, run the same arms at 10 and 60 seconds as report-only
curve points. They do not reopen the retired 10-second gate and cannot rescue a
30-second miss.

## 9. Ballot

The signed form is:

```text
I read the complete file identified by its SHA-256, checked it against the
paper, Sparrow source, frozen campaign evidence, current implementation seam,
and the three reconciliation memos, and CONFIRM it without reservation or
hidden amendment.
```

Implementation begins only after three identical `CONFIRM` votes on one
digest. A correction creates a new digest and resets all votes.
