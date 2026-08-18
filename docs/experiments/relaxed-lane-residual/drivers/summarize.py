#!/usr/bin/env python3
"""Collects every artifact this stage produced into one evidence document.

    python3 summarize.py [outputPath]

Reads the driver output directories under /tmp/rl and writes the numbers this
stage's README quotes, as measured. Nothing here recomputes a measurement; it
only reshapes what the runners already wrote, so a number in the README can be
traced to the file it came out of.
"""
import json
import os
import sys

OUT = sys.argv[1] if len(sys.argv) > 1 else (
    os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                 'evidence.json'))
ROOT = '/tmp/rl'


def load(path, default=None):
    try:
        with open(path) as handle:
            return json.load(handle)
    except (OSError, json.JSONDecodeError):
        return default


def scan_shape(counters, generic_scans):
    """The candidate scan's structure, per generic scan."""
    if not counters or not generic_scans:
        return None
    returned = counters['scanNeighborsReturned']
    visited = counters['scanNeighborsVisited']
    return {
        'genericScans': generic_scans,
        'candidateQueries': counters['candidateQueries'],
        'dynamicPathCalls': counters['candidateQueries'] - generic_scans,
        'catalogDescents': counters['scanCatalogDescents'],
        'neighborsReturned': returned,
        'neighborsVisited': visited,
        'neighborsReturnedPerScan': returned / generic_scans,
        'neighborsVisitedPerScan': visited / generic_scans,
        'neighborsReturnedNeverVisitedPercent':
            (returned - visited) * 100.0 / returned,
        'upperBoundCutoffs': counters['scanUpperBoundCutoffs'],
        'upperBoundCutoffPercent':
            counters['scanUpperBoundCutoffs'] * 100.0 / generic_scans,
        'collisionRows': counters['scanCollisionRows'],
        'collisionRowsPerScan': counters['scanCollisionRows'] / generic_scans,
        'collideRatePercent': counters['scanCollisionRows'] * 100.0 / visited,
        # The two lookups each arm removes, as a share of every descent.
        'descentsRemovedByShapeReuse': generic_scans,
        'descentsRemovedByCachedPoseBounds':
            counters.get('boundaryPenaltyCalls'),
    }


def phases_of(node):
    return {name: row for name, row in (node or {}).get('phases', {}).items()}


evidence = {
    'stage': 'relaxed-lane-residual',
    'baseCommit': 'b522373ebbc01dd22b19831256fc52f1bcd9c289',
    'request': 'tests/fixtures/mixed-61/mixed61-request-exact-clearance.json',
    'box': {'cpu': 'Intel(R) Core(TM) Ultra 7 270K Plus', 'lanes': 8},
    'caveats': [
        'Phase milliseconds are summed across the eight lane threads, so a '
        'phase total may exceed the stream wall clock and is not a wall-clock '
        'claim.',
        'The census build arms three spans and five counters inside a scorer '
        'that runs millions of times per stream; it costs about 9% of the '
        'mode-20 stream, so its milliseconds are a decomposition and only its '
        'counters are exact.',
        'Every wall-clock number is a paired interleaved A/B with arms '
        'alternating order each round, never a profiled build.',
    ],
    'decomposition': {},
    'ab': {},
    'equivalence': {},
    # Transcribed rather than parsed: the suite writes its totals across many
    # `test result:` lines and its exit status is read from a file, never
    # through a pipe.
    'suite': {
        'command': 'cargo test --release --features <set>',
        'results': {
            'jagua-experimental': {'passed': 1238, 'failed': 0, 'exit': 0},
            'jagua-experimental,relaxed-scan-shape-reuse,'
            'relaxed-cached-pose-bounds':
                {'passed': 1238, 'failed': 0, 'exit': 0},
            'jagua-experimental,search-profiling,relaxed-lane-census':
                {'passed': 1238, 'failed': 0, 'exit': 0},
        },
        'note': 'cargo build --release with no features also succeeds. '
                'cargo fmt --all -- --check fails at this commit on eleven '
                'files; ten are untouched by this stage and fail at the base '
                'commit b522373 too, and the eleventh (general_relaxed.rs) has '
                'no remaining diff in the lines this stage wrote.',
    },
}

for tag in ('g1', 'g2'):
    both = load(f'{ROOT}/decompose/decompose-{tag}.json') or {}
    node = both.get('fcpcensus')
    if not node:
        continue
    allocator = both.get('fcpalloc')
    phases = phases_of(node)
    generic = (phases.get('scoreScan') or {}).get('calls')
    if allocator:
        counters = allocator['counters']
        evidence.setdefault('allocation', {})[tag] = {
            'binary': 'the census build plus profiling-allocator; gross demand, '
                      'not residency - dealloc is not subtracted and a realloc '
                      'counts as a fresh request for the whole new size',
            'engineElapsedSeconds': allocator['engineElapsedSeconds'],
            'allocationCount': counters['allocationCount'],
            'allocationBytes': counters['allocationBytes'],
            'genericScans': generic,
            'allocationsPerGenericScan': counters['allocationCount'] / generic,
            'collisionRows': counters['scanCollisionRows'],
        }
    evidence['decomposition'][tag] = {
        'binary': 'jagua-experimental,fast-constructor-profile,'
                  'fast-constructor-confirm,fast-constructor-reject,'
                  'search-profiling,relaxed-lane-census',
        'engineElapsedSeconds': node['engineElapsedSeconds'],
        'threads': node['threads'],
        'leafMilliseconds': node['leafMilliseconds'],
        'gateHit': node['gateHit'],
        'phases': phases,
        'counters': node['counters'],
        'scanShape': scan_shape(node['counters'], generic),
    }

