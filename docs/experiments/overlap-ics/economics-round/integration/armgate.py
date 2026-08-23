#!/usr/bin/env python3
"""**The control arm is the trajectory the member closed.** Cross-binary.

    python3 armgate.py <base-binary> [work-dir]

Wave 3 of the economics round replaces `Engine::separate`'s inline strike
ladder with `overlap_ics_meter::StrikeMeter`, adds a strike arm to
`ScheduleConfig` and a calibrated-work budget to `Budget`. The one thing an
integration wave is never allowed to do is change the answer on the arm that
was already closed, and the meter's own property vectors cannot prove that:
they prove the *function* is a faithful transcription, driven by a reference,
over synthetic sequences. Whether `Engine::separate` still *calls* it in the
same order with the same arguments is a question about two binaries.

So this is a measurement across binaries, exactly as `census/identity.py` is:

  A1  the pre-Wave-3 binary against this one, **left-subset**: every field the
      old document carried is present in the new one with a bit-identical
      value. New fields are allowed - `arm`, `armLabel`, `strikePolicy`,
      `strikeMeter`, `calibrated`, `strikeArm`, `calibratedPlan`,
      `calibratedAttemptsPerBite` - and nothing else may differ.
  A2  two processes of the new binary on the control arm, bit-identical after
      stripping `wall`. The shipped determinism claim, re-run.
  A3  the treatment arm runs, is labelled as the treatment, and carries the
      frozen KNOB quanta in its own document. It is NOT required to differ
      from the control on these cells: on a fixed-work cell with a small
      iteration cap neither arm's patience is ever spent, and a vector that
      demanded a difference would be demanding one the spec does not promise.
      What is required is that the *policy* the document reports is the
      work-denominated one.

`<base-binary>` is a copy of the example built at Wave 3's base commit
(`883b297`, the merge of waves 1 and 2b). It has to be passed in rather than
rebuilt here: a script that builds its own "before" can only ever compare a
tree to itself.

Exit 0 iff every comparison holds.
"""
import json
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                    '..', '..', '..', '..', '..'))
REQUEST = (f'{ROOT}/tests/fixtures/mixed-61/'
           'mixed61-request-exact-clearance.json')
DEFAULT_BIN = f'{ROOT}/target/release/examples/overlap_ics_benchmark'

# Four fixed-work cells: no clock is read inside any trajectory, so none of
# these comparisons is load-dependent. The shapes are the census's own, so a
# reader can line the two documents up cell by cell.
#
#   A  8 explore bites,  8 workers, seed 0  - the FAST K=8 shape
#   B  21 explore bites, 8 workers, seed 0  - the 179 shelf's parent
#   C  21 explore bites, 8 workers, seed 5  - the strike-starved watch seed
#   D  3 bites + 1 compress, 8 workers, seed 0, fingerprints on - the arm's
#      per-iteration record, and the only cell that exercises compress
CELLS = {
    'A': ['--mode=fixed', '--bites=8', '--attempts=1', '--iters=400',
          '--compressbites=0', '--workers=8', '--seed=0'],
    'B': ['--mode=fixed', '--bites=21', '--attempts=1', '--iters=400',
          '--compressbites=0', '--workers=8', '--seed=0'],
    'C': ['--mode=fixed', '--bites=21', '--attempts=1', '--iters=400',
          '--compressbites=0', '--workers=5', '--seed=5'],
    'D': ['--mode=fixed', '--bites=3', '--attempts=2', '--iters=120',
          '--compressbites=2', '--workers=8', '--seed=0',
          '--fingerprints=1'],
}

# Keys whose *value* is allowed to differ between the two binaries. `wall` is
# the clock object every determinism comparison in this campaign has always
# stripped; the two build keys identify the binary and are the point of the
# comparison rather than a casualty of it.
BUILD_KEYS = {'wall', 'executableSha256', 'buildFeatures'}


def run(binary, argv, out_path):
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    command = ([binary, '--cell=cutclose', f'--request={REQUEST}',
                '--edge=5', '--pair=5'] + argv)
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False)
    try:
        with open(out_path) as handle:
            document = json.load(handle)
    except (json.JSONDecodeError, OSError):
        document = None
    return document, result.returncode, (result.stderr or b'').decode()[-800:]


def left_subset(old, new, path=''):
    """Every leaf the OLD document carried, present and bit-identical in NEW.

    New keys are allowed anywhere; a changed value is not, and neither is a
    dropped one. Floats are compared through `repr`, so a value that moved by
    one ulp is a difference rather than a rounding.
    """
    problems = []
    if isinstance(old, dict):
        if not isinstance(new, dict):
            return [f'{path}: object became {type(new).__name__}']
        for key, value in old.items():
            here = f'{path}.{key}' if path else key
            if key in BUILD_KEYS:
                continue
            if key not in new:
                problems.append(f'{here}: dropped')
                continue
            problems.extend(left_subset(value, new[key], here))
        return problems
    if isinstance(old, list):
        if not isinstance(new, list):
            return [f'{path}: array became {type(new).__name__}']
        if len(old) != len(new):
            return [f'{path}: {len(old)} entries became {len(new)}']
        for index, value in enumerate(old):
            problems.extend(left_subset(value, new[index], f'{path}[{index}]'))
        return problems
    if isinstance(old, float) or isinstance(new, float):
        if repr(old) != repr(new):
            problems.append(f'{path}: {old!r} -> {new!r}')
        return problems
    if old != new:
        problems.append(f'{path}: {old!r} -> {new!r}')
    return problems


