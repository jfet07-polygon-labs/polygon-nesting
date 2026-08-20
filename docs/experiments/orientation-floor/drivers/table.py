#!/usr/bin/env python3
"""Emit the round's descent table as markdown, from the pin index.

Every row's `via` is read off the cascade adoption logs and the standalone
sweep documents; the raws and the deltas are computed here rather than
transcribed, which is the point.
"""
import json

INDEX = ('/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/'
         'wf_87eab7d7-d70-1/docs/experiments/persistent-vacancy-descent/'
         'exact-contract/true-contract/orientation-floor/index.json')
PRIOR = 155.42229074464285

LINE = [
    # Not "a certified fixpoint of 36 arms": `probeArms: 36` folds the 6 replay
    # arms into the search count, so the battery is 30 search arms plus 6
    # replays, and a finite negative on a declared battery is not a fixpoint.
    # This string is the one the README table carries; keep the two in step.
    ('record-line-cascade/pinned-fs-155.4223.json', PRIOR,
     'the prior record, a finite negative on a declared battery of 30 search '
     "arms + 6 replays (not a certified fixpoint — see that round's §7)"),
    ('pinned-fs-155.42197.json', 155.42196626072334,
     'flatten 0.001 -> mode 33 p0.05, accepted rung **0.00128**'),
    ('pinned-fs-155.41964177680.json', 155.4196417768017,
     'flatten 0.005 -> mode 33 p0.05, accepted rung 0.00128'),
    ('pinned-fs-155.41373.json', 155.4137281129324,
     'flatten **0.03** -> mode 33 p2.0 (deep entry grid)'),
    ('pinned-fs-155.40872811293.json', 155.4087281129324,
     'flatten 0.008 -> mode 33 p0.05'),
    ('pinned-fs-155.39673.json', 155.3967281129324,
     'flatten **0.25 -> mode 30** (tier H, entry -> legalization)'),
    ('pinned-fs-155.38372811293.json', 155.3837281129324,
     'flatten 0.1 -> mode 30'),
    ('pinned-fs-155.37372811293.json', 155.37372811293238,
     'flatten 0.08 -> mode 30'),
    ('pinned-fs-155.36572811293.json', 155.36572811293237,
     'flatten 0.05 -> mode 30'),
    ('pinned-fs-155.36072811293.json', 155.36072811293235,
     'flatten 0.05 -> mode 30'),
    ('pinned-fs-155.35272811293.json', 155.35272811293234,
     'mode 22, slack 0.8, seed 0'),
    ('pinned-fs-155.35181307831.json', 155.35181307831448,
     'flatten 0.0005 -> mode 33 p0.05'),
    ('pinned-fs-155.34181307831.json', 155.3418130783145,
     'flatten 0.2 -> mode 30'),
    ('pinned-fs-155.33681307831.json', 155.33681307831446,
     'flatten 0.05 -> mode 30'),
    ('pinned-fs-155.33281307831.json', 155.33281307831447,
     'mode 22, slack 0.8, seed 0'),
    ('pinned-fs-155.33181307831.json', 155.33181307831444,
     'flatten 0.0005 -> mode 33 p0.05'),
    ('pinned-fs-155.33141597700.json', 155.33141597699955,
     'flatten 0.001 -> mode 33 p0.05'),
    ('pinned-fs-155.33041597700.json', 155.33041597699957,
     'flatten 0.002 -> mode 33 p0.05'),
]

index = {row['pin']: row for row in json.load(open(INDEX))}
print('| pin | declared raw (mm) | via | delta |')
print('|---|---:|---|---:|')
previous = None
for name, raw, via in LINE:
    delta = '—' if previous is None else f'{raw - previous:+.6f}'
    label = f'**`{name}`**' if name.endswith('155.33041597700.json') else f'`{name}`'
    print(f'| {label} | {raw!r} | {via} | {delta} |')
    previous = raw
final = LINE[-1][1]
print()
print(f'net {final - PRIOR:+.11f} mm from the prior record')
row = index['pinned-fs-155.33041597700.json']
print('final sha256', row['sha256'])
print('final fingerprint', row['fingerprint'])
