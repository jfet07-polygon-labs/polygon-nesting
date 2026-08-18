#!/usr/bin/env python3
"""Checks that the equal-work allowance is non-binding.

    python3 allowancecheck.py UNBOUNDED.json EQUALWORK.json

"Equal work" is only a fair comparison if the allowance did not truncate any
arm. The check is direct: run every arm again with an allowance far above any
arm's own spend, and compare the two batteries arm by arm. Zero differing
fields means the number every arm reports is the number it reaches on its own,
not the number the allowance let it reach.
"""
import json
import sys

FIELDS = ('probeWorkUnitsSpent', 'deltaRawMm', 'exitRawDepthMm',
          'probePublications', 'probeExitCause', 'exitDualGateValid',
          'probeOperatorCalls')


def main():
    unbounded = json.load(open(sys.argv[1]))
    equalwork = json.load(open(sys.argv[2]))

    def index(document):
        return {(row['seed'], row['arm']): row for row in document['rows']}

    left, right = index(unbounded), index(equalwork)
    differences = []
    for key in sorted(left):
        for field in FIELDS:
            if left[key].get(field) != right[key].get(field):
                differences.append({
                    'seed': key[0], 'arm': key[1], 'field': field,
                    'unbounded': left[key].get(field),
                    'equalwork': right[key].get(field),
                })
    print(json.dumps({
        'unboundedAllowance': unbounded['probeWork'],
        'equalWorkAllowance': equalwork['probeWork'],
        'armsCompared': len(left),
        'fieldsPerArm': len(FIELDS),
        'differences': len(differences),
        'detail': differences,
        'ALLOWANCE_NON_BINDING': not differences,
    }, indent=1))


if __name__ == '__main__':
    main()
