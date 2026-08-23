#!/usr/bin/env python3
"""Prints the `detail` of named vectors out of an audit document.

    python3 show.py <audit.json> <substring> [<substring> ...]
"""
import json
import sys


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    document = json.load(open(sys.argv[1]))
    wanted = sys.argv[2:]
    for row in document.get('vectors', []):
        if any(needle in row['vector'] for needle in wanted):
            print('==', row['vector'], '->', 'OK' if row['ok'] else 'FAIL')
            print(json.dumps(row['detail'], indent=1))
    return 0


if __name__ == '__main__':
    sys.exit(main())
