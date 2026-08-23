#!/usr/bin/env python3
"""**Recorded poses, pushed back through the two authorities.**

The rerun's 27 wall cells record **no poses** - `schedule_json` emits
`placementCount` and a fingerprint and nothing else - so none of the 1,701
publications can be re-validated from committed evidence. Two pose sets in this
campaign *are* recorded and *can* be:

  * **S0** - `docs/experiments/gate-a-sparrow-import/fixture/
    sparrow-10s-x86-poses.json`, the pinned Sparrow layout the regression floor
    asserts against (`cutclose-rerun/evidence/smoke.json`, `s0.pins`);
  * **arm B** - every `ctl-B-seed*.json` of the AB/BA control carries its final
    61 `placements` in exactly the `PoseFixture` schema the `s0` cell reads.

Each is imported by the shipped `s0` cell, which runs
`raw_depth_of` on the placements (an independent recomputation of
`raw_source_depth_mm`) and then, at proposal ordinal 0, the Exclusive
r = 2.500 mm grid scan and the untouched
`validate_placements_against_contract`. The verdicts come back in
`outcome.exactCheckpoints[0]`.

**And a sensitivity ladder**, because a gate that only ever says yes proves
nothing: the S0 layout is re-imported with one piece translated by delta along
the short axis, for a ladder of deltas, and the delta at which each authority
flips is measured. If both authorities pass every rung the path is a rubber
stamp on this fixture and the S0 pin means nothing.

    python3 rv_poses.py <binary> <out.json>
"""
import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ICS = os.path.abspath(os.path.join(HERE, '..', '..'))
ROOT = os.path.abspath(os.path.join(ICS, '..', '..', '..'))
REQUEST = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
SPARROW = (f'{ROOT}/docs/experiments/gate-a-sparrow-import/fixture/'
           'sparrow-10s-x86-poses.json')
RAW = os.environ.get('ICS_RAW', '/var/lib/t3/tmp/overlapics')
TMP = os.environ.get('RV_TMP', '/var/lib/t3/tmp/rv-audit-out')
LADDER_MM = [0.0, 0.001, 0.002, 0.004, 0.008, 0.016, 0.05, 0.1, 0.25, 0.5,
             1.0, 2.0, 5.0]


def import_poses(binary, poses_path, tag, target=None):
    """One `s0` process: import, measure, judge. No perturbation, no budget."""
    out = f'{TMP}/{tag}.json'
    os.makedirs(TMP, exist_ok=True)
    cmd = [binary, '--cell=s0', f'--request={REQUEST}', '--edge=5', '--pair=5',
           f'--poses={poses_path}', '--budget=0']
    if target is not None:
        cmd.append(f'--target={target}')
    with open(out, 'w') as handle:
        result = subprocess.run(cmd, stdout=handle, stderr=subprocess.PIPE,
                                check=False)
    if result.returncode != 0:
        return {'tag': tag, 'exit': result.returncode,
                'stderr': result.stderr.decode()[-400:]}
    with open(out) as handle:
        doc = json.load(handle)
    cps = doc['outcome']['exactCheckpoints']
    first = cps[0] if cps else {}
    return {
        'tag': tag,
        'exit': 0,
        'placementCount': doc['poses']['placementCount'],
        'posesSha256': doc['poses']['sha256'],
        'importedRawSourceDepthMm': doc['poses']['importedRawSourceDepthMm'],
        'entryRawPhi': doc['entry']['rawPhi'],
        'entryRawPhiBits': doc['entry']['rawPhiBits'],
        'entryMaxViolationMm': doc['entry']['maxViolationMm'],
        'entryRawSourceDepthMm': doc['entry']['rawSourceDepthMm'],
        'twoRMicron': doc['contract']['twoRMicron'],
        'pairClearanceMm': doc['contract']['pairClearanceMm'],
        'sheetEdgeClearanceMm': doc['contract']['sheetEdgeClearanceMm'],
        'kernelExclusiveValid': first.get('kernelExclusiveValid'),
        'contractValid': first.get('contractValid'),
        'refusal': first.get('refusal'),
        'publishedRawDepthMm': first.get('publishedRawDepthMm'),
        'proxyRawDepthMm': first.get('proxyRawDepthMm'),
        'repairRows': first.get('repairRows'),
        'repairMaxDisplacementMm': first.get('repairMaxDisplacementMm'),
        'repairDepthGivebackMm': first.get('repairDepthGivebackMm'),
        'invalidPublications': doc['outcome']['invalidPublications'],
    }


