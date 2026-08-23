#!/usr/bin/env python3
"""**The two driver repairs, red and green, on evidence that already exists.**

    python3 frame_vector.py [raw-cell-dir]

Two things need proving and neither needs a wall second, because both are
reductions of documents the campaign has already written:

  F1  **`wall.py`'s checkpoint frame did not move on any committed cell.** The
      audit's repair to `wall.py` is committed evidence; this round moved its
      arithmetic into `lib.within_budget` so `control.py` could share it. That
      refactor must be a no-op. This re-runs the pre-refactor expression,
      inline and verbatim, against `lib.within_budget` over every raw cell
      document the committed `wall.json` reduced, and requires the three counts
      and the qualifying depth to be identical on every one.

      On **new** cells the shared helper is deliberately not identical: it
      prefers the `loopEntrySeconds` this round started emitting, which is the
      clock offset itself rather than a bracket that also contains the document
      build. That is a tightening of the upper bound, in the direction of
      deciding more publications rather than fewer, and it is what keeps the
      per-publication poses this round added from widening the undecided band.
      Committed cells carry no such field and fall back, which is why F1 is
      green over all 27 of them.

  C1  **`control.py`'s missing filter, red and green.** The audit's caveat on
      that file is "min over all publications, no frame": arm A's reported
      depth was a plain minimum with no time filter at all, so a publication
      that landed after the arm's own budget could become the number the
      control is read on. The repaired reduction is the same one `wall.py`
      uses. This computes both on the same documents and reports where they
      differ - which is the red vector, and it is nonzero.

The raw cells default to `/var/lib/t3/tmp/overlapics/rerun/`, the directory the
audit's revalidation chapter used to settle F2. If they are gone, the script
says so and exits 2 rather than reporting a green it did not measure.
"""
import json
import os
import sys

sys.path.insert(0, os.path.abspath(os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    '..', '..', 'drivers')))
import lib  # noqa: E402

RAW = '/var/lib/t3/tmp/overlapics/rerun'
BUDGETS = {'3': 3.000, '10': 10.000, '30': 30.000}
SEEDS = list(range(9))


def old_wall_filter(publications, document, limit):
    """`wall.py`'s post-audit, pre-refactor block, copied verbatim.

    Nothing here may be tidied. It is a *witness*, and a witness that has been
    improved is not one.
    """
    constructor_s = document.get('wall', {}).get('constructorSeconds')
    search_s = document.get('wall', {}).get('searchSeconds')
    total_s = document.get('wall', {}).get('totalSeconds')
    lower_offset = constructor_s
    upper_offset = (None if total_s is None or search_s is None
                    else total_s - search_s)

    def request_seconds(row, offset):
        loop_s = row.get('wallSeconds')
        if loop_s is None or offset is None:
            return None
        return offset + loop_s

    within, late, undecided = [], [], []
    for row in publications:
        low = request_seconds(row, lower_offset)
        high = request_seconds(row, upper_offset)
        if low is not None and low > limit:
            late.append(row)
            continue
        within.append(row)
        if low is not None and high is not None and high > limit:
            undecided.append(row)
    return within, late, undecided


def old_control_reduction(publications, constructor_fingerprint):
    """`control.py`'s pre-repair `arm_a` depth: a minimum over everything."""
    strict = [row for row in publications
              if row['placementFingerprint'] != constructor_fingerprint]
    return min((row['publishedRawDepthMm'] for row in strict), default=None)


def new_reduction(publications, document, limit, constructor_fingerprint):
    within, _, _ = lib.within_budget(publications, document, limit)
    strict = [row for row in within
              if row['placementFingerprint'] != constructor_fingerprint]
    return min((row['publishedRawDepthMm'] for row in strict), default=None)


def main():
    raw = sys.argv[1] if len(sys.argv) > 1 else RAW
    cells = []
    for budget, limit in sorted(BUDGETS.items(), key=lambda row: row[1]):
        for seed in SEEDS:
            path = f'{raw}/wall-{budget}s-seed{seed}.json'
            try:
                with open(path) as handle:
                    document = json.load(handle)
            except (OSError, json.JSONDecodeError) as error:
                cells.append({'path': path, 'readable': False,
                              'error': f'{error}'})
                continue
            outcome = document.get('outcome') or {}
            publications = outcome.get('publications') or []
            fingerprint = (document.get('constructor') or {}).get(
                'placementFingerprint')
            old = old_wall_filter(publications, document, limit)
            new = lib.within_budget(publications, document, limit)
            old_control = old_control_reduction(publications, fingerprint)
            repaired = new_reduction(publications, document, limit, fingerprint)
            cells.append({
                'path': path,
                'sourceSha256': lib.source_sha256(path),
                'readable': True,
                'budgetSeconds': limit,
                'seed': seed,
                'publications': len(publications),
                'carriesWallSeconds': all(row.get('wallSeconds') is not None
                                          for row in publications),
                # F1: the refactor is a no-op.
                'frameCountsIdentical': (
                    [len(part) for part in old] == [len(part) for part in new]),
                'frameRowsIdentical': all(
                    [id(row) for row in one] == [id(row) for row in two]
                    for one, two in zip(old, new)),
                'within': len(new[0]),
                'excludedAsLate': len(new[1]),
                'undecidedByFrame': len(new[2]),
                # C1: the control's red/green on the same cell.
                'controlUnfilteredDepthMm': old_control,
                'controlFilteredDepthMm': repaired,
                'controlDiffers': old_control != repaired,
            })

    readable = [row for row in cells if row.get('readable')]
    if not readable:
        print(json.dumps({
            'experiment': 'overlap-ics',
            'battery': 'economics-round-census-driver-frame-vector',
            'REFUSED': True,
            'reason': f'no raw cell document under {raw}; nothing measured',
            'cells': cells,
        }, indent=1))
        return 2

    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-census-driver-frame-vector',
        'rawCellDirectory': raw,
        'cells': cells,
        'cellsRead': len(readable),
        'F1_refactorIsANoOp': all(row['frameCountsIdentical']
                                  and row['frameRowsIdentical']
                                  for row in readable),
        'F1_excludedAsLateTotal': sum(row['excludedAsLate'] for row in readable),
        'F1_undecidedTotal': sum(row['undecidedByFrame'] for row in readable),
        # The red vector: cells where the control's old unfiltered minimum is
        # not the filtered one. Nonzero means the missing filter mattered.
        'C1_cellsWhereControlDiffers': [
            {'budgetSeconds': row['budgetSeconds'], 'seed': row['seed'],
             'unfilteredMm': row['controlUnfilteredDepthMm'],
             'filteredMm': row['controlFilteredDepthMm']}
            for row in readable if row['controlDiffers']],
        'C1_controlDifferedOnCells': sum(1 for row in readable
                                         if row['controlDiffers']),
        'note': ('The committed cells carry no `loopEntrySeconds`, so both '
                 'expressions take the same fallback branch and F1 is a '
                 'no-op claim about exactly these 27 documents. A cell '
                 'written by this round\'s binary takes the tighter branch '
                 'on purpose; `bracketWidthMs` below is measured on a fresh '
                 'cell by `census/README.md` rather than here.'),
    }
    print(json.dumps(document, indent=1))
    out = os.environ.get('ICS_OUT', '/var/lib/t3/tmp/census-wave1')
    os.makedirs(out, exist_ok=True)
    with open(f'{out}/frame-vector.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    return 0 if document['F1_refactorIsANoOp'] else 1


if __name__ == '__main__':
    sys.exit(main())
