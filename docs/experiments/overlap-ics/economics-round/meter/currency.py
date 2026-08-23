#!/usr/bin/env python3
"""**The currency calibration cell, and the spec's >10 % reject check.**

    python3 currency.py [work-dir]

docs/economics-round-spec.md, funded change 3, verbatim:

> currency `U = sample_evaluations + B*master_batches
> + E*actual_publication_attempt_calls + R*repair_rows + D*disruption_moves`;
> **B/E/R/D from timing-only microbenchmarks on all three fixtures,
> conservative rounding; REJECT the currency if wall-prediction error >10 % on
> any transfer fixture.**

This driver is the evidence agent's entry point for that sentence. It runs one
fixed-work `--cell=cutclose` on each of the three fixtures under the
`ics-profile` build, hands the three documents to the `ics_meter` example, and
reads its exit status directly - never through a pipe.

# Why fixed work and not wall

The counters are the calibration's *design matrix* and the nanoseconds are its
*response*. Under `--mode=fixed` the counters are a deterministic function of
the request and the seed, so two runs of this driver differ only in the thing
being measured. Under `--mode=wall` they would differ in both, and a
coefficient fitted across that is a coefficient fitted to a lottery.

The seconds the calibration divides by are `wall.searchSeconds`, which the
example measures **around** the phases. No clock enters the trajectory: the
audit's finding that `Instant` appears in exactly one place under
`search/overlap_ics/` is why a fixed-work document is bit-identical across
processes, and this driver does not disturb it.

# The three cells are not tuned to pass

`BITES`, `ATTEMPTS` and `ITERS` are chosen to make each fixture spend all five
terms - in particular to make a separation *fail*, because a fixture that never
fails never disrupts, and a term with a zero count is a term the calibration
**refuses to price** rather than prices free. If a fixture still spends no
disruption move, the refusal is the result, and it is in the document.

# The exit status is the verdict

* `0` - the currency transfers: no fixture pair is off by more than 10 %.
* `1` - the currency is **rejected** by the spec's own clause. That is a
  finding. The first suspect named in `currency.rs` is the `D` bound, which is
  the whole search wall the separations did not claim and therefore also
  contains the pool restore, the pose installation, the cut and the publication
  commit.
* `2` - the check could not run: a missing binary, a build without
  `ics-profile`, or a fixture whose cell spent no occurrences of a term.

Nothing here decides anything. Every number comes from
`search::overlap_ics_meter::currency`, which has its own unit vectors, so this
file cannot pass by agreeing with a copy of the rule.
"""
import hashlib
import json
import os
import platform
import subprocess
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                    '..', '..', '..', '..', '..'))
sys.path.insert(0, f'{ROOT}/docs/experiments/overlap-ics/drivers')
import lib  # noqa: E402

# The census's own `ics-profile` build, in its own target directory so the
# default build every gate is measured on is never overwritten by it.
PROFILE_BIN = os.environ.get(
    'ICS_PROFILE_BIN',
    f'{ROOT}/target/profile-build/release/examples/overlap_ics_benchmark')
METER_BIN = os.environ.get(
    'ICS_METER_BIN', f'{ROOT}/target/release/examples/ics_meter')

FIXTURES = ['mixed-61', 'shapes-17', 'triangle-20']
SEED = 0
# Enough explore bites to carry mixed-61 past the 21 published 0.1 % bites and
# into the 179 shelf, enough attempts for the shelf's failures to disrupt, and
# a per-separation cap that keeps one cell inside a couple of minutes.
BITES = int(os.environ.get('ICS_CURRENCY_BITES', '30'))
ATTEMPTS = int(os.environ.get('ICS_CURRENCY_ATTEMPTS', '3'))
ITERS = int(os.environ.get('ICS_CURRENCY_ITERS', '120'))
COMPRESS_BITES = int(os.environ.get('ICS_CURRENCY_COMPRESSBITES', '4'))

# Quoted, never chosen here. The threshold lives in
# `search::overlap_ics_meter::currency::WALL_PREDICTION_TOLERANCE`; this string
# is only what the document prints beside it.
REJECT_QUOTE = ('docs/economics-round-spec.md, funded change 3: "REJECT the '
                'currency if wall-prediction error >10% on any transfer '
                'fixture."')


def sha256_of(path):
    try:
        with open(path, 'rb') as handle:
            return hashlib.sha256(handle.read()).hexdigest()
    except OSError:
        return None


def loadavg():
    try:
        with open('/proc/loadavg') as handle:
            return handle.read().split()[:3]
    except OSError:
        return None


