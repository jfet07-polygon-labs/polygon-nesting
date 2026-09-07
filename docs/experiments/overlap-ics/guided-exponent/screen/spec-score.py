#!/usr/bin/env python3
"""spec-score.py: scorer for the ICS guided-exponent development screen and for the prospective
specification ICS-guided-exponent-v1 (polygon-nesting overlap-ICS engine). Standard library only.

    spec-score.py <dir> <prefix> --control A --treatment B [--treatment2 C]
        [--seeds 18..26] [--profiles legacy,wall10s]
        [--knobs A='proxymargin=8' B='proxymargin=8,guidedexponent=1' C='proxymargin=8,guidedexponent=0.75']
        [--select] [--originals <dir>] [--frozen <dir> <label>] [--json <out.json>]

Cells are JSON documents <dir>/<prefix>-<arm>-<profile>-r<rep>-s<seed>.json; a .json.gz twin is read
when the .json is absent (both present is a duplicated repetition). Frozen cells are
<dir>/<label>-<profile>-r<rep>-s<seed>.json. `--knobs` declares each arm's (or the frozen label's)
expected proxyMarginUm / guidedExponent; an arm not named is expected at the engine defaults
(margin 0, exponent 2). `--originals <dir> [<dir> ...]` names the as-run originals of re-run cells
(for example screen/exp1-perturbed-originals and screen/exp1-C-contaminated-originals): the as-run
(primary) archive takes a cell from there when a file of the same name exists (in exactly one of
them), else from <dir>; the replacement archive is <dir> alone. Both archives are scored and their
verdicts compared.

Accounting (GPT-6 Astra, review 6, Q10/Q11), per cell
    total evaluations         = outcome.work.sampleEvaluations
    explore evaluations       = sum of bites[].strikeMeter.chargedWorkSampleEvaluations over bites with
                                phase == 'explore'; failed and deadline-truncated bites are charged
    explore bites started     = number of explore bites (opportunities)
    published explore bites   = explore bites with published == true
    master iterations         = sum of bites[].masterIterations over all bites
    give-ups                  = exactCheckpoints[] whose refusal string contains 'band'
  and per arm (27 cells), aggregated as sums before dividing:
    explore evaluations per published explore bite   = sum explore evaluations / sum published
    explore elapsed time per published explore bite  = sum wall.loopExploreSeconds / sum published
    publication fraction                             = sum published / sum started
    total evaluations per cell, published explore bites per cell, master iterations per cell = means
  The per-bite charges must sum exactly to outcome.work.sampleEvaluations in every cell.

Conditions, per profile, control X against treatment Y, with D[arm][seed] the median over r0..r2 of
outcome.depthMm and g_s = D[X][s] - D[Y][s] (positive = treatment deeper):
    1 depth         median_s g_s > 0
    2 tail          min_s g_s >= -1.000 mm (unrounded)
    3 engineering   Y lower: total evaluations per cell, explore evaluations per published explore
                    bite, explore elapsed time per published explore bite; Y not lower: published
                    explore bites per cell, publication fraction
    4 integrity     zero invalid publications in every cell of both arms; every identity check passed
  Verdict PROMOTE only if all four pass on both profiles; otherwise the failures are listed.

Selection (--select, development only, control A, treatments B = p 1 and C = p 0.75): a treatment is
eligible if the four conditions pass against A on both profiles. Exactly one eligible -> select it;
none -> 'neither'; both -> H_s = D[B][s] - D[C][s]; select C only if Legacy median H > 1.000, Wall10s
median H >= 0 and no seed on either profile has H < -1.000; otherwise B. With --originals the
selection is computed on both archives; if they disagree, B is retained if eligible under both, else
'neither'.

Frozen record (--frozen <dir> <label>, p = 2, margin 0, another executable): request sha and contract
must equal the run's; its executable sha is printed. The treatment is scored against it with the depth
and tail conditions (the separate claim 'the package improves the frozen record'), with the Sparrow
statement: median of seed medians, its residual gap to 150.165 mm, the frozen gap, the reduction, and
the count of treatment seed medians at or below 150.165 mm.

Refusals (exit 2, naming the file and the field; nothing is skipped or defaulted): a tripwire key
(startedFrom, biteMicroscope, replay) at top level; an unreadable or non-JSON file; a missing, extra,
or duplicated repetition; absent or non-integer outcome.invalidPublications; non-finite depth;
scheduleProfile or seed disagreeing with the file name; knobs disagreeing with the declaration;
request sha, contract or executable differing across cells; wallIterationCap inconsistent within a
profile; per-bite charges not summing to the cell's work. Exit 0 on a complete, verified scoring,
whatever the verdict.
"""
import argparse
import gzip
import hashlib
import json
import math
import os
import re
import statistics
import sys

SPARROW_MM = 150.165
TRIPWIRES = ('startedFrom', 'biteMicroscope', 'replay')
REPS = (0, 1, 2)
TAIL_MM = 1.000
SELECT_HURDLE_MM = 1.000
WIN_MM = 0.001
DEFAULT_KNOBS = {'proxymargin': 0.0, 'guidedexponent': 2.0}
KNOB_FIELD = {'proxymargin': 'proxyMarginUm', 'guidedexponent': 'guidedExponent'}
KNOB_DEFAULT_IN_DOCUMENT = {'proxyMarginUm': 0, 'guidedExponent': 2.0}


class Refusal(Exception):
    """Anything that makes the run unscoreable. main() prints it and exits 2."""


def refuse(msg, path=None):
    raise Refusal(f'{path}: {msg}' if path else msg)


# ----------------------------------------------------------------------------- field checks

def is_int(v):
    return isinstance(v, int) and not isinstance(v, bool)