coordinator = (load(f'{ROOT}/coordinator/coordinator-10000.json') or {}).get('census')
if coordinator:
    phases = phases_of(coordinator)
    generic = (phases.get('scoreScan') or {}).get('calls')
    evidence['decomposition']['coordinator10s'] = {
        'spec': coordinator['spec'],
        'coordinatorSeconds': coordinator['coordinatorSeconds'],
        'rawDepthMm': coordinator['rawDepthMm'],
        'dualGateValid': coordinator['dualGateValid'],
        'threads': coordinator['threads'],
        'leafMilliseconds': coordinator['leafMilliseconds'],
        'phases': phases,
        'counters': coordinator['counters'],
        'scanShape': scan_shape(coordinator['counters'], generic),
    }

for name, path in (
        ('shapeReuse-g1', f'{ROOT}/ab/ab-fcp-fcpreuse-g1.json'),
        ('shapeReuse-g2', f'{ROOT}/ab/ab-fcp-fcpreuse-g2.json'),
        ('both-g1', f'{ROOT}/ab/ab-fcp-fcpboth-g1.json'),
        ('both-g2', f'{ROOT}/ab/ab-fcp-fcpboth-g2.json'),
        # Rebuilt from the committed tree, so the headline is reproducible from
        # the commit rather than from the tree it was measured in.
        ('both-g2-rebuiltFromCommit', f'{ROOT}/ab/ab-finalA-finalB-g2.json'),
):
    row = load(path)
    if row:
        evidence['ab'][name] = {k: v for k, v in row.items() if k != 'rows'}
        evidence['ab'][name]['rows'] = row['rows']

coordab = load(f'{ROOT}/coordab/coordab-fcp-fcpboth-20000000.json')
if coordab:
    outcomes = coordab.get('outcomes', {})
    stripped = {}
    for label, values in outcomes.items():
        seen = set()
        for value in values:
            parsed = json.loads(value)
            for publication in parsed.get('publications') or []:
                publication.pop('seconds', None)
            seen.add(json.dumps(parsed, sort_keys=True))
        stripped[label] = sorted(seen)
    evidence['ab']['coordinatorWorkBudget'] = {
        k: v for k, v in coordab.items() if k not in ('rows', 'outcomes')}
    evidence['ab']['coordinatorWorkBudget']['rows'] = coordab['rows']
    evidence['ab']['coordinatorWorkBudget']['outcomesWithoutWallClock'] = stripped
    evidence['ab']['coordinatorWorkBudget'][
        'outcomesIdenticalAcrossArmsIgnoringWallClock'] = (
        len(set(sum(stripped.values(), []))) == 1)

for name, path in (
        ('base-vs-default', f'{ROOT}/diff/diffall-base-default.json'),
        ('default-vs-shapeReuse', f'{ROOT}/diff/diffall-default-reuse.json'),
        ('default-vs-both', f'{ROOT}/diff/diffall-default-both.json'),
        ('fcp-vs-fcpBoth', f'{ROOT}/diff/diffall-fcp-fcpboth.json'),
):
    row = load(path)
    if row:
        evidence['equivalence'][name] = {
            tag: {'fieldsCompared': node['fieldsCompared'],
                  'fieldsDiffering': len(node['diffs']),
                  'differingFields': [d['field'] for d in node['diffs']]}
            for tag, node in row.items()}

for name, path in (('default', f'{ROOT}/gates_default.json'),
                   ('shapeReuse', f'{ROOT}/gates_reuse.json'),
                   ('base', f'{ROOT}/gates_base.json')):
    row = load(path)
    if row:
        evidence['equivalence'].setdefault('gates', {})[name] = {
            'ALL_PASS': row['ALL_PASS'],
            'gates': {tag: {k: v for k, v in node.items()
                            if k in ('hit', 'raw', 'fp', 'depth', 'depths',
                                     'exactValid', 'contractValid')}
                      for tag, node in row['gates'].items()}}

json.dump(evidence, open(OUT, 'w'), indent=1)
print(json.dumps({'wrote': OUT,
                  'decompositions': sorted(evidence['decomposition']),
                  'abSamples': sorted(evidence['ab']),
                  'equivalenceSets': sorted(evidence['equivalence'])}, indent=1))