def stripped(document):
    return {key: value for key, value in document.items()
            if key not in ('wall',)}


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    base_bin = os.path.abspath(sys.argv[1])
    out = sys.argv[2] if len(sys.argv) > 2 else '/var/lib/t3/tmp/overlapics/armgate'
    new_bin = os.environ.get('ICS_BIN', DEFAULT_BIN)
    os.makedirs(out, exist_ok=True)

    rows = []
    for name, argv in sorted(CELLS.items()):
        old_doc, old_exit, old_err = run(base_bin, argv, f'{out}/{name}-base.json')
        new_doc, new_exit, new_err = run(new_bin, argv, f'{out}/{name}-new.json')
        # The control arm is the default: the invocation above does not name
        # `--arm` at all, which is the property that matters. A driver that
        # had to ask for the control would mean every committed cell had
        # silently changed arm.
        new_named, named_exit, named_err = run(
            new_bin, argv + ['--arm=control'], f'{out}/{name}-new-named.json')
        problems = (left_subset(old_doc, new_doc)
                    if old_doc is not None and new_doc is not None
                    else ['a document failed to parse'])
        row = {
            'cell': name,
            'argv': argv,
            'baseExit': old_exit,
            'newExit': new_exit,
            'namedArmExit': named_exit,
            'stderr': (old_err or new_err or named_err),
            'fieldsCompared': None,
            'differences': problems[:20],
            'differenceCount': len(problems),
            'defaultArmIsControl': (
                new_doc is not None
                and new_doc.get('schedule', {}).get('armLabel')
                == 'control-iteration-strikes'),
            'namingTheControlChangesNothing': (
                new_doc is not None and new_named is not None
                and stripped(new_doc) == stripped(new_named)),
            'frozenLiteralsIntact': (
                new_doc is not None
                and new_doc.get('schedule', {})
                .get('strikePolicy', {}).get('frozenLiteralsIntact') is True),
        }
        row['pass'] = bool(old_exit == 0 and new_exit == 0 and named_exit == 0
                           and not problems and row['defaultArmIsControl']
                           and row['namingTheControlChangesNothing']
                           and row['frozenLiteralsIntact'])
        rows.append(row)

    # A2: two processes of the new binary, same cell, stripped-identical.
    two_process = []
    for name in ('A', 'D'):
        first, exit_a, _ = run(new_bin, CELLS[name], f'{out}/{name}-p1.json')
        second, exit_b, _ = run(new_bin, CELLS[name], f'{out}/{name}-p2.json')
        two_process.append({
            'cell': name,
            'exitA': exit_a,
            'exitB': exit_b,
            'bitIdentical': (exit_a == 0 and exit_b == 0
                             and first is not None and second is not None
                             and stripped(first) == stripped(second)),
        })
    two_process_pass = all(row['bitIdentical'] for row in two_process)

    # A3: the treatment arm is reachable, labelled, and carries the KNOB.
    treatment_doc, treatment_exit, treatment_err = run(
        new_bin, CELLS['D'] + ['--arm=treatment'], f'{out}/D-treatment.json')
    policy = ((treatment_doc or {}).get('schedule', {})
              .get('strikePolicy', {}))
    treatment = {
        'exit': treatment_exit,
        'stderr': treatment_err,
        'armLabel': (treatment_doc or {}).get('schedule', {}).get('armLabel'),
        'explorePatience': policy.get('explore', {}).get('patience'),
        'exploreQuantum': policy.get('explore', {})
        .get('workQuantumSampleEvaluations'),
        'compressQuantum': policy.get('compress', {})
        .get('workQuantumSampleEvaluations'),
        'exploreIterationsWithoutImprovement': policy.get('explore', {})
        .get('iterationsWithoutImprovement'),
        'exploreStrikes': policy.get('explore', {}).get('strikes'),
        'compressStrikes': policy.get('compress', {}).get('strikes'),
    }
    treatment['pass'] = bool(
        treatment_exit == 0
        and treatment['armLabel'] == 'treatment-work-strikes'
        and treatment['explorePatience'] == 'work'
        and treatment['exploreQuantum'] == 1_630_000
        and treatment['compressQuantum'] == 815_000
        # The iteration patience does not exist on this arm, and the document
        # must say so rather than reporting the control's literal.
        and treatment['exploreIterationsWithoutImprovement'] is None
        and treatment['exploreStrikes'] == 3
        and treatment['compressStrikes'] == 5)

    failures = ([row['cell'] for row in rows if not row['pass']]
                + ([] if two_process_pass else ['twoProcess'])
                + ([] if treatment['pass'] else ['treatment']))
    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-integration-armgate',
        'baseBinary': base_bin,
        'newBinary': new_bin,
        'request': REQUEST,
        'crossBinary': rows,
        'twoProcess': two_process,
        'treatmentArm': treatment,
        'failures': failures,
        'CONTROL_ARM_IS_THE_CLOSED_MEMBER': not failures,
    }
    print(json.dumps(document, indent=1))
    with open(f'{out}/armgate.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    return 0 if not failures else 1


if __name__ == '__main__':
    sys.exit(main())