def is_num(v):
    return isinstance(v, (int, float)) and not isinstance(v, bool)


def is_finite(v):
    return is_num(v) and math.isfinite(v)


def is_text(v):
    return isinstance(v, str) and bool(v)


def field(container, key, path, where, check, what):
    """Required field with a type/value check; refuses naming the file and the field."""
    if not isinstance(container, dict) or key not in container:
        refuse(f'{where}.{key} is absent', path)
    value = container[key]
    if not check(value):
        refuse(f'{where}.{key} = {value!r} is not {what}', path)
    return value


# ----------------------------------------------------------------------------- documents

def read_document(path):
    """Bytes -> (dict, sha256 of the decoded bytes). Unreadable or non-JSON refuses; never skips."""
    try:
        if path.endswith('.gz'):
            with gzip.open(path, 'rb') as fh:
                raw = fh.read()
        else:
            with open(path, 'rb') as fh:
                raw = fh.read()
    except (OSError, EOFError) as e:
        refuse(f'unreadable ({e})', path)
    try:
        doc = json.loads(raw)
    except (ValueError, UnicodeDecodeError) as e:
        refuse(f'not JSON ({e})', path)
    if not isinstance(doc, dict):
        refuse('top level is not a JSON object', path)
    return doc, hashlib.sha256(raw).hexdigest()


def parse_cell(doc, path, *, arm, profile, seed, rep, knobs):
    """Verify one document against its file name and the arm's declared knobs; extract the metrics."""
    for key in TRIPWIRES:
        if key in doc:
            refuse(f'tripwire: top-level key {key!r} present (a diagnostic document, not a scored cell)', path)
    exe = field(doc, 'executableSha256', path, 'top', is_text, 'a non-empty string')
    request = field(doc, 'request', path, 'top', lambda v: isinstance(v, dict), 'an object')
    req_sha = field(request, 'sha256', path, 'request', is_text, 'a non-empty string')
    req_path = field(request, 'path', path, 'request', lambda v: isinstance(v, str), 'a string')
    contract = field(doc, 'contract', path, 'top', lambda v: isinstance(v, dict) and bool(v), 'a non-empty object')
    doc_profile = field(doc, 'scheduleProfile', path, 'top', is_text, 'a non-empty string')
    if doc_profile != profile:
        refuse(f'scheduleProfile = {doc_profile!r} but the file name says {profile!r}', path)
    doc_seed = field(doc, 'seed', path, 'top', is_int, 'an integer')
    if doc_seed != seed:
        refuse(f'seed = {doc_seed} but the file name says {seed}', path)
    knob_values = {}
    for knob, key in KNOB_FIELD.items():
        value = doc.get(key, KNOB_DEFAULT_IN_DOCUMENT[key])
        if not is_finite(value):
            refuse(f'top.{key} = {value!r} is not a finite number', path)
        if float(value) != float(knobs[knob]):
            shown = value if key in doc else f'absent (= {value})'
            refuse(f'top.{key} is {shown} but arm {arm} is declared {knob}={knobs[knob]:g}', path)
        knob_values[key] = float(value)
    cap = field(doc, 'wallIterationCap', path, 'top', is_int, 'an integer')
    wall = field(doc, 'wall', path, 'top', lambda v: isinstance(v, dict), 'an object')
    explore_s = field(wall, 'loopExploreSeconds', path, 'wall', lambda v: is_finite(v) and v >= 0, 'a finite non-negative number')
    total_s = field(wall, 'totalSeconds', path, 'wall', lambda v: is_finite(v) and v >= 0, 'a finite non-negative number')
    outcome = field(doc, 'outcome', path, 'top', lambda v: isinstance(v, dict), 'an object')
    depth = field(outcome, 'depthMm', path, 'outcome', is_finite, 'a finite number')
    invalid = field(outcome, 'invalidPublications', path, 'outcome', lambda v: is_int(v) and v >= 0, 'a non-negative integer (required, never defaulted)')
    work = field(outcome, 'work', path, 'outcome', lambda v: isinstance(v, dict), 'an object')
    total_ev = field(work, 'sampleEvaluations', path, 'outcome.work', lambda v: is_int(v) and v >= 0, 'a non-negative integer')
    bites = field(outcome, 'bites', path, 'outcome', lambda v: isinstance(v, list), 'a list')
    publications = field(outcome, 'publicationCount', path, 'outcome', lambda v: is_int(v) and v >= 0, 'a non-negative integer')
    checkpoints = field(outcome, 'exactCheckpoints', path, 'outcome', lambda v: isinstance(v, list), 'a list')

    charged_all = explore_ev = explore_started = explore_published = compress_bites = iterations = 0
    for i, bite in enumerate(bites):
        where = f'outcome.bites[{i}]'
        if not isinstance(bite, dict):
            refuse(f'{where} is not an object', path)
        phase = field(bite, 'phase', path, where, lambda v: v in ('compress', 'explore'), "'compress' or 'explore'")
        published = field(bite, 'published', path, where, lambda v: isinstance(v, bool), 'a boolean')
        iterations += field(bite, 'masterIterations', path, where, lambda v: is_int(v) and v >= 0, 'a non-negative integer')
        meter = field(bite, 'strikeMeter', path, where, lambda v: isinstance(v, dict), 'an object')
        charged = field(meter, 'chargedWorkSampleEvaluations', path, where + '.strikeMeter', lambda v: is_int(v) and v >= 0, 'a non-negative integer')
        charged_all += charged
        if phase == 'explore':
            explore_ev += charged
            explore_started += 1
            explore_published += int(published)
        else:
            compress_bites += 1
    if charged_all != total_ev:
        refuse(f'sum of bites[].strikeMeter.chargedWorkSampleEvaluations = {charged_all} differs from '
               f'outcome.work.sampleEvaluations = {total_ev}', path)

    refusals = giveups = 0
    for i, cp in enumerate(checkpoints):
        where = f'outcome.exactCheckpoints[{i}]'
        if not isinstance(cp, dict) or 'refusal' not in cp:
            refuse(f'{where}.refusal is absent', path)
        reason = cp['refusal']
        if reason is None:
            continue
        if not isinstance(reason, str):
            refuse(f'{where}.refusal = {reason!r} is neither null nor a string', path)
        refusals += 1
        giveups += int('band' in reason)

    return {
        'path': path, 'arm': arm, 'profile': profile, 'seed': seed, 'rep': rep,
        'executableSha256': exe, 'requestSha256': req_sha, 'requestPath': req_path,
        'contract': contract, 'contractKey': json.dumps(contract, sort_keys=True),
        'proxyMarginUm': knob_values['proxyMarginUm'], 'guidedExponent': knob_values['guidedExponent'],
        'wallIterationCap': cap, 'exploreSeconds': explore_s, 'totalSeconds': total_s,
        'depthMm': depth, 'invalidPublications': invalid, 'totalEvaluations': total_ev,
        'exploreEvaluations': explore_ev, 'exploreBitesStarted': explore_started,
        'exploreBitesPublished': explore_published, 'compressBites': compress_bites,
        'masterIterations': iterations, 'publicationCount': publications,
        'exactCheckpoints': len(checkpoints), 'checkpointRefusals': refusals, 'giveUps': giveups,
    }


