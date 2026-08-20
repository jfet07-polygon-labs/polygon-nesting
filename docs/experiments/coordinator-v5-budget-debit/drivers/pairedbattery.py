#!/usr/bin/env python3
"""The paired interleaved *two-binary* battery for the budget-debit fix.

`coordinator-v4/drivers/battery.py` compares arms of one binary selected by
portfolio spec keys. The question here is different and cannot be asked that
way: the debit is a *code* change, so the two arms are two builds of the same
commit range - `fixed` (this branch) and `unfixed` (its parent, f32c629) - run
under one identical spec. Everything else is battery.py's protocol, kept
deliberately: every round runs every arm once per seed, back to back, with the
arm order rotating each round, because the box is shared and an unpaired number
on it would be worthless.

    pairedbattery.py NAME ROUNDS REQUEST SEEDS SPEC ARM=BINARY [ARM=BINARY ...]

`SPEC` is the portfolio spec tail with `{seed}` and `{cells}` placeholders,
e.g. 'work=40000000,cells={cells},v3=1,sched=1,barren=1,divq=1'.

Reports, in addition to battery.py's per-seed depths and pairwise deltas:

* `debit`: what the fix actually charged - per arm, the schedule actions seen,
  their `selfMeteredUnits`, `debitedUnits`, and the total debit;
* `ordering`: a *weak* in-battery check for Sol review 6 §1 finding 4 - for
  every operator call that was debited, whether the publication and the
  archived basin that call produced are stamped at or after the debited
  reading. An unfixed binary has no such fields at all and reports `null`.

  This is a smoke check and nothing more: a cumulative reading can clear that
  bound by accident once the run is far enough along. The claim is settled by
  `orderingcheck.py` (the exact identity `workUnits == globalUnits +
  debitedUnits`, which the pre-fix ordering cannot produce) and by
  `stampdelta.py` (the same identity across the paired arms' stamps, where the
  corrected and pre-fix orderings predict different numbers). Do not cite this
  field on its own.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlibv6 as runlib  # noqa: E402


def spec_for(template, seed):
    cells = runlib.SALT_SETS[seed % len(runlib.SALT_SETS)]
    return template.format(seed=seed, cells=cells)


def debit_rows(doc):
    """Every self-metered operator call in one run's document."""
    portfolio = doc.get('portfolio') or {}
    rows = []
    for call in portfolio.get('operatorCalls') or []:
        if call.get('selfMeteredUnits') is None:
            continue
        rows.append({
            'operator': call['operator'],
            'phase': call['phase'],
            'globalUnits': call.get('globalUnits'),
            'selfMeteredUnits': call.get('selfMeteredUnits'),
            'debitedUnits': call.get('debitedUnits'),
            'workUnits': call.get('workUnits'),
            'resultFingerprint': call.get('resultFingerprint'),
            'published': call.get('published'),
            'startedSeconds': call.get('startedSeconds'),
        })
    return rows


def ordering_check(doc):
    """Sol review 6 §1 finding 4, checked on a real run.

    For every operator call that was actually debited: the publication it
    produced (matched on fingerprint) and the archived basin it produced must
    be stamped with a work reading that already contains the debit. The
    pre-fix ordering stamps them *before* the debit, so the reading is
    strictly below the debited amount for the first such action of a run.
    """
    portfolio = doc.get('portfolio') or {}
    calls = [c for c in (portfolio.get('operatorCalls') or [])
             if (c.get('debitedUnits') or 0) > 0]
    if not calls:
        return None
    publications = {p['fingerprint']: p
                    for p in portfolio.get('publications') or []}
    basins = {b['fingerprint']: b
              for b in (portfolio.get('archive') or {}).get('members') or []}
    checks = []
    for call in calls:
        fingerprint = call.get('resultFingerprint')
        row = {'operator': call['operator'],
               'debitedUnits': call['debitedUnits'],
               'callWorkUnits': call['workUnits'],
               'callIncludesOwnDebit':
                   call['workUnits'] >= call['debitedUnits']}
        publication = publications.get(fingerprint)
        if publication:
            row['publicationWorkUnits'] = publication['workUnits']
            row['publicationIncludesDebit'] = \
                publication['workUnits'] >= call['debitedUnits']
        basin = basins.get(fingerprint)
        if basin and basin.get('birthWorkUnits') is not None:
            row['birthWorkUnits'] = basin['birthWorkUnits']
            row['birthIncludesDebit'] = \
                basin['birthWorkUnits'] >= call['debitedUnits']
        checks.append(row)
    return checks


