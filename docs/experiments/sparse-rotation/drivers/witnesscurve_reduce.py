#!/usr/bin/env python3
"""Re-reduce `witnesscurve.py`'s saved certificate documents without re-running
them.

    witnesscurve_reduce.py OUTDIR ITERS[,ITERS...]

The first run of `witnesscurve.py` matched the program enum on its Rust variant
spelling (`DepthOnly`/`Se2`) while the certificate serializes camelCase
(`depthOnly`/`se2`), so every cell reduced to `None` and the summary was empty.
The documents themselves were correct and are on disk; this re-reads them under
the fixed matcher rather than spending the wall again, and the wall column is
therefore taken from the re-run's own `witnesscurve.json` where present.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import witnesscurve  # noqa: E402


def main():
    outdir = sys.argv[1]
    iterations = [int(v) for v in sys.argv[2].split(',')]
    previous = {}
    path = f'{outdir}/witnesscurve.json'
    if os.path.exists(path):
        for cell in json.load(open(path))['cells']:
            previous[(cell['seed'], cell['iterations'])] = cell
    result = {'outdir': outdir, 'iterations': iterations, 'cells': []}
    for iters in iterations:
        for name in sorted(os.listdir(outdir)):
            if not name.endswith(f'-i{iters}.json'):
                continue
            seed = int(name.split('-')[0].removeprefix('seed'))
            doc = json.load(open(f'{outdir}/{name}'))
            program = witnesscurve.headline(doc)
            if program is None:
                continue
            witness = program.get('witness') or {}
            cell = {
                'seed': seed, 'iterations': iters,
                'wallSeconds': previous.get((seed, iters), {})
                .get('wallSeconds'),
                'publishedDepthMm': doc.get('publishedDepthMm'),
                'rows': doc.get('rows'),
                'thetaCapMaxDeg': doc.get('thetaCapMaxDeg'),
                'deltaMm': witness.get('deltaMm'),
                'scale': witness.get('scale'),
                'fullVectorExactValid': witness.get('fullVectorExactValid'),
                'movedPieces': witness.get('movedPieces'),
                'validations': witness.get('validations'),
                'maxAbsDthetaDeg': witness.get('maxAbsDthetaDeg'),
                'primalLowerMm': program['lp'].get('primalLowerMm'),
                'dualUpperMm': program['lp'].get('dualUpperMm'),
                'verdict': program.get('verdict'),
            }
            # The translation-only column, so "did rotation add anything" is a
            # comparison rather than an absolute number.
            for other in doc.get('programs', []):
                if (other['program'].lower() == 'depthonly'
                        and other['motion'].lower() == 'translationonly'):
                    cell['translationDeltaMm'] = \
                        (other.get('witness') or {}).get('deltaMm')
            result['cells'].append(cell)

    summary = {}
    for iters in iterations:
        rows = [c for c in result['cells']
                if c['iterations'] == iters and c.get('deltaMm') is not None]
        if not rows:
            continue
        deltas = [c['deltaMm'] for c in rows]
        translation = [c.get('translationDeltaMm') for c in rows
                       if c.get('translationDeltaMm') is not None]
        walls = [c['wallSeconds'] for c in rows if c['wallSeconds']]
        summary[str(iters)] = {
            'cells': len(rows),
            'medianDeltaMm': statistics.median(deltas),
            'minDeltaMm': min(deltas), 'maxDeltaMm': max(deltas),
            'positiveCells': sum(1 for d in deltas if d > 0),
            'medianTranslationDeltaMm': (statistics.median(translation)
                                         if translation else None),
            'se2BeatsTranslationCells': sum(
                1 for c in rows
                if c.get('translationDeltaMm') is not None
                and c['deltaMm'] > c['translationDeltaMm']),
            'medianScale': statistics.median([c['scale'] for c in rows]),
            'fullVectorValidCells': sum(1 for c in rows
                                        if c['fullVectorExactValid']),
            'medianValidations': statistics.median(
                [c['validations'] for c in rows]),
            'medianMaxAbsDthetaDeg': statistics.median(
                [c['maxAbsDthetaDeg'] for c in rows]),
            'medianWallSeconds': statistics.median(walls) if walls else None,
            # Four programs share one row build, so a quarter over-estimates the
            # one program design C solves and is used as an over-estimate.
            'medianPerProgramSeconds': (statistics.median(walls) / 4.0
                                        if walls else None),
        }
    result['summary'] = summary
    json.dump(result, open(f'{outdir}/witnesscurve-reduced.json', 'w'),
              indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
