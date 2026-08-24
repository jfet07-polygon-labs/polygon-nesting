# Pool-Retry Tracker Rebase — exact round specification

## 0. Status and authority

This is a preimplementation specification. No treatment implementation,
Gate-0 treatment cell, or quality scout may precede three identical
confirmations of one SHA-256 digest of this complete file.

The frozen behavioral control is commit
`b1235a11cf4a57d7437accbfc2348a05692fe0be`, built with `overlap-ics` and
without the feature introduced below. That commit closes Minimum-Conflict
Binary Close after its valid Gate-0 pass and valid Primary30 failure. Nothing
here licenses a MinCut retry, retune, late-only variant, alternate cut, rescue,
or quality run.

The consultation corpus is frozen as:

- Sparrow paper, 29 pages, SHA-256
  `8452eef76ad9fd77734a05bbe0e423f9cb30a5145b0640ebccc77512712c81df`;
- Sparrow source commit `14f4868fcd7e97036700dbebaf193fb159180aa9`,
  all 36 Rust files under `src`;
- the complete campaign ledger through `b1235a1`, including the constructor,
  operator, population, scalar, CutCloseRelocate, strike/economics,
  deterministic-work, conflict-cluster, and binary-close rounds;
- the exact current `overlap_ics` implementation and the raw nine-seed Centre
  and MinCut Primary30 cells bound by
  `docs/experiments/overlap-ics/minimum-conflict-binary-close-round/evidence/primary30.json`.

Sol, Grok, and ox-alpha each reread the complete paper, Sparrow source, current
source, and negative ledger before proposing independently. After cross-review
all three withdrew their alternatives and selected the single lifecycle seam
specified here. This round takes inspiration from an observed lifecycle
distinction in Sparrow; it copies no source, formula, proxy, data structure, or
implementation.

## 1. Observation, hypothesis, and prior boundary

The current engine handles an explore failure as:

```text
failed separation
-> retain its least-infeasible poses and GLS weights in the pool
-> select a pool entry by the frozen biased rank
-> cold-install that entry's poses
-> restore that entry's saved GLS weights
-> disrupt the restored layout
-> retry at the same width
```

Sparrow makes a narrower distinction. Rollback inside one separation retains
the evolved collision weights, but restoring a solution from the exploration
pool calls `rollback(selected_sol, None)`. `None` rebuilds the collision tracker
with floor weights before disruption. The hypothesis is that weights learned
before a topological disruption are stale guidance for the disrupted layout.
The treatment changes only whether that saved weight vector crosses the
pool-restore/disruption boundary.

The seam is live in the frozen Centre Primary30 evidence. Across seeds `0..=8`
there are 164 failed explore separations and 155 executed disruptions, with
per-seed disruption counts:

```text
seed          0   1   2   3   4   5   6   7   8
disruptions  12  18  12  26  28  11  35   7   6
```

There are another 65 failed compression attempts, for 229 failed separations
in total; those compression failures do not enter this treatment seam.

This is not a revival or rename of:

- aggressive GLS, boundary GLS, shape-factor, exact-area, pole, or scalar
  reranking: their values or update formulas changed; both are frozen here;
- retained-infeasible, retained-parent, population, basin-race, or archive
  work: pool membership, poses, rank, and capacity are frozen here;
- large-piece swap, follower evacuation, ruin, crossover, MinCut, or another
  transition: the disruption and every moved pose are frozen here;
- strike/patience, work currency, executor, pacer, or schedule work: they only
  decide when this seam is reached and are frozen here;
- finalist diversity or one-/two-endpoint PGS: no sample, candidate, relocate,
  projection, acceptance, or publication operation changes.

The previously documented source divergence was deliberately frozen while the
strike defect was isolated. It has never received a saved-weights-versus-floor
ablation. A renamed member of any closed family above is `AUTOFAIL`.

## 2. The sole treatment

Add Cargo feature `pool-retry-tracker-rebase = []`, disabled by default and
reachable only with `overlap-ics`. Its runtime arms are:

- `Saved`: the current control, restoring the selected pool entry's saved pair
  and edge weights;
- `Rebase`: the treatment below;
- `ComputeIgnore`: execute and record the reset, then restore the saved weights
  before the unchanged disruption and control retry.

At the current explore-failure call site, after the existing
`install_poses(&entry.poses, width_mm)` has cold-rebuilt every row and before
calling the existing `disrupt`, `Rebase` must call the existing authoritative
weight reset so that every pair and edge row has weight exactly `1.0`.

In pseudocode, the only behavioral branch is:

```text
entry = unchanged selected pool entry
install_poses(entry.poses, width)

Saved:
    restore entry.pair_weights and entry.edge_weights

Rebase:
    reset every pair and edge weight to exactly 1.0

ComputeIgnore:
    reset every pair and edge weight to exactly 1.0
    emit the treatment diagnostic
    restore entry.pair_weights and entry.edge_weights

unchanged disrupt(seed, bite, attempt, ...)
unchanged retry at the same width
```

