#!/usr/bin/env python3
"""Totals for the round-boundary suites, read off the logs `run-suites.sh` wrote.

    python3 suitetotals.py

`cargo test` prints one `test result:` line per target, so a suite's headline
number is the sum over its targets and not the last line - which is what a
`| tail -1` would report and how a partially red suite gets written up as green.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

EVIDENCE = f'{lib.ROOT}/docs/experiments/overlap-ics/evidence'
SUITES = [
    ('1', 'jagua-experimental', 'suite-jagua'),
    ('2', 'the protocol full combo', 'suite-combo'),
    ('3', 'jagua-experimental --example general_request_benchmark',
     'suite-example'),
    ('4', 'jagua-experimental,overlap-ics', 'suite-overlap-ics-stacked'),
    ('5', 'overlap-ics --lib --tests', 'suite-overlap-ics'),
]
PATTERN = re.compile(
    r'^test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored', re.M)


def main():
    rows = []
    for number, features, name in SUITES:
        path = f'{EVIDENCE}/{name}.log'
        try:
            with open(path, errors='replace') as handle:
                text = handle.read()
        except OSError:
            rows.append({'suite': number, 'features': features,
                         'log': name, 'error': 'missing log'})
            continue
        found = PATTERN.findall(text)
        rows.append({
            'suite': number,
            'features': features,
            'log': f'{name}.log',
            'targets': len(found),
            'passed': sum(int(item[1]) for item in found),
            'failed': sum(int(item[2]) for item in found),
            'ignored': sum(int(item[3]) for item in found),
            'rerunAfterKnownFlake': os.path.exists(
                f'{EVIDENCE}/{name}-run1-flaky.log'),
        })
    document = {
        'experiment': 'overlap-ics',
        'battery': 'round-boundary-suites',
        'suites': rows,
        'SUITES_PASS': all(row.get('failed') == 0 for row in rows),
    }
    print(json.dumps(document, indent=1))
    with open(f'{EVIDENCE}/suites.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    return 0 if document['SUITES_PASS'] else 1


if __name__ == '__main__':
    sys.exit(main())
