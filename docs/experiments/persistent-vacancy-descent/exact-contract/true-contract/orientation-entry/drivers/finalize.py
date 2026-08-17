#!/usr/bin/env python3
"""Rewrites the generated summary.json into the committed, narrated form."""
import json, sys

EVID = ('/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_5b536ff5-ed0-1/docs/'
        'experiments/persistent-vacancy-descent/exact-contract/true-contract/'
        'orientation-entry')
d = json.load(open(f'{EVID}/summary.json'))
table = d['resultsTable']

d['pinnedParent']['file'] = 'pinned-parent-159.083.json'
d['certification']['pin'] = 'pinned-parent-159.083.json'
d['fixpointProbe']['pin'] = 'pinned-parent-159.083.json'
d['parent'] = {
    'file': '../record-159.092/pinned-parent-159.092.json',
    'rawSourceDepthMm': 159.09233022733062,
    'placementFingerprint':
        'fa01012af1d559ae09ce7295c146f0cdc6569cfad6f24b6154f0153c4393dbbc',
}
d['improvementMm'] = 159.09233022733062 - d['pinnedParent']['rawSourceDepthMm']
d['modes'] = {
    '32': 'mode 28 (conflict ejection + anchor-local re-insertion) with the '
          'orientation-perturbation candidate stream armed',
    '33': 'mode 29 (joint whole-component re-placement) with the same stream armed',
    'shared': 'both share the legacy modes\' seed domains, so the legacy part of the '
              'candidate stream is literally the same poses in the same order; the '
              'orientation candidates rank behind every anchor-local candidate and '
              'ahead of the station stream',
}
d['mechanism'] = {
    'ladder': 'ORIENTATION_PERTURBATION_LADDER_DEG = [0.02, 0.05, 0.125, 0.3125, '
              '0.78125, 1.953125, 4.8828125] - geometric, ratio 5/2, a named constant '
              'set rather than anything derived from this request; both signs, plus '
              'the mirror family where allowMirror is set, giving 29 variants',
    'reCentring': "each variant's translation is shifted so that its collision "
                  "bounding-box centre lands where the vacated pose's centre was, "
                  'which is what makes a rung a rotation IN PLACE: rotation is applied '
                  'about the source origin, so a 4.88-degree rung on a piece whose '
                  'material sits 100 mm from that origin would otherwise be an 8.5 mm '
                  'translation into a different pocket',
    'neighbourhood': "each variant is searched over the SAME local translation "
                     "neighbourhood the vacated pose gets (the separating projection's "
                     'trajectory, the peers\' vacated pockets, the aimed displacement '
                     'cloud), rigidly carried along by that same re-centring shift',
    'ordering': 'rank-major - every variant\'s re-centred pose before any variant\'s '
                'first displacement - so the row budget divides across the orientation '
                'set instead of being spent by the first rung',
    'budget': 'ORIENTATION_PERTURBATION_ROWS = ORIENTATION_PERTURBATION_VARIANTS * '
              'ANCHOR_LOCAL_ROWS = 5568, derived from the stream it has to cover; '
              'load-bearing, see budgetIsLoadBearing below',
    'reusedVerbatim': 'violation survey, vertex-cover and whole-component ejection, '
                      'kept-sub-layout micro-legalization, component limit, ejection '
                      'limit of 7, insertion-order enumeration, pose-swap round, '
                      'finalist beam, bound contract, and '
                      'validate_and_measure_placements as sole publication authority',
}
d['attribution'] = {
    'why': 'the mandatory measurement: which of the four candidate families the pose '
           'each re-placed piece actually committed to came from. Reported per piece '
           'in replacementRepair.pieces[].orientation / '
           'jointReplacement.orders[].pieces[].orientation as four mutually exclusive '
           'counters plus the accepted rotation and its signed offset from the '
           'vacated orientation.',
    'headline': 'EVERY sub-record publication on the record line carries '
                'acceptedOrientation >= 1. The depth-setting piece '
                '54345eb7-a37e-45eb-b0fd-eccffdfa14cc-copy-3 - measured by the '
                'pose-entry negative as having ZERO re-placement freedom - returned 0 '
                'anchor-local finalists out of 122 candidates (unchanged) and 2 '
                'orientation finalists out of 3509, and committed to one of them at '
                'rotation 179.957611 (delta -0.02 deg).',
    'acceptedRungDistribution': 'across the whole campaign the accepted orientation '
                                'poses are overwhelmingly at the ladder\'s FINEST rung '
                                '(+/-0.02 deg), with a minority at +/-0.125 deg and a '
                                'family of pure mirror flips (delta 0.0, mirror '
                                'flipped). The mechanism\'s per-move quantum is now the '
                                'ladder\'s finest rung, exactly as it used to be the '
                                '0.001 mm pose grid.',
}
d['budgetIsLoadBearing'] = (
    'at ORIENTATION_PERTURBATION_ROWS = 512 the stream was truncated to its leading '
    '~17 neighbourhood ranks and the record basin\'s depth-setting piece produced 0 '
    'finalists from 3509 candidates; at full coverage (5568) the same piece produced '
    '2, and mode 33 published below the record on the very next arm. Same binary, same '
    'fixture, same perturbation - only the row budget differed.'
)
d['verdict'] = (
    'YES, continuous-angle re-insertion breaks the translation fixpoint - on the '
    'record line. The 159.092330 fixpoint had survived 149 arms across seven modes; it '
    'fell to the first orientation-armed arm tried. The improvement is small (0.009693 '
    'mm, cascaded over three rounds) and the 155.000 goal threshold remains far off, '
    'but the qualitative barrier the whole campaign had measured is gone: the pieces '
    'that set the depth are frozen in TRANSLATION, not in pose. On the from-scratch '
    'line at 164.038568 the same mechanism produced accepted orientation poses but no '
    'publication below the incumbent across 288 arms, which is a real asymmetry: that '
    'frontier\'s ranks 1-8 sit within 0.0225 mm, so the pieces that would have to '
    'rotate are not the ones that set the depth.'
)
d['tierAsymmetry'] = (
    'mode 33 is the productive tier and mode 32 is not. Mode 32 ejects a vertex cover '
    'and leaves the partner in place, so the rotated piece still has to clear a '
    'neighbour that cannot move; mode 33 ejects both endpoints, and the orientation '
    'freedom pays only when the partner can move out of the way at the same time. On '
    'the record grid mode 33 took 4 of the 4 sub-record publications and mode 32 took '
    'none, though mode 32 did accept 2 orientation poses.'
)
d['perturbationFindings'] = {
    'productiveArms': 'frontier flatten 0.004 (mode 33 -> 159.089637), single-piece '
                      'nudge of the depth-setting piece at d=0.006 (159.089637) and '
                      'd=0.02 (159.085986, the best single arm), and the rank-1+2 pair '
                      'nudge at d=0.006 (159.089637)',
    'boundIsLoose': 'every publication used a bound of parent + 2.0 mm; the bound gates '
                    'acceptance and does not drive descent, exactly as the from-scratch '
                    'line had already measured',
    'vertexCoverTieBreak': 'still self-defeating for mode 32 - a single-piece nudge '
                           'gets the INNOCENT NEIGHBOUR ejected - which is the second '
                           'reason mode 33 is the productive tier: it ejects both '
                           'endpoints regardless of the tie-break',
}
d['fixpointProbe']['composition'] = (
    'mode 31 tiny steps {0.006, 0.012, 0.025, 0.04}, mode 22 seeds 0-3, frontier '
    'flatten {0.001, 0.002, 0.003, 0.004, 0.005, 0.008, 0.012, 0.02, 0.03} x modes '
    '{33, 32, 29, 28} x bound slack {0.05, 2.0}, single-piece nudges on frontier ranks '
    '1-4 x d {0.002, 0.006, 0.012, 0.02} x modes {33, 32}, and rank-1+2 pair nudges x '
    'the same deltas x modes {33, 32}'
)
d['fixpointProbe']['caveat'] = (
    'the mode-26 clamped-ladder tier is NOT in this probe. It ran in all four cascade '
    'rounds (drops {0.3, 0.55, 1.0} x seeds {0, 1}), adopted nothing, and consumed most '
    'of the wall clock; the cascade itself was stopped mid-round-4 for time rather than '
    'run to a certified fixpoint. So this is "a fixpoint of everything except the '
    'mode-26 ladder tier", measured over 120 arms.'
)
d['cascadeCaveat'] = (
    'the cascade ran 74 arms over three adopting rounds and was stopped part-way '
    'through round 4; the recordCascade block of resultsTable therefore counts '
    'publications against the ORIGINAL 159.092330 record rather than against the '
    'moving incumbent, and its publicationsBelowIncumbent is not a count of independent '
    'wins.'
)
d['failureClasses'] = {
    'recordLine': table['recordLine']['failureClasses'],
    'fromScratchLine': table['fromScratchLine']['failureClasses'],
    'fromScratchPads': table['fromScratchPads']['failureClasses'],
    'fixpointProbeAndCascade': table['recordCascade']['failureClasses'],
}
d['regressionGates'] = {
    'mode20': 'independentDepthMm 206.869 at fingerprint '
              '8a7737381238fa4d4979cbd95a4f08500b6608039475243c0a24c45828f9e437 '
              '(default allowance)',
    'mode22Record': 'rawSourceDepthMm 159.09233022733062 at fingerprint '
                    'fa01012af1d559ae09ce7295c146f0cdc6569cfad6f24b6154f0153c4393dbbc, '
                    'exactValid and contractValid',
    'modes2829AB': '16 arms (8 perturbed fixtures across both lines x modes 28 and 29) '
                   'against the base-commit binary 3cc5376df2e03152: identical in every '
                   'field except elapsed times, executable hash and worktree status',
    'suite': '806 lib tests plus every other target green',
}
d['evidence'] = (
    'drivers/ (lib.py, drv.py, grid.py, cascade.py, pads.py, fixpoint.py, certify.py, '
    'gates.py, ab.py, geodiff.py), logs grid/cascade/pads/fixpoint/certify, lineage/ '
    '(the three intermediate cascade pins), pinned-parent-159.083.json; 560 run JSONs '
    'remain in /var/lib/t3/tmp/orient/runs/'
)
json.dump(d, open(f'{EVID}/summary.json', 'w'), indent=1)
print('ok')
