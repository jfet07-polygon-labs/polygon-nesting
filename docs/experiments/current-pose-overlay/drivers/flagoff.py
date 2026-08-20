#!/usr/bin/env python3
"""The flag-off wall A/B Sol review 6 §2 asks for.

    python3 flagoff.py OUT.json ROUNDS LABEL=BINARY [LABEL=BINARY ...]

Sol review 6 §2, "Flag-off":

    Quello che i gate non provano è l'assenza di regressione prestazionale: ora
    quei siti hot chiamano un helper passando `GeneralRelaxedSettings`.
    Renderei il booleano lane-local/inlined e misurerei il flag-off con la
    feature compilata.

The gates prove the flag is *semantically* inert. This measures whether it is
free. Every binary here is built with `jagua-experimental,compression-schedule`
- the feature compiled in - and run with `POLYGON_NESTING_CURRENT_POSE_OVERLAY`
unset, so the only thing under test is what the key-derivation sites cost when
nobody has armed the overlay.

Two gate streams, both from `lib.GATES`, because they exercise different
engines: `g1` is mode 20 (the constructor/native seed path) and `g2` is mode 22
(the persistent-vacancy replay of the 159.092 record parent). `m20 g1` and
`m22 g2` are the two the task names.

Pairing: each round runs every arm on the same gate back to back, and the arm
order is reversed on odd rounds, so a monotone drift in machine load over the
round (the box is shared) cancels between arms rather than accruing to
whichever arm goes first. The reported statistic is the per-round *paired*
delta, never a difference of separately-pooled medians.

The clock is `medianElapsedMs` - the benchmark's own measurement around the
measured stream only, request loading and serialisation excluded - and the
process wall is carried alongside it as the box's own number.

Every round also re-checks the gate's pinned depth and fingerprint, so a run
that drifted semantically cannot be reported as a wall result.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

# The two gate streams, by tag, out of `lib.GATES`.
STREAMS = ('g1', 'g2')


def sign_test_p(wins, total):
    """Two-sided exact binomial p at p=0.5, without SciPy."""
    if total == 0:
        return None
    from math import comb
    tail = min(wins, total - wins)
    p = sum(comb(total, k) for k in range(0, tail + 1)) / 2 ** total * 2
    return min(1.0, p)


def main():
    out_path = sys.argv[1]
    rounds = int(sys.argv[2])
    arms = []
    for item in sys.argv[3:]:
        label, _, binary = item.partition('=')
        arms.append((label, binary))
    outdir = os.path.dirname(os.path.abspath(out_path)) + '/flagoff-runs'
    os.makedirs(outdir, exist_ok=True)

    gates = {g[0]: g for g in lib.GATES if g[0] in STREAMS}
    # `POLYGON_NESTING_CURRENT_POSE_OVERLAY` deliberately absent: this is the
    # flag-off measurement.
    env = {}
    samples = {tag: {label: [] for label, _ in arms} for tag in gates}
    walls = {tag: {label: [] for label, _ in arms} for tag in gates}
    misses = []

    for round_index in range(rounds):
        order = arms if round_index % 2 == 0 else list(reversed(arms))
        for tag, gate in gates.items():
            for label, binary in order:
                doc, wall, _ = lib.run_gate(
                    binary, gate, outdir, env=env,
                    label=f'{label}-r{round_index}-')
                check = lib.gate_check(gate, doc)
                if not check.get('hit'):
                    misses.append({'round': round_index, 'gate': tag,
                                   'arm': label, 'check': check})
                seconds = lib.engine_seconds(doc)
                samples[tag][label].append(seconds)
                walls[tag][label].append(wall)
                print(json.dumps({'round': round_index, 'gate': tag,
                                  'arm': label, 'engineSeconds': seconds,
                                  'processWallSeconds': wall,
                                  'gateHit': check.get('hit')}), flush=True)

    result = {
        'rounds': rounds,
        'arms': [{'label': label, 'binary': binary} for label, binary in arms],
        'note': 'flag compiled in, POLYGON_NESTING_CURRENT_POSE_OVERLAY unset',
        'gateMisses': misses,
        'perGate': {},
    }
    reference = arms[0][0]
    for tag in gates:
        entry = {'reference': reference, 'arms': {}}
        for label, _ in arms:
            values = [v for v in samples[tag][label] if v is not None]
            entry['arms'][label] = {
                'engineSecondsMedian':
                    statistics.median(values) if values else None,
                'engineSecondsMin': min(values) if values else None,
                'processWallSecondsMedian': statistics.median(walls[tag][label]),
            }
        for label, _ in arms:
            if label == reference:
                continue
            paired = [(b - a) for a, b in
                      zip(samples[tag][reference], samples[tag][label])
                      if a is not None and b is not None]
            wins = sum(1 for d in paired if d < 0)
            ties = sum(1 for d in paired if d == 0)
            entry['arms'][label].update({
                'pairedDeltaSecondsMedian':
                    statistics.median(paired) if paired else None,
                'pairedDeltaPercentMedian': (
                    statistics.median(
                        [(b - a) / a * 100.0 for a, b in
                         zip(samples[tag][reference], samples[tag][label])
                         if a])),
                'roundsFasterThanReference': wins,
                'roundsTied': ties,
                'roundsCompared': len(paired),
                'signTestP': sign_test_p(wins, len(paired) - ties),
            })
        result['perGate'][tag] = entry

    json.dump(result, open(out_path, 'w'), indent=1)
    print(json.dumps({k: v for k, v in result.items() if k != 'gateMisses'},
                     indent=1))
    print(f'gate misses: {len(misses)}')


if __name__ == '__main__':
    main()
