#!/usr/bin/env python3
"""Run one arm of the combination over a set of parent states, in parallel."""

import concurrent.futures
import json
import os
import sys

sys.path.insert(0, '/var/lib/t3/tmp/combo')
import base  # noqa: E402


def sweep(jobs, outdir, workers=3):
    """jobs: [(tag, mode, parent_path, target, seed)] -> [(tag, run_json)]"""
    os.makedirs(outdir, exist_ok=True)
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        futures = {pool.submit(base.run, tag, mode, parent, target, seed, outdir): tag
                   for (tag, mode, parent, target, seed) in jobs}
        for future in concurrent.futures.as_completed(futures):
            results.append((futures[future], future.result()))
    return sorted(results, key=lambda row: row[0])


def report(results, incumbent_raw):
    """Prints one line per job; returns the improving rows, deepest first."""
    wins = []
    for tag, run_json in results:
        depth = base.published(run_json)
        pop = base.population(run_json)
        raw = (pop or {}).get('rawSourceDepthMm')
        mark = ''
        if depth is not None and raw is not None and raw < incumbent_raw - 1e-9:
            mark = '  <-- IMPROVES'
            wins.append((raw, depth, tag, run_json))
        print(f'{base.line(tag, run_json)}{mark}', flush=True)
    return sorted(wins)
