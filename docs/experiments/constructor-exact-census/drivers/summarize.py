#!/usr/bin/env python3
"""Collects this stage's artifacts into one evidence document.

    python3 summarize.py [outpath]
"""
import json
import os
import sys

RUNS = '/var/lib/t3/tmp/ccensus'
HERE = os.path.dirname(os.path.abspath(__file__))
out_path = sys.argv[1] if len(sys.argv) > 1 else f'{HERE}/../evidence.json'


def load(path, default=None):
    try:
        return json.load(open(path))
    except (OSError, json.JSONDecodeError):
        return default


census = load(f'{RUNS}/census/census-g1.json', {}).get('census', {})
totals = census.get('totals', {})
sites = census.get('bySite', {})


def ratio(numerator, denominator):
    return None if not denominator else numerator / denominator


derived = {}
if totals:
    clipper = totals['pairsReachingClipper']
    clean = clipper - totals['pairsClipperOverlapping']
    union_floor = max(totals['cleanSeparatedByHull'], totals['cleanSeparatedBySlabs'])
    derived = {
        'pairsOffered': totals['pairsOffered'],
        'pairsRejectedByAabb': totals['pairsRejectedByAabb'],
        'aabbRejectShare': ratio(totals['pairsRejectedByAabb'],
                                 totals['pairsOffered']),
        'pairsReachingClipper': clipper,
        'clipperOverlapping': totals['pairsClipperOverlapping'],
        'clipperClean': clean,
        'cleanShareOfClipperQueries': ratio(clean, clipper),
        'cleanSeparatedBySlabs': totals['cleanSeparatedBySlabs'],
        'cleanSeparatedByHull': totals['cleanSeparatedByHull'],
        'slabsShareOfClean': ratio(totals['cleanSeparatedBySlabs'], clean),
        'hullShareOfClean': ratio(totals['cleanSeparatedByHull'], clean),
        'hullShareOfAllClipperQueries': ratio(totals['cleanSeparatedByHull'],
                                              clipper),
        'atLeastOneTierShareOfCleanFloor': ratio(union_floor, clean),
        'collisionBuilds': totals['collisionBuilds'],
        'collisionBuildsWasted': totals['collisionBuildsWasted'],
        'wastedBuildShare': ratio(totals['collisionBuildsWasted'],
                                  totals['collisionBuilds']),
        'rows': totals['rows'],
        'rowsRejectedByContainment': totals['rowsRejectedByContainment'],
        'rowsRejectedByOverlap': totals['rowsRejectedByOverlap'],
        'rowsAccepted': totals['rowsAccepted'],
        'soundnessViolationsSlabs': totals['soundnessViolationsSlabs'],
        'soundnessViolationsHull': totals['soundnessViolationsHull'],
        'meanClipperInputVerticesPerQuery': ratio(
            totals['clipperInputVertices'], clipper),
    }

evidence = {
    'description': (
        'The constructor exact-confirmation census, and the grid-exact '
        'separation prefilter it sized. Every timing figure is a paired '
        'interleaved A/B on a box shared with another benchmarking agent; no '
        'absolute time here is a claim.'),
    'request': 'tests/fixtures/mixed-61/mixed61-request-exact-clearance.json',
    'baseCommit': '0cf1163',
    'platform': {'arch': 'x86_64', 'cores': 16},
    'census': {
        'stream': 'mode 20, gate 1 (parent ex5-seed-native.json, target 320.000)',
        'build': 'jagua-experimental,constructor-census,search-profiling',
        'caveat': (
            'a counting build runs a convex hull on every observed pair, so '
            'its clock is meaningless; only the counts are quotable'),
        'gateReproduced': load(f'{RUNS}/census/census-g1.json', {})
                          .get('gateCheck', {}).get('hit'),
        'totals': totals,
        'bySite': sites,
        'derived': derived,
    },
    'prefilter': {
        'flag': 'fast-constructor-confirm (stacked on fast-constructor-profile)',
        'pairedInterleavedAB': {
            'sample1': load(f'{RUNS}/ab/ab-profile-confirm-g1.json'),
            'sample2': load(f'{RUNS}/ab/ab-profile-confirmB-g1.json'),
            'preAllocationRework': load(f'{RUNS}/ab/ab-profile-confirm2-g1.json'),
        },
        'phaseProfile': load(f'{RUNS}/profile/profile.json'),
        'qualityGate': load(f'{RUNS}/quality/quality-gate.json'),
    },
    'gates': {
        'base': load(f'{RUNS}/gates/base/gates-base.json'),
        'worktreeDefaultFeatures': load(
            f'{RUNS}/gates/worktree/gates-worktree.json'),
        'debugAssertLive': load(
            f'{RUNS}/gates/debugconfirm/gates-debugconfirm.json'),
    },
    'wholeDocumentDiffs': {
        name: load(f'{RUNS}/diff/{name}.json')
        for name in os.listdir(f'{RUNS}/diff')
        if name.startswith('diff-') and name.endswith('.json')
    } if os.path.isdir(f'{RUNS}/diff') else {},
}
json.dump(evidence, open(out_path, 'w'), indent=1)
print(json.dumps(derived, indent=1))
print(f'wrote {out_path}')
