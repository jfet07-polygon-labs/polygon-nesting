#!/usr/bin/env python3
"""The descent table, assembled from the cascade state documents.

    python3 lineage.py STATE.json [STATE.json ...]

Prints one row per adoption - tier, tag, declared raw, delta - so the round's
table is generated from the drivers' own output rather than transcribed.
"""
import json
import sys

previous = None
total_arms = 0
rows = []
for path in sys.argv[1:]:
    state = json.load(open(path))
    total_arms += state.get('arms', 0)
    label = state.get('label')
    for adoption in state.get('adoptions', []):
        rows.append((label, adoption['round'], adoption['tier'],
                     adoption['tag'], adoption['from'], adoption['to'],
                     adoption['delta'], adoption.get('alsoBelow'),
                     adoption.get('sha256', '')[:16],
                     (adoption.get('fingerprint') or '')[:16]))

print(f'{"cascade":6s} {"rnd":>4s} {"tier":14s} {"tag":34s} '
      f'{"raw":>22s} {"delta":>14s} {"also":>5s}')
for (label, rnd, tier, tag, src, dst, delta, also, sha, fp) in rows:
    print(f'{label:6s} {rnd:4d} {tier:14s} {tag:34s} {dst:>22s} '
          f'{delta:+14.11f} {also if also is not None else "":>5}')
if rows:
    first, last = rows[0][4], rows[-1][5]
    print(f'\ntotal adoptions {len(rows)}, arms {total_arms}, '
          f'{first} -> {last}, '
          f'net {float(last) - float(first):+.11f} mm')
