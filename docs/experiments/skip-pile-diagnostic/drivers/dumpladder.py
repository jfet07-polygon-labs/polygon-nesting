#!/usr/bin/env python3
"""Cells of the round-envelope gate's miter ladder, re-run with the skip-pile
dump armed, and checked against that gate's committed evidence.

    dumpladder.py OUTDIR BINARY PARENTSJSON MATCHEDJSON CELLS [DUMPDIR] [CAP]

`CELLS` is a comma-separated list of `seed@work`.

# Why this is a reproduction and not a fresh measurement

The dump hook costs wall, and wall is exactly what an instrument must not spend
if it wants to be believed. It is affordable here for one structural reason: a
mode-34 slice under `past=1,rollback=0,work=W,lanes=1,pconfirm=0` is capped in
**work units**, not in seconds, so nothing in the trajectory reads a clock. That
is an argument, and this driver replaces it with a measurement - every cell it
runs is checked against `round-envelope-gate/evidence/matched.json` on

* `schedule_stepDigest`               - the whole walk, in one number;
* `confirmationsSkippedInfeasible`    - the pile's own size;
* `confirmationsAttempted/Accepted/Refused`;
* `stepsTaken` and `rawSourceDepthMm`.

If the dump moved the search, the step digest moves and this driver exits 1. The
`--features` set is `round-envelope-gate/drivers/collect.sh`'s `COMBO` plus that
round's `round-envelope-kernel` plus this round's `skip-pile-dump`, because the
cell being reproduced was measured on that feature set and a cell is a function
of its binary.

# The arm

`POLYGON_NESTING_ROUND_ENVELOPE_KERNEL` is **unset**: this is the control arm,
the miter authority, which is the arm whose proxy suppressed the frontiers. The
gate's own evidence records the union arm's skips as cell-for-cell identical to
the control's at all 48 matched cells, so the control's pile is the union's pile
and dumping one of them is dumping both.
"""
import hashlib
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

# `matchedgate.py`'s spec string, character for character.
SPEC = 'past=1,rollback=0,work={work},lanes=1,pconfirm=0'
DROP_MM = 1.0
DUMP_ENV = 'POLYGON_NESTING_SKIP_PILE_DUMP'
CAP_ENV = 'POLYGON_NESTING_SKIP_PILE_DUMP_CAP'
# Every round-scoped environment name this repository has ever armed an operator
# with, scrubbed so an inherited one cannot become an unlabelled arm.
ROUND_ENV = ('POLYGON_NESTING_CONTACT_BLOCK', 'POLYGON_NESTING_SE2_WITNESS',
             'POLYGON_NESTING_CONTINUOUS_ROTATION',
             'POLYGON_NESTING_SPARSE_ROTATION',
             'POLYGON_NESTING_COMPRESSION_SCHEDULE',
             'POLYGON_NESTING_ROUND_ENVELOPE_KERNEL',
             DUMP_ENV, CAP_ENV)

# What the reproduction is checked on. Every one of these is a function of the
# walk and none of them is a clock.
PINNED = ('schedule_stepDigest', 'schedule_confirmationsSkippedInfeasible',
          'schedule_confirmationsAttempted', 'schedule_confirmationsAccepted',
          'schedule_confirmationsRefused', 'schedule_stepsTaken',
          'schedule_workUnits', 'schedule_exitCause', 'schedule_finalDepthMm',
          'rawSourceDepthMm', 'fingerprint', 'exactValid', 'contractValid')


def population(doc):
    return ((doc.get('relaxedDiagnostics') or {})
            .get('coupledDynamicSeparator') or {}).get(
                'persistentVacancyPopulation')


def read_cell(doc):
    """`matchedgate.py:run_cell`'s reader, on the fields this round pins."""
    row = {}
    profile = (doc.get('searchProfile') or {}).get('counters') or {}
    queries = profile.get('candidateQueries', 0)
    tests = profile.get('exactPairTests', 0)
    row['processWorkUnits'] = queries + 5 * tests
    elapsed = doc.get('medianElapsedMs')
    row['operatorWallSeconds'] = (elapsed / 1000.0
                                  if elapsed is not None else None)
    pop = population(doc)
    if pop is None:
        row['error'] = 'no population'
        return row
    row['exactValid'] = pop.get('exactValid')
    row['contractValid'] = pop.get('contractValid')
    row['rawSourceDepthMm'] = pop.get('rawSourceDepthMm')
    row['fingerprint'] = pop.get('finalPlacementFingerprint')
    schedule = pop.get('compressionSchedule') or {}
    for key in ('workUnits', 'confirmationsAttempted', 'confirmationsAccepted',
                'confirmationsRefused', 'confirmationsSkippedInfeasible',
                'stepsTaken', 'exitCause', 'stepDigest', 'finalDepthMm'):
        row['schedule_' + key[0].lower() + key[1:]] = schedule.get(key)
    return row


def committed(matched, seed, work):
    """The gate's own row for one cell, on the miter arm."""
    for cell in matched['cells']:
        if cell['seed'] != seed:
            continue
        row = cell['arms'].get(f'miter:{work}')
        if row is None:
            return None, None
        return row, cell['parentRawDepthMm']
    return None, None


def run_cell(binary, seed, fixture, target, work, out_path, dump_path, cap):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    command = ([binary, runlib.REQUESTS['mixed-61']] + args
               + ['34', fixture, f'{target:.17g}', '',
                  runlib.DEFAULT_ALLOWANCE])
    env = dict(os.environ)
    for name in ROUND_ENV:
        env.pop(name, None)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = SPEC.format(work=int(work))
    if dump_path is not None:
        env[DUMP_ENV] = dump_path
        env[CAP_ENV] = str(cap)
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    try:
        doc = json.load(open(out_path))
    except json.JSONDecodeError:
        return {'exitCode': proc.returncode, 'processWallSeconds': wall,
                'error': (proc.stderr or b'').decode()[-800:]}, None
    row = read_cell(doc)
    row['exitCode'] = proc.returncode
    row['processWallSeconds'] = wall
    return row, doc


