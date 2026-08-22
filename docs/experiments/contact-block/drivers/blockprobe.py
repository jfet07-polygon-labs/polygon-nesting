#!/usr/bin/env python3
"""The out-of-band decomposition: what the block operator finds, per parent.

    blockprobe.py OUTDIR BINARY PARENTSJSON SPEC[,SPEC...] [REQUEST]

A `SPEC` is the `POLYGON_NESTING_CONTACT_BLOCK` string with its `,` written as
`;`, because the argument separator is already taken. The environment sees the
comma form, so the table's row labels and the command a reader would type are
the same object.

This runs **before** any in-search gate on purpose. Sol review 10 §3's gate asks
for "components found, block proposals, exact-validation pass rate, depth
deltas, work spent" per parent; all five are properties of the operator applied
to a parent, and none of them needs a coordinator. Measuring them out of band
costs one process per cell instead of one search per cell, and — more
importantly — it cannot be confounded by a schedule that would have found the
same depth anyway, which is exactly the confound that made design C's in-search
numbers ambiguous in `docs/experiments/sparse-rotation/` §3.2.

The column that decides whether a null is about the operator or about the
layout is `headroomMm`: the published depth minus the deepest piece the block
does **not** contain. A block cannot buy more than that no matter how good its
program is, because the publication measure is a maximum over all pieces.
"""
import hashlib
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def cell_summary(doc):
    """The five gate columns, plus the two that explain them."""
    rounds = doc.get('rounds') or []
    refusals = {}
    for entry in rounds:
        key = entry.get('refusal') or 'moved'
        refusals[key] = refusals.get(key, 0) + 1
    blocks = [len(entry['block']) for entry in rounds]
    headrooms = [entry['headroomMm'] for entry in rounds
                 if entry.get('headroomMm') is not None
                 and entry['headroomMm'] != float('inf')]
    uppers = [entry['modelUpperMm'] for entry in rounds
              if entry.get('modelUpperMm') is not None]
    full_valid = [entry['fullStepExactValid'] for entry in rounds
                  if entry.get('rows')]
    return {
        'parentDepthMm': doc.get('parentDepthMm'),
        'finalDepthMm': doc.get('finalDepthMm'),
        'deltaMm': doc.get('deltaMm'),
        'rounds': len(rounds),
        'roundsAccepted': doc.get('roundsAccepted'),
        'solves': doc.get('solves'),
        'validations': doc.get('validations'),
        'rowsTotal': doc.get('rowsTotal'),
        'elapsedMs': doc.get('elapsedMs'),
        'refusals': refusals,
        'blockSizes': blocks,
        'medianBlockSize': statistics.median(blocks) if blocks else None,
        'medianHeadroomMm': (statistics.median(headrooms)
                             if headrooms else None),
        'minHeadroomMm': min(headrooms) if headrooms else None,
        'medianModelUpperMm': statistics.median(uppers) if uppers else None,
        'fullStepExactValidRate': (sum(1 for v in full_valid if v)
                                   / len(full_valid)) if full_valid else None,
        'depthBandPieces': [entry['depthBandPieces'] for entry in rounds],
        'edgesFirstRound': len(rounds[0]['edges']) if rounds else None,
        'blockFirstRound': rounds[0]['block'] if rounds else None,
        'setterFirstRound': rounds[0]['setter'] if rounds else None,
    }


def main():
    outdir, binary, parents_json = sys.argv[1], sys.argv[2], sys.argv[3]
    specs = [s.replace(';', ',') for s in sys.argv[4].split(',')]
    request = sys.argv[5] if len(sys.argv) > 5 else 'mixed-61'
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'parents': parents_json,
        'request': request,
        'specs': specs,
        'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        cell = {'seed': seed, 'parentRawDepthMm': parent['rawDepthMm'],
                'specs': {}}
        for spec in specs:
            tag = spec.replace('=', '').replace(',', '-').replace('.', 'p')
            path = f'{outdir}/seed{seed}-{tag}.json'
            doc, wall, err, code = runlib.probe(
                binary, request, seed, parent['fixture'],
                {'POLYGON_NESTING_CONTACT_BLOCK': spec}, path)
            if doc is None:
                cell['specs'][spec] = {'error': err, 'exitCode': code}
                print(f'seed{seed} {spec}: FAILED {err[-200:]}', flush=True)
                continue
            row = cell_summary(doc)
            row['processWallSeconds'] = wall
            cell['specs'][spec] = row
            print(f"seed{seed} {spec}: parent={row['parentDepthMm']:.4f} "
                  f"final={row['finalDepthMm']:.4f} "
                  f"delta={row['deltaMm']:.6f} "
                  f"acc={row['roundsAccepted']}/{row['rounds']} "
                  f"block~{row['medianBlockSize']} "
                  f"headroom~{row['medianHeadroomMm']} "
                  f"upper~{row['medianModelUpperMm']} "
                  f"val={row['validations']} {row['elapsedMs']:.0f}ms "
                  f"{row['refusals']}", flush=True)
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/blockprobe.json', 'w'), indent=1)

    summary = {}
    for spec in specs:
        rows = [cell['specs'][spec] for cell in result['cells']
                if 'deltaMm' in cell['specs'].get(spec, {})]
        if not rows:
            continue
        deltas = [r['deltaMm'] for r in rows]
        refusals = {}
        for r in rows:
            for key, count in r['refusals'].items():
                refusals[key] = refusals.get(key, 0) + count
        headrooms = [r['medianHeadroomMm'] for r in rows
                     if r['medianHeadroomMm'] is not None]
        uppers = [r['medianModelUpperMm'] for r in rows
                  if r['medianModelUpperMm'] is not None]
        rates = [r['fullStepExactValidRate'] for r in rows
                 if r['fullStepExactValidRate'] is not None]
        summary[spec] = {
            'cells': len(rows),
            'medianDeltaMm': statistics.median(deltas),
            'maxDeltaMm': max(deltas),
            'cellsMoved': sum(1 for d in deltas if d > 0),
            'roundsAccepted': sum(r['roundsAccepted'] for r in rows),
            'roundsTotal': sum(r['rounds'] for r in rows),
            'solves': sum(r['solves'] for r in rows),
            'validations': sum(r['validations'] for r in rows),
            'refusals': refusals,
            'medianHeadroomMm': (statistics.median(headrooms)
                                 if headrooms else None),
            'medianModelUpperMm': (statistics.median(uppers)
                                   if uppers else None),
            'medianFullStepExactValidRate': (statistics.median(rates)
                                             if rates else None),
            'medianElapsedMs': statistics.median(
                [r['elapsedMs'] for r in rows]),
            'medianBlockSize': statistics.median(
                [r['medianBlockSize'] for r in rows
                 if r['medianBlockSize'] is not None]),
        }
    result['summary'] = summary
    json.dump(result, open(f'{outdir}/blockprobe.json', 'w'), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
