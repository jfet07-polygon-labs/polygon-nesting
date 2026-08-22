#!/usr/bin/env python3
"""The decomposition Sol review 10 §3 asks for when the gate fails.

    why.py PROBEDIR BLOCKPROBEJSON [OUT]

Sol names three candidate explanations for a negative and says the deliverable
is which one it is: **no components**, **blocks rejected by exact**, or **gains
dominated**. They are distinguishable, so this reducer separates them off the
raw per-round documents rather than off a summary:

* *no components* would show as `no-component` refusals and blocks of size 1.
* *rejected by exact* would show as `exact-rejected` refusals and a low
  `fullStepExactValid` rate.
* *gains dominated* shows as a round that **moved** but whose `validatedDeltaMm`
  sits at its own `headroomMm` - the block moved, the program was not the
  binding constraint, and the piece behind it became the setter. That is the one
  that needs proving rather than asserting, so it is measured directly: the
  share of productive rounds whose validated delta is within a grid step of the
  ceiling the layout imposed on them.

`headroomAtCeilingShare` is therefore the headline number of a failed gate. A
value near 1 says the operator is not the thing that is short.
"""
import glob
import json
import os
import statistics
import sys

GRID_MM = 1e-6


def census(paths):
    rounds = []
    for path in paths:
        try:
            doc = json.load(open(path))
        except json.JSONDecodeError:
            continue
        if 'rounds' not in doc:
            continue
        for entry in doc['rounds']:
            rounds.append(entry)
    if not rounds:
        return None
    refusals = {}
    for entry in rounds:
        key = entry.get('refusal') or 'moved'
        refusals[key] = refusals.get(key, 0) + 1
    moved = [e for e in rounds if e.get('refusal') is None]
    at_ceiling = [e for e in moved
                  if e.get('headroomMm') is not None
                  and e['headroomMm'] != float('inf')
                  and abs(e['validatedDeltaMm'] - e['headroomMm']) <= GRID_MM]
    priced = [e for e in rounds if e.get('rows')]
    headrooms = [e['headroomMm'] for e in rounds
                 if e.get('headroomMm') is not None
                 and e['headroomMm'] != float('inf')]
    uppers = [e['modelUpperMm'] for e in rounds
              if e.get('modelUpperMm') is not None]
    blocks = [len(e['block']) for e in rounds]
    edges = [len(e['edges']) for e in rounds]
    band = [e['depthBandPieces'] for e in rounds
            if e.get('depthBandPieces') is not None]
    return {
        'rounds': len(rounds),
        'refusalCount': dict(sorted(refusals.items(), key=lambda x: -x[1])),
        'refusalShare': {k: v / len(rounds) for k, v in
                         sorted(refusals.items(), key=lambda x: -x[1])},
        'roundsMoved': len(moved),
        'roundsAtHeadroomCeiling': len(at_ceiling),
        'headroomAtCeilingShare': (len(at_ceiling) / len(moved)
                                   if moved else None),
        'roundsWithComponentOfTwoOrMore': sum(1 for b in blocks if b >= 2),
        'componentShare': sum(1 for b in blocks if b >= 2) / len(rounds),
        'medianBlockSize': statistics.median(blocks),
        'medianContactEdges': statistics.median(edges),
        'medianDepthBandPieces': statistics.median(band) if band else None,
        'medianHeadroomMm': statistics.median(headrooms) if headrooms else None,
        'medianModelUpperMm': statistics.median(uppers) if uppers else None,
        'fullStepExactValidShare': (
            sum(1 for e in priced if e['fullStepExactValid']) / len(priced)
            if priced else None),
        'medianScale': statistics.median(
            [e['scale'] for e in priced]) if priced else None,
        'medianValidatedDeltaMm': statistics.median(
            [e['validatedDeltaMm'] for e in rounds]),
        'medianMaxAbsDthetaDeg': statistics.median(
            [e['maxAbsDthetaDeg'] for e in rounds]),
        'medianMaxAbsTranslationMm': statistics.median(
            [e['maxAbsTranslationMm'] for e in rounds]),
    }


def main():
    probe_dir, probe_json = sys.argv[1], sys.argv[2]
    out_path = sys.argv[3] if len(sys.argv) > 3 else None
    probe = json.load(open(probe_json))
    result = {'probeDir': probe_dir, 'source': probe_json, 'specs': {}}
    for spec in probe['specs']:
        tag = spec.replace('=', '').replace(',', '-').replace('.', 'p')
        paths = sorted(glob.glob(os.path.join(probe_dir, f'seed*-{tag}.json')))
        entry = census(paths)
        if entry:
            entry['documents'] = len(paths)
            result['specs'][spec] = entry
    print(json.dumps(result, indent=1))
    if out_path:
        json.dump(result, open(out_path, 'w'), indent=1)


if __name__ == '__main__':
    main()