# ----------------------------------------------------------------------------- archives

def find_cell(dirs, stem, rep, seed):
    """<stem>-r<rep>-s<seed>.json[.gz] from the first directory holding it; dirs are in priority order,
    the last being the main archive. A cell present in two originals directories is ambiguous."""
    name = f'{stem}-r{rep}-s{seed}'
    hits = []
    for d in dirs:
        plain = os.path.join(d, name + '.json')
        gz = plain + '.gz'
        has_plain, has_gz = os.path.isfile(plain), os.path.isfile(gz)
        if has_plain and has_gz:
            refuse('both .json and .json.gz exist: duplicated repetition', plain)
        if has_plain or has_gz:
            hits.append((plain if has_plain else gz, d))
    originals_hits = [h for h in hits if h[1] != dirs[-1]]
    if len(originals_hits) > 1:
        refuse(f'{name}: present in more than one --originals directory: ' + ', '.join(h[0] for h in originals_hits))
    return hits[0] if hits else (None, None)


def extra_repetitions(dirs, stem, seeds):
    pattern = re.compile(rf'^{re.escape(stem)}-r(\d+)-s(\d+)\.json(?:\.gz)?$')
    extra = []
    for d in dirs:
        if not os.path.isdir(d):
            refuse(f'{d} is not a directory')
        for name in sorted(os.listdir(d)):
            m = pattern.match(name)
            if m and int(m[2]) in seeds and int(m[1]) not in REPS:
                extra.append(os.path.join(d, name))
    return extra


def load_arm(dirs, stem_of, arm, profiles, seeds, knobs, registry):
    """{profile: {seed: {rep: cell}}} with exactly r0, r1, r2 per seed, each a distinct document."""
    arm_cells = {}
    for profile in profiles:
        stem = stem_of(profile)
        extra = extra_repetitions(dirs, stem, seeds)
        if extra:
            refuse(f'arm {arm} {profile}: more than three repetitions: {", ".join(extra)}')
        per_seed = {}
        for seed in seeds:
            per_seed[seed] = {}
            for rep in REPS:
                path, source = find_cell(dirs, stem, rep, seed)
                if path is None:
                    refuse(f'arm {arm} {profile} seed {seed}: missing repetition r{rep} '
                           f'({stem}-r{rep}-s{seed}.json not in {", ".join(dirs)})')
                doc, digest = read_document(path)
                if digest in registry:
                    refuse(f'byte-identical to {registry[digest]}: duplicated repetition', path)
                registry[digest] = path
                cell = parse_cell(doc, path, arm=arm, profile=profile, seed=seed, rep=rep, knobs=knobs)
                cell['sourceDir'] = source
                per_seed[seed][rep] = cell
        arm_cells[profile] = per_seed
    return arm_cells


def flat(per_seed):
    return [reps[r] for seed in sorted(per_seed) for r, reps in [(r, per_seed[seed]) for r in REPS]]


def all_cells(arms):
    return [c for arm in arms.values() for per_seed in arm.values() for c in flat(per_seed)]


def verify_identity(cells, profiles, what):
    """Same request, contract and executable across all cells; wallIterationCap consistent per profile."""
    def uniform(key, label, group=None):
        seen = {}
        for c in cells:
            if group is not None and c['profile'] != group:
                continue
            seen.setdefault(c[key], c['path'])
        if len(seen) > 1:
            listing = '; '.join(f'{p} has {v}' for v, p in seen.items())
            refuse(f'{what}: {label} differs across cells: {listing}')
        return next(iter(seen))
    return {
        'executableSha256': uniform('executableSha256', 'executableSha256'),
        'requestSha256': uniform('requestSha256', 'request.sha256'),
        'requestPath': sorted({c['requestPath'] for c in cells}),
        'contract': json.loads(uniform('contractKey', 'contract')),
        'wallIterationCap': {p: uniform('wallIterationCap', f'wallIterationCap of profile {p}', group=p) for p in profiles},
    }


# ----------------------------------------------------------------------------- statistics

