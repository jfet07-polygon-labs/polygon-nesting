# Pool-Retry Tracker Rebase — quorum decision

## Status

**Exact mechanism, experimental law, and quality gate selected by a 3/3
quorum. Implementation is licensed.**

The immutable specification is
[`pool-retry-tracker-rebase-spec.md`](pool-retry-tracker-rebase-spec.md),
352 lines, SHA-256
`b5038979351bf2fc114a1d7a220751f0704e362e2dd62809632dceca9245a3a1`.

## Consultation and reconciliation

Sol, Grok, and ox-alpha each reread the complete 29-page Sparrow paper, all 36
Rust source files at frozen commit `14f4868fcd7e97036700dbebaf193fb159180aa9`,
the current engine, and the campaign's negative ledger before proposing one
independent direction.

- Sol proposed positive-row-set diversity among the three coordinate-descent
  finalists.
- Grok proposed rebasing GLS weights after a pool restore and before
  disruption.
- ox-alpha proposed one atomic two-endpoint pass at terminal strike.

Cross-review rejected the two operator changes. Their local mechanism gates
could repeat the error exposed by Minimum-Conflict Binary Close: one local
inversion did not predict the full 30-second trajectory. The three consultants
converged instead on the narrower source-backed lifecycle seam. It is active
155 times across the nine frozen Centre Primary30 cells and was previously
documented but never ablated.

## Exact ballot

Sol, Grok, and ox-alpha each returned the following ballot on the same digest,
without amendment:

> I read the complete file identified by its SHA-256, checked it against the
> paper, Sparrow source, frozen campaign evidence, current implementation seam,
> and the three reconciliation memos, and CONFIRM it without reservation or
> hidden amendment.

The next authorization boundary is independent implementation review. Gate 0
may run only after all three return `REVIEW PASS` on one frozen source commit.
