# Conflict-cluster budget round

Authority: [`../../../conflict-cluster-budget-spec.md`](../../../conflict-cluster-budget-spec.md).
The signed specification SHA-256 is
`0cfdf0e2557967e5aab3a48534e4ff6508c38b3d1054344360aedd61ce284ce9`.

`gate0.py` is the mandatory pre-quality runner. It requires an externally built
copy of the frozen `a6e5d1b` example binary; it never rebuilds its own control.
The candidate binary defaults to the release example in this worktree and can
be overridden with `ICS_CCB_BIN`.

```bash
python3 gate0.py /absolute/path/to/a6e5d1b/overlap_ics_benchmark \
  /var/lib/t3/tmp/overlapics/conflict-cluster-budget-gate0
```

The runner executes, in order:

1. exact four-cell cross-binary runtime-Off identity;
2. the printed exact arithmetic vectors and both test corpora;
3. complete Shadow engagement for seeds 0 through 8;
4. five alternating AB/BA compute-ignore cost pairs;
5. direct B/C/D accounting cells and a fresh-process arm-B replay.

It exits zero only if every Gate-0 clause passes. A miss stops the round; the
quality battery is deliberately a separate, unavailable step until that exit.
