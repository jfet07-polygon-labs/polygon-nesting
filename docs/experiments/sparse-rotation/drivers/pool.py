#!/usr/bin/env python3
"""Pool two batteries of the same request and report **per seed**, not per cell.

    pool.py OUT BUDGET BATTERY [BATTERY ...]

Sol review 7 §3c and docs/experiments/rotation-tax/ §4.6 both say the same thing
about this campaign's headline statistic: *"three rounds of the same seed are not
three independent samples"*. Nine cells at a wall budget are three results
repeated, and a median over them reports a repetition count as if it were
evidence.

So this reducer collapses each seed's rounds to that seed's **median** paired
difference first, and reports the distribution over *seeds*. The per-round table
is kept beside it, because the within-seed spread is what says whether a wall
budget is reproducible at all on this box, but the count that carries a verdict
is the seed count.
"""
import json
import statistics
import sys


def main():
    out_path = sys.argv[1]
    budget = sys.argv[2]
    base_label, arm_label = f'baseat{budget}', f'sparseat{budget}'
    rows = []
    binaries = set()
    for path in sys.argv[3:]:
        battery = json.load(open(path))
        binaries.add(battery['binary'])
        rows.extend(battery['rows'])
    by_key = {(r['arm'], r['seed'], r['round']): r for r in rows}
    seeds = sorted({r['seed'] for r in rows})
    per_seed = {}
    per_round = []
    for seed in seeds:
        deltas = []
        for rnd in sorted({r['round'] for r in rows if r['seed'] == seed}):
            left = by_key.get((base_label, seed, rnd))
            right = by_key.get((arm_label, seed, rnd))
            if not left or not right:
                continue
            if left['engineDepthMm'] is None or right['engineDepthMm'] is None:
                continue
            delta = right['engineDepthMm'] - left['engineDepthMm']
            deltas.append(delta)
            per_round.append({'seed': seed, 'round': rnd,
                              'baseMm': left['engineDepthMm'],
                              'armMm': right['engineDepthMm'],
                              'deltaMm': delta})
        if deltas:
            base_depths = [by_key[(base_label, seed, r)]['engineDepthMm']
                           for r in sorted({x['round'] for x in rows
                                            if x['seed'] == seed})
                           if (base_label, seed, r) in by_key]
            arm_depths = [by_key[(arm_label, seed, r)]['engineDepthMm']
                          for r in sorted({x['round'] for x in rows
                                           if x['seed'] == seed})
                          if (arm_label, seed, r) in by_key]
            per_seed[str(seed)] = {
                'rounds': len(deltas),
                'medianDeltaMm': statistics.median(deltas),
                'minDeltaMm': min(deltas), 'maxDeltaMm': max(deltas),
                'baseMedianMm': statistics.median(base_depths),
                'armMedianMm': statistics.median(arm_depths),
                'baseWithinSeedSpreadMm': max(base_depths) - min(base_depths),
                'armWithinSeedSpreadMm': max(arm_depths) - min(arm_depths),
            }
    seed_medians = [v['medianDeltaMm'] for v in per_seed.values()]
    round_deltas = [r['deltaMm'] for r in per_round]
    report = {
        'budget': budget, 'binaries': sorted(binaries),
        'batteries': sys.argv[3:],
        'perSeed': per_seed,
        'perRound': per_round,
        'seedLevel': {
            'seeds': len(seed_medians),
            'medianOfSeedMediansMm': (statistics.median(seed_medians)
                                      if seed_medians else None),
            'meanOfSeedMediansMm': (statistics.fmean(seed_medians)
                                    if seed_medians else None),
            'seedsArmBetter': sum(1 for d in seed_medians if d < 0),
            'seedsBaseBetter': sum(1 for d in seed_medians if d > 0),
            'seedsEqual': sum(1 for d in seed_medians if d == 0),
            'minSeedMedianMm': min(seed_medians) if seed_medians else None,
            'maxSeedMedianMm': max(seed_medians) if seed_medians else None,
        },
        'roundLevel': {
            'cells': len(round_deltas),
            'medianDeltaMm': (statistics.median(round_deltas)
                              if round_deltas else None),
            'armBetter': sum(1 for d in round_deltas if d < 0),
            'baseBetter': sum(1 for d in round_deltas if d > 0),
        },
    }
    json.dump(report, open(out_path, 'w'), indent=1)
    print(f"=== {budget}s, {len(seed_medians)} seeds")
    for seed, cell in sorted(per_seed.items(), key=lambda kv: int(kv[0])):
        print(f"  seed {seed:>2}: base {cell['baseMedianMm']:.3f} "
              f"arm {cell['armMedianMm']:.3f} "
              f"delta {cell['medianDeltaMm']:+.3f} "
              f"[{cell['minDeltaMm']:+.3f}, {cell['maxDeltaMm']:+.3f}] "
              f"within-seed spread base {cell['baseWithinSeedSpreadMm']:.3f} "
              f"arm {cell['armWithinSeedSpreadMm']:.3f}")
    level = report['seedLevel']
    print(f"  SEED LEVEL: median of seed medians "
          f"{level['medianOfSeedMediansMm']:+.3f} mm, "
          f"arm better on {level['seedsArmBetter']}/{level['seeds']} seeds, "
          f"range [{level['minSeedMedianMm']:+.3f}, "
          f"{level['maxSeedMedianMm']:+.3f}]")
    print(f"  ROUND LEVEL: median {report['roundLevel']['medianDeltaMm']:+.3f}"
          f" mm over {report['roundLevel']['cells']} cells, arm better "
          f"{report['roundLevel']['armBetter']}")


if __name__ == '__main__':
    main()
