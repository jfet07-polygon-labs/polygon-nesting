#!/usr/bin/env python3
"""Pin one run document as a parent fixture and report its identity.

    python3 pinit.py RUNJSON OUTPATH DESCRIPTION
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

run, out, description = sys.argv[1], sys.argv[2], sys.argv[3]
doc = json.load(open(run))
pop = lib.population(doc)
assert pop and pop['exactValid'] and pop['contractValid'], 'not a publication'
os.makedirs(os.path.dirname(out), exist_ok=True)
lib.pin(doc, out, description)
print(json.dumps({
    'pin': out,
    'raw': repr(pop['rawSourceDepthMm']),
    'independentDepthMm': repr(pop['independentDepthMm']),
    'fingerprint': pop['finalPlacementFingerprint'],
    'sha256': hashlib.sha256(open(out, 'rb').read()).hexdigest(),
}, indent=1))