def seed_medians(per_seed):
    return {seed: statistics.median(c['depthMm'] for c in reps.values()) for seed, reps in per_seed.items()}


def depth_summary(medians):
    values = list(medians.values())
    center = statistics.median(values)
    return {'medianOfSeedMedians': center, 'meanOfSeedMedians': statistics.mean(values),
            'gapToSparrowMm': center - SPARROW_MM,
            'seedMediansAtOrBelowSparrow': sum(1 for v in values if v <= SPARROW_MM), 'seeds': len(values)}


def aggregate(cells):
    n = len(cells)
    total = sum(c['totalEvaluations'] for c in cells)
    ex_ev = sum(c['exploreEvaluations'] for c in cells)
    started = sum(c['exploreBitesStarted'] for c in cells)
    published = sum(c['exploreBitesPublished'] for c in cells)
    ex_s = sum(c['exploreSeconds'] for c in cells)
    return {
        'cells': n,
        'totalEvaluationsPerCell': total / n,
        'exploreEvaluations': ex_ev,
        'exploreBitesStarted': started,
        'exploreBitesPublished': published,
        'exploreEvaluationsPerPublishedExploreBite': ex_ev / published if published else None,
        'exploreSecondsPerPublishedExploreBite': ex_s / published if published else None,
        'publishedExploreBitesPerCell': published / n,
        'exploreBitesStartedPerCell': started / n,
        'publicationFraction': published / started if started else None,
        'masterIterationsPerCell': sum(c['masterIterations'] for c in cells) / n,
        'giveUps': sum(c['giveUps'] for c in cells),
        'checkpointRefusals': sum(c['checkpointRefusals'] for c in cells),
        'invalidPublications': sum(c['invalidPublications'] for c in cells),
        'cellsWithInvalidPublications': sum(1 for c in cells if c['invalidPublications'] > 0),
        'publicationsPerCell': sum(c['publicationCount'] for c in cells) / n,
        'exploreSecondsPerCell': ex_s / n,
        'totalSecondsPerCell': sum(c['totalSeconds'] for c in cells) / n,
    }


def lower(treatment, control):
    """Strictly lower; an undefined (None) value counts as infinite."""
    if treatment is None:
        return False
    return control is None or treatment < control


def not_lower(treatment, control):
    if treatment is None:
        return control is None
    return control is None or treatment >= control


def compare(control, treatment, seeds):
    """Conditions 1-4 for one profile. control/treatment are {seed: {rep: cell}}."""
    Dx, Dy = seed_medians(control), seed_medians(treatment)
    g = {s: Dx[s] - Dy[s] for s in seeds}
    gains = [g[s] for s in seeds]
    ax, ay = aggregate(flat(control)), aggregate(flat(treatment))
    engineering = [
        ('total evaluations per cell', 'totalEvaluationsPerCell', 'lower'),
        ('explore evaluations per published explore bite', 'exploreEvaluationsPerPublishedExploreBite', 'lower'),
        ('explore elapsed time per published explore bite', 'exploreSecondsPerPublishedExploreBite', 'lower'),
        ('published explore bites per cell', 'publishedExploreBitesPerCell', 'not lower'),
        ('publication fraction', 'publicationFraction', 'not lower'),
    ]
    eng = []
    for label, key, relation in engineering:
        ok = (lower if relation == 'lower' else not_lower)(ay[key], ax[key])
        eng.append({'measure': label, 'key': key, 'requirement': relation, 'control': ax[key], 'treatment': ay[key], 'pass': ok})
    conditions = {
        'depth': {'medianGain': statistics.median(gains), 'pass': statistics.median(gains) > 0},
        'tail': {'minGain': min(gains), 'bound': -TAIL_MM, 'pass': min(gains) >= -TAIL_MM},
        'engineering': {'checks': eng, 'pass': all(e['pass'] for e in eng)},
        'integrity': {'controlInvalidPublications': ax['invalidPublications'],
                      'treatmentInvalidPublications': ay['invalidPublications'],
                      'controlCellsWithInvalid': ax['cellsWithInvalidPublications'],
                      'treatmentCellsWithInvalid': ay['cellsWithInvalidPublications'],
                      'identityChecksPassed': True,
                      'pass': ax['invalidPublications'] == 0 and ay['invalidPublications'] == 0},
    }
    return {
        'controlSeedMedians': Dx, 'treatmentSeedMedians': Dy, 'gain': g,
        'controlDepth': depth_summary(Dx), 'treatmentDepth': depth_summary(Dy),
        'wins': sum(1 for x in gains if x > WIN_MM), 'losses': sum(1 for x in gains if x < -WIN_MM),
        'seeds': len(gains), 'controlWork': ax, 'treatmentWork': ay,
        'conditions': conditions, 'allPass': all(c['pass'] for c in conditions.values()),
    }


def frozen_compare(frozen, treatment, seeds):
    """Depth and tail of the treatment against the frozen record, plus the Sparrow statement."""
    Df, Dt = seed_medians(frozen), seed_medians(treatment)
    g = {s: Df[s] - Dt[s] for s in seeds}
    gains = [g[s] for s in seeds]
    sf, st = depth_summary(Df), depth_summary(Dt)
    return {
        'frozenSeedMedians': Df, 'treatmentSeedMedians': Dt, 'gain': g,
        'wins': sum(1 for x in gains if x > WIN_MM), 'losses': sum(1 for x in gains if x < -WIN_MM),
        'frozenDepth': sf, 'treatmentDepth': st,
        'conditions': {
            'depth': {'medianGain': statistics.median(gains), 'pass': statistics.median(gains) > 0},
            'tail': {'minGain': min(gains), 'bound': -TAIL_MM, 'pass': min(gains) >= -TAIL_MM},
        },
        'packageImprovesFrozenRecord': statistics.median(gains) > 0 and min(gains) >= -TAIL_MM,
        'sparrow': {
            'referenceMm': SPARROW_MM,
            'treatmentMedianOfSeedMedians': st['medianOfSeedMedians'], 'treatmentGapMm': st['gapToSparrowMm'],
            'frozenMedianOfSeedMedians': sf['medianOfSeedMedians'], 'frozenGapMm': sf['gapToSparrowMm'],
            'gapReductionMm': sf['gapToSparrowMm'] - st['gapToSparrowMm'],
            'treatmentSeedMediansAtOrBelowReference': st['seedMediansAtOrBelowSparrow'],
            'frozenSeedMediansAtOrBelowReference': sf['seedMediansAtOrBelowSparrow'],
        },
    }


