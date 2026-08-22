#!/usr/bin/env python3
"""The contact corpus as a pass/fail gate, for the FAST tier and the HEAVY one.

    python3 corpus_gate.py [states]      # 1,000 fast, 10,000 heavy

Exits non-zero when any clause fails. The clauses are Sol review 14 §3's:

  * zero proxy-feasible / exact-invalid states outside the 4 um band;
  * no containment false-feasible case;
  * incremental rows == cold rows, bit for bit;
  * an accepted negative-force step improves the independent active violation
    in >= 95 % of cases, and does not worsen the independent total in >= 80 %.

The force clauses are scored on the population the spec defines for them - the
`compressed` family, 1 %/3 %/10 % residual compression plus SE(2) perturbation.
The `grazing` and `containment` families are this round's additions, they exist
so the band and containment clauses have a population at all, and their rates
are reported next to the scored ones rather than folded into them. Both numbers
are in the output; nothing is hidden by the split.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402


def main():
    out = os.environ.get('ICS_OUT', lib.OUT)
    states = int(sys.argv[1]) if len(sys.argv) > 1 else 1_000
    doc, wall, status, err = lib.run(
        'corpus', 'mixed-61', f'{out}/corpus-{states}.json',
        states=states, seed=0)
    if status != 0:
        print(json.dumps({'exit': status, 'stderr': err, 'CORPUS_PASS': False},
                         indent=1))
        return 1
    corpus = doc.get('corpus', {})
    verdict = doc.get('verdict', {})
    document = {
        'experiment': 'overlap-ics',
        'battery': f'contact-corpus-{states}',
        'binary': lib.BIN,
        'corpus': corpus,
        'verdict': verdict,
        'forceMisses': doc.get('forceMisses', [])[:16],
        'wallSeconds': wall,
        'CORPUS_PASS': bool(verdict.get('pass')),
    }
    print(json.dumps(document, indent=1))
    with open(f'{out}/corpus-gate-{states}.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    return 0 if document['CORPUS_PASS'] else 1


if __name__ == '__main__':
    sys.exit(main())
