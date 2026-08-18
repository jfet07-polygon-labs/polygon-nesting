#!/usr/bin/env python3
"""One mode-34 compression-schedule arm, on the record lineage's `0.0005` tail.

    python3 sched.py TAG PARENT TARGET SEED SPEC [OUTDIR]

`SPEC` is the POLYGON_NESTING_COMPRESSION_SCHEDULE value verbatim, so the
schedule's knobs stay out of the pinned positional tail exactly as the port
designed. The target is formatted `%.17g`, which is what
`docs/experiments/compression-schedule/drivers/records.py` used to produce the
159.668 from-scratch state; a `%.6f` target is a *different* bound and the two
are not interchangeable at this depth.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import drv  # noqa: E402
import lib  # noqa: E402

LOG = os.environ.get('CASCADE_LOG', '/var/lib/t3/tmp/recordline/sched.log')
RUNS = '/var/lib/t3/tmp/recordline/sched-runs'


def sched_arm(tag, parent, target, seed, spec, logfile=LOG, outdir=RUNS):
    """Mode 34 with `spec`, returned as (document, wall seconds)."""
    os.makedirs(outdir, exist_ok=True)
    started = time.time()
    out = lib.run(tag, 34, parent, f'{target:.17g}', seed, outdir,
                  binary=lib.SCHED_BIN,
                  env={'POLYGON_NESTING_COMPRESSION_SCHEDULE': spec})
    wall = time.time() - started
    drv.log(logfile, f'[{wall:7.1f}s] ' + lib.line(tag, out) + f'  spec={spec}')
    pop = lib.population(out) or {}
    schedule = dict((pop.get('compressionSchedule') or {}))
    schedule.pop('steps', None)
    if schedule:
        drv.log(logfile, '   schedule ' + json.dumps(
            {k: v for k, v in sorted(schedule.items())
             if not isinstance(v, (dict, list))}))
    return out, wall


if __name__ == '__main__':
    tag, parent, target, seed, spec = (
        sys.argv[1], sys.argv[2], float(sys.argv[3]), int(sys.argv[4]),
        sys.argv[5])
    outdir = sys.argv[6] if len(sys.argv) > 6 else RUNS
    doc, wall = sched_arm(tag, parent, target, seed, spec, outdir=outdir)
    print(json.dumps({'tag': tag, 'wall': wall,
                      'published': drv.published_raw(doc)}, indent=1))
