#!/usr/bin/env python3
"""The per-confirmation wall on the fixtures the previous round did not run.

    python3 wallfixtures.py OUTDIR OFF_BINARY ON_BINARY ROUNDS [SPEC]

docs/experiments/fast-contract-validator/ §5 named the gap itself: "Only
mixed-61 was measured... shapes-17 and triangle-20 [were not] run", and "the 96%
is measured at one density... the skip rate at the 155 mm record line was not
measured and could be materially lower". Sol review 7 §1 turned that into a
promotion blocker. This closes it on four fronts:

  shapes-17     a different piece profile entirely
  triangle-20   the fixture whose pieces are most nearly identical
  small-N       eight pieces: 28 pairs against mixed-61's 1,830
  record-155    the 155.264 mm record-lineage parent, where the layout is
                ~15 mm tighter than the 171-179 mm band the census used

The protocol is `wall.py`'s, unchanged and for the same reason: paired,
interleaved, arm order reversed on odd rounds, equal walk (`past=0`, no work
cap, fixed drop), census disabled, and the within-arm spread printed beside
every ratio because the box is shared. `stepsTaken`, `confirmationsAccepted`,
`rawSourceDepthMm` and `fingerprint` must agree between arms or the cell is
void, and the summary says so per fixture rather than in aggregate.

**The density prediction is falsifiable here.** The filter proves pairs clear,
so a tighter layout should prove fewer and the speedup should fall. If
record-155 shows the same 5.6x as the 171-179 mm band, the mechanism is not what
§3.1 says it is.
"""
import hashlib
import json
import os
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

DEFAULT_SPEC = 'past=0,rollback=0,lanes=1,pconfirm=0'

ROOT = runlib.ROOT
TRUE = (f'{ROOT}/docs/experiments/persistent-vacancy-descent/exact-contract/'
        'true-contract')
EVIDENCE = f'{ROOT}/docs/experiments/fast-contract-validator/evidence'

# label -> (request, parent fixture, drop mm, allowance)
#
# The drop is what makes the walk equal and non-trivial: the schedule descends
# from the parent's depth by this much, confirming as it goes.
#
# The three generated parents are read from `../evidence/parents/`, which is
# where `buildparents.py` output was committed, so this driver is reproducible
# from the repository alone rather than from a scratch directory that will not
# survive the round. `record-155` is the campaign's own pinned record parent and
# is read from its committed home.
FIXTURES = {
    'shapes-17': {
        'request': f'{ROOT}/tests/fixtures/shapes-17/2000x2700-compact/request.json',
        'parent': f'{EVIDENCE}/parents/shapes-17.json',
        'drop': 1.5,
        'allowance': '0.002',
    },
    'triangle-20': {
        'request': f'{ROOT}/tests/fixtures/triangle-20/2000x2700-compact/request.json',
        'parent': f'{EVIDENCE}/parents/triangle-20.json',
        'drop': 1.5,
        'allowance': '0.002',
    },
    'small-8': {
        'request': f'{ROOT}/tests/vectors/core/'
                   'thread-equality-mixed61-8-piece-request.json',
        'parent': f'{EVIDENCE}/parents/small-8.json',
        'drop': 1.5,
        'allowance': '0.002',
    },
    # The record lineage: exact-clearance request, empty warm start, 0.0005
    # allowance - the tail the four pinned gates use, not the 0.002 from-request
    # one, because this parent lives on the record line.
    'record-155': {
        'request': f'{ROOT}/tests/fixtures/mixed-61/'
                   'mixed61-request-exact-clearance.json',
        'parent': f'{TRUE}/orientation-floor/pinned-fs-155.26442950833.json',
        'drop': 0.3,
        'allowance': '0.0005',
    },
}


def parent_depth(path):
    doc = json.load(open(path))
    return doc.get('independentDepthMm') or doc.get('reportedDepthMm')


def run_arm(binary, request, parent, target, allowance, spec, out_path,
            seed=0):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['34', parent, f'{target:.17g}', '', allowance]
    command = [binary, request] + args + tail
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env.pop('POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS', None)
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = spec
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    try:
        doc = json.load(open(out_path))
    except json.JSONDecodeError:
        return {'error': (proc.stderr or b'').decode()[-400:],
                'processWallSeconds': wall, 'exitCode': proc.returncode}
    pop = ((doc.get('relaxedDiagnostics') or {})
           .get('coupledDynamicSeparator') or {}).get(
               'persistentVacancyPopulation') or {}
    schedule = pop.get('compressionSchedule') or {}
    accepted = schedule.get('confirmationsAccepted') or 0
    confirmation_ms = schedule.get('confirmationMs')
    return {
        'processWallSeconds': wall,
        'confirmationMs': confirmation_ms,
        'confirmationsAccepted': accepted,
        'confirmationsAttempted': schedule.get('confirmationsAttempted'),
        'perConfirmationMs': (confirmation_ms / accepted
                              if confirmation_ms is not None and accepted
                              else None),
        'repairMs': schedule.get('repairMs'),
        'sliceMs': (schedule.get('repairMs') or 0) + (confirmation_ms or 0),
        'stepsTaken': schedule.get('stepsTaken'),
        'workUnits': schedule.get('workUnits'),
        'candidateQueries': schedule.get('candidateQueries'),
        'rawSourceDepthMm': pop.get('rawSourceDepthMm'),
        'fingerprint': pop.get('finalPlacementFingerprint'),
        'exactValid': pop.get('exactValid'),
        'contractValid': pop.get('contractValid'),
    }


