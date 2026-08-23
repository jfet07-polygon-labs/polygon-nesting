#!/usr/bin/env python3
"""**Is the dual gate a rubber stamp?** — classified by which authority refused.

The code auditor's F6 is that `invalidPublications` has no reachable witness:
`published_raw_depth_mm` is only written after `kernel_exclusive_valid` and
`contract_valid` are both true, so "published but not dual-valid" is an
invariant of the emitter rather than a measurement. That is true, and it is
also not the interesting question. The interesting question is whether the two
authorities ever *say no* on this fixture - a gate that never refuses is a gate
that proves nothing about the layouts it lets through.

Every exact checkpoint of the 27 cells is classified:

  * `kernelRefused`   - `kernelExclusiveValid == false` (the Exclusive r=2.500
                        grid scan said no; the contract validator was never
                        reached, because publish.rs returns first);
  * `contractRefused` - kernel said yes and
                        `validate_placements_against_contract` said no; this is
                        the untouched validator refusing real geometry;
  * `targetRefused`   - both would have passed but the repair enlarged the
                        locked strip;
  * `published`       - both said yes.

`contractRefused > 0` is the witness F6 says does not exist for
`invalidPublications`: it is the same predicate, evaluated on the same
placements, disagreeing.
"""
import json
import os
import sys
import collections

RAW = os.environ.get('ICS_RAW', '/var/lib/t3/tmp/overlapics/rerun')
BUDGETS = ('3', '10', '30')
TARGET_REFUSAL = 'repair would have enlarged the locked strip; the target is immutable'


def main():
    tally = collections.Counter()
    per_cell = []
    contract_examples = []
    for budget in BUDGETS:
        for seed in range(9):
            with open(f'{RAW}/wall-{budget}s-seed{seed}.json') as handle:
                doc = json.load(handle)
            local = collections.Counter()
            for c in doc['outcome']['exactCheckpoints']:
                k, t = c['kernelExclusiveValid'], c['contractValid']
                if c['publishedRawDepthMm'] is not None:
                    key = 'published'
                elif not k:
                    key = ('targetRefused' if c['refusal'] == TARGET_REFUSAL
                           else 'kernelRefused')
                elif not t:
                    key = ('targetRefused' if c['refusal'] == TARGET_REFUSAL
                           else 'contractRefused')
                else:
                    key = 'unclassified'
                local[key] += 1
                tally[key] += 1
                if key == 'contractRefused' and len(contract_examples) < 8:
                    contract_examples.append(
                        {'cell': f'{budget}s-seed{seed}',
                         'proposalOrdinal': c['proposalOrdinal'],
                         'maxViolationMm': c['maxViolationMm'],
                         'repairRows': c['repairRows'],
                         'repairMaxDisplacementMm':
                             c['repairMaxDisplacementMm'],
                         'refusal': c['refusal']})
            per_cell.append({'cell': f'{budget}s-seed{seed}', **local})
    doc = {
        'what': 'exact checkpoints of the committed round, by refusing authority',
        'raw': RAW,
        'totals': dict(tally),
        'perCell': per_cell,
        'contractValidatorRefusalExamples': contract_examples,
        'CONTRACT_VALIDATOR_IS_LIVE': tally['contractRefused'] > 0,
        'KERNEL_IS_LIVE': tally['kernelRefused'] > 0,
        'unclassified': tally['unclassified'],
    }
    if len(sys.argv) > 1:
        with open(sys.argv[1], 'w') as handle:
            handle.write(json.dumps(doc, indent=1, sort_keys=True) + '\n')
    print(json.dumps(doc['totals'], indent=1))
    print('CONTRACT_VALIDATOR_IS_LIVE:', doc['CONTRACT_VALIDATOR_IS_LIVE'],
          ' KERNEL_IS_LIVE:', doc['KERNEL_IS_LIVE'],
          ' unclassified:', doc['unclassified'])
    for e in contract_examples[:4]:
        print('  contract refusal:', e['cell'], e['proposalOrdinal'],
              e['refusal'][:78])
    return 0 if not doc['unclassified'] else 1


if __name__ == '__main__':
    sys.exit(main())
