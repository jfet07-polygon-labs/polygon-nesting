# Signed round: `ICS-guided-exponent-v2-wall10s`

**Committed before any scored cell runs.** Nothing below may be edited after the first scored
cell; an inconvenient clause is a FAIL, not an amendment. The specification is GPT-6 Astra's
(review 7, Q16–Q17, `docs/astra-review-7-the-verdict.md`), transcribed with its own words wherever
a clause is quoted, on the owner's instruction that Astra replaces the three-model quorum. The
frozen identities are appended below before the first cell, as the protocol requires.

## What this round tests, said plainly

The same candidate as `ICS-guided-exponent-v1` (`docs/quorum/ics-guided-exponent-v1-spec.md`):
the guided objective `Σ w·v^p` at **p = 1** against the frozen **p = 2**, both arms with the proxy
margin of 8 µm and the profile's own cap, everything else identical, on the **Wall10s profile
only** (0.032 step, unbounded cap): review 7 Q16, "a Wall10s-only successor is legitimate … the
Legacy failure ends this candidate's claim to promotion across both profiles; it does not end
research on the exponent. Register Wall10s as the successor's sole promotion scope, preserve the
failed joint result, and validate independently." The joint v1 result (not promoted; Legacy tail
failed on seeds 29 and 31) stands and is not reopened. "The nine Wall10s seeds support choosing
that next question; they cannot also serve as its fresh confirmation."

## Population: thirty fresh seeds, drawn by the recorded procedure (review 7, Q17)

- **Exclusion registry** (`evidence/ics-guided-exponent-v2-wall10s/seed-registry.txt`): every seed
  exercised on this fixture by any archived document (cells, replays, capsules under
  `docs/experiments`, `docs/quorum`, `/var/lib/t3/tmp/astra`), every seed named by a harness
  script, and the blanket 0–99: 100 entries.
- **Nonce**: one fresh 32-byte value from `/dev/urandom`, recorded in `evidence/…/nonce.txt`:
  `98a30ec4a92213ba57b6d120f3b049ce434672e1f4c2ee3d490db186366dcdb9`.
- **Procedure** (`draw-seeds.py`): for counter k = 0, 1, 2, …, the candidate is the unsigned 64-bit
  big-endian integer of the first eight bytes of SHA-256(nonce ‖ k as 8-byte big-endian);
  candidates in the registry or already drawn are rejected; the first thirty admissible are the
  population. The complete draw log is `evidence/…/draw-log.txt` (thirty counters tried, thirty
  admitted, none rejected). The seeds, in draw order, are in `evidence/…/seeds.txt`:
  6185102792240140886,8755853977552987277,12153648923418518200,11151532166486038253,14782797682586776746,2586638353860241226,7797222088981460957,16914106676243337695,17380016107309069740,4872519857840070441,6460741950156061217,15320042189458576624,4481581871270843759,2526133772389730536,5671471283886933426,10835273698295202758,17316774662183274765,8176539551367944027,10882105840989348892,18390115156762500293,13500830528455456567,1401127988487338646,7700681765202323159,15807079958706394540,9319425482262338153,2093561245321342307,10636268072709740349,1266588499924407455,245054974385835927,16197236351957724232.
- The engine takes a 64-bit seed unchanged (`--seed=<u64>`; a fixed-work sanity cell at seed
  6185102792240140886 ran to a valid document before the freeze).
- Why thirty: "a treatment with a 10 % probability of a greater-than-1 mm seed-level regression
  would escape detection on all 30 seeds with probability 0.9^30 = 4.24 %; with nine seeds that
  probability is 38.74 %. This is a tail-screen rationale, not a power calculation."
- These thirty seeds are untouched by any treatment, diagnostic or baseline until this round; no
  difficulty-based substitution and no additional seed after inspecting results.

## Arms

| arm | flags | cap |
|---|---|---|
| Control `A` | `--proxymargin=8` (p = 2) | Wall10s unbounded |
| Treatment `B` | `--proxymargin=8 --guidedexponent=1` | identical |

The exponent is fixed for the entire process, across both phases, all workers, retries and
publications; no other flag differs; no CD limit, sampling count, equality acceptance, weight
cap, boundary margin, publication rule or profile schedule changes.

## Execution: 180 cells, adjacent pairs, no early stopping

- Wall10s only, thirty seeds, three repetitions: **180 new cells**, each
  `--cell=cutclose --request=tests/fixtures/mixed-61/mixed61-request-exact-clearance.json
  --edge=5 --pair=5 --mode=wall --wall=10.0 --orders=1 --workers=8 --arm=control --profile=wall10s
  <arm flags> --seed=<s>`, one fresh eight-worker process, the existing ten-second accounting,
  the bench lock held for the whole round.
- Order: the committed manifest (`evidence/…/manifest-v2.txt`): per repetition the thirty pairs
  are ordered by SHA-256(nonce ‖ "order" ‖ rep ‖ seed) ascending and the pair starts with the
  control when (position + rep) mod 2 = 0, else with the treatment: 45 control-first and 45
  treatment-first positions, each seed control-first in one or two of its three repetitions.
