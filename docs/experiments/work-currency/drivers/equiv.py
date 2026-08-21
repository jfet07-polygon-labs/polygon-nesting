#!/usr/bin/env python3
"""§3.1: the two equivalences claim (a) rests on.

**Arm 1, `base` vs `ship` at `cur2=0`.** The whole point of a *parallel*
currency is that the shipped one does not move. This runs the campaign base
commit's combo binary and this tree's, at a pinned work budget, on the bare
request, and compares the whole documents. Equal means: every pinned work
number in this repository - the ledger's spends, `portfolio.workUnits`, every
operator call's `workUnits` - reproduces bit for bit, because the same
trajectory ran.

**Arm 2, `ship` at `cur2=0` vs `cur2=2`.** The observing mode has to be a pure
observer. It reads the counter array twice more per operator call and computes
a price; if it changed anything else, every rate in §1 would be a measurement
of the instrument. Equal here means: the two documents differ **only** in the
currency's own block, which is exactly what `digestNoCurrency` tests while
`digest` differs.

Both arms are work budgets, so both are functions of counters and neither is a
statement about the clock. A wall column is reported and is the box's.

    python3 equiv.py OUT_JSON BASE_BIN SHIP_BIN [work_units]
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

CELLS = [(request, seed)
         for request in ('mixed-61', 'shapes-17', 'triangle-20')
         for seed in (0, 1, 2)]


def cell(base_bin, ship_bin, request, seed, units, outdir):
    # The base binary is run with **no `cur2` key at all**, and that is
    # deliberate rather than a workaround: an unarmed binary refuses a key it
    # cannot honour, which is the campaign's own rule (`m34pconfirm`, `fcv`,
    # `crot`). So this arm proves the stronger of the two available
    # statements - a spec *without* the key on the old binary and a spec with
    # the key at `0` on the new one are the same run - rather than the weaker
    # one where both sides name the key.
    spec_bare = runlib.spec_for(seed, 'work', units, True)
    spec_off = runlib.spec_for(seed, 'work', units, True, 'cur2=0')
    spec_obs = runlib.spec_for(seed, 'work', units, True, 'cur2=2')
    tag = f'{request}-s{seed}'
    base, base_wall, base_err = runlib.run(
        base_bin, request, seed, spec_bare, f'{outdir}/equiv-{tag}-base.json')
    ship, ship_wall, ship_err = runlib.run(
        ship_bin, request, seed, spec_off, f'{outdir}/equiv-{tag}-ship.json')
    obs, obs_wall, obs_err = runlib.run(
        ship_bin, request, seed, spec_obs, f'{outdir}/equiv-{tag}-obs.json')

    def depth(doc):
        return ((doc.get('portfolio') or {}).get('incumbent') or {}).get(
            'rawDepthMm')

    def units_of(doc):
        return (doc.get('portfolio') or {}).get('workUnits')

    return {
        'request': request, 'seed': seed, 'units': units,
        'specs': {'base': spec_bare, 'ship': spec_off, 'observe': spec_obs},
        'baseDigest': runlib.doc_digest(base),
        'shipDigest': runlib.doc_digest(ship),
        'observeDigest': runlib.doc_digest(obs),
        'observeDigestNoCurrency': runlib.doc_digest_without_currency(obs),
        'shipDigestNoCurrency': runlib.doc_digest_without_currency(ship),
        # Arm 1: the shipped meter is untouched by the presence of the code.
        'baseEqualsShip': runlib.doc_digest(base) == runlib.doc_digest(ship),
        # Arm 2: observing changes the reporting and nothing else.
        'observeIsPureObserver': (
            runlib.doc_digest_without_currency(obs)
            == runlib.doc_digest_without_currency(ship)),
        'observeAddsItsOwnBlock': (
            runlib.doc_digest(obs) != runlib.doc_digest(ship)),
        # The digest's own audit: what actually differed, and how much of it
        # the clock accounts for. A digest that matched because the strip list
        # grew is a digest that proves nothing, so both arms report the leaf
        # diff beside the boolean.
        'baseShipLeafDiff': runlib.leaf_diff(base, ship),
        'observeLeafDiff': runlib.leaf_diff(ship, obs, runlib.CURRENCY_KEYS),
        'baseDepthMm': depth(base), 'shipDepthMm': depth(ship),
        'observeDepthMm': depth(obs),
        'baseWorkUnits': units_of(base), 'shipWorkUnits': units_of(ship),
        'observeWorkUnits': units_of(obs),
        'walls': [base_wall, ship_wall, obs_wall],
        'errors': [e[-200:] for e in (base_err, ship_err, obs_err) if e],
    }


def main():
    out, base_bin, ship_bin = sys.argv[1], sys.argv[2], sys.argv[3]
    units = int(sys.argv[4]) if len(sys.argv) > 4 else runlib.WORK_10S
    outdir = os.path.dirname(out)
    rows = [cell(base_bin, ship_bin, request, seed, units, outdir)
            for request, seed in CELLS]
    document = {
        'baseBinary': base_bin, 'baseSha256': runlib.sha256_of(base_bin),
        'shipBinary': ship_bin, 'shipSha256': runlib.sha256_of(ship_bin),
        'workUnits': units,
        'rows': rows,
        'summary': {
            'cells': len(rows),
            'baseEqualsShip': sum(r['baseEqualsShip'] for r in rows),
            'observeIsPureObserver': sum(
                r['observeIsPureObserver'] for r in rows),
            'observeAddsItsOwnBlock': sum(
                r['observeAddsItsOwnBlock'] for r in rows),
        },
        'boxLoad': runlib.LOAD,
    }
    with open(out, 'w') as handle:
        json.dump(document, handle, indent=1, sort_keys=True)
    print(json.dumps(document['summary'], indent=1))
    for row in rows:
        diff = row['baseShipLeafDiff']
        print(f"{row['request']:<12} s{row['seed']}  "
              f"base==ship {str(row['baseEqualsShip']):>5}  "
              f"observe pure {str(row['observeIsPureObserver']):>5}  "
              f"leaves {diff['leaves']} differing {diff['differing']} "
              f"(clock alone {diff['differingBeforeWallStrip']})  "
              f"depth {row['shipDepthMm']}")


if __name__ == '__main__':
    main()
