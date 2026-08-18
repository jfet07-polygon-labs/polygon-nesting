#!/usr/bin/env python3
"""Pin a run document that is only a run report into a replayable fixture.

    python3 pinrun.py RUNJSON OUTPIN "description"

The lib.pin pattern from the finer-ladder drivers: placements + the engine's own
`finalPlacementFingerprint`, and the depth fields taken from the run's own
`independentDepthMm` so the fixture describes the layout it carries. Prints the
declared raw (the publication-authority `rawSourceDepthMm`), the fingerprint and
the fixture sha256, which is what a replay has to reproduce.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

run_path, out_path, description = sys.argv[1], sys.argv[2], sys.argv[3]
doc = json.load(open(run_path))
pop = lib.population(doc)
assert pop and pop['exactValid'] and pop['contractValid'], 'not publishable'
os.makedirs(os.path.dirname(out_path), exist_ok=True)
lib.pin(doc, out_path, description)
print(json.dumps({
    'pin': out_path,
    'declaredRawSourceDepthMm': repr(pop['rawSourceDepthMm']),
    'independentDepthMm': pop['independentDepthMm'],
    'fingerprint': pop['finalPlacementFingerprint'],
    'sha256': hashlib.sha256(open(out_path, 'rb').read()).hexdigest(),
    'reDerivedDepthMm': repr(lib.depth_mm(json.load(open(out_path))['placements'])),
}, indent=1))