def select_between(cmpB, cmpC, profiles, seeds):
    """Astra's rule (review 6, Q12; selection-rule.md). cmpB/cmpC: {profile: compare()} against A."""
    eligible = {'B': all(cmpB[p]['allPass'] for p in profiles), 'C': all(cmpC[p]['allPass'] for p in profiles)}
    H = {p: {s: cmpB[p]['treatmentSeedMedians'][s] - cmpC[p]['treatmentSeedMedians'][s] for s in seeds} for p in profiles}
    out = {'eligible': eligible, 'H': H,
           'medianH': {p: statistics.median(H[p].values()) for p in profiles},
           'minH': {p: min(H[p].values()) for p in profiles}}
    if eligible['B'] and not eligible['C']:
        out.update(decision='B', reason='only B is eligible')
    elif eligible['C'] and not eligible['B']:
        out.update(decision='C', reason='only C is eligible')
    elif not eligible['B'] and not eligible['C']:
        out.update(decision='neither', reason='neither treatment is eligible; do not proceed to validation')
    else:
        tests = {
            f'Legacy median H > {SELECT_HURDLE_MM:.3f}': out['medianH']['legacy'] > SELECT_HURDLE_MM,
            'Wall10s median H >= 0': out['medianH']['wall10s'] >= 0,
            f'no seed on either profile with H < {-TAIL_MM:.3f}': all(out['minH'][p] >= -TAIL_MM for p in profiles),
        }
        out['bothEligibleTests'] = tests
        if all(tests.values()):
            out.update(decision='C', reason='both eligible and all three direct-comparison tests hold')
        else:
            failed = [k for k, v in tests.items() if not v]
            out.update(decision='B', reason='both eligible; direct comparison fails: ' + '; '.join(failed))
    return out


# ----------------------------------------------------------------------------- printing

def fM(x):
    return 'n/a' if x is None else f'{x / 1e6:.2f}M'


def fI(x):
    return 'n/a' if x is None else f'{x:,.0f}'


def fP(x):
    return 'n/a' if x is None else f'{100 * x:.2f} %'


def fS(x):
    return 'n/a' if x is None else f'{x:.4f} s ({1000 * x:.2f} ms)'


def f2(x):
    return 'n/a' if x is None else f'{x:.2f}'


def rel(control, treatment):
    if control in (None, 0) or treatment is None:
        return ''
    return f'  ({100 * (treatment / control - 1):+.1f} %)'


def flag(ok):
    return 'PASS' if ok else 'FAIL'


def knob_text(knobs):
    return f"margin {knobs['proxymargin']:g} um, p = {knobs['guidedexponent']:g}"


WORK_ROWS = [
    ('total evaluations per cell', 'totalEvaluationsPerCell', fM, True),
    ('explore evaluations per published explore bite', 'exploreEvaluationsPerPublishedExploreBite', fI, True),
    ('published explore bites per cell', 'publishedExploreBitesPerCell', f2, False),
    ('explore bites started per cell', 'exploreBitesStartedPerCell', f2, False),
    ('publication fraction', 'publicationFraction', fP, False),
    ('explore elapsed time per published explore bite', 'exploreSecondsPerPublishedExploreBite', fS, True),
    ('master iterations per cell (all bites)', 'masterIterationsPerCell', lambda x: f'{x:.0f}', False),
    ('give-ups (checkpoint refusals containing "band")', 'giveUps', str, False),
    ('checkpoint refusals, all reasons', 'checkpointRefusals', str, False),
    ('publications per cell', 'publicationsPerCell', f2, False),
    ('explore elapsed seconds per cell', 'exploreSecondsPerCell', lambda x: f'{x:.3f}', False),
    ('wall total seconds per cell', 'totalSecondsPerCell', lambda x: f'{x:.3f}', False),
]


