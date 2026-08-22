#!/usr/bin/env python3
"""Reduces the scoring document to the one number and the joint table.

    summarize.py SCOREJSON OUTJSON

The one number is the **released fraction**: sampled skipped frontiers the disc
kernel accepts and the miter authority refuses, over sampled skipped frontiers,
at the allowance the skip happened at. Both composite verdicts run the material
contract on the same placements, so a released row is a layout HEAD cannot
publish and an engine with the kernel wired in could.

The joint table is `contract x miter x kernel`, one row per combination per
allowance, and it is printed whole - including the combinations with zero
records - because a table that only listed what happened would hide the shape of
what did not.

Beside it: the class histogram of every per-pair excursion the censuses found
where the disc admits a pair and the miter refuses it. That is the join tax,
measured on the pile itself, and it is what says whether a release - if there
were one - would be worth a millimetre or a micrometre.
"""
import json
import statistics
import sys

CLASSES = ('sub-micron-class (<=10 um)', 'intermediate (10 um .. 0.1 mm)',
           'join-tax-class (>=0.1 mm)')


def quantiles(values):
    if not values:
        return {}
    ordered = sorted(values)
    return {
        'count': len(ordered), 'min': ordered[0], 'max': ordered[-1],
        'median': statistics.median(ordered),
        'mean': statistics.fmean(ordered),
        'p10': ordered[max(0, int(0.10 * (len(ordered) - 1)))],
        'p90': ordered[min(len(ordered) - 1, int(0.90 * (len(ordered) - 1)))],
    }


def main():
    score = json.load(open(sys.argv[1]))
    out_path = sys.argv[2]
    allowances = sorted({row['allowanceMm']
                         for row in score['jointDistribution']})
    table = {}
    for allowance in allowances:
        rows = {}
        for contract in (True, False):
            for miter in (True, False):
                for kernel in (True, False):
                    rows[f'contract={contract},miter={miter},'
                         f'kernel={kernel}'] = 0
        for row in score['jointDistribution']:
            if row['allowanceMm'] != allowance:
                continue
            key = (f"contract={row['contractAccepts']},"
                   f"miter={row['miterAccepts']},"
                   f"kernel={row['kernelAccepts']}")
            rows[key] += row['records']
        total = sum(rows.values())
        released = sum(count for key, count in rows.items()
                       if 'miter=False' in key and 'kernel=True' in key)
        p0 = sum(count for key, count in rows.items()
                 if 'miter=True' in key and 'kernel=False' in key)
        all_refuse = rows['contract=False,miter=False,kernel=False']
        contract_only = sum(count for key, count in rows.items()
                            if key.startswith('contract=True')
                            and 'miter=False' in key and 'kernel=False' in key)
        table[allowance] = {
            'records': total, 'rows': rows,
            'released': released,
            'releasedFraction': released / total if total else None,
            'kernelRefusesMiterAccepts': p0,
            'allThreeRefuse': all_refuse,
            'allThreeRefuseFraction': all_refuse / total if total else None,
            'contractAcceptsBothEnvelopesRefuse': contract_only,
        }

    excursions, by_class = [], {name: 0 for name in CLASSES}
    layouts_with_released_pairs = 0
    censused = 0
    kernel_pair_failures, miter_pair_failures, proxy_pairs = [], [], []
    for cell in score['cells']:
        for record in cell['records']:
            proxy_pairs.append(record['proxyCollisionPairs'])
            census = record.get('census')
            if not census:
                continue
            censused += 1
            kernel_pair_failures.append(census['kernelPairFailures'])
            miter_pair_failures.append(census['miterPairFailures'])
            attributed = census['kernelAdmitsMiterRefusesAttributed']
            if attributed:
                layouts_with_released_pairs += 1
            for row in attributed:
                value = row.get('excursionMm')
                if value is None:
                    continue
                excursions.append(value)
                if row.get('class') in by_class:
                    by_class[row['class']] += 1

    summary = {
        'score': sys.argv[1],
        'planSha256': score.get('planSha256'),
        'requestSha256': score.get('requestSha256'),
        'dumpedRecordsTotal': score['dumpedRecordsTotal'],
        'sampledRecordsTotal': score['sampledRecordsTotal'],
        'censusAllowanceMm': score['censusAllowanceMm'],
        'compositeReadingsAgree': score['compositeReadingsAgree'],
        'jointTable': {str(k): v for k, v in table.items()},
        'releasedLayoutCount': score['releasedLayoutCount'],
        'kernelRefusesMiterAcceptsCount': score[
            'kernelRefusesMiterAcceptsCount'],
        'censusedRecords': censused,
        'censusedLayoutsWithAtLeastOneReleasedPair':
            layouts_with_released_pairs,
        'pairExcursionMm': quantiles(excursions),
        'pairExcursionClassHistogram': by_class,
        'kernelPairFailuresPerCensusedLayout': quantiles(kernel_pair_failures),
        'miterPairFailuresPerCensusedLayout': quantiles(miter_pair_failures),
        'proxyCollisionPairsPerSampledRecord': quantiles(proxy_pairs),
        'perCell': [{
            'label': cell['label'],
            'dumpedRecords': cell['dumpedRecords'],
            'distinctFingerprints': cell['distinctFingerprints'],
            'expectedSkips': cell['expectedSkips'],
            'dumpIsWholeSkipPile': cell['dumpIsWholeSkipPile'],
            'sampledRecords': cell['sampledRecords'],
            'releasedAtCensusAllowance': cell['releasedAtCensusAllowance'],
            'allThreeRefuseAtCensusAllowance': cell[
                'allThreeRefuseAtCensusAllowance'],
        } for cell in score['cells']],
    }
    json.dump(summary, open(out_path, 'w'), indent=1)
    print(json.dumps({k: v for k, v in summary.items()
                      if k != 'perCell'}, indent=1))
    for cell in summary['perCell']:
        print(json.dumps(cell))


if __name__ == '__main__':
    main()