def write_fixture(placements, path):
    with open(path, 'w') as handle:
        json.dump({'placements': placements}, handle)
    return path


def main():
    binary = sys.argv[1]
    out_path = sys.argv[2] if len(sys.argv) > 2 else None
    rows = {}

    # --- S0, against the committed pins ------------------------------------
    s0 = import_poses(binary, SPARROW, 's0-pin', target=150.16547)
    with open(os.path.join(ICS, 'cutclose-rerun/evidence/smoke.json')) as h:
        pins = json.load(h)['s0']['pins']
    s0_check = {
        'committedPins': pins,
        'reproduced': {
            'placementCount': s0['placementCount'],
            'rawSourceDepthMm': s0['importedRawSourceDepthMm'],
            'phiBits': s0['entryRawPhiBits'],
            'twoRMicron': s0['twoRMicron'],
            'pairClearanceMm': s0['pairClearanceMm'],
            'kernelExclusiveValid': s0['kernelExclusiveValid'],
            'contractValid': s0['contractValid'],
            'repairRows': s0['repairRows'],
            'repairDepthGivebackMm': s0['repairDepthGivebackMm'],
        },
    }
    s0_check['ALL_PINS_REPRODUCE'] = all(
        s0_check['reproduced'][k] == v for k, v in pins.items())
    rows['S0'] = {'run': s0, 'check': s0_check}

    # --- arm B's recorded layouts ------------------------------------------
    arm_b = []
    for round_tag, folder in (('rerun', f'{RAW}/rerun/control'),
                              ('round1', f'{RAW}/round1/control')):
        for seed in range(9):
            src = f'{folder}/ctl-B-seed{seed}.json'
            if not os.path.exists(src):
                continue
            with open(src) as handle:
                doc = json.load(handle)
            placements = doc.get('placements') or []
            if not placements:
                arm_b.append({'round': round_tag, 'seed': seed,
                              'skipped': 'no placements'})
                continue
            fixture = write_fixture(
                placements, f'{TMP}/armB-{round_tag}-seed{seed}-poses.json')
            row = import_poses(binary, fixture, f'armB-{round_tag}-seed{seed}')
            row.update({
                'round': round_tag, 'seed': seed,
                'armBIncumbentRawDepthMm':
                    (doc.get('portfolio') or {}).get('incumbent', {})
                    .get('rawDepthMm'),
                'armBDualGateValid':
                    (doc.get('portfolio') or {}).get('incumbent', {})
                    .get('dualGateValid'),
                'armBUsedLongAxisDepthMm': doc.get('usedLongAxisDepthMm'),
                'armBIndependentDepthMm':
                    doc.get('independentUsedLongAxisDepthMm'),
                'armBExecutableSha256': doc.get('executableSha256'),
            })
            arm_b.append(row)
    rows['armB'] = arm_b

    # --- the sensitivity ladder --------------------------------------------
    with open(SPARROW) as handle:
        base = json.load(handle)['placements']
    ladder = []
    for delta in LADDER_MM:
        moved = json.loads(json.dumps(base))
        moved[0]['translateShortAxis'] = \
            moved[0]['translateShortAxis'] + delta
        fixture = write_fixture(
            moved, f'{TMP}/ladder-{str(delta).replace(".", "_")}-poses.json')
        row = import_poses(binary, fixture, f'ladder-{delta}')
        row['deltaMm'] = delta
        row['movedPieceId'] = moved[0]['pieceId']
        with open(f'{TMP}/ladder-{delta}.json') as handle:
            rung = json.load(handle)
        row['checkpointsEmitted'] = len(rung['outcome']['exactCheckpoints'])
        row['authoritiesConsulted'] = row['checkpointsEmitted'] > 0
        ladder.append(row)
    rows['sensitivityLadder'] = ladder
    kernel_flip = next((r['deltaMm'] for r in ladder
                        if r.get('kernelExclusiveValid') is False), None)
    contract_flip = next((r['deltaMm'] for r in ladder
                          if r.get('contractValid') is False), None)

    doc = {
        'what': 'recorded pose sets pushed back through the two authorities',
        'binary': binary,
        'binarySha256': subprocess.run(
            ['sha256sum', binary], capture_output=True,
            text=True).stdout.split()[0],
        'request': REQUEST,
        'S0': rows['S0'],
        'armB': rows['armB'],
        'sensitivityLadder': ladder,
        'kernelFirstRefusalDeltaMm': kernel_flip,
        'contractFirstRefusalDeltaMm': contract_flip,
        # **Read this with `evidence/authorities.json`.** The ladder cannot
        # reach an authority refusal, and that is itself the measurement: for
        # delta >= 0.016 mm the entry `maxViolationMm` exceeds the 4 um band, so
        # `separate` never calls `attempt_publication` and *no checkpoint is
        # emitted at all* - the `null` verdicts below are "not asked", not
        # "said yes". The authorities' own liveness is measured on the committed
        # round instead, where they refused 1,588 of 3,298 candidate layouts
        # (1,227 kernel, 361 contract).
        'LADDER_REACHED_AN_AUTHORITY_REFUSAL':
            kernel_flip is not None or contract_flip is not None,
        'ladderNote': 'null kernel/contract verdicts mean no checkpoint was '
                      'emitted: the 4 um proxy band refused before either '
                      'authority was consulted. See authorities.json for the '
                      'authorities refusing on the committed round.',
        'S0_PINS_REPRODUCE': s0_check['ALL_PINS_REPRODUCE'],
        # Nine S0 pins, three clauses per arm-B layout (kernel, contract, and
        # the imported depth against the recorded incumbent, bit for bit), and
        # one rung each on the ladder.
        'claimsChecked': len(pins) + 3 * len(arm_b) + len(ladder),
        'armBLayoutsDualValid': sum(
            1 for r in arm_b
            if r.get('kernelExclusiveValid') and r.get('contractValid')),
        'armBDepthsBitIdenticalToRecordedIncumbent': sum(
            1 for r in arm_b
            if r.get('importedRawSourceDepthMm')
            == r.get('armBIncumbentRawDepthMm')),
        'ARM_B_LAYOUTS_JUDGED': [
            {'round': r.get('round'), 'seed': r.get('seed'),
             'importedDepthMm': r.get('importedRawSourceDepthMm'),
             'armBIncumbentMm': r.get('armBIncumbentRawDepthMm'),
             'armBIndependentMm': r.get('armBIndependentDepthMm'),
             'kernelExclusiveValid': r.get('kernelExclusiveValid'),
             'contractValid': r.get('contractValid'),
             'refusal': (r.get('refusal') or '')[:90]}
            for r in arm_b],
    }
    if out_path:
        with open(out_path, 'w') as handle:
            handle.write(json.dumps(doc, indent=1, sort_keys=True) + '\n')
    print('S0 pins reproduce:', doc['S0_PINS_REPRODUCE'])
    print('  committed:', json.dumps(pins))
    print('  measured :', json.dumps(s0_check['reproduced']))
    print()
    print('arm B layouts, judged by the overlap-ics dual gate:')
    for r in doc['ARM_B_LAYOUTS_JUDGED']:
        print(f"  {r['round']:>7} seed{r['seed']}  imported="
              f"{r['importedDepthMm']}  armB.incumbent={r['armBIncumbentMm']}"
              f"  armB.independent={r['armBIndependentMm']}"
              f"  kernel={r['kernelExclusiveValid']}"
              f"  contract={r['contractValid']}  {r['refusal']}")
    print()
    print('sensitivity ladder (S0 with piece 0 shifted along the short axis):')
    for r in ladder:
        print(f"  delta={r['deltaMm']:>6} mm  kernel={r.get('kernelExclusiveValid')}"
              f"  contract={r.get('contractValid')}"
              f"  depth={r.get('importedRawSourceDepthMm')}"
              f"  {(r.get('refusal') or '')[:70]}")
    print('first kernel refusal at delta =', kernel_flip,
          ' first contract refusal at delta =', contract_flip)
    return 0 if doc['S0_PINS_REPRODUCE'] else 1


if __name__ == '__main__':
    sys.exit(main())