def main():
    name = sys.argv[1]
    rounds = int(sys.argv[2])
    request = sys.argv[3]
    seeds = [int(v) for v in sys.argv[4].split(',')]
    template = sys.argv[5]
    arms = []
    for entry in sys.argv[6:]:
        label, _, binary = entry.partition('=')
        arms.append((label, binary))

    out_dir = f'{runlib.OUT}/{name}'
    result = {'name': name, 'request': request, 'rounds': rounds,
              'seeds': seeds, 'spec': template,
              'arms': [{'label': a, 'binary': b} for a, b in arms],
              'rows': []}
    for round_index in range(rounds):
        for seed in seeds:
            ordered = arms[round_index % len(arms):] + \
                arms[:round_index % len(arms)]
            for label, binary in ordered:
                tag = f'{label}-s{seed}-r{round_index}'
                spec = spec_for(template, seed)
                doc, seconds, _ = runlib.run(
                    binary, request, seed, spec, f'{out_dir}/runs/{tag}.json')
                row = runlib.summarize(tag, doc, seconds)
                row.pop('phases', None)
                row.pop('publications', None)
                row.pop('operatorCalls', None)
                row.pop('archive', None)
                row.update({'arm': label, 'seed': seed, 'round': round_index,
                            'spec': spec, 'binary': binary,
                            'debitCalls': debit_rows(doc),
                            'ordering': ordering_check(doc)})
                schedule = row.get('schedule')
                if schedule:
                    row['scheduleActions'] = [
                        {k: a.get(k) for k in
                         ('iteration', 'class', 'actualCost', 'meteredCost',
                          'selfMeteredUnits', 'debitedUnits', 'workUnits',
                          'publications')}
                        for a in schedule['actions']]
                    row['scheduleClasses'] = schedule['classes']
                    row['scheduleExit'] = schedule['exitCause']
                    row['scheduleIterations'] = schedule['iterations']
                    row.pop('schedule')
                result['rows'].append(row)
                print(f"{tag}: engine={row['engineDepthMm']} "
                      f"raw={row.get('rawDepthMm')} "
                      f"work={row.get('workUnits')} "
                      f"debit={sum(c['debitedUnits'] or 0 for c in row['debitCalls'])} "
                      f"process={seconds:.2f}s", flush=True)
    os.makedirs(out_dir, exist_ok=True)
    report(result)
    json.dump(result, open(f'{out_dir}/battery.json', 'w'), indent=1)
    print(f'wrote {out_dir}/battery.json')


def report(result):
    rows = result['rows']
    labels = [arm['label'] for arm in result['arms']]
    by_key = {(r['arm'], r['seed'], r['round']): r for r in rows}
    seeds = sorted({r['seed'] for r in rows})
    rounds = sorted({r['round'] for r in rows})
    summary = {'perSeed': {}, 'pairwise': {}, 'processSeconds': {},
               'workUnits': {}, 'debit': {}, 'ordering': {}}
    for seed in seeds:
        summary['perSeed'][str(seed)] = {
            label: [by_key[(label, seed, r)]['engineDepthMm']
                    for r in rounds if (label, seed, r) in by_key]
            for label in labels}
    for left in labels:
        for right in labels:
            if left >= right:
                continue
            deltas, work_deltas = [], []
            for seed in seeds:
                for r in rounds:
                    a, b = by_key.get((left, seed, r)), by_key.get((right, seed, r))
                    if not a or not b:
                        continue
                    if a['engineDepthMm'] is None or b['engineDepthMm'] is None:
                        continue
                    deltas.append(b['engineDepthMm'] - a['engineDepthMm'])
                    if a.get('workUnits') and b.get('workUnits'):
                        work_deltas.append(b['workUnits'] - a['workUnits'])
            if deltas:
                summary['pairwise'][f'{right}-minus-{left}'] = {
                    'medianMm': statistics.median(deltas),
                    'meanMm': statistics.fmean(deltas),
                    'minMm': min(deltas), 'maxMm': max(deltas),
                    'cellsRightBetter': sum(1 for d in deltas if d < 0),
                    'cellsLeftBetter': sum(1 for d in deltas if d > 0),
                    'cellsEqual': sum(1 for d in deltas if d == 0),
                    'cells': len(deltas),
                    'medianWorkUnitsDelta':
                        statistics.median(work_deltas) if work_deltas else None,
                }
    for label in labels:
        values = [r['processSeconds'] for r in rows if r['arm'] == label]
        summary['processSeconds'][label] = {
            'median': statistics.median(values),
            'min': min(values), 'max': max(values)}
        work = [r['workUnits'] for r in rows
                if r['arm'] == label and r.get('workUnits')]
        summary['workUnits'][label] = {
            'median': statistics.median(work), 'min': min(work),
            'max': max(work)} if work else None
        calls = [c for r in rows if r['arm'] == label for c in r['debitCalls']]
        summary['debit'][label] = {
            'selfMeteredCalls': len(calls),
            'callsWithDebit': sum(1 for c in calls if (c['debitedUnits'] or 0) > 0),
            'totalDebited': sum(c['debitedUnits'] or 0 for c in calls),
            'medianSelfUnits':
                statistics.median([c['selfMeteredUnits'] for c in calls])
                if calls else None,
            'medianGlobalUnits':
                statistics.median([c['globalUnits'] for c in calls])
                if calls else None,
        }
        checks = [c for r in rows if r['arm'] == label
                  for c in (r['ordering'] or [])]
        summary['ordering'][label] = {
            'debitedCalls': len(checks),
            'callIncludesOwnDebit': sum(1 for c in checks
                                        if c.get('callIncludesOwnDebit')),
            'publicationsChecked': sum(1 for c in checks
                                       if 'publicationIncludesDebit' in c),
            'publicationIncludesDebit':
                sum(1 for c in checks if c.get('publicationIncludesDebit')),
            'birthsChecked': sum(1 for c in checks if 'birthIncludesDebit' in c),
            'birthIncludesDebit':
                sum(1 for c in checks if c.get('birthIncludesDebit')),
        } if checks else None
    summary['errors'] = [{'tag': r['tag'], 'error': r['loadError'][-300:]}
                         for r in rows if 'loadError' in r]
    result['summary'] = summary
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
