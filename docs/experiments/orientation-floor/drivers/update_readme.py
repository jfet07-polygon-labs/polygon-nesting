#!/usr/bin/env python3
"""Re-point the README's headline numbers at the c2f line's final state."""
import json
import pathlib

README = pathlib.Path(
    '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_87eab7d7-d70-1/'
    'docs/experiments/orientation-floor/README.md')
text = README.read_text()

PRIOR = 155.42229074464285
FINAL = 155.26442950832842
NET = FINAL - PRIOR

state = json.load(open('/var/lib/t3/tmp/wf87/c2f-state.json'))
rows = ['| c2f round | tier | via | declared raw (mm) | delta | others below |',
        '|---:|---|---|---:|---:|---:|']
for a in state['adoptions']:
    tag = a['tag'].split('-', 1)[1]
    rows.append(f'| {a["round"]} | `{a["tier"]}` | `{tag}` | {a["to"]} | '
                f'{a["delta"]:+.8f} | {a["alsoBelow"]} |')
C2F_TABLE = '\n'.join(rows)

REPLACEMENTS = [
    ('The line reached **155.33041597699957 mm**, 0.09187476764 mm below the previous record.',
     'The line reached **155.26442950832842 mm**, 0.15786123631 mm below the '
     'previous record — and it did so by finding a *fourth* instrument once the '
     'first three had run out, which is §5a below.'),
    ("""**155.33041597699957 mm is 0.09187476764 mm below the previous record** and
3.748 mm below the 159.07876040364792 that stood before the record-line round.
Its fixture sha256 is
`67bef07c498ca6d979ccd37e0191d4ec9255edd8d262f2f1c07108e1feebf002` and its
placement fingerprint is
`77965c9fbb9ebf783cf54bf6fcfe47e86297f5ee1453f0e993bbba9b24237fb2`.""",
     """That line ends at 155.33041597699957, holding a finite negative on a declared
battery of 132 search arms plus 6 replays (`probeArms: 138` folds the replays
into the search count, exactly as the record-line round's own `probeArms: 36`
does). It is not a certified fixpoint — and §11 below is the proof, since the
fourth instrument then walked straight past it. The
fourth instrument (§5a) then took it a further 0.0660 mm over eighteen rounds:

""" + C2F_TABLE + """

**155.26442950832842 mm is 0.15786123631 mm below the previous record** and
3.814 mm below the 159.07876040364792 that stood before the record-line round.
Its fixture sha256 is
`111082132fa7610a1000dd96b36f13e8f9282c28a4006e2333c485b7066ad7b7` and its
placement fingerprint is
`82eaa9762e0df399537226f34490e59d658867a5dec0f4b4ea67537d927a0c72`.

That second table is the round's clearest single statement. Fourteen of its
eighteen adoptions are rotation entries or the flatten grid *re-opened* by one,
and the `others below` column shows the mechanism: a round that a rotation entry
wins hands the next round a state on which 40-70 arms are below. The instruments
are not alternatives, they are a cycle."""),
    ("""with `index.json` carrying each one's sha256 and fingerprint. Five cascades
contributed **2,413 arms** (`evidence/cascade-c2*-state.json` and the matching
logs), the standalone sweeps a further 549, and the certification battery 138 —
**3,100 arms** in total, every one of them on the same request, contract and
CLI tail (`drivers/armcount.py` totals them from the drivers' own documents).""",
     """with `index.json` carrying each one's sha256 and fingerprint. Six cascades
contributed **6,299 arms** (`evidence/cascade-c2*-state.json` and the matching
logs), the standalone sweeps a further 629, and the two certification batteries
276 — **7,204 arms** in total, every one of them on the same request, contract
and CLI tail (`drivers/armcount.py` totals them from the drivers' own
documents)."""),
]
for old, new in REPLACEMENTS:
    assert text.count(old) == 1, old[:70]
    text = text.replace(old, new)
README.write_text(text)
print('updated', repr(NET))
