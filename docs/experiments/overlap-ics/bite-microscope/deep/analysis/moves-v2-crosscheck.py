#!/usr/bin/env python3
"""Do the microscope documents' bite-5 outcomes agree with the scored v2 cells
on the same seed and arm?  (The microscope runs are separate, diagnostic-only
runs; fifth-cut-rows.json has three reps per arm x seed.)"""
import json, glob, os, re, collections
v2 = json.load(open('/var/lib/t3/tmp/astra/v2/fifth-cut-rows.json'))
V = collections.defaultdict(list)
for r in v2: V[(r['arm'], r['seed'])].append(r)
agree = disagree = 0
for path in sorted(glob.glob('/var/lib/t3/tmp/astra/deep/deep-*-wall10s-s*.json')):
    m = re.match(r'deep-([AB])-wall10s-s(\d+)\.json', os.path.basename(path)); arm, seed = m.group(1), int(m.group(2))
    d = json.load(open(path)); b5 = [b for b in d['biteMicroscope']['bites'] if b['ordinal'] == 5][0]
    cells = V.get((arm, seed), [])
    v2pub = [c['fifth'] for c in cells]; v2iters = [c['iters'] for c in cells]
    same = (not cells) or (all(x == b5['published'] for x in v2pub))
    agree += same; disagree += (not same)
    print(f"{arm} {seed}: microscope bite5 published={b5['published']} iters={b5['masterIterations']}; v2 cells fifth published={v2pub} iters={v2iters} {'(seed not in v2: seeds 27-35 are the v1 seeds)' if not cells else ''}")
print('agree', agree, 'disagree', disagree)
