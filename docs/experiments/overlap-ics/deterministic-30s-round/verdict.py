#!/usr/bin/env python3
"""Apply the pre-committed gates to the frozen battery reductions."""

import json
import os
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
EVIDENCE = f'{HERE}/evidence'
ARMS = ('control', 'orders', 'factor', 'composed')

BAR_MM = 168.484
PRIMARY_MEDIAN_MM = 163.00461
PRIMARY_QUORUM = 7
PRIMARY_GAIN_MM = 1.000
TEN_QUORUM = 5
TEN_P95_SECONDS = 10.000
SIXTY_WATCH_MM = 161.00


def load(name, required=False):
    path = f'{EVIDENCE}/{name}.json'
    try:
        with open(path) as handle:
            return json.load(handle)
    except FileNotFoundError:
        if required:
            raise
        return None


def floor(document):
    summaries = document['summary']['arms']
    return {
        'binaryUnchanged': document['binaryUnchangedDuringBattery'],
        'allCellsValid': all(summaries[arm]['allCellsValid'] for arm in ARMS),
        'zeroInvalidPublications': all(
            summaries[arm]['invalidPublications'] == 0 for arm in ARMS),
        'allPlansAndKeysHold': all(
            summaries[arm]['allPlansAndKeysHold'] for arm in ARMS),
        'allLedgersAndKeysHold': all(
            summaries[arm]['allLedgersAndKeysHold'] for arm in ARMS),
        'allFrozenLiteralsIntact': all(
            summaries[arm]['allFrozenLiteralsIntact'] for arm in ARMS),
    }


def all_true(mapping):
    return all(mapping.values())


def primary_verdict(curve30):
    composed = curve30['summary']['arms']['composed']
    contrast = curve30['summary']['primaryContrast']
    validity = floor(curve30)
    clauses = {
        '1_composedMedianAtMost163.00461': bool(
            composed['medianMm'] is not None
            and composed['medianMm'] <= PRIMARY_MEDIAN_MM),
        '2_composedAtLeast7of9AtMost168.484':
            composed['quorumReached'] >= PRIMARY_QUORUM,
        '3_pairedMedianGainAtLeast1mm': bool(
            contrast['medianGainMm'] is not None
            and contrast['medianGainMm'] >= PRIMARY_GAIN_MM),
        '4_zeroInvalidAndRevalidated': bool(
            validity['allCellsValid']
            and validity['zeroInvalidPublications']),
        '5_planAndChargeIdentitiesHold': bool(
            validity['binaryUnchanged']
            and validity['allPlansAndKeysHold']
            and validity['allLedgersAndKeysHold']
            and validity['allFrozenLiteralsIntact']),
    }
    return {
        'gateOwner': 'composed',
        'clauses': clauses,
        'PASS': all_true(clauses),
        'composedMedianMm': composed['medianMm'],
        'composedBestMm': composed['bestMm'],
        'composedQualifyingSeeds': composed['qualifyingSeeds'],
        'composedQuorum': composed['quorumReached'],
        'pairedMedianGainMm': contrast['medianGainMm'],
        'floor': validity,
        'wallReportOnly': {
            arm: {
                'p95Seconds': curve30['summary']['arms'][arm][
                    'p95WallSeconds'],
                'maxSeconds': curve30['summary']['arms'][arm][
                    'maxWallSeconds'],
            } for arm in ARMS
        },
        'no30SecondP95Clause': True,
    }