- "Complete the registered population without efficacy-based early stopping."
- Contamination, defined now from independently logged events: the load sampler
  (`load-v2.log`, one `/proc/loadavg` sample every 5 s, started before the first cell) and the
  agents' transcripts. A **pair** is re-run whole if any sample inside either of its cells'
  ten-second windows shows a 1-minute load average above 10.0, or if any cargo invocation ran
  outside the bench lock during either window; originals are preserved beside the replacements
  and both scores are reported. No cell is excluded or re-run for any other reason, never
  because its work count looks low.
- No diagnostic cell runs on these seeds in this round.

## Clock, quality, statistics, scoring

As in v1: ten seconds of wall from the bare request; quality is the final valid published depth
(`outcome.depthMm`); `D_(a,s)` the median over the three repetitions; `g_s = D_(2,s) − D_(1,s)`
(positive = treatment deeper); work from the documents as v1 defines it (explore evaluations =
Σ over explore bites of `strikeMeter.chargedWorkSampleEvaluations`, failed bites charged; explore
elapsed = `wall.loopExploreSeconds`; per-published-bite ratios as sums over the 90 cells of an
arm). Scorer: `docs/experiments/overlap-ics/guided-exponent/screen/spec-score.py` at this commit
(unchanged since v1), command fixed now:
`python3 spec-score.py /var/lib/t3/tmp/astra/v2 v2 --control A --treatment B --knobs A='proxymargin=8' B='proxymargin=8,guidedexponent=1' --profiles wall10s --seeds <the thirty, comma-separated, from seeds.txt> --json v2-score.json`.

## PASS — required, on the Wall10s profile (review 6 Q11, review 7 Q16)

1. **Depth:** `median_s g_s > 0`.
2. **Tail:** `min_s g_s ≥ −1.000 mm`, unrounded, over the thirty seeds.
3. **Engineering:** lower total evaluations per cell; lower explore evaluations and lower explore
   elapsed time per published explore bite; no reduction in published explore bites per cell or
   in their publication fraction.
4. **Integrity:** exactly three uniquely identified repetitions per seed and arm; every document
   verified (request sha256, contract, executable sha256, profile, seed, `proxyMarginUm = 8`,
   `guidedExponent` absent in A and 1 in B, no tripwire); zero invalid publications; the identity
   checks of the frozen executable; no unregistered configuration or timing change.

**Promote only if all four pass.** Otherwise record which failed; do not change p, caps,
margin, seeds or the guard after inspecting results. Promotion means: "on the Wall10s profile,
`p = 1` with margin 8 is the successor of `p = 2` with margin 8" — a profile-scoped claim, the
Legacy result of v1 preserved beside it.

## Reported, not required

- **Sparrow** (residual-gap claim only): "This round tests whether the selected ICS successor
  improves ICS and reduces its observed distance from the archived Sparrow reference of
  150.165 mm. It does not test population-level superiority over Sparrow." Reported: the
  treatment's median of seed medians, its gap to 150.165, the control's gap, the reduction, and
  the count of seed medians at or below 150.165. No archived frozen-record cell exists on these
  seeds, so no package comparison is made here.
- **Forecast outcomes** (never requirements): the halving and the −30 % work target.
- **The v1 diagnostics' hazards** as counts from the documents: failed bites (count, elapsed,
  evaluations, iterations, stop reason), publications, disruptions.

## Refusal

The round is refused as a whole, before any depth is read, if any scored document carries a
tripwire; any (arm, seed) has other than three uniquely identified repetitions; any document
fails a verification; the frozen identities were not appended before the first cell; a cell ran
outside the bench lock's serialization; or a seed outside `seeds.txt` appears.

## Frozen identities (appended before the first scored cell)

- Base commit: `bb226b5` on `engine/topology-archive-search`; its code is `1e913f4`'s (every
  commit since is documentation), suite 862 passed.
- Executable: `/var/lib/t3/tmp/astra/frozen-1e913f4`, sha256
  `8734404a249e06fb57c387a9e1d310147a5b924b255378ff880ed59f189227b2`, the v1 round's binary,
  whose identity checks are recorded in `evidence/ics-guided-exponent-v1/freeze-1e913f4.log`
  (default, treatment p = 1 / margin 8 and control margin 8 paths bit-identical to `frozen-f856d19`
  on the three fixed-work cells; replay identity 43/43 at p = 2 and at `none`); rustc 1.97.1,
  x86_64.
- Request sha256 `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3`; contract
  pair 5.0 / edge 5.0.
- Runner `evidence/…/spec-run.sh` (the v1 runner, unchanged) with `evidence/…/manifest-v2.txt`;
  cells `/var/lib/t3/tmp/astra/v2/v2-<A|B>-wall10s-r<rep>-s<seed>.json`, archived gzipped under
  `evidence/…/cells/` after the round with `load-v2.log`, `spec-run.log` and `contamination.txt`.