The reset is after pose installation and before disruption. The disruption
does not read weights. Its row rebuilds must preserve the selected arm's
weights exactly as current incremental rebuilds do. The subsequent retry
therefore begins with identical disrupted poses and raw rows in all arms; only
the GLS vector differs between `Saved` and `Rebase`.

Rollback to a minimum-raw snapshot inside the same separation continues to use
the current keep-weights behavior. Existing resets at new width/bite remain
unchanged. A failed or deadline-ended retry follows the current pool lifecycle;
the treatment is applied independently at each later real pool restore, never
inside a separation and never on the compression path.

## 3. Frozen seams and `AUTOFAIL` conditions

The following must be identical through the treatment boundary:

- constructor, installed exact parent, bite target, centre cut, moved subset,
  width, pool content/order/capacity, selected rank, selected poses, and raw
  rows;
- disruption key, selected roots, follower set/order, pose transforms, counter
  keys, work charge, and resulting disrupted poses/raw rows;
- relocate sample keys, 25+50 samples, 16 angles, three finalists, both
  coordinate descents, accept-equal rule, and commit rule;
- colliding-piece permutation, eight-worker tournament, ordinal merge, raw
  Phi, `max_g`, GLS formula and update timing;
- patience/strike, 80/20 split, work currency, calibrated plan, pacer, and
  compression schedule;
- exact-clear band, repair, Exclusive `r=2.500`, contract validation,
  independent revalidation, incumbent arbitration, and every publication
  authority.

Before the first downstream decision after disruption, `Saved` and `Rebase`
may differ only in pair/edge weight bits and their arm/diagnostic identity.
Any treatment-dependent difference in the preceding list is `AUTOFAIL`.

Without the feature, the current path must compile unchanged. With the feature
compiled, absent/runtime-`Saved` must call the current behavior and emit no new
behavioral output field. After removing only wall, executable/build identity,
and the named treatment diagnostic, `ComputeIgnore` must be recursively
document-identical to `Saved`.

## 4. Required telemetry and digests

Every probed pool retry must emit, in stable input/row order:

- `(request seed, explore bite ordinal, attempt ordinal)`, width/target bits,
  pool length, selected rank, pool-entry raw Phi, and selected-pose digest;
- saved pair/edge weight bits, their canonical digest, count strictly above
  `1.0`, minimum, maximum, and finiteness verdict;
- post-install raw-row and pose digests;
- post-policy weight bits/digest and a literal `allWeightsExactlyOne` verdict
  for `Rebase` before disruption;
- disruption key, root IDs, follower IDs/order, pose-transform digest, work
  delta, and post-disruption pose/raw-row digests;
- downstream master-iteration winner ordinals and committed-relocate pose
  digests until publication or stop;
- complete work, strike, band, publication, authority, validation, and
  independent-revalidation records.

Digests serialize domain tags, lengths, integer IDs, booleans, and `f64` via
`to_bits().to_le_bytes()`. Non-finite saved or reset weights, a reset bit other
than exact `1.0`, a cold/incremental raw-row mismatch, or any pre-decision
cross-arm pose/raw/disruption mismatch is an invalid retry and Gate-0 failure.

## 5. Gate 0 — mandatory and pre-quality

Gate 0 runs once against one frozen candidate binary. Any clause miss closes
this mechanism before quality. There is no retry, seed selection, reset decay,
partial reset, alternate floor, extra attempt, threshold repair, or scout.

### G0.1 — feature/runtime isolation and reset vectors

Build an external frozen `b1235a1` release example with `overlap-ics`, and the
candidate with `overlap-ics,pool-retry-tracker-rebase`. On the same four
fixed-work identity cells used by the preceding Gate 0, compare the frozen
binary with candidate runtime `Saved`. After removing only wall, executable
SHA, and build-feature identity, require exact recursive document identity on
all four cells.

Before real-corpus treatment require focused tests and printed exact vectors
which prove:

1. every pair and edge weight becomes bit-exact `1.0` under `Rebase`;
2. `Saved` restores a nontrivial saved vector exactly;
3. `ComputeIgnore` resets and then restores that vector exactly;
4. rollback inside one separation retains nontrivial evolved weights;
5. new-width reset behavior is unchanged;
6. cold install and disruption rebuild preserve authoritative raw rows and the
   arm-selected weights exactly;
7. non-finite input weights fail visibly rather than being masked by reset.

The complete default and feature `overlap-ics` test corpora must pass.

### G0.2 — nine paired first-retry probes

Use bare mixed-61, order 1, workers 8, strike control, Centre explore injection,
currency U0, seeds `0..=8`, and fresh processes. Follow the frozen deterministic
30-second plan from the bare request, but stop each control trajectory at its
first real explore pool selection, after rank selection and before installing
the selected entry. Every seed must reach that checkpoint within its unchanged
plan. Fork `Saved` and `Rebase` from bit-identical serialized copies of that
checkpoint.

For each arm, install the same selected poses, apply its section-2 weight
policy, run the same disruption, and execute exactly one normal explore retry
at that same width. The retry uses the unchanged strike control and a hard cap
of 400 completed master iterations. Publication or the unchanged strike/deadline
stop may end it earlier. No second pool selection or disruption is allowed in
the probe.