def cell(out, fixture):
    """One fixed-work cutclose document for one fixture."""
    path = f'{out}/currency-{fixture}.json'
    command = [PROFILE_BIN, '--cell=cutclose',
               f'--request={lib.REQUESTS[fixture]}',
               f'--edge={lib.EDGE_MM}', f'--pair={lib.PAIR_MM}',
               '--mode=fixed', '--workers=8', f'--seed={SEED}',
               f'--bites={BITES}', f'--attempts={ATTEMPTS}',
               f'--iters={ITERS}', f'--compressbites={COMPRESS_BITES}']
    started = time.monotonic()
    with open(path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False)
    wall = time.monotonic() - started
    try:
        with open(path) as handle:
            document = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        document = {'_loadError': f'{error}'}
    outcome = document.get('outcome') or {}
    rows = outcome.get('bites') or []
    terms = {'sampleEvaluations': 0, 'masterBatches': 0, 'exactCalls': 0,
             'repairRows': 0, 'disruptionMoves': 0}
    for row in rows:
        profile = row.get('profile') or {}
        terms['sampleEvaluations'] += profile.get('sampleEvaluations', 0)
        terms['masterBatches'] += profile.get('iterations', 0)
        terms['exactCalls'] += profile.get('exactCalls', 0)
        terms['repairRows'] += profile.get('repairRows', 0)
        terms['disruptionMoves'] += profile.get('disruptionMoves', 0)
    return {
        'fixture': fixture,
        'path': path,
        'sourceSha256': sha256_of(path),
        'command': command,
        'exit': result.returncode,
        'stderr': (result.stderr or b'').decode()[-500:],
        'driverWallSeconds': wall,
        'searchSeconds': (document.get('wall') or {}).get('searchSeconds'),
        'bites': len(rows),
        'publications': outcome.get('publicationCount'),
        'depthMm': outcome.get('depthMm'),
        # The five terms this cell really spent. A zero here is why a
        # calibration refuses, and the refusal is easier to read from this row
        # than from the meter's error string.
        'terms': terms,
        'profileMeasured': bool(rows and (rows[0].get('profile') or {})
                                .get('measured')),
    }, path


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else '/var/lib/t3/tmp/overlapics/currency'
    os.makedirs(out, exist_ok=True)
    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-currency-calibration',
        'spec': REJECT_QUOTE,
        'profileBinary': PROFILE_BIN,
        'profileBinarySha256': sha256_of(PROFILE_BIN),
        'meterBinary': METER_BIN,
        'meterBinarySha256': sha256_of(METER_BIN),
        'machine': {
            'platform': platform.platform(),
            'cpus': os.cpu_count(),
            'loadBefore': loadavg(),
        },
        'cellShape': {'seed': SEED, 'workers': 8, 'mode': 'fixed',
                      'bites': BITES, 'attempts': ATTEMPTS, 'iters': ITERS,
                      'compressBites': COMPRESS_BITES},
    }
    for binary in (PROFILE_BIN, METER_BIN):
        if not os.path.exists(binary):
            document['error'] = (
                f'{binary} is missing. Build them:\n'
                '  CARGO_TARGET_DIR=target/profile-build cargo build '
                '-p polygon-nesting-core --release '
                '--features overlap-ics,ics-profile '
                '--example overlap_ics_benchmark\n'
                '  cargo build -p polygon-nesting-core --release '
                '--features overlap-ics --example ics_meter')
            print(json.dumps(document, indent=2))
            with open(f'{out}/currency.json', 'w') as handle:
                json.dump(document, handle, indent=2)
            return 2

    cells = []
    paths = {}
    for fixture in FIXTURES:
        row, path = cell(out, fixture)
        cells.append(row)
        paths[fixture] = path
        print(f'[currency] {fixture} exit={row["exit"]} '
              f'bites={row["bites"]} terms={row["terms"]}', file=sys.stderr)
    document['cells'] = cells
    document['machine']['loadAfter'] = loadavg()

    if any(row['exit'] != 0 for row in cells):
        document['error'] = 'a cell did not exit 0; nothing was calibrated'
        with open(f'{out}/currency.json', 'w') as handle:
            json.dump(document, handle, indent=2)
        print(json.dumps(document, indent=2))
        return 2

    meter_out = f'{out}/currency-meter.json'
    command = [METER_BIN, f'--out={meter_out}'] + [
        f'--cell={fixture}={paths[fixture]}' for fixture in FIXTURES]
    result = subprocess.run(command, stdout=subprocess.DEVNULL,
                            stderr=subprocess.PIPE, check=False)
    status = result.returncode
    try:
        with open(meter_out) as handle:
            meter = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        meter = {'_loadError': f'{error}'}
    document['meterCommand'] = command
    document['meterExit'] = status
    document['meterStderr'] = (result.stderr or b'').decode()[-500:]
    document['meter'] = meter
    document['CURRENCY_ACCEPTED'] = bool(meter.get('CURRENCY_ACCEPTED'))
    document['WORST_RELATIVE_ERROR'] = meter.get('WORST_RELATIVE_ERROR')
    with open(f'{out}/currency.json', 'w') as handle:
        json.dump(document, handle, indent=2)
    print(json.dumps({
        'CURRENCY_ACCEPTED': document['CURRENCY_ACCEPTED'],
        'WORST_RELATIVE_ERROR': document['WORST_RELATIVE_ERROR'],
        'summary': meter.get('summary'),
        'error': meter.get('error'),
        'document': f'{out}/currency.json',
    }, indent=2))
    return status


if __name__ == '__main__':
    sys.exit(main())