def ten_verdict(gate10):
    composed = gate10['summary']['arms']['composed']
    validity = floor(gate10)
    identity = bool(gate10.get('ALL_TWO_PROCESS_BIT_IDENTICAL'))
    clauses = {
        '1_composedAtLeast5of9AtMost168.484':
            composed['quorumReached'] >= TEN_QUORUM,
        '2_composedMedianAtMost168.484': bool(
            composed['medianMm'] is not None
            and composed['medianMm'] <= BAR_MM),
        '3_composedP95AtMost10.000': bool(
            composed['p95WallSeconds'] is not None
            and composed['p95WallSeconds'] <= TEN_P95_SECONDS),
        '4_twoProcessBitIdentityEverySeed': identity,
        '5_validityAndPlanChargeFloor': bool(
            all_true(validity)),
    }
    quality = clauses['1_composedAtLeast5of9AtMost168.484'] and clauses[
        '2_composedMedianAtMost168.484']
    floor_green = all_true(validity) and identity
    return {
        'gateOwner': 'composed',
        'clauses': clauses,
        'PASS': all_true(clauses),
        'qualityPass': quality,
        'floorGreen': floor_green,
        'composedMedianMm': composed['medianMm'],
        'composedBestMm': composed['bestMm'],
        'composedQualifyingSeeds': composed['qualifyingSeeds'],
        'composedQuorum': composed['quorumReached'],
        'composedP95Seconds': composed['p95WallSeconds'],
        'composedMaxWallSeconds': composed['maxWallSeconds'],
        'allTwoProcessBitIdentical': identity,
        'allFiveBitIdentical': gate10.get('ALL_FIVE_BIT_IDENTICAL'),
        'floor': validity,
        'permanentRetirementRequired': bool(not quality and floor_green),
        'consequence': (
            '10-second gate passes' if quality else
            ('retire the 10-second quality gate permanently; a new mechanism '
             'requires a new pre-committed specification') if floor_green else
            'measurement defect: repair the floor, do not retarget'),
    }


def curve_report(document, watch=None):
    if document is None:
        return None
    result = {
        'gated': False,
        'arms': {
            arm: {
                key: document['summary']['arms'][arm][key]
                for key in ('bestMm', 'medianMm', 'qualifyingSeeds',
                            'p95WallSeconds', 'maxWallSeconds')
            } for arm in ARMS
        },
        'floor': floor(document),
    }
    if watch is not None:
        median = result['arms']['composed']['medianMm']
        result['composedMedianWatchMm'] = watch
        result['watchReached'] = bool(median is not None and median <= watch)
        result['watchIsNotAClause'] = True
    return result


def main():
    gate0 = load('gate0/gate0', required=True)
    budget = load('budget/budget', required=True)
    curve30 = load('curve30', required=True)
    gate10 = load('gate10')
    curve3 = load('curve3')
    curve60 = load('curve60')
    document = {
        'experiment': 'overlap-ics',
        'battery': 'deterministic-30s-round-verdict',
        'spec': 'docs/deterministic-30s-round-spec.md',
        'gate0': {
            'PASS': gate0['GATE0_PASS'],
            'orders4P95Seconds': gate0['summaries']['mixed-61'][
                'arms']['A']['p95Seconds'],
            'orders1P95Seconds': gate0['summaries']['mixed-61'][
                'arms']['B']['p95Seconds'],
            'pairedMedianSavingSeconds': gate0['summaries']['mixed-61'][
                'pairedMedianSavingSeconds'],
        },
        'budget': {
            'PASS': budget['BUDGET_PASS'],
            'f4*': budget['factorDerivation']['f4']['chosenFactor'],
            'f1*': budget['factorDerivation']['f1']['chosenFactor'],
            'kappaSeconds': budget['factorDerivation']['kappaSeconds'],
            'shelfWorkPlanWitnessGreen': budget[
                'shelfWorkPlanWitness']['green'],
            'allPlanHits': budget['ALL_PLAN_HITS'],
        },
        'primary30Seconds': primary_verdict(curve30),
        'tenSeconds': ten_verdict(gate10) if gate10 else None,
        'threeSeconds': curve_report(curve3),
        'sixtySeconds': curve_report(curve60, SIXTY_WATCH_MM),
        'sparrowHorizonMm': 150.165,
    }
    prerequisites = bool(
        document['gate0']['PASS'] and document['budget']['PASS'])
    document['ROUND_VALID_SO_FAR'] = bool(
        prerequisites and document['primary30Seconds']['floor']
        and all_true(document['primary30Seconds']['floor']))
    document['PRIMARY_GATE_PASS'] = document['primary30Seconds']['PASS']
    document['TEN_SECOND_GATE_PASS'] = (
        document['tenSeconds']['PASS'] if document['tenSeconds'] else None)
    with open(f'{EVIDENCE}/verdict.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    print(json.dumps(document, indent=1))
    return 0


if __name__ == '__main__':
    sys.exit(main())