def print_comparison(profile, ctrl, treat, knobs, cmp):
    Dx, Dy, g = cmp['controlSeedMedians'], cmp['treatmentSeedMedians'], cmp['gain']
    print(f"== {profile}: control {ctrl} ({knob_text(knobs[ctrl])}) vs treatment {treat} ({knob_text(knobs[treat])}); "
          f"{cmp['controlWork']['cells']} + {cmp['treatmentWork']['cells']} cells")
    print(f'   seed      D[{ctrl}]        D[{treat}]      g = D[{ctrl}] - D[{treat}]')
    for s in sorted(g):
        print(f'   {s:<5} {Dx[s]:10.3f}  {Dy[s]:10.3f}  {g[s]:+10.3f}')
    c = cmp['conditions']
    print(f"   median g {c['depth']['medianGain']:+.3f} mm, min g {c['tail']['minGain']:+.3f} mm, "
          f"{treat} wins {cmp['wins']}/{cmp['seeds']} (> {WIN_MM}), losses {cmp['losses']}/{cmp['seeds']} (< -{WIN_MM})")
    for name, d in ((ctrl, cmp['controlDepth']), (treat, cmp['treatmentDepth'])):
        print(f"   {name}: median of seed medians {d['medianOfSeedMedians']:.3f} mm (mean {d['meanOfSeedMedians']:.3f}); "
              f"gap to Sparrow {SPARROW_MM} = {d['gapToSparrowMm']:+.3f} mm; seed medians <= {SPARROW_MM}: "
              f"{d['seedMediansAtOrBelowSparrow']}/{d['seeds']}")
    ax, ay = cmp['controlWork'], cmp['treatmentWork']
    print(f'   work, {ctrl} -> {treat}:')
    for label, key, fmt, show_rel in WORK_ROWS:
        print(f'      {label:<52} {fmt(ax[key])} -> {fmt(ay[key])}{rel(ax[key], ay[key]) if show_rel else ""}')
    print(f"   [{flag(c['depth']['pass'])}] 1 depth: median g = {c['depth']['medianGain']:+.3f} mm > 0")
    print(f"   [{flag(c['tail']['pass'])}] 2 tail: min g = {c['tail']['minGain']:+.3f} mm >= {-TAIL_MM:.3f}")
    print(f"   [{flag(c['engineering']['pass'])}] 3 engineering:")
    for e in c['engineering']['checks']:
        fmt = {k: f for _, k, f, _ in WORK_ROWS}[e['key']]
        print(f"        [{flag(e['pass'])}] {e['measure']}: {fmt(e['treatment'])} {e['requirement']} than {fmt(e['control'])}")
    i = c['integrity']
    print(f"   [{flag(i['pass'])}] 4 integrity: invalid publications {ctrl} {i['controlInvalidPublications']} "
          f"({i['controlCellsWithInvalid']} cells), {treat} {i['treatmentInvalidPublications']} "
          f"({i['treatmentCellsWithInvalid']} cells); identity checks passed")


def print_frozen(profile, label, treat, fz):
    Df, Dt, g = fz['frozenSeedMedians'], fz['treatmentSeedMedians'], fz['gain']
    print(f'== {profile}: frozen record {label} vs treatment {treat} (separate claim: the package improves the frozen record)')
    print(f'   seed      D[{label}]     D[{treat}]      g = D[{label}] - D[{treat}]')
    for s in sorted(g):
        print(f'   {s:<5} {Df[s]:10.3f}  {Dt[s]:10.3f}  {g[s]:+10.3f}')
    c = fz['conditions']
    print(f"   median g {c['depth']['medianGain']:+.3f} mm, min g {c['tail']['minGain']:+.3f} mm, "
          f"{treat} wins {fz['wins']}/{len(g)}, losses {fz['losses']}/{len(g)}")
    print(f"   [{flag(c['depth']['pass'])}] depth: median g = {c['depth']['medianGain']:+.3f} mm > 0")
    print(f"   [{flag(c['tail']['pass'])}] tail: min g = {c['tail']['minGain']:+.3f} mm >= {-TAIL_MM:.3f}")
    sp = fz['sparrow']
    print(f"   Sparrow ({profile}): treatment median of seed medians {sp['treatmentMedianOfSeedMedians']:.3f} mm, "
          f"residual gap to {SPARROW_MM} = {sp['treatmentGapMm']:+.3f} mm; frozen {sp['frozenMedianOfSeedMedians']:.3f} mm, "
          f"gap {sp['frozenGapMm']:+.3f} mm; reduction {sp['gapReductionMm']:+.3f} mm; treatment seed medians "
          f"<= {SPARROW_MM}: {sp['treatmentSeedMediansAtOrBelowReference']}/{len(g)} "
          f"(frozen {sp['frozenSeedMediansAtOrBelowReference']}/{len(g)})")
    print(f"   package improves the frozen record: {'YES' if fz['packageImprovesFrozenRecord'] else 'NO'}")


def print_selection(sel, profiles, seeds):
    print('== selection between B (p = 1) and C (p = 0.75), Astra review 6 Q12')
    print(f"   eligible: B {sel['eligible']['B']}, C {sel['eligible']['C']}")
    for p in profiles:
        row = ' '.join(f"{s}:{sel['H'][p][s]:+.3f}" for s in seeds)
        print(f"   H = D[B] - D[C] ({p}): {row}; median {sel['medianH'][p]:+.3f}, min {sel['minH'][p]:+.3f}")
    for k, v in sel.get('bothEligibleTests', {}).items():
        print(f'   [{flag(v)}] {k}')
    print(f"   decision: {sel['decision']} ({sel['reason']})")


# ----------------------------------------------------------------------------- driver

def parse_seeds(text):
    seeds = []
    for part in text.split(','):
        part = part.strip()
        if not part:
            continue
        m = re.fullmatch(r'(\d+)\s*(?:\.\.|-)\s*(\d+)', part)
        if m:
            seeds.extend(range(int(m[1]), int(m[2]) + 1))
        elif part.isdigit():
            seeds.append(int(part))
        else:
            refuse(f'--seeds: cannot parse {part!r} (use 18..26 or 18,19,20)')
    if not seeds or len(set(seeds)) != len(seeds):
        refuse(f'--seeds: empty or repeated seeds in {text!r}')
    return seeds


