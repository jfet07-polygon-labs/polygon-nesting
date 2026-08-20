# Round 2 evidence index (Sol review 6 §1 corrections)

Everything here was produced by `../../drivers/`, on one box, in one session.
The per-run documents themselves (ninety of them, 100–500 KB each) are not
committed; their scratch location is in the README's provenance table.

## Batteries

Each `battery-*.json` is the full paired document — every cell's depth, work
total, schedule actions, self-metered calls and debits — and each
`table-*.md` is the same battery as a readable per-cell table.

| file | what it is |
|---|---|
| `battery-work-40000000.json` | the authentic v4 rerun at 40M, 3 seeds × 3 paired rounds, fixed vs `f32c629`. **The headline: the debit binds on every seed and costs 4.376 mm on seed 2.** |
| `battery-work-120000000.json` | the same at 120M. Nothing moves. |
| `battery-work-52000000.json` | the equal-true-cost control: 52M is above every unfixed *true* spend at the 40M point, so this asks whether the honest accounting costs anything once both arms do the same amount of work. |
| `battery-barren1-40000000.json` | the literal reading of Sol's `barren=1`, which is a patience of 1 rather than the v4 default of 16. Run so the round cannot be accused of picking the convenient reading. |
| `battery-wall-{3000,10000,30000}.json` | the 3/10/30 s wall curves. The debit is a no-op under a wall budget by construction; these are the end-to-end version of that claim, not a search A/B. |

## Derived checks

| file | what it proves |
|---|---|
| `truecost.json` | what each arm *really* spent. The unfixed binary still reports `actualCost` and `meteredCost` per action, so its discarded debit is recoverable; this adds it back. Reproduces Sol's counterfactuals independently. |
| `ordering-work40M.json` | Sol finding 4, inside one document: `workUnits == globalUnits + debitedUnits` on every debited call. The pre-fix ordering could only emit `workUnits == globalUnits`. |
| `stampdelta-work40M.json` | Sol finding 4, across the paired arms: the difference between the two arms' publication and `birthWorkUnits` stamps is the cumulative debit *through the current call inclusive*, never the pre-fix exclusive sum. |

## Gates and suites

| file | what it is |
|---|---|
| `gates-fixed.json` | the four pinned gates on a binary built from the committed tree. `ALL_PASS: true`. |
| `gates-unfixed.json` | the same four on the `f32c629` binary. |
| `gates-docdiff.json` | whole-document equality of the two, build-identity and clock fields stripped. `ALL_IDENTICAL: true` — the default path reproducing as documents, not as four scalars. |
| `suite-jagua.log` | `cargo test --release --features jagua-experimental`, exit 0. |
| `suite-jagua-sched.log` | the same with `,compression-schedule` — the combination this change's only live path compiles under, and the one Sol flagged as missing. Exit 0. |
| `binaries.txt` | sha256 of all four binaries measured with. |
| `measurement-binary-rebuild.json` | the measurement binary rebuilt from the committed tree, re-running the headline cell: same depth, same work units, same debit, every field identical bar wall-clock seconds. |
| `portfolio-rs.diff` | the whole change against `f32c629`. |
