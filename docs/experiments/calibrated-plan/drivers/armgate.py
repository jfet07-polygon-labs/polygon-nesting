#!/usr/bin/env python3
"""What the promoted defaults actually did, measured as whole documents.

    python3 armgate.py OUTDIR NEW_BINARY BASE_BINARY UNARMED_BINARY \\
        REQUESTS SEEDS WORKUNITS

A default that is claimed and not measured is a comment. Five claims, each one
a document comparison at a **work** budget so both sides are deterministic and
the box cannot be the difference:

  1. `<default>` == `m34pconfirm=1,fcv=1`
     The defaults are what the doc comment says they are. If this fails the
     promotion did not happen.

  2. `<default>` == base binary's `m34pconfirm=1`
     The new binary's default *is* the old binary's shipping configuration.
     This is the one that says the promotion changed a default and not an
     engine.

  3. `<default>` == `m34pconfirm=0`, as a **document**
     Expected *equal*, and that is not a bug: `pconfirm` is documented as
     semantics-preserving, and the certificate is a proof of clearance rather
     than an estimate. Neither touches `Counter::ExactPairTests`, so at a work
     budget both arms take the same branches and produce the same layout. What
     they differ in is **wall**, which is why the per-confirmation microbench
     below is the half of this claim that carries the weight.

  4. `m34pconfirm=0` == base binary's `<default>`
     Opting out reproduces the *old* default exactly, so nothing was lost.

  5. `<default>` == `fcv=0`, same shape and for the same reason.

Because claims 3 and 5 are document-*equal* by construction, the evidence that
the two opt-outs are live is the **per accepted confirmation** wall each arm
reports - `scheduleSlice.confirmationMs / confirmationsAccepted`, summed over
the run, which is the same microbenchmark `fast-contract-validator` §12 ran and
is re-run here on the promoted binary. A key that changed nothing would show
the same milliseconds.

Plus one negative: an **unarmed** binary (a build without the features) must
*refuse* `fcv=0` and `m34pconfirm=0` rather than run the other arm under their
label.

"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import planbattery  # noqa: E402
import runlib  # noqa: E402


def confirmation_cost(portfolio):
    """`(ms per accepted confirmation, accepted, total ms)` over the whole run.

    Taken from the run that produced the depth rather than from a separate
    battery, exactly as `fast-contract-validator/drivers/factorial.py` takes
    it: every mode-34 slice reports its own `confirmationMs` and
    `confirmationsAccepted`, so the sum over the run is the cost of one
    accepted confirmation in the regime the depth was actually produced in.
    """
    total_ms, accepted = 0.0, 0
    for call in portfolio.get('operatorCalls') or []:
        slice_report = call.get('scheduleSlice') or {}
        total_ms += slice_report.get('confirmationMs') or 0.0
        accepted += slice_report.get('confirmationsAccepted') or 0
    return (total_ms / accepted if accepted else None), accepted, total_ms


def run_cell(binary, request, seed, units, extra, outdir, tag):
    spec = runlib.spec_for(seed, 'work', units, True, extra)
    doc, wall, err = runlib.run(binary, request, seed, spec,
                                f'{outdir}/{tag}.json')
    portfolio = doc.get('portfolio') or {}
    per_confirm, accepted, total_ms = confirmation_cost(portfolio)
    return {
        'tag': tag, 'spec': spec, 'binary': os.path.basename(binary),
        'processWallSeconds': wall,
        'digest': planbattery.digest(doc) if portfolio else None,
        'rawDepthMm': (portfolio.get('incumbent') or {}).get('rawDepthMm'),
        'workUnits': portfolio.get('workUnits'),
        'msPerAcceptedConfirmation': per_confirm,
        'confirmationsAccepted': accepted,
        'confirmationMsTotal': total_ms,
        'error': None if portfolio else err[-300:],
    }


def main():
    outdir, new_bin, base_bin, unarmed_bin = sys.argv[1:5]
    requests = sys.argv[5].split(',')
    seeds = [int(v) for v in sys.argv[6].split(',')]
    units = sys.argv[7]
    os.makedirs(outdir, exist_ok=True)

    # (label, binary, extra spec)
    ARMS = [
        ('new-default', 'new', ''),
        ('new-explicit', 'new', 'm34pconfirm=1,fcv=1'),
        ('new-pconfirm0', 'new', 'm34pconfirm=0'),
        ('new-fcv0', 'new', 'fcv=0'),
        ('base-default', 'base', ''),
        ('base-pconfirm1', 'base', 'm34pconfirm=1'),
    ]
    binaries = {'new': new_bin, 'base': base_bin}
    cells = {}
    for request in requests:
        for seed in seeds:
            for label, which, extra in ARMS:
                tag = f'{request}-s{seed}-{label}'
                row = run_cell(binaries[which], request, seed, units, extra,
                               outdir, tag)
                cells[(request, seed, label)] = row
                mspc = row['msPerAcceptedConfirmation']
                print(f'{tag}: depth={row["rawDepthMm"]} dg={row["digest"]} '
                      f'wall={row["processWallSeconds"]:.2f} '
                      f'msPerConfirm={mspc if mspc is None else round(mspc, 4)}'
                      f' n={row["confirmationsAccepted"]}', flush=True)

    CLAIMS = [
        ('1 default is pconfirm=1,fcv=1', 'new-default', 'new-explicit', True),
        ('2 default is the old shipping arm', 'new-default', 'base-pconfirm1',
         True),
        ('3 pconfirm=0 is document-equal (semantics-preserving)',
         'new-default', 'new-pconfirm0', True),
        ('4 pconfirm=0 reproduces the old default', 'new-pconfirm0',
         'base-default', True),
        ('5 fcv=0 is document-equal (the certificate is a proof)',
         'new-default', 'new-fcv0', True),
    ]
    claims = {}
    for name, left, right, expect_equal in CLAIMS:
        rows = []
        for request in requests:
            for seed in seeds:
                a = cells[(request, seed, left)]
                b = cells[(request, seed, right)]
                rows.append({'cell': f'{request}-s{seed}',
                             'leftDigest': a['digest'],
                             'rightDigest': b['digest'],
                             'equal': a['digest'] == b['digest']
                             and a['digest'] is not None,
                             'leftDepthMm': a['rawDepthMm'],
                             'rightDepthMm': b['rawDepthMm']})
        equal_count = sum(1 for r in rows if r['equal'])
        claims[name] = {
            'left': left, 'right': right, 'expectEqual': expect_equal,
            'equalCells': equal_count, 'cells': len(rows), 'rows': rows}
        if expect_equal is None:
            claims[name]['verdict'] = 'reported'
        elif expect_equal:
            claims[name]['verdict'] = ('PASS' if equal_count == len(rows)
                                       else 'FAIL')
        else:
            claims[name]['verdict'] = 'PASS' if equal_count == 0 else 'FAIL'
        print(f'{name}: {claims[name]["verdict"]} '
              f'({equal_count}/{len(rows)} equal)', flush=True)

    # The half of claims 3 and 5 that carries the weight: a key that changed
    # nothing would report the same milliseconds per accepted confirmation.
    microbench = {}
    for label in ('new-default', 'new-pconfirm0', 'new-fcv0', 'base-default'):
        rows = []
        for request in requests:
            for seed in seeds:
                row = cells[(request, seed, label)]
                rows.append({'cell': f'{request}-s{seed}',
                             'msPerAcceptedConfirmation':
                                 row['msPerAcceptedConfirmation'],
                             'confirmationsAccepted':
                                 row['confirmationsAccepted']})
        microbench[label] = rows
    live = {}
    for name, arm in (('m34pconfirm=0', 'new-pconfirm0'), ('fcv=0', 'new-fcv0')):
        ratios = []
        for request in requests:
            for seed in seeds:
                base_ms = cells[(request, seed, 'new-default')][
                    'msPerAcceptedConfirmation']
                arm_ms = cells[(request, seed, arm)][
                    'msPerAcceptedConfirmation']
                if base_ms and arm_ms:
                    ratios.append(arm_ms / base_ms)
        live[name] = {
            'msRatioVsDefault': ratios,
            # A live opt-out must make confirmations *slower*: both defaults
            # exist to make them faster.
            'liveOnEveryCell': bool(ratios) and all(r > 1.05 for r in ratios),
        }
        print(f'{name} is live (slower confirmations): '
              f'{live[name]["liveOnEveryCell"]} ratios='
              f'{[round(r, 3) for r in ratios]}', flush=True)

    # The negative: an unarmed binary must refuse both keys.
    refusals = {}
    for key in ('fcv=0', 'm34pconfirm=0'):
        spec = runlib.spec_for(seeds[0], 'work', units, True, key)
        argv = runlib.argv(unarmed_bin, requests[0], seeds[0], spec)
        proc = subprocess.run(argv, capture_output=True, check=False)
        stderr = (proc.stderr or b'').decode()
        refusals[key] = {
            'exitCode': proc.returncode,
            'refused': proc.returncode != 0
            and 'unknown portfolio spec key' in stderr,
            'stderrTail': stderr[-200:],
        }
        print(f'unarmed refuses {key}: {refusals[key]["refused"]} '
              f'(exit {proc.returncode})', flush=True)

    result = {
        'newBinary': new_bin, 'baseBinary': base_bin,
        'unarmedBinary': unarmed_bin,
        'requests': requests, 'seeds': seeds, 'workUnits': units,
        'claims': claims,
        'microbench': microbench,
        'optOutIsLive': live,
        'unarmedRefusals': refusals,
        'cells': {f'{k[0]}-s{k[1]}-{k[2]}': v for k, v in cells.items()},
    }
    result['ALL_PASS'] = (
        all(c['verdict'] in ('PASS', 'reported') for c in claims.values())
        and all(v['liveOnEveryCell'] for v in live.values())
        and all(r['refused'] for r in refusals.values()))
    json.dump(result, open(f'{outdir}/armgate.json', 'w'), indent=1)
    print(f'ALL_PASS={result["ALL_PASS"]}')
    return 0 if result['ALL_PASS'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
