#!/usr/bin/env python3
"""Extract a run's published layout into a pinned parent fixture and replay it."""

import json
import sys

REQ_SHA = 'ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3'


def extract(run_path, out_path, description):
    with open(run_path) as handle:
        run = json.load(handle)
    pop = (run['relaxedDiagnostics']['coupledDynamicSeparator']
           ['persistentVacancyPopulation'])
    assert pop['exactValid'], 'refusing to pin a state that did not validate'
    placements = [{
        'pieceId': p['pieceId'],
        'rotationDeg': p['rotationDeg'],
        'mirrored': p['mirrored'],
        'translateShortAxis': p['translateShortAxis'],
        'translateLongAxis': p['translateLongAxis'],
    } for p in pop['finalPlacements']]
    depth = pop['independentDepthMm']
    fixture = {
        'schemaVersion': 1,
        'description': description,
        'requestSha256': REQ_SHA,
        'expectedPlacementFingerprint': pop['finalPlacementFingerprint'],
        'reportedDepthMm': depth,
        'independentDepthMm': depth,
        'provenance': {'producedBy': description},
        'placements': placements,
    }
    with open(out_path, 'w') as handle:
        json.dump(fixture, handle, indent=1)
    print(f'pinned {out_path}: depth={depth} raw={pop["rawSourceDepthMm"]} '
          f'fp={pop["finalPlacementFingerprint"][:16]} pieces={len(placements)}')
    return out_path


if __name__ == '__main__':
    extract(sys.argv[1], sys.argv[2], sys.argv[3])