PASS requires every clause:

1. all nine prefixes/checkpoints are valid and bit-identical across paired
   arms before the weight-policy branch;
2. saved weights are finite and at least one weight is strictly above `1.0` in
   every checkpoint;
3. selected pool rank/poses, target, disruption key/roots/followers/work,
   post-disruption poses, authoritative raw rows, and authority fingerprints
   are bit-identical across paired arms;
4. all `Rebase` pair/edge weights are bit-exact `1.0` immediately before
   disruption;
5. at least two distinct seeds change from `Saved` unpublished to `Rebase`
   published within the same retry cap;
6. zero seeds change from `Saved` published to `Rebase` unpublished;
7. in at least two distinct seeds, before each treatment-only publication, a
   downstream winner ordinal or committed-relocate pose digest differs across
   arms, proving a behavioral consequence rather than a printed-weight change;
8. every publication is strict, Exclusive `r=2.500`, contract-valid,
   independently revalidated, and the invalid-publication count is zero;
9. complete iteration, work, strike, band, authority, digest, and stop records
   are emitted for all cells without seed subsampling.

There is no raw-Phi percentage, endpoint-depth, or timing clause in G0.2.

### G0.3 — compute-ignore cost and identity

Use seed 0 and its exact G0.2 checkpoint. `Saved` and `ComputeIgnore` each run
the complete install-policy-disruption-retry path. Run five fresh-process pairs
in frozen order `AB, BA, AB, BA, AB`, beginning the battery on a quiet box. The
timed interval begins immediately before pose installation and ends at retry
publication/stop.

For pair `p`:

```text
rate_arm_p = sample_evaluations / search seconds
ratio_p    = rate_compute_ignore_p / rate_saved_p
```

PASS exactly when the median of the five `ratio_p` values is at least `0.95`.
No reciprocal median is substituted. Within every pair require identical
poses, raw rows, weights at disruption entry, disruption trace, downstream
winner/commit digests, complete work vector, publication/revalidation record,
pacer charges, and whole document after removing only timing,
executable/build identity, arm identity, and the named reset diagnostic.

### G0.4 — determinism, authority, and provenance

Run the seed-0 `Rebase` G0.2 prefix/checkpoint/retry twice in fresh processes
with fingerprints enabled. After removing only wall, require identical complete
documents, weights, disruption, downstream decision digests, counters,
publications, and revalidations. Require zero invalid retries and zero invalid
publications in every Gate-0 treatment cell.

Audit the feature diff for provenance. No Sparrow/Jagua source, dependency,
formula, collision proxy, data structure, or implementation is copied or
linked. Source comments may cite the paper/source only to document the observed
tracker-lifecycle distinction.

## 6. Independent implementation review

After implementation and before executing Gate 0, Sol, Grok, and ox-alpha each
review the same frozen worktree against this exact specification. Every blocker
must be corrected and the corrected source re-reviewed. Gate 0 is licensed only
by three `REVIEW PASS` verdicts on one source commit.

## 7. Primary30, only after complete Gate-0 PASS

The quality arms are frozen:

| arm | pool-retry policy | role |
| --- | --- | --- |
| A | `Saved` | contemporaneous control |
| B | `Rebase` | mandatory treatment |

Use bare mixed-61, order 1, workers 8, Centre explore injection, strike control,
currency U0, seeds `0..=8`, fresh process per `(arm,seed)`, and independent
revalidation. Clone the deterministic 30-second plan without recalibration:

```text
explore units/s  = 2759025.975468987
compress units/s = 1408465.9444235826
factor           = 1.00
30-second search budget = 27.67205079595 s-equivalent work
```

Only binary key, source hash, feature provenance, and this arm identity change
before any quality run. Primary30 PASS requires every standing clause:

1. B median raw-source depth at most `163.00461 mm`;
2. at least `7/9` B seeds at or below `168.484 mm`;
3. paired median `A depth - B depth` at least `1.000 mm`;
4. every publication in both arms is Exclusive `r=2.500`, contract-valid, and
   independently revalidated, with zero invalid publications;
5. plan, charge, authority, source, and executable identities are green in
   every cell.

There is no 30-second wall-p95 clause. Wall p95, maximum, and overshoots are
report-only. A valid Primary30 miss closes Pool-Retry Tracker Rebase without
retuning, a second reset point, partial/decayed weights, another retry rule, or
rescue run.

Only after Primary30 PASS, run the same arms at 10 and 60 seconds as report-only
curve points. They do not reopen the retired 10-second gate and cannot rescue a
30-second miss.

## 8. Ballot

The signed form is:

```text
I read the complete file identified by its SHA-256, checked it against the
paper, Sparrow source, frozen campaign evidence, current implementation seam,
and the three reconciliation memos, and CONFIRM it without reservation or
hidden amendment.
```

Implementation begins only after three identical `CONFIRM` votes on one
digest. A correction creates a new digest and resets all votes.
