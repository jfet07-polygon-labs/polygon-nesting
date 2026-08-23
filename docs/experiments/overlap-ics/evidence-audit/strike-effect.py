#!/usr/bin/env python3
"""**Did the one semantic change actually change the trajectory population?**

    python3 strike-effect.py <round1-bites-red.json> <rerun wall.json> [out.json]

The rerun's whole licence is one line: `Engine::separate`'s no-improvement
counter now pauses on a marginal minimum instead of resetting. The stated
consequence is that separations on the Φ ≈ 1e-4 shelf can now strike out, so
Algorithm 12's disruption can fire.

That is a claim about two populations - `strikes` and `disruptions` - and both
are recorded per bite in both rounds' committed evidence. This script counts
them on each side. It asserts nothing; it prints the two populations so a reader
can see whether the repair is demonstrated or merely asserted.
"""
import json
import os
import sys


def totals_from_bite_arrays(arrays):
    strikes = disruptions = attempts = bites = published = 0
    cells_with_strike = cells_with_disruption = 0
    for rows in arrays:
        rows = rows or []
        cell_strikes = sum(row.get('strikes', 0) for row in rows)
        cell_disruptions = sum(row.get('disruptions', 0) for row in rows)
        strikes += cell_strikes
        disruptions += cell_disruptions
        attempts += sum(row.get('attempts', 0) for row in rows)
        bites += len(rows)
        published += sum(1 for row in rows if row.get('published'))
        cells_with_strike += 1 if cell_strikes else 0
        cells_with_disruption += 1 if cell_disruptions else 0
    return {
        'cells': len(arrays),
        'bites': bites,
        'publishedBites': published,
        'failedSeparations': attempts,
        'strikes': strikes,
        'disruptions': disruptions,
        'cellsWithAnyStrike': cells_with_strike,
        'cellsWithAnyDisruption': cells_with_disruption,
    }


def bite_of(rows, ordinal):
    for row in rows or []:
        if row.get('ordinal') == ordinal:
            return row
    return None


def named_bite_table(round1, rerun, budget, ordinal):
    """The README's own bite-22 tables, recomputed from the two bite files."""
    table = []
    for seed in range(9):
        old = bite_of(round1.get('cells', {})
                      .get(f'{budget}s-seed{seed}', {}).get('bites'), ordinal)
        new_rows = None
        for seed_row in rerun.get('cells', {}).get(budget, {}).get('seeds', []):
            if seed_row.get('seed') == seed:
                new_rows = seed_row.get('bites')
        new = bite_of(new_rows, ordinal)
        table.append({
            'seed': seed,
            'round1': None if old is None else {
                'masterIterations': old['masterIterations'],
                'strikes': old['strikes'],
                'disruptions': old['disruptions'],
                'attempts': old['attempts'],
                'published': old['published'],
                'minRawPhi': old.get('minRawPhi'),
            },
            'rerun': None if new is None else {
                'masterIterations': new['masterIterations'],
                'strikes': new['strikes'],
                'disruptions': new['disruptions'],
                'attempts': new['attempts'],
                'published': new['published'],
                'minRawPhi': new.get('minRawPhi'),
            },
        })
    return table


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    round1 = json.load(open(sys.argv[1]))
    rerun = json.load(open(sys.argv[2]))
    out_path = sys.argv[3] if len(sys.argv) > 3 else None

    round1_arrays = [cell.get('bites') for cell in round1.get('cells', {}).values()]
    rerun_arrays = []
    for cell in rerun.get('cells', {}).values():
        for seed_row in cell.get('seeds', []):
            if seed_row.get('valid'):
                rerun_arrays.append(seed_row.get('bites'))

    document = {
        'experiment': 'overlap-ics',
        'battery': 'evidence-audit-strike-effect',
        'round1': totals_from_bite_arrays(round1_arrays),
        'rerun': totals_from_bite_arrays(rerun_arrays),
        # The README's headline tables (§5 and §6), recomputed. The claim there
        # is scoped to ONE bite - seed 1's 22nd - not to the whole population,
        # and the population totals below show why the scoping matters: round 1
        # was not globally strike-free.
        'bite22At30s': named_bite_table(round1, rerun, '30', 22),
        'bite22At10s': named_bite_table(round1, rerun, '10', 22),
    }
    named = document['bite22At30s'][1]
    document['namedCellRepairDemonstrated'] = bool(
        named['round1'] and named['rerun']
        and named['round1']['strikes'] == 0
        and named['round1']['disruptions'] == 0
        and named['rerun']['strikes'] >= 3
        and named['rerun']['disruptions'] >= 1
        and named['rerun']['published'])
    document['round1WasGloballyStrikeFree'] = document['round1']['strikes'] == 0
    print(json.dumps(document, indent=1))
    if out_path:
        os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
        with open(out_path, 'w') as handle:
            json.dump(document, handle, indent=1)
    return 0


if __name__ == '__main__':
    sys.exit(main())
