#!/usr/bin/env python3
"""The overlay's setup cost, before and after Sol review 6 §2.2's fix.

    python3 setupcost.py OUT.json LABEL=BINARY [LABEL=BINARY ...]

§2.2:

    `catalog.orientations.clone()` clona poligoni, triangoli, assi, poles e
    indici per tutte le rotazioni solo per aggiungere poche entry. È esattamente
    il tipo di costo setup che non possiamo introdurre nel path 10s.

`currentPoseOverlaySetupMs` is the engine's own measurement of that cost:
`Instant` around `build_current_pose_overlay` plus the installation step, and
nothing else. Running the same parent through two binaries that differ only in
the installation step therefore prices the installation directly, rather than
inferring it from a process wall dominated by everything else mode 34 does.

Each arm runs every campaign parent at a one-unit work budget: the overlay is
built before the schedule takes its first step, so no budget is needed to reach
the number, and a one-unit run keeps the surrounding noise small.

Arms are run back to back per parent, with the order reversed on alternate
parents, for the same reason `flagoff.py` does it.
"""
import json
import os
import statistics
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import campaign  # noqa: E402


def run(binary, parent, outdir, tag):
    args = [a.format(pressure='structured') for a in campaign.ARGS]
    target = parent['depthMm'] - campaign.DEFAULT_DROP_MM
    allowance = parent.get('allowance', campaign.DEFAULT_ALLOWANCE)
    tail = ['34', parent['fixture'], f'{target:.17g}', '', allowance]
    env = dict(os.environ)
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = \
        f'{campaign.SCHEDULE_V4},work=1'
    env['POLYGON_NESTING_CURRENT_POSE_OVERLAY'] = '1'
    env.pop('POLYGON_NESTING_CURRENT_POSE_OVERLAY_CLASSIFY', None)
    env.pop('POLYGON_NESTING_PROFILE', None)
    path = f"{outdir}/{parent['name']}-{tag}.json"
    os.makedirs(outdir, exist_ok=True)
    with open(path, 'w') as handle:
        subprocess.run([binary, campaign.REQUEST] + args + tail, stdout=handle,
                       stderr=subprocess.DEVNULL, check=False, env=env)
    try:
        doc = json.load(open(path))
    except json.JSONDecodeError:
        return None
    pop = ((doc.get('relaxedDiagnostics') or {})
           .get('coupledDynamicSeparator') or {}).get(
               'persistentVacancyPopulation')
    return (pop or {}).get('compressionSchedule')


def main():
    out_path = sys.argv[1]
    arms = []
    for item in sys.argv[2:]:
        label, _, binary = item.partition('=')
        arms.append((label, binary))
    outdir = os.path.dirname(os.path.abspath(out_path)) + '/setupcost-runs'

    rows = []
    for index, parent in enumerate(campaign.PORT_PARENTS):
        order = arms if index % 2 == 0 else list(reversed(arms))
        row = {'parent': parent['name']}
        for label, binary in order:
            sched = run(binary, parent, outdir, label)
            if sched is None:
                row[label] = {'error': 'no compressionSchedule'}
                continue
            row[label] = {
                'setupMs': sched.get('currentPoseOverlaySetupMs'),
                'entries': sched.get('currentPoseOverlayEntries'),
                'offGridPieces': sched.get('currentPoseOverlayOffGridPieces'),
            }
        rows.append(row)
        print(json.dumps(row), flush=True)

    summary = {}
    for label, _ in arms:
        values = [r[label]['setupMs'] for r in rows
                  if isinstance(r.get(label), dict) and r[label].get('setupMs')
                  is not None]
        summary[label] = {
            'parents': len(values),
            'setupMsMedian': statistics.median(values) if values else None,
            'setupMsMin': min(values) if values else None,
            'setupMsMax': max(values) if values else None,
            'setupMsTotal': sum(values) if values else None,
        }
    if len(arms) == 2:
        # `reference` is arm 1 (the *before*), `other` is arm 2 (the *after*).
        # Parents whose overlay is empty are excluded: their setup is zero on
        # both arms by construction and would only dilute the ratio.
        reference, other = arms[0][0], arms[1][0]
        paired = [(r[reference]['setupMs'], r[other]['setupMs']) for r in rows
                  if isinstance(r.get(reference), dict)
                  and isinstance(r.get(other), dict)
                  and r[reference].get('setupMs')
                  and r[other].get('setupMs') is not None]
        summary['paired'] = {
            'reference': reference,
            'other': other,
            'parentsWithOverlayEntries': len(paired),
            'otherOverReferenceRatioMedian':
                statistics.median([o / ref for ref, o in paired])
                if paired else None,
            'speedupMedian':
                statistics.median([ref / o for ref, o in paired if o > 0])
                if paired else None,
            'savedMsMedian':
                statistics.median([ref - o for ref, o in paired])
                if paired else None,
            'savedMsTotal': sum(ref - o for ref, o in paired) if paired else None,
        }
    out = {'arms': [{'label': l, 'binary': b} for l, b in arms],
           'schedule': f'{campaign.SCHEDULE_V4},work=1',
           'summary': summary, 'rows': rows}
    json.dump(out, open(out_path, 'w'), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
