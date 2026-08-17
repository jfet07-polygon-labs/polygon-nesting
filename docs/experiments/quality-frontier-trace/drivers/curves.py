#!/usr/bin/env python3
"""Turns the raw event streams into the curves, the milestones and the plot.

Reads every `*.jsonl` the frontier runs produced and writes, into the committed
experiment directory:

  * `curve-<tag>.json`  - the public-incumbent series and the exact-valid
                          candidate series for one run, with work ordinals;
  * `summary.json`      - milestones, marginal delta-mm/s, work attribution;
  * `frontier.png`      - the plot.

Depth convention: the incumbent series is quoted in the RAW source depth of the
candidate that produced it wherever the join by fingerprint succeeds, and in
the engine's canonical-grid `used_long_axis_depth_mm` otherwise. Both are
carried per point (`rawDepthMm`, `gridDepthMm`) so a reader never has to guess
which basis a number is in.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

RAW = '/var/lib/t3/tmp/qft/frontier'
OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
MILESTONES = (200.0, 190.0, 185.0, 182.0, 181.6, 180.0, 175.0, 170.0)
# Categorical slots 1 and 2 of the validated reference palette; two series per
# panel, which is inside the all-pairs cap the palette documents.
SERIES_COLOR = {0: '#2a78d6', 1: '#eb6834'}
TEXT_PRIMARY = '#0b0b0b'
TEXT_SECONDARY = '#52514e'
SURFACE = '#fcfcfb'
GRID = '#dcdbd6'


def load(tag):
    return lib.read_trace(f'{RAW}/{tag}.jsonl')


def curve(events):
    """The public-incumbent series, the candidate series and the scope ledger."""
    header = next(e for e in events if e['event'] == 'run')
    candidates = [e for e in events if e['event'] == 'exactCandidate']
    by_fingerprint = {}
    for event in candidates:
        by_fingerprint.setdefault(event['fingerprint'], event)

    incumbents = []
    best = None
    for event in events:
        if event['event'] != 'incumbent':
            continue
        grid = event['depthMm']
        source = by_fingerprint.get(event['fingerprint'])
        raw = source['rawDepthMm'] if source else None
        depth = raw if raw is not None else grid
        if best is not None and depth >= best:
            # The trace records every incumbent installation; a curve only
            # moves on a strict improvement.
            continue
        best = depth
        incumbents.append({
            't': event['t'],
            'rawDepthMm': raw,
            'gridDepthMm': grid,
            'depthMm': depth,
            'source': event['source'],
            'operator': event['operator'],
            'fingerprint': event['fingerprint'],
            'work': event['work'],
        })

    scopes = {}
    stack = []
    for event in events:
        if event['event'] == 'scopeEnter':
            stack.append(event)
        elif event['event'] == 'scopeExit' and stack:
            entered = stack.pop()
            row = scopes.setdefault(entered['operator'], {
                'operator': entered['operator'], 'entries': 0, 'seconds': 0.0,
                'candidateQueries': 0, 'effectiveMoves': 0,
                'exactPairTests': 0, 'collisionPolygonBuilds': 0,
                'publicationAttempts': 0, 'proxySurvivors': 0,
                'exactValidCandidates': 0,
            })
            row['entries'] += 1
            row['seconds'] += event['t'] - entered['t']
            for key in ('candidateQueries', 'effectiveMoves', 'exactPairTests',
                        'collisionPolygonBuilds', 'publicationAttempts',
                        'proxySurvivors'):
                row[key] += event['work'][key] - entered['work'][key]
    for event in candidates:
        row = scopes.get(event['operator'])
        if row:
            row['exactValidCandidates'] += 1

    end = next((e for e in events if e['event'] == 'end'), events[-1])
    return {
        'header': header,
        'incumbents': incumbents,
        'candidates': [{
            't': e['t'], 'rawDepthMm': e['rawDepthMm'],
            'operator': e['operator'], 'pieceCount': e['pieceCount'],
            'fingerprint': e['fingerprint'], 'thread': e['thread'],
            'work': e['work'],
        } for e in candidates],
        'modeResults': [e for e in events if e['event'] == 'modeResult'],
        'publications': [e for e in events if e['event'] == 'publication'],
        'scopes': sorted(scopes.values(), key=lambda row: -row['seconds']),
        'endSeconds': end['t'],
        'endWork': end['work'],
    }


def milestones(data, piece_count):
    complete = [c for c in data['candidates']
                if c['pieceCount'] == piece_count and c['rawDepthMm']]
    out = {
        'timeToFirstCompleteLayoutSeconds': min(
            (c['t'] for c in complete), default=None),
        'firstCompleteLayoutDepthMm': (
            min(complete, key=lambda c: c['t'])['rawDepthMm']
            if complete else None),
        'timeToFirstPublicIncumbentSeconds': (
            data['incumbents'][0]['t'] if data['incumbents'] else None),
        'depthMilestones': {},
    }
    for level in MILESTONES:
        hit = next((row for row in data['incumbents']
                    if row['depthMm'] <= level), None)
        out['depthMilestones'][f'{level:g}'] = None if hit is None else {
            'seconds': hit['t'], 'depthMm': hit['depthMm'],
            'candidateQueries': hit['work']['candidateQueries'],
            'effectiveMoves': hit['work']['effectiveMoves'],
            'source': hit['source'],
        }
    return out


def marginal(data):
    rows = data['incumbents']
    if len(rows) < 2:
        return {'segments': [], 'overallMmPerSecond': None}
    segments = []
    for previous, current in zip(rows, rows[1:]):
        span = current['t'] - previous['t']
        gain = previous['depthMm'] - current['depthMm']
        segments.append({
            'fromSeconds': previous['t'], 'toSeconds': current['t'],
            'deltaMm': gain,
            'mmPerSecond': gain / span if span > 0 else None,
            'deltaCandidateQueries': (current['work']['candidateQueries']
                                      - previous['work']['candidateQueries']),
            'mmPerMillionQueries': (
                gain * 1e6 / (current['work']['candidateQueries']
                              - previous['work']['candidateQueries'])
                if current['work']['candidateQueries']
                > previous['work']['candidateQueries'] else None),
            'source': current['source'],
        })
    total_gain = rows[0]['depthMm'] - rows[-1]['depthMm']
    return {
        'segments': segments,
        'firstIncumbentDepthMm': rows[0]['depthMm'],
        'finalIncumbentDepthMm': rows[-1]['depthMm'],
        'totalGainMm': total_gain,
        'gainWindowSeconds': rows[-1]['t'] - rows[0]['t'],
        'overallMmPerSecondOverWholeRun': total_gain / data['endSeconds'],
        'idleTailSeconds': data['endSeconds'] - rows[-1]['t'],
        'idleTailGainMm': 0.0,
    }


def plot(curves, path):
    """Two configurations x two vertical ranges.

    The top row carries every exact-valid candidate, which spans 179-258 mm and
    therefore flattens the staircase that matters; the bottom row is the same
    two series clipped to the incumbent band. Two ranges of one measure are
    shown as two panels rather than two y-scales on one - a dual axis would
    invite exactly the comparison it makes impossible.
    """
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt

    figure, axes = plt.subplots(2, 2, figsize=(12.5, 8.0), facecolor=SURFACE)
    panels = (
        ('m0coupled', 'Protected m0 + coupled separator', 0),
        ('mode20', 'Mode 20 basin construction, same process', 1),
    )
    for config, title, column in panels:
        for row in (0, 1):
            axis = axes[row][column]
            axis.set_facecolor(SURFACE)
            for spine in ('top', 'right'):
                axis.spines[spine].set_visible(False)
            for spine in ('left', 'bottom'):
                axis.spines[spine].set_color(GRID)
            axis.grid(True, color=GRID, linewidth=0.8, alpha=0.9)
            axis.set_axisbelow(True)
            axis.tick_params(colors=TEXT_SECONDARY, labelsize=9)
            if row == 0:
                axis.set_title(title, color=TEXT_PRIMARY, fontsize=11.5,
                               loc='left', pad=10)
            else:
                axis.set_xlabel('elapsed seconds (monotonic, from request)',
                                color=TEXT_SECONDARY, fontsize=9.5)
            for seed in (0, 1):
                data = curves.get(f'{config}-seed{seed}-clock')
                if not data:
                    continue
                colour = SERIES_COLOR[seed]
                complete = [c for c in data['candidates']
                            if c['pieceCount'] == data['header']['pieces']
                            and c['rawDepthMm']]
                axis.scatter([c['t'] for c in complete],
                             [c['rawDepthMm'] for c in complete],
                             s=34, facecolor='none', edgecolor=colour,
                             linewidth=1.4, zorder=3,
                             label=f'seed {seed}: exact-valid candidate')
                rows = data['incumbents']
                times = [r['t'] for r in rows] + [data['endSeconds']]
                depths = [r['depthMm'] for r in rows] + [rows[-1]['depthMm']]
                axis.step(times, depths, where='post', color=colour,
                          linewidth=2.0, zorder=4,
                          label=f'seed {seed}: public incumbent')
                if row == 1:
                    axis.annotate(
                        f"{rows[-1]['depthMm']:.3f} mm",
                        (data['endSeconds'], rows[-1]['depthMm']),
                        textcoords='offset points',
                        xytext=(-6, 8 if seed == 0 else -14),
                        ha='right', fontsize=9, color=TEXT_PRIMARY)
            if row == 1:
                axis.set_ylim(178.8, 184.2)
        axes[0][column].set_ylim(175, 265)
    axes[0][0].set_ylabel('raw source depth (mm), lower is better',
                          color=TEXT_SECONDARY, fontsize=9.5)
    axes[1][0].set_ylabel('same, clipped to the incumbent band',
                          color=TEXT_SECONDARY, fontsize=9.5)
    axes[0][1].legend(frameon=False, fontsize=8.5, loc='upper right',
                      labelcolor=TEXT_SECONDARY)
    figure.suptitle(
        'Quality frontier: depth versus time, one process, from request only',
        color=TEXT_PRIMARY, fontsize=13, x=0.008, ha='left', y=0.985)
    figure.text(
        0.008, 0.028,
        'Mixed-61 exact-clearance request, undistorted-clock runs (work '
        'ordinals off). Circles are every exact-valid candidate the search '
        'saw, not only the published ones; the staircase is the engine '
        'result.',
        color=TEXT_SECONDARY, fontsize=8.5, ha='left')
    figure.text(
        0.008, 0.008,
        'Mode 20 spends 25.4 s producing eight complete layouts, all deeper '
        'than the incumbent it started from, and the adoption rule refuses '
        'all eight - the engine result is the same on both panels.',
        color=TEXT_SECONDARY, fontsize=8.5, ha='left')
    figure.tight_layout(rect=(0, 0.048, 1, 0.955))
    figure.savefig(path, dpi=160, facecolor=SURFACE)


def main():
    os.makedirs(f'{OUT}/curves', exist_ok=True)
    manifest = json.load(open(f'{RAW}/manifest.json'))
    curves = {}
    summary = {
        'request': manifest['request'],
        'areaLowerBoundDepthMm': manifest['areaLowerBoundDepthMm'],
        'mode20ClampMm': manifest['mode20ClampMm'],
        'mode20ClampMultiple': manifest['mode20ClampMultiple'],
        'runs': {},
    }
    for row in manifest['runs']:
        tag = row['tag']
        data = curve(load(tag))
        curves[tag] = data
        piece_count = data['header']['pieces']
        json.dump(data, open(f'{OUT}/curves/curve-{tag}.json', 'w'), indent=1)
        summary['runs'][tag] = {
            'config': row['config'], 'seed': row['seed'],
            'variant': row['variant'],
            'workOrdinalsArmed': row['workOrdinalsArmed'],
            'wallSeconds': row['wallSeconds'],
            'traceEndSeconds': data['endSeconds'],
            'engineDepthMm': row['engineDepthMm'],
            'modeExactValid': row['modeExactValid'],
            'modeDepthMm': row['modeDepthMm'],
            'exactValidCandidates': len(data['candidates']),
            'completeExactValidCandidates': sum(
                1 for c in data['candidates']
                if c['pieceCount'] == piece_count),
            'milestones': milestones(data, piece_count),
            'marginal': marginal(data),
            'endWork': data['endWork'],
            'scopes': data['scopes'],
            'publications': [{
                't': p['t'], 'reason': p['reason'],
                'disposition': p['disposition'],
                'publishedDepthMm': p['publishedDepthMm'],
                'legacyDepthMm': p['legacyDepthMm'],
            } for p in data['publications']],
        }
    plot(curves, f'{OUT}/frontier.png')
    json.dump(summary, open(f'{OUT}/summary-measured.json', 'w'), indent=1)
    print(json.dumps({tag: {
        'first': row['milestones']['timeToFirstCompleteLayoutSeconds'],
        'final': row['marginal']['finalIncumbentDepthMm'],
        'idleTail': row['marginal']['idleTailSeconds'],
    } for tag, row in summary['runs'].items()}, indent=1))


if __name__ == '__main__':
    main()