def spread(values):
    values = [v for v in values if v is not None]
    if not values:
        return None
    median = statistics.median(values)
    return {'n': len(values), 'median': median, 'min': min(values),
            'max': max(values),
            'relSpread': (max(values) - min(values)) / median if median else None}


def main():
    outdir, off_binary, on_binary = sys.argv[1], sys.argv[2], sys.argv[3]
    rounds = int(sys.argv[4])
    spec = sys.argv[5] if len(sys.argv) > 5 else DEFAULT_SPEC
    labels = (sys.argv[6].split(',') if len(sys.argv) > 6
              else list(FIXTURES))
    arms = {'off': off_binary, 'on': on_binary}
    result = {
        'arms': arms,
        'armSha256': {k: hashlib.sha256(open(v, 'rb').read()).hexdigest()
                      for k, v in arms.items()},
        'spec': spec, 'rounds': rounds, 'fixtures': {},
        'protocol': 'paired interleaved; arm order reversed on odd rounds; '
                    'equal walk (past=0, no work cap); census disabled',
        'observations': [],
    }
    os.makedirs(outdir, exist_ok=True)
    targets = {}
    for label in labels:
        spec_row = FIXTURES[label]
        depth = parent_depth(spec_row['parent'])
        targets[label] = depth - spec_row['drop']
        result['fixtures'][label] = {
            'request': spec_row['request'], 'parent': spec_row['parent'],
            'parentDepthMm': depth, 'targetDepthMm': targets[label],
            'dropMm': spec_row['drop'], 'allowance': spec_row['allowance'],
            'parentSha256': hashlib.sha256(
                open(spec_row['parent'], 'rb').read()).hexdigest(),
        }

    for rnd in range(rounds):
        order = ['off', 'on'] if rnd % 2 == 0 else ['on', 'off']
        for label in labels:
            row_spec = FIXTURES[label]
            for arm in order:
                row = run_arm(arms[arm], row_spec['request'],
                              row_spec['parent'], targets[label],
                              row_spec['allowance'], spec,
                              f'{outdir}/{label}-r{rnd}-{arm}.json')
                row.update({'round': rnd, 'fixture': label, 'arm': arm})
                result['observations'].append(row)
        json.dump(result, open(f'{outdir}/wallfixtures.json', 'w'), indent=1)
        print(f'round {rnd} done', file=sys.stderr)

    result['summary'] = summarise(result, labels, rounds)
    json.dump(result, open(f'{outdir}/wallfixtures.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


def summarise(result, labels, rounds):
    obs = result['observations']
    out = {}
    for label in labels:
        rows = [r for r in obs if r['fixture'] == label]
        cell = {'perArm': {}, 'paired': {}, 'equalWalk': {}}
        for arm in ('off', 'on'):
            for field in ('perConfirmationMs', 'confirmationMs', 'sliceMs',
                          'processWallSeconds', 'confirmationsAccepted'):
                cell['perArm'].setdefault(arm, {})[field] = spread(
                    [r.get(field) for r in rows if r['arm'] == arm])

        def pick(arm, rnd, field):
            for r in rows:
                if r['arm'] == arm and r['round'] == rnd:
                    return r.get(field)
            return None

        for field in ('stepsTaken', 'confirmationsAccepted', 'rawSourceDepthMm',
                      'fingerprint', 'candidateQueries', 'workUnits'):
            mismatches = []
            for rnd in range(rounds):
                a, b = pick('off', rnd, field), pick('on', rnd, field)
                if a != b:
                    mismatches.append({'round': rnd, 'off': a, 'on': b})
            cell['equalWalk'][field] = {'mismatches': len(mismatches),
                                        'examples': mismatches[:3]}
        cell['equalWalkHolds'] = all(v['mismatches'] == 0
                                     for v in cell['equalWalk'].values())
        for field in ('perConfirmationMs', 'confirmationMs', 'sliceMs',
                      'processWallSeconds'):
            ratios = []
            for rnd in range(rounds):
                a, b = pick('on', rnd, field), pick('off', rnd, field)
                if a and b:
                    ratios.append(b / a)
            cell['paired'][field] = {
                'n': len(ratios),
                'medianSpeedup': statistics.median(ratios) if ratios else None,
                'min': min(ratios, default=None),
                'max': max(ratios, default=None),
                'cellsAboveParity': sum(1 for r in ratios if r > 1.0),
            }
        errors = [r for r in rows if 'error' in r]
        if errors:
            cell['errors'] = len(errors)
            cell['firstError'] = errors[0]['error'][:300]
        out[label] = cell
    return out


if __name__ == '__main__':
    main()
