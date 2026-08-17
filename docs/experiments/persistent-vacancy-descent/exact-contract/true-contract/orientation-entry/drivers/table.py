#!/usr/bin/env python3
"""Assembles the mechanism x perturbation x line results table and the
accepted-pose attribution totals from every run JSON produced."""
import sys, json, glob, os, collections
sys.path.insert(0, '/var/lib/t3/tmp/orient')
import drv

RUNS = '/var/lib/t3/tmp/orient/runs'


def scan(pattern, incumbent):
    rows = []
    for path in sorted(glob.glob(f'{RUNS}/{pattern}')):
        try:
            doc = json.load(open(path))
        except json.JSONDecodeError:
            # A run still being written by a live job.
            continue
        pop = drv.lib.population(doc) or {}
        block = drv.blk(doc) or {}
        attr = drv.attribution(doc)
        published = drv.published_raw(doc)
        rows.append({
            'tag': os.path.basename(path)[:-5],
            'mode': pop.get('mode'),
            'published': published,
            'belowIncumbent': published is not None and published < incumbent - 1e-12,
            'orientationVariants': attr.get('variants', 0),
            'orientationCandidates': attr.get('candidates', 0),
            'orientationRows': attr.get('rows', 0),
            'orientationFinalists': attr.get('finalists', 0),
            'acceptedVacated': attr.get('acceptedVacated', 0),
            'acceptedAnchorLocal': attr.get('acceptedAnchorLocal', 0),
            'acceptedOrientation': attr.get('acceptedOrientation', 0),
            'acceptedStation': attr.get('acceptedStation', 0),
            'acceptedAngles': attr.get('acceptedAngles', []),
            'failure': (block.get('skippedReason') or block.get('rejectionReason')
                        or pop.get('failureReason') or ''),
        })
    return rows


def totals(rows):
    out = collections.OrderedDict()
    for mode in (28, 29, 32, 33):
        sub = [r for r in rows if r['mode'] == mode]
        if not sub:
            continue
        out[str(mode)] = {
            'arms': len(sub),
            'exactValidPublications': sum(1 for r in sub if r['published'] is not None),
            'publicationsBelowIncumbent': sum(1 for r in sub if r['belowIncumbent']),
            'orientationCandidates': sum(r['orientationCandidates'] for r in sub),
            'orientationRows': sum(r['orientationRows'] for r in sub),
            'orientationFinalists': sum(r['orientationFinalists'] for r in sub),
            'acceptedVacated': sum(r['acceptedVacated'] for r in sub),
            'acceptedAnchorLocal': sum(r['acceptedAnchorLocal'] for r in sub),
            'acceptedOrientation': sum(r['acceptedOrientation'] for r in sub),
            'acceptedStation': sum(r['acceptedStation'] for r in sub),
        }
    return out


def failure_classes(rows):
    counter = collections.Counter()
    for row in rows:
        if row['published'] is not None:
            continue
        reason = row['failure']
        if 'no exact-valid pose for piece' in reason:
            key = 'no exact-valid pose for the ejected piece inside the bound'
        elif 'no insertion order re-placed' in reason:
            key = 'no insertion order / swap / beam combination re-placed the component'
        elif 'no violating pair' in reason:
            key = 'perturbation produced no violating pair (structural non-experiment)'
        elif 'exceeds the local-repair limit' in reason:
            key = 'ejection set above the local-repair limit of 7'
        elif 'above the local-repair limit' in reason:
            key = 'violation component above the component limit'
        elif reason:
            key = reason[:70]
        else:
            key = 'published a state that was not below the incumbent'
        counter[key] += 1
    return dict(counter.most_common())


if __name__ == '__main__':
    report = {}
    for name, pattern, incumbent in (
            ('recordLine', 'rec-*', drv.RECORD_RAW),
            ('fromScratchLine', 'fs-*', drv.SCRATCH_RAW),
            ('fromScratchPads', 'pad*', drv.SCRATCH_RAW),
            ('recordCascade', 'r[0-9]*', drv.RECORD_RAW)):
        rows = scan(pattern, incumbent)
        report[name] = {
            'arms': len(rows),
            'incumbent': incumbent,
            'byMode': totals(rows),
            'failureClasses': failure_classes(rows),
            'acceptedOrientationAngles': [a for r in rows for a in r['acceptedAngles']],
        }
    json.dump(report, open('/var/lib/t3/tmp/orient/table.json', 'w'), indent=1)
    print(json.dumps({k: {'arms': v['arms'], 'byMode': v['byMode']} for k, v in report.items()},
                     indent=1))