def parse_knobs(items):
    knobs = {}
    for item in items:
        arm, sep, rest = item.partition('=')
        if not sep or not arm:
            refuse(f"--knobs: expected ARM='proxymargin=8,guidedexponent=1', got {item!r}")
        spec = dict(DEFAULT_KNOBS)
        for kv in filter(None, (x.strip() for x in rest.split(','))):
            key, sep2, value = kv.partition('=')
            key = key.strip().lower()
            if not sep2 or key not in spec:
                refuse(f'--knobs {arm}: unknown knob {kv!r} (known: {", ".join(spec)})')
            try:
                spec[key] = float(value)
            except ValueError:
                refuse(f'--knobs {arm}: {kv!r} is not a number')
            if not math.isfinite(spec[key]):
                refuse(f'--knobs {arm}: {kv!r} is not finite')
        if arm in knobs:
            refuse(f'--knobs: arm {arm} declared twice')
        knobs[arm] = spec
    return knobs


def load_archive(name, dirs, args, arms, profiles, seeds, knobs, registry):
    """Load every arm from dirs (priority order), verify identity across all cells."""
    cells = {}
    for arm in arms:
        cells[arm] = load_arm(dirs, lambda p, a=arm: f'{args.prefix}-{a}-{p}', arm, profiles, seeds, knobs[arm], registry)
    identity = verify_identity(all_cells(cells), profiles, f'archive {name}')
    overlaid = sorted(c['path'] for c in all_cells(cells) if c['sourceDir'] != args.dir)
    return {'name': name, 'dirs': dirs, 'cells': cells, 'identity': identity, 'overlaid': overlaid}


def score_archive(archive, args, arms, profiles, seeds, knobs, frozen):
    ctrl, treat, treat2 = args.control, args.treatment, args.treatment2
    cells = archive['cells']
    comparisons = {t: {p: compare(cells[ctrl][p], cells[t][p], seeds) for p in profiles} for t in arms if t != ctrl}
    failures = [f'{p}/{k}' for p in profiles for k, c in comparisons[treat][p]['conditions'].items() if not c['pass']]
    verdict = 'PROMOTE' if not failures else 'DO NOT PROMOTE'
    result = {'name': archive['name'], 'dirs': archive['dirs'], 'overlaid': archive['overlaid'],
              'identity': archive['identity'], 'comparisons': comparisons,
              'verdict': verdict, 'failures': failures,
              'cells': {a: {p: [c for c in flat(cells[a][p])] for p in profiles} for a in arms}}
    if frozen is not None:
        result['frozen'] = {p: frozen_compare(frozen['cells'][p], cells[treat][p], seeds) for p in profiles}
    if args.select:
        result['selection'] = select_between(comparisons[treat], comparisons[treat2], profiles, seeds)
    return result


def print_archive(res, args, arms, profiles, seeds, knobs, frozen):
    ctrl, treat, treat2 = args.control, args.treatment, args.treatment2
    print('=' * 100)
    print(f"ARCHIVE {res['name']}: {' overlaid on '.join(res['dirs'])}")
    if res['overlaid']:
        print(f"   cells taken from --originals ({len(res['overlaid'])}): " + ', '.join(os.path.basename(p) for p in res['overlaid']))
    ident = res['identity']
    print(f"   executableSha256 {ident['executableSha256']}")
    print(f"   request.sha256 {ident['requestSha256']}  path {', '.join(ident['requestPath'])}")
    print(f"   contract {json.dumps(ident['contract'], sort_keys=True)}")
    print('   wallIterationCap ' + ', '.join(f'{p} {v}' for p, v in ident['wallIterationCap'].items()))
    for t in arms:
        if t == ctrl:
            continue
        for p in profiles:
            print_comparison(p, ctrl, t, knobs, res['comparisons'][t][p])
        eligible = all(res['comparisons'][t][p]['allPass'] for p in profiles)
        print(f"   {t} against {ctrl}: all four conditions on both profiles: {'PASS' if eligible else 'FAIL'}")
    print(f"VERDICT ({res['name']}, treatment {treat} vs control {ctrl}): {res['verdict']}"
          + (f"; failed: {', '.join(res['failures'])}" if res['failures'] else ''))
    if 'frozen' in res:
        for p in profiles:
            print_frozen(p, frozen['label'], treat, res['frozen'][p])
    if 'selection' in res:
        print_selection(res['selection'], profiles, seeds)


