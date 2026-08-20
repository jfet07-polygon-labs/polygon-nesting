import json
import os
import shutil

SRC = '/var/lib/t3/tmp/pl34'
DST = ('/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/'
       'wf_960e7225-201-2/docs/experiments/parallel-compression-schedule/'
       'evidence')
os.makedirs(DST, exist_ok=True)

for src, name in [
    (f'{SRC}/occupancy/occupancy.json', 'occupancy.json'),
    (f'{SRC}/anatomy/anatomy-m34-slice10.json', 'anatomy-m34-design-slice.json'),
    (f'{SRC}/anatomy/anatomy-preamble.json', 'anatomy-preamble.json'),
    (f'{SRC}/workgate/workgate.json', 'workgate.json'),
    (f'{SRC}/wall/wall.json', 'wall.json'),
    (f'{SRC}/anytime/anytime.json', 'anytime.json'),
    (f'{SRC}/determinism2/determinism.json', 'determinism.json'),
]:
    if os.path.exists(src):
        shutil.copy(src, f'{DST}/{name}')
        print('copied', name, os.path.getsize(src) // 1024, 'KB')
    else:
        print('MISSING', src)

# The four gates on four binaries, merged into one document.
gates = {}
for b in ['base-jagua', 'mine-jagua', 'mine-csched', 'mine-parallel']:
    path = f'{SRC}/gates/{b}/gates-{b}.json'
    if os.path.exists(path):
        gates[b] = json.load(open(path))
digests = {b: {g: v['docDigest'] for g, v in d['gates'].items()}
           for b, d in gates.items()}
reference = digests.get('base-jagua')
json.dump({
    'note': 'the four pinned regression gates on every binary this round '
            'produced; `mine-parallel` is the armed BUILD run with an unarmed '
            'spec, which the gates never arm because they are modes 20 and 22',
    'allPass': {b: d['ALL_PASS'] for b, d in gates.items()},
    'docDigestsMatchBaseJagua': {b: (dg == reference)
                                 for b, dg in digests.items()},
    'docDigests': digests,
    'gates': gates,
}, open(f'{DST}/gates.json', 'w'), indent=1)
print('wrote gates.json')

# The phase attribution behind section 3.
phase = {}
for label, path in [('preamble', f'{SRC}/phasediff2/ppre-seed0.json'),
                    ('serial', f'{SRC}/phasediff2/pserial-seed0.json'),
                    ('pconfirm', f'{SRC}/phasediff2/ppar-seed0.json')]:
    if not os.path.exists(path):
        continue
    doc = json.load(open(path))
    phase[label] = {p['phase']: {'milliseconds': p['milliseconds'],
                                 'calls': p['calls']}
                    for p in doc['searchProfile']['phases']}
    population = ((doc.get('relaxedDiagnostics') or {})
                  .get('coupledDynamicSeparator') or {}).get(
                      'persistentVacancyPopulation')
    schedule = (population or {}).get('compressionSchedule') or {}
    phase[label]['_schedule'] = {k: v for k, v in schedule.items()
                                 if k != 'steps'}
keys = ['publicationValidate', 'exactOverlapTest', 'collisionPolygonBuild']
attribution = {}
if 'preamble' in phase and 'serial' in phase:
    for label in ('serial', 'pconfirm'):
        if label not in phase:
            continue
        attribution[label] = {}
        for k in keys:
            ms = phase[label][k]['milliseconds'] - phase['preamble'][k]['milliseconds']
            calls = phase[label][k]['calls'] - phase['preamble'][k]['calls']
            confirmations = phase[label]['_schedule']['confirmationsAttempted']
            attribution[label][k] = {
                'scheduleOnlyMilliseconds': ms,
                'scheduleOnlyCalls': calls,
                'perConfirmationMs': ms / confirmations if confirmations else None,
                'callsPerConfirmation': calls / confirmations if confirmations else None,
            }
json.dump({
    'note': 'mode-34 phase costs with the identical mode-0 preamble subtracted, '
            'equal-walk shape (1.5 mm drop, past=0), seed 0, profiled build. '
            'This is the evidence for the correction to '
            'compression-schedule/README.md section 6.1: an accepted '
            'confirmation costs 4.92 ms and the collision-grid overlap loop is '
            '0.13 ms of it, because `exactOverlapTest` is entered past the '
            'broad-phase bounds reject about 98 times per confirmation rather '
            'than 1830 times.',
    'scheduleOnlyAttribution': attribution,
    'rawPhases': phase,
}, open(f'{DST}/phase-attribution.json', 'w'), indent=1)
print('wrote phase-attribution.json')

# The pinned-CPU proof that the job pool is reached.
pinned = {}
for label in ('1cpu', 'all'):
    for arm in ('serial', 'lanes8', 'pconfirm'):
        path = f'{SRC}/taskset/{label}-{arm}.json'
        if not os.path.exists(path):
            continue
        s = (json.load(open(path))['relaxedDiagnostics']
             ['coupledDynamicSeparator']['persistentVacancyPopulation'])
        sched = s['compressionSchedule']
        pinned.setdefault(label, {})[arm] = {
            'repairMs': sched['repairMs'],
            'confirmationMs': sched['confirmationMs'],
            'sliceMs': sched['repairMs'] + sched['confirmationMs'],
            'candidateQueries': sched['candidateQueries'],
            'stepsTaken': sched['stepsTaken'],
            'rawSourceDepthMm': s['rawSourceDepthMm'],
        }
json.dump({
    'note': 'one CPU (taskset -c 2) against all sixteen, equal-walk shape, '
            'seed 0. The direct proof that each lever reaches the job pool: a '
            'lever that silently fell back to serial iteration would not move.',
    'arms': pinned,
}, open(f'{DST}/taskset.json', 'w'), indent=1)
print('wrote taskset.json')
