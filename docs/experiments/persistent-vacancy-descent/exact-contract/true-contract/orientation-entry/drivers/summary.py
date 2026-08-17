#!/usr/bin/env python3
"""Assembles the committed summary.json for the orientation-entry experiment."""
import sys, json, hashlib, os, subprocess, glob, collections
sys.path.insert(0, '/var/lib/t3/tmp/orient')
import drv

OUT = sys.argv[1]
FINAL_PIN = sys.argv[2]
FINAL_RAW = float(sys.argv[3])
table = json.load(open('/var/lib/t3/tmp/orient/table.json'))
certify = json.load(open('/var/lib/t3/tmp/orient/certify.json'))
final = json.load(open(FINAL_PIN))
sha = hashlib.sha256(open(FINAL_PIN, 'rb').read()).hexdigest()

trajectory = []
for line in open('/var/lib/t3/tmp/orient/cascade.log'):
    if line.startswith('*** ADOPT'):
        head = line.split('ADOPT ', 1)[1]
        tag, rest = head.split(': ', 1)
        before, after = rest.split(' -> ')
        trajectory.append({'via': tag, 'rawSourceDepthMm': float(after.split(' ')[0])})

geo = json.loads(subprocess.run(
    [sys.executable, '/var/lib/t3/tmp/orient/geodiff.py',
     f'{drv.lib.TRUE}/record-159.092/pinned-parent-159.092.json', FINAL_PIN],
    capture_output=True, text=True).stdout)

runs = len(glob.glob('/var/lib/t3/tmp/orient/runs/*.json'))
fixpoint = json.load(open('/var/lib/t3/tmp/orient/fixpoint.json'))
pads = json.load(open('/var/lib/t3/tmp/orient/pads.json'))
pad_rows = {
    'arms': len(pads),
    'pads': len({row['pad'] for row in pads}),
    'publicationsBelowIncumbent': sum(1 for row in pads if row['belowIncumbent']),
    'publicationsBelowOwnPad': sum(1 for row in pads if row['belowPad']),
    'acceptedOrientation': sum(row['attribution'].get('acceptedOrientation', 0)
                               for row in pads),
    'orientationFinalists': sum(row['attribution'].get('finalists', 0) for row in pads),
}
json.dump({
    'milestone': json.load(open(f'{OUT}/milestone.json'))['milestone'],
    'request': 'tests/fixtures/mixed-61/mixed61-request-exact-clearance.json',
    'requestSha256': drv.lib.REQ_SHA,
    'searchOffsetAllowanceMm': 0.0005,
    'binarySha256': hashlib.sha256(open(drv.lib.BIN, 'rb').read()).hexdigest(),
    'machine': 'x86_64, 16 cores, runs pinned at 8 threads',
    'runs': runs,
    'trajectory': trajectory,
    'pinnedParent': {
        'file': os.path.basename(FINAL_PIN),
        'rawSourceDepthMm': FINAL_RAW,
        'independentDepthMm': final['independentDepthMm'],
        'placementFingerprint': final['expectedPlacementFingerprint'],
        'fixtureSha256': sha,
    },
    'geometricDiffAgainstOldRecord': geo,
    'resultsTable': table,
    'fromScratchPads': pad_rows,
    'fixpointProbe': fixpoint,
    'certification': certify,
}, open(f'{OUT}/summary.json', 'w'), indent=1)
print(f'{OUT}/summary.json')