def dump_stats(path):
    """The dump, and the sink's own tally beside it.

    The sink deduplicates by placement fingerprint, so a cell's line count is
    the number of *distinct* suppressed frontiers and is legitimately smaller
    than its skip count. The sidecar is what makes that checkable instead of
    plausible: `written + duplicates + overCap` is every offer the hook
    received, and it must equal the schedule's own
    `confirmationsSkippedInfeasible` exactly. Without it, a dump that silently
    lost records would look the same as one that deduplicated them.
    """
    if not os.path.exists(path):
        return {'lines': 0, 'distinctFingerprints': 0, 'bytes': 0,
                'firstStep': None, 'lastStep': None, 'tally': None}
    seen = set()
    lines = 0
    first = last = None
    with open(path) as handle:
        for text in handle:
            text = text.strip()
            if not text:
                continue
            record = json.loads(text)
            lines += 1
            seen.add(record['fingerprint'])
            if first is None:
                first = record['step']
            last = record['step']
    tally_path = path + '.tally.json'
    tally = (json.load(open(tally_path))
             if os.path.exists(tally_path) else None)
    return {'lines': lines, 'distinctFingerprints': len(seen),
            'bytes': os.path.getsize(path), 'firstStep': first,
            'lastStep': last, 'tally': tally}


def main():
    outdir, binary, parents_json, matched_json, cells = sys.argv[1:6]
    dumpdir = sys.argv[6] if len(sys.argv) > 6 else f'{outdir}/dump'
    cap = int(sys.argv[7]) if len(sys.argv) > 7 else 20000
    parents = {row['seed']: row
               for row in json.load(open(parents_json))['rows']}
    matched = json.load(open(matched_json))
    os.makedirs(outdir, exist_ok=True)
    os.makedirs(dumpdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'parents': parents_json,
        'committedEvidence': matched_json,
        'committedEvidenceBinarySha256': matched.get('binarySha256'),
        'spec': SPEC, 'dropMm': DROP_MM, 'arm': 'miter (control)',
        'dumpCap': cap, 'pinnedFields': list(PINNED), 'cells': [],
    }
    ok = True
    for item in cells.split(','):
        seed_text, _, work_text = item.partition('@')
        seed, work = int(seed_text), int(work_text)
        parent = parents[seed]
        reference, parent_depth = committed(matched, seed, work)
        if reference is None:
            raise SystemExit(f'no committed miter row for seed{seed}@{work}')
        target = parent_depth - DROP_MM
        label = f'seed{seed}@{work}'
        dump_path = f'{dumpdir}/seed{seed}-{work}.jsonl'
        row, _ = run_cell(binary, seed, parent['fixture'], target, work,
                          f'{outdir}/seed{seed}-{work}.json', dump_path, cap)
        differences = {key: [reference.get(key), row.get(key)]
                       for key in PINNED if reference.get(key) != row.get(key)}
        stats = dump_stats(dump_path)
        skips = row.get('schedule_confirmationsSkippedInfeasible')
        tally = stats['tally'] or {}
        # The dump has to account for the *whole* pile, not a prefix of it, or
        # the sample downstream would be a sample of the shallow end. It
        # accounts for it exactly: every offer the hook received is written,
        # deduplicated or over cap, and those three sum to the schedule's own
        # skip count. `lines == written` checks the file against the sink that
        # wrote it; `overCap == 0` checks that the cap never bound.
        whole_pile = (tally.get('offered') == skips
                      and stats['lines'] == tally.get('written')
                      and stats['lines'] == stats['distinctFingerprints']
                      and tally.get('overCap') == 0)
        reproduced = row.get('exitCode') == 0 and not differences
        ok = ok and reproduced and whole_pile
        result['cells'].append({
            'label': label, 'seed': seed, 'workCap': work,
            'parentRawDepthMm': parent_depth, 'targetDepthMm': target,
            'fixture': parent['fixture'], 'dumpPath': dump_path,
            'committed': {key: reference.get(key) for key in PINNED},
            'measured': {key: row.get(key) for key in PINNED},
            'differences': differences, 'reproduced': reproduced,
            'exitCode': row.get('exitCode'),
            'processWallSeconds': row.get('processWallSeconds'),
            'operatorWallSeconds': row.get('operatorWallSeconds'),
            'committedOperatorWallSeconds': reference.get(
                'operatorWallSeconds'),
            'dump': stats, 'skipPileFullyAccountedFor': whole_pile,
            'skipsSuppressed': skips,
        })
        json.dump(result, open(f'{outdir}/dumpladder.json', 'w'), indent=1)
        print(f"{label} reproduced={reproduced} accountedFor={whole_pile} "
              f"skips={skips} dumped={stats['lines']} "
              f"dup={tally.get('duplicates')} overCap={tally.get('overCap')}"
              f" digest={row.get('schedule_stepDigest')} "
              f"wall={row.get('processWallSeconds'):.1f}s "
              f"(committed opwall {reference.get('operatorWallSeconds')}) "
              f"diffs={list(differences)}", flush=True)
    result['ALL_REPRODUCED'] = ok
    json.dump(result, open(f'{outdir}/dumpladder.json', 'w'), indent=1)
    print(json.dumps({'ALL_REPRODUCED': ok,
                      'cells': len(result['cells']),
                      'dumpedTotal': sum(c['dump']['lines']
                                         for c in result['cells'])}, indent=1))
    raise SystemExit(0 if ok else 1)


if __name__ == '__main__':
    main()