def run(args):
    seeds = parse_seeds(args.seeds)
    profiles = [p.strip() for p in args.profiles.split(',') if p.strip()]
    if not profiles:
        refuse('--profiles: none given')
    arms = [args.control, args.treatment] + ([args.treatment2] if args.treatment2 else [])
    if len(set(arms)) != len(arms):
        refuse('--control / --treatment / --treatment2 must name distinct arms')
    if args.select and not args.treatment2:
        refuse('--select needs --treatment2')
    if args.select and not {'legacy', 'wall10s'} <= set(profiles):
        refuse("--select applies Astra's rule, which names the legacy and wall10s profiles; both must be scored")
    declared = parse_knobs(args.knobs)
    knobs = {a: declared.get(a, dict(DEFAULT_KNOBS)) for a in arms}
    if not os.path.isdir(args.dir):
        refuse(f'{args.dir} is not a directory')

    frozen = None
    frozen_registry = {}
    if args.frozen:
        fdir, label = args.frozen
        fknobs = declared.get(label, dict(DEFAULT_KNOBS))
        fcells = load_arm([fdir], lambda p: f'{label}-{p}', label, profiles, seeds, fknobs, frozen_registry)
        fidentity = verify_identity([c for per_seed in fcells.values() for c in flat(per_seed)], profiles, f'frozen {label}')
        frozen = {'dir': fdir, 'label': label, 'knobs': fknobs, 'cells': fcells, 'identity': fidentity}

    archives = []
    if args.originals:
        for d in args.originals:
            if not os.path.isdir(d):
                refuse(f'--originals {d} is not a directory')
        archives.append(load_archive('as-run (primary)', list(args.originals) + [args.dir], args, arms, profiles, seeds, knobs, dict(frozen_registry)))
        if not archives[-1]['overlaid']:
            refuse(f'--originals {", ".join(args.originals)}: no cell of this run is present there')
        archives.append(load_archive('replacement', [args.dir], args, arms, profiles, seeds, knobs, dict(frozen_registry)))
    else:
        archives.append(load_archive('archive', [args.dir], args, arms, profiles, seeds, knobs, dict(frozen_registry)))

    if frozen is not None:
        for a in archives:
            fi, ai = frozen['identity'], a['identity']
            if fi['requestSha256'] != ai['requestSha256']:
                refuse(f"frozen {frozen['label']}: request.sha256 {fi['requestSha256']} differs from the run's {ai['requestSha256']}")
            if json.dumps(fi['contract'], sort_keys=True) != json.dumps(ai['contract'], sort_keys=True):
                refuse(f"frozen {frozen['label']}: contract {json.dumps(fi['contract'], sort_keys=True)} differs from the run's "
                       f"{json.dumps(ai['contract'], sort_keys=True)}")
            for p in profiles:
                if fi['wallIterationCap'][p] != ai['wallIterationCap'][p]:
                    refuse(f"frozen {frozen['label']}: wallIterationCap of {p} is {fi['wallIterationCap'][p]} but the run's is {ai['wallIterationCap'][p]}")

    # Everything is loaded and verified; from here nothing refuses.
    results = [score_archive(a, args, arms, profiles, seeds, knobs, frozen) for a in archives]

    print('spec-score: ICS-guided-exponent development screen / prospective specification ICS-guided-exponent-v1')
    print(f'   dir {args.dir}  prefix {args.prefix}  seeds {seeds[0]}..{seeds[-1]} ({len(seeds)})  profiles {", ".join(profiles)}  repetitions r0,r1,r2')
    print('   arms: ' + '; '.join(f'{a} = {knob_text(knobs[a])}' + (' [control]' if a == args.control else '') for a in arms))
    if args.originals:
        print(f'   originals {", ".join(args.originals)} (as-run archive = originals overlaid on dir; primary)')
    if frozen is not None:
        fi = frozen['identity']
        print(f"   frozen record {frozen['label']} from {frozen['dir']} ({knob_text(frozen['knobs'])}): "
              f"executableSha256 {fi['executableSha256']}; request.sha256 and contract equal the run's; "
              f"caps " + ', '.join(f'{p} {v}' for p, v in fi['wallIterationCap'].items()))
    for res in results:
        print_archive(res, args, arms, profiles, seeds, knobs, frozen)

    summary = {'verdicts': {r['name']: r['verdict'] for r in results}}
    if len(results) > 1:
        print('=' * 100)
        agree = len({r['verdict'] for r in results}) == 1
        summary['verdictsAgree'] = agree
        print('ARCHIVES: verdicts ' + ('agree' if agree else 'DISAGREE') + ': '
              + '; '.join(f"{r['name']} {r['verdict']}" for r in results))
    if args.select:
        decisions = {r['name']: r['selection']['decision'] for r in results}
        if len(results) == 1:
            final = results[0]['selection']['decision']
            note = 'single archive'
        elif len(set(decisions.values())) == 1:
            final = results[0]['selection']['decision']
            note = 'both archives agree'
        else:
            b_ok = all(r['selection']['eligible']['B'] for r in results)
            final = 'B' if b_ok else 'neither'
            note = 'archives disagree: ' + ('B retained, eligible under both' if b_ok else 'B not eligible under both, so neither')
        summary['selection'] = {'perArchive': decisions, 'final': final, 'note': note}
        print('=' * 100)
        print('SELECTION: ' + '; '.join(f'{k} -> {v}' for k, v in decisions.items()) + f'  =>  {final} ({note})')

    return {
        'run': {'dir': args.dir, 'prefix': args.prefix, 'seeds': seeds, 'profiles': profiles, 'arms': arms,
                'control': args.control, 'treatment': args.treatment, 'treatment2': args.treatment2,
                'knobs': knobs, 'originals': args.originals, 'select': args.select,
                'frozen': None if frozen is None else {'dir': frozen['dir'], 'label': frozen['label'],
                                                       'knobs': frozen['knobs'], 'identity': frozen['identity']},
                'constants': {'sparrowMm': SPARROW_MM, 'tailMm': TAIL_MM, 'selectHurdleMm': SELECT_HURDLE_MM, 'winMm': WIN_MM}},
        'archives': results,
        'summary': summary,
    }


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0], formatter_class=argparse.RawDescriptionHelpFormatter,
                                 epilog=__doc__)
    ap.add_argument('dir')
    ap.add_argument('prefix')
    ap.add_argument('--control', required=True)
    ap.add_argument('--treatment', required=True)
    ap.add_argument('--treatment2')
    ap.add_argument('--seeds', default='18..26')
    ap.add_argument('--profiles', default='legacy,wall10s')
    ap.add_argument('--knobs', nargs='*', default=[], metavar="ARM='proxymargin=8,guidedexponent=1'")
    ap.add_argument('--select', action='store_true')
    ap.add_argument('--originals', nargs='+', action='extend', metavar='DIR')
    ap.add_argument('--frozen', nargs=2, metavar=('DIR', 'LABEL'))
    ap.add_argument('--json', metavar='OUT')
    args = ap.parse_args(argv)
    try:
        report = run(args)
        if args.json:
            with open(args.json, 'w') as fh:
                json.dump(report, fh, indent=1, sort_keys=True, allow_nan=False)
            print(f'json written: {args.json}')
    except Refusal as e:
        print(f'REFUSED: {e}')
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
