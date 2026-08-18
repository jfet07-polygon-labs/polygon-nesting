#!/usr/bin/env python3
"""Collects every artefact this stage produced into one `evidence.json`.

    python3 summarize.py [outputPath]

Reads whatever the other drivers left under `/var/lib/t3/tmp/cinner` and folds
it into the document the README quotes from, so no number in the write-up is
transcribed by hand.
"""
import glob
import json
import os
import sys

TMP = '/var/lib/t3/tmp/cinner'
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'evidence.json')


def load(path):
    try:
        return json.load(open(path))
    except (OSError, json.JSONDecodeError):
        return None


def shares(census):
    """The derived percentages the README quotes, computed once, here."""
    out = {}
    for site, row in list(census['bySite'].items()) + [('deepTotal', census['totals'])]:
        rows = row['rows']
        if not rows:
            continue
        overlap = row['rowsRejectedByOverlap']
        out[site] = {
            'rows': rows,
            'accepted': row['rowsAccepted'],
            'acceptedShare': row['rowsAccepted'] / rows,
            'rejectedByOverlap': overlap,
            'rejectedByContainment': row['rowsRejectedByContainment'],
            'certified': {k: row[f'rowsCertified{k}'] for k in (1, 2, 4, 8)},
            'certifiedUninflated': row['rowsCertifiedUninflated'],
            'certified4ShareOfRows': row['rowsCertified4'] / rows,
            'certified4ShareOfOverlapRejects': (
                row['rowsCertified4'] / overlap if overlap else None),
            'certified8ShareOfOverlapRejects': (
                row['rowsCertified8'] / overlap if overlap else None),
            'uninflatedShareOfCertified8': (
                row['rowsCertifiedUninflated'] / row['rowsCertified8']
                if row['rowsCertified8'] else None),
            'soundnessViolations': row['soundnessViolationsCertificate'],
        }
    return out


evidence = {}

census = load(f'{TMP}/census/census-g1.json')
if census:
    evidence['census'] = {
        'gateReproduced': census['gateCheck']['hit'],
        'countingBuildWallSeconds': census['countingBuildWallSeconds'],
        'raw': census['census'],
        'derived': shares(census['census']),
    }
    order = census['census']['candidateOrdering']
    evidence['census']['ordering'] = dict(order, **{
        'proxyShareOfActual': order['prefixProxy'] / order['prefixActual'],
        'proxyReductionFactor': order['prefixActual'] / order['prefixProxy'],
    })

evidence['gates'] = {
    os.path.basename(path)[len('gates-'):-len('.json')]: load(path)
    for path in sorted(glob.glob(f'{TMP}/gates/*/gates-*.json'))
}

evidence['documentDiffs'] = {}
for path in sorted(glob.glob(f'{TMP}/diff/diff-*.json')):
    name = os.path.basename(path)[len('diff-'):-len('.json')]
    doc = load(path)
    if doc:
        evidence['documentDiffs'][name] = {
            'fieldsCompared': doc['fieldsCompared'],
            'differingFields': [row['field'] for row in doc['diffs']],
            'values': {row['field'].split('/')[-1]: [
                v for k, v in row.items() if k != 'field']
                for row in doc['diffs']
                if 'Elapsed' not in row['field']
                and 'executable' not in row['field']},
        }

evidence['profile'] = load(f'{TMP}/profile/profile.json')
evidence['qualityGate'] = load(f'{TMP}/quality/quality-gate.json')
evidence['abSamples'] = {
    os.path.basename(path)[len('ab-'):-len('.json')]: {
        k: v for k, v in (load(path) or {}).items() if k != 'rows'}
    for path in sorted(glob.glob(f'{TMP}/ab/ab-*.json'))
}

json.dump(evidence, open(OUT, 'w'), indent=1)
print(json.dumps({
    'wrote': OUT,
    'sections': sorted(evidence),
    'censusSoundnessViolations': (
        evidence.get('census', {}).get('raw', {})
        .get('totals', {}).get('soundnessViolationsCertificate')),
    'qualityGateMaxAbsDelta': (
        evidence.get('qualityGate') or {}).get('maxAbsDescendedDelta'),
}, indent=1))
