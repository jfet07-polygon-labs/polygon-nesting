#!/usr/bin/env python3
"""The battery's corpus verdicts on this round's publications, as a table.

    pubaudit.py BATTERYJSON INDEXJSON OUT.json

Reduces the battery's `population1CanonicalCorpus` rows - run on the layouts
`publications.py` extracted - to one row per published layout:

* `miterAccepts`    HEAD's authority on that layout;
* `unionAccepts`    the armed authority. Accepting implies the untouched
  material contract accepted, because the wire point runs the contract on both
  of its branches and returns `Err` if the contract refuses;
* `roundAccepts`    the exclusive kernel, for the record;
* `newAdmission`    union accepts and miter refuses - the class the kill about
  the contact-block 0.5 mm depth ceiling is written against;
* `regression`      miter accepts and union refuses. The union cannot produce
  one by construction; it is computed rather than assumed.

`constructor-fresh` is the battery's own synthetic entry and is reported
separately, because it is not one of this round's publications.
"""
import json
import sys


def accepts(node):
    return bool((node or {}).get('accepted'))


def main():
    battery = json.load(open(sys.argv[1]))
    index = json.load(open(sys.argv[2]))
    out_path = sys.argv[3]
    rows = []
    extras = []
    for entry in battery['population1CanonicalCorpus']['rows']:
        label = entry['label']
        row = {
            'label': label,
            'searchOffsetAllowanceMm': entry['searchOffsetAllowanceMm'],
            'miterAccepts': accepts(entry.get('compositeMiterVerdict')),
            'roundAccepts': accepts(entry.get('compositeRoundVerdict')),
            'unionAccepts': accepts(entry.get('compositeUnionVerdict')),
            'miterMessage': (entry.get('compositeMiterVerdict') or {})
            .get('message'),
            'unionMessage': (entry.get('compositeUnionVerdict') or {})
            .get('message'),
            'rowsMiterAdmitsKernelRefuses':
                len(entry.get('miterAdmitsKernelRefusesAttributed') or []),
            'layoutP0': entry.get('layoutP0'),
            'layoutP0Union': entry.get('layoutP0Union'),
        }
        row['newAdmission'] = row['unionAccepts'] and not row['miterAccepts']
        row['regression'] = row['miterAccepts'] and not row['unionAccepts']
        meta = index.get(label)
        if meta is None:
            extras.append(row)
            continue
        row.update(meta)
        rows.append(row)
    summary = {
        'layouts': len(rows),
        'unionAcceptsAll': all(r['unionAccepts'] for r in rows),
        'miterAcceptsAll': all(r['miterAccepts'] for r in rows),
        'roundAcceptsCount': sum(1 for r in rows if r['roundAccepts']),
        'newAdmissionCount': sum(1 for r in rows if r['newAdmission']),
        'regressionCount': sum(1 for r in rows if r['regression']),
        'armLayouts': sum(1 for r in rows if r.get('arm') != 'miter'),
        'unionRefusals': [r['label'] for r in rows if not r['unionAccepts']],
        'newAdmissions': [r for r in rows if r['newAdmission']],
        'rows': rows,
        'notThisRoundsPublications': extras,
    }
    json.dump(summary, open(out_path, 'w'), indent=1)
    print(json.dumps({k: v for k, v in summary.items()
                      if k not in ('rows', 'notThisRoundsPublications',
                                   'newAdmissions')}, indent=1))


if __name__ == '__main__':
    main()
