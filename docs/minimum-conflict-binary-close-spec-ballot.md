# Minimum-Conflict Binary Close — preimplementation ballot

## Exact text

- specification: [`minimum-conflict-binary-close-spec.md`](minimum-conflict-binary-close-spec.md)
- line count: `357`
- SHA-256: `7ac45b62247bbae8e0390a3e5ade1f7d60f24c4d418f37ca918c0fb67706b3d4`
- implementation or treatment quality before ballot: **none**

Each reviewer had already read the complete 29-page Sparrow paper, all 36 Rust
source files at Sparrow commit `14f4868`, and the campaign ledger. For this
round they first proposed independently, then cross-ranked all three proposals.
The initial rankings were two votes for Minimum-Conflict Binary Close and one
for true two-endpoint joint PGS. Sol's objection to binary submodularity was
audited against the authoritative PairRow field; Sol withdrew it because a
pair-legal parent and common translation give zero diagonal costs while both
cross costs are non-negative. The final direction vote was 3/3.

All three then read the complete 357-line specification at the digest above.
Their literal final votes are:

```text
OX-ALPHA CONFIRM 7ac45b62247bbae8e0390a3e5ade1f7d60f24c4d418f37ca918c0fb67706b3d4
SOL CONFIRM 7ac45b62247bbae8e0390a3e5ade1f7d60f24c4d418f37ca918c0fb67706b3d4
GROK CONFIRM 7ac45b62247bbae8e0390a3e5ade1f7d60f24c4d418f37ca918c0fb67706b3d4
```

No correction followed any vote. The digest is therefore licensed for
implementation. The later code-review and Gate-0 quorums remain separate and
mandatory.
