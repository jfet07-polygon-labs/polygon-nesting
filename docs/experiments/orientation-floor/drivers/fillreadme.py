#!/usr/bin/env python3
"""Substitute the round's measured numbers into the README's placeholders."""
import pathlib

README = pathlib.Path(
    '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_87eab7d7-d70-1/'
    'docs/experiments/orientation-floor/README.md')
TABLE = pathlib.Path('/var/lib/t3/tmp/wf87/table.md').read_text()
TABLE = TABLE.split('\nnet ')[0].strip()

CERT = """`drivers/certify_full.py` on `pinned-fs-155.33041597700.json`, declared raw
`155.33041597699957`, in 1,247 s (`evidence/cert-final.json`):

| half | what | result |
|---|---|---|
| replay | modes 27, 30 and 22 seeds 0-3 | 6 of 6 `exactValid` **and** `contractValid`, all six reproducing fingerprint `77965c9f…` at **0 ULPs** from the declared raw |
| battery | **132 search arms** (plus the 6 replay arms above, for `probeArms: 138`): mode 31 x 4 tiny steps, the eleven-delta flatten grid x 2 slacks -> mode 33 and x 1 slack -> mode 32 under **both ladder generations**, the nudge tier x 16, **tier H's twelve-delta grid -> modes 30 and 31**, mode 26 x 3 drops x 2 seeds, and mode 34 x 8 step/budget specs x 2 seeds | **0 below the incumbent** |

`replayPass: true`, `belowIncumbent: 0`, `finiteNegativeOnBattery: true` (the
field the driver recorded as `fixpoint: true` when this ran; the archived JSON
is left as produced). The battery is 138
arms against the record-line round's 36 — 132 search arms against 30, both
counts folding the same 6 replays — and the three additions are the three
instruments that moved this round: the wide entry grid, mode 32, and tier H. A
coverage claim that does not probe the tier that produced the descent is a claim
about the tiers that were already exhausted.

Two independent replays are kept rather than one. The record was produced by the
ten-rung binary, so `evidence/replay-final.json` is the in-family check — and
`evidence/replay-final-basebinary.json` is the stronger one: the **pristine
base-commit binary, which has no knowledge of the 0.00128 rung**, replays the
final layout `exactValid` and `contractValid` on modes 27, 30 and 22 seeds 0-1,
reproducing raw `155.33041597699957` at **0 ULPs** and fingerprint
`77965c9fbb9ebf78…` exactly. The ladder change is what *found* the state; it is
not needed to *verify* it.

The battery's accepted-rung histogram is worth recording, because it is the
diagnostic that justified this round's change and it now reads differently:
5 acceptances at 0.00128, 13 at 0.0032, 36 at 0.3125, 4 at 0.78125, plus 12
mirror-family acceptances. The distribution no longer piles on the floor, which
is what a correctly-placed floor looks like — and is the honest reason not to
add another rung even before the arithmetic in §2 forbids it."""

GOALNOTE = """The gap to 155.000 is **0.330 mm**, down
  from 0.422 mm. This round did not reach the threshold and does not claim a
  path to it: the descent decayed from 0.013 mm a round to 0.0004 mm a round
  inside the last cascade, and the final state holds a finite negative on a
  declared battery of 132 search arms plus 6 replays (`probeArms: 138`), plus a
  further 110 arms of untried compositions (§8's last three rows) — a negative
  on the instruments that were fired, not a certified fixpoint.
  What it does claim is that the two levers it found are not exhausted in
  *kind* — both were found by asking what the state's own diagnostics were
  saying rather than by widening a grid, and the diagnostics are still talking."""

text = README.read_text()
for token, value in (
        ('@FINAL@', '155.33041597699957'),
        ('@NETDELTA@', '0.09187476764'),
        ('@RESULTTABLE@', TABLE + """

**155.33041597699957 mm is 0.09187476764 mm below the previous record** and
3.748 mm below the 159.07876040364792 that stood before the record-line round.
Its fixture sha256 is
`67bef07c498ca6d979ccd37e0191d4ec9255edd8d262f2f1c07108e1feebf002` and its
placement fingerprint is
`77965c9fbb9ebf783cf54bf6fcfe47e86297f5ee1453f0e993bbba9b24237fb2`.

Every pin in the table is in
`docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/orientation-floor/`,
with `index.json` carrying each one's sha256 and fingerprint. Five cascades
contributed **2,413 arms** (`evidence/cascade-c2*-state.json` and the matching
logs); the standalone sweeps contributed a further 560.

Three of the eighteen steps are the three instruments, and they are visible in
the deltas: the ladder rung buys 0.0003-0.002 mm a step, the deep entry grid
0.005-0.006, and tier H 0.005-0.013. The line is not one lever applied
repeatedly — it is three, interleaved, each opening the fixpoint the previous
one left."""),
        ('@CROSSARMS@', '90 in-cascade + 70 dedicated'),
        ('@SUITEPASS@', '1,250'),
        ('@SUITEFAIL@', '0'),
        ('@SUITEIGN@', '2'),
        ('@CERTSECTION@', CERT),
        ('@GOALSTATUS@', 'not reached'),
        ('@GOALNOTE@', GOALNOTE)):
    assert token in text, token
    text = text.replace(token, value)
README.write_text(text)
print('filled')
