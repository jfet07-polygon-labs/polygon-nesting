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

# `skip_pile_score.rs::shortfall_class`'s own vocabulary, verbatim. Listed here
# rather than discovered from the data so that a class with zero rows still
# appears in the histogram - "no row was grid-class" is the finding, and a
# histogram that omitted the empty bucket would not report it.
CLASSES = ('grid-class (<=10 um)', 'intermediate (10 um .. 0.1 mm)',
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

    # The same joint table, split by the proxy's own reason for calling the
    # frontier infeasible. `feasible()` is `boundary_violations == 0 and
    # collision_pairs.is_empty()`, so a skip is either an overlap skip or a
    # boundary-only skip, and the two are not the same population at all.
    by_reason = {}
    for cell in score['cells']:
        for record in cell['records']:
            reason = ('overlap' if record['proxyCollisionPairs'] > 0
                      else 'boundary-only')
            for row in record['allowances']:
                key = (f"{row['allowanceMm']:.4f}", reason,
                       row['contractAccepts'],
                       row['miterVerdict']['accepted'],
                       row['kernelVerdict']['accepted'])
                by_reason[key] = by_reason.get(key, 0) + 1
    reason_table = {}
    for (allowance, reason, contract, miter, kernel), count in sorted(
            by_reason.items()):
        bucket = reason_table.setdefault(allowance, {}).setdefault(
            reason, {'records': 0, 'rows': {}})
        bucket['records'] += count
        bucket['rows'][f'contract={contract},miter={miter},'
                       f'kernel={kernel}'] = count
    for allowance, reasons in reason_table.items():
        for reason, bucket in reasons.items():
            released = sum(count for key, count in bucket['rows'].items()
                           if 'miter=False' in key and 'kernel=True' in key)
            bucket['released'] = released
            bucket['releasedFraction'] = (released / bucket['records']
                                          if bucket['records'] else None)

    # What each row of the joint table was worth, and not only whether it was
    # legal. `gainOverIncumbentMm` is the slice's own incumbent at the step of
    # the skip minus this frontier's raw source depth, so a positive value is a
    # publication the schedule declined to ask about.
    gains = {}
    for cell in score['cells']:
        for record in cell['records']:
            for row in record['allowances']:
                key = (f"{row['allowanceMm']:.4f}",
                       f"contract={row['contractAccepts']},"
                       f"miter={row['miterVerdict']['accepted']},"
                       f"kernel={row['kernelVerdict']['accepted']}")
                bucket = gains.setdefault(key, {'records': 0, 'wouldPublish': 0,
                                                'values': []})
                bucket['records'] += 1
                bucket['wouldPublish'] += int(bool(row['wouldHavePublished']))
                if row['wouldHavePublished']:
                    bucket['values'].append(row['gainOverIncumbentMm'])
    gain_table = {}
    for (allowance, key), bucket in sorted(gains.items()):
        gain_table.setdefault(allowance, {})[key] = {
            'records': bucket['records'],
            'wouldHavePublished': bucket['wouldPublish'],
            'wouldHavePublishedFraction': (bucket['wouldPublish']
                                           / bucket['records']),
            'gainWhenItWouldMm': quantiles(bucket['values']),
        }

    excursions, by_class = [], {name: 0 for name in CLASSES}
    bexcursions, bby_class = [], {name: 0 for name in CLASSES}
    released_excursions = []
    released_by_class = {name: 0 for name in CLASSES}
    released_bexcursions = []
    released_bby_class = {name: 0 for name in CLASSES}
    layouts_with_released_pairs = 0
    layouts_with_released_boundaries = 0
    censused = 0
    kernel_pair_failures, miter_pair_failures, proxy_pairs = [], [], []
    proxy_boundaries = []
    for cell in score['cells']:
        for record in cell['records']:
            proxy_pairs.append(record['proxyCollisionPairs'])
            proxy_boundaries.append(record['proxyBoundaryViolations'])
            census = record.get('census')
            if not census:
                continue
            censused += 1
            kernel_pair_failures.append(census['kernelPairFailures'])
            miter_pair_failures.append(census['miterPairFailures'])
            # Two populations, and they are not the same one. Every censused
            # layout can carry pairs the disc admits and the miter refuses -
            # that is the join tax, present whether or not the layout as a whole
            # was released. The `released` half is the subset that lives on a
            # layout the disc accepts *whole*, and that is the population the
            # question is about.
            was_released = any(row['released'] for row in record['allowances'])
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
                if was_released:
                    released_excursions.append(value)
                    if row.get('class') in released_by_class:
                        released_by_class[row['class']] += 1
            battributed = census['boundaries'][
                'kernelAdmitsMiterRefusesAttributed']
            if battributed:
                layouts_with_released_boundaries += 1
            for row in battributed:
                value = row.get('excursionMm')
                if value is None:
                    continue
                bexcursions.append(value)
                if row.get('class') in bby_class:
                    bby_class[row['class']] += 1
                if was_released:
                    released_bexcursions.append(value)
                    if row.get('class') in released_bby_class:
                        released_bby_class[row['class']] += 1

    # The cleanest form of the finding, and the one that needs no counterfactual
    # at all: the deepest layout in a cell's whole skip pile, under each
    # authority, against what that cell actually published.
    #
    # `gainOverIncumbentMm` above mixes two depth conventions by one grid step -
    # the slice's own `published_depth_mm` is measured on grid-snapped bounds
    # and the scorer's `rawSourceDepthMm` on untouched source rings - and it is
    # a counterfactual besides, because publishing one frontier would move the
    # incumbent the next one is judged against. This comparison has neither
    # problem: both numbers are `raw_source_long_axis_depth_mm` of a finished
    # layout, and "the run's own answer was beaten by a layout it never asked
    # about" is a statement about the run that happened.
    final_depth = {}
    if len(sys.argv) > 3:
        for cell in json.load(open(sys.argv[3]))['cells']:
            final_depth[cell['label']] = cell['measured'].get(
                'rawSourceDepthMm')
    best = []
    for cell in score['cells']:
        published = final_depth.get(cell['label'])
        row = {'label': cell['label'], 'cellPublishedRawSourceDepthMm':
               published}
        for authority, predicate in (
                ('miter', lambda a: a['miterVerdict']['accepted']),
                ('kernel', lambda a: a['kernelVerdict']['accepted']),
                ('allThree', lambda a: (a['contractAccepts']
                                        and a['miterVerdict']['accepted']
                                        and a['kernelVerdict']['accepted'])),
                ('releasedOnly', lambda a: a['released'])):
            depths = [a['rawSourceDepthMm'] for record in cell['records']
                      for a in record['allowances']
                      if a['allowanceMm'] == score['censusAllowanceMm']
                      and predicate(a)]
            deepest = min(depths) if depths else None
            row[authority] = {
                'records': len(depths),
                'deepestRawSourceDepthMm': deepest,
                'beatsCellPublishedByMm': (
                    published - deepest
                    if deepest is not None and published is not None else None),
            }
        best.append(row)

    summary = {
        'score': sys.argv[1],
        'planSha256': score.get('planSha256'),
        'requestSha256': score.get('requestSha256'),
        'dumpedRecordsTotal': score['dumpedRecordsTotal'],
        'sampledRecordsTotal': score['sampledRecordsTotal'],
        'censusAllowanceMm': score['censusAllowanceMm'],
        'compositeReadingsAgree': score['compositeReadingsAgree'],
        'jointTable': {str(k): v for k, v in table.items()},
        'jointTableByProxyReason': reason_table,
        'publicationValueByJointRow': gain_table,
        'deepestSuppressedFrontierPerCell': best,
        'releasedLayoutCount': score['releasedLayoutCount'],
        'kernelRefusesMiterAcceptsCount': score[
            'kernelRefusesMiterAcceptsCount'],
        'censusedRecords': censused,
        # Named for what they count: a censused layout carrying at least one row
        # where the disc admits and the miter refuses. That is NOT the same as a
        # released layout - the layout as a whole may still be refused by both -
        # and an earlier draft of this file called it one.
        'censusedLayoutsWithAKernelAdmitsMiterRefusesPair':
            layouts_with_released_pairs,
        'censusedLayoutsWithAKernelAdmitsMiterRefusesBoundary':
            layouts_with_released_boundaries,
        # Over every censused layout, released or not: the join tax as it
        # exists in the pile.
        'censusedPairExcursionMm': quantiles(excursions),
        'censusedPairExcursionClassHistogram': by_class,
        'censusedBoundaryExcursionMm': quantiles(bexcursions),
        'censusedBoundaryExcursionClassHistogram': bby_class,
        # Over released layouts only: the class of the material the disc would
        # actually have freed. This is the answer to "1 um class or 0.5 mm
        # class?".
        'releasedPairExcursionMm': quantiles(released_excursions),
        'releasedPairExcursionClassHistogram': released_by_class,
        'releasedBoundaryExcursionMm': quantiles(released_bexcursions),
        'releasedBoundaryExcursionClassHistogram': released_bby_class,
        'releasedLayoutsThatWouldHavePublished': sum(
            1 for row in score['releasedLayouts']
            if row.get('wouldHavePublished')),
        'releasedLayoutGainOverIncumbentMm': quantiles(
            [row['gainOverIncumbentMm'] for row in score['releasedLayouts']
             if row.get('gainOverIncumbentMm') is not None]),
        'kernelPairFailuresPerCensusedLayout': quantiles(kernel_pair_failures),
        'miterPairFailuresPerCensusedLayout': quantiles(miter_pair_failures),
        'proxyCollisionPairsPerSampledRecord': quantiles(proxy_pairs),
        'proxyBoundaryViolationsPerSampledRecord': quantiles(proxy_boundaries),
        'perCell': [{
            'label': cell['label'],
            'dumpedRecords': cell['dumpedRecords'],
            'distinctFingerprints': cell['distinctFingerprints'],
            'skipsSuppressed': cell['skipsSuppressed'],
            'distinctExpected': cell['distinctExpected'],
            'duplicateSkips': cell['duplicateSkips'],
            'dumpMatchesSinkTally': cell['dumpMatchesSinkTally'],
            'sampledRecords': cell['sampledRecords'],
            'releasedAtCensusAllowance': cell['releasedAtCensusAllowance'],
            'allThreeRefuseAtCensusAllowance': cell[
                'allThreeRefuseAtCensusAllowance'],
        } for cell in score['cells']],
    }
    json.dump(summary, open(out_path, 'w'), indent=1)
    print(json.dumps({k: v for k, v in summary.items()
                      if k not in ('perCell', 'publicationValueByJointRow',
                                   'jointTableByProxyReason',
                                   'deepestSuppressedFrontierPerCell')},
                     indent=1))
    print(json.dumps({'deepestSuppressedFrontierPerCell':
                      summary['deepestSuppressedFrontierPerCell']}, indent=1))
    print(json.dumps({'publicationValueByJointRow':
                      summary['publicationValueByJointRow']}, indent=1))
    print(json.dumps({'jointTableByProxyReason':
                      summary['jointTableByProxyReason']}, indent=1))
    for cell in summary['perCell']:
        print(json.dumps(cell))


if __name__ == '__main__':
    main()
