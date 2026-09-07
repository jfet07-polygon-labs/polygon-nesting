# The guided exponent, live: `--guidedexponent=<p>`

The knob (commit "The guided exponent: --guidedexponent=<p> ...", 6d55efb, loose ends f856d19)
makes the separation *rank* on `sum w v^p` instead of `sum w v^2`: `energy::fold` (the
tournament's winner) and `energy::incident_totals` (the relocate's candidates and both CD stages)
are now the `_with_exponent` variants at a process-level knob, `p = 2` instantiated as
`w * (v * v)` so the default is the frozen engine to the bit (bitcheck ALL_IDENTICAL against
frozen-2b5f16d and frozen-9c38526). Raw Φ (`sum v^2`), the max violation, the 4 µm band, the
strike meter, `gls_update`'s `v / v_max` growth, the pool, the disruption, `publish.rs`, the
kernel and `validate_placements_against_contract` are untouched; the lexicographic rule "any
clear pose beats every colliding pose" (`relocate::eval_cmp` on `raw`) stays. The exponent is a
landscape change of our own design (Grok review 12 line 188 chose `v^2` for "one guided path"),
not Sparrow's pole proxy: `v` is still the source-ring signed-gap residual. Why it exists: the
bite microscope's pinned column (`../bite-microscope/README.md`) and the replay exponent probe
there; GPT-6 Astra reviews 5 (Q3) and 5b (Q6–Q9, `docs/astra-review-5b-the-exponent.md`).

The verifier (workflow `ics-mechanisms-round`, adversarial read against the forbidden-rescue
table of `docs/grok-review-12-reading-sparrow.md` §5.2) refuted nothing and named four
non-refuting defects: the corpus cell's gradient probe would mix a `v^2` direction with a `v^p`
acceptance (the cell now refuses the knob, f856d19); a doc drift on `guided_term_with_exponent`
(fixed, f856d19); the knob tests share the existing `knob_lock` exposure of `ProxyMarginGuard`;
and one `Relaxed` atomic load per `fold` / `incident_totals` call on the default path
(trajectory-neutral under fixed work; only wall-mode timing could notice).

## Development measurement (dev seeds 18–26, one repetition, no margin, profile caps)

Both arms without the proxy margin, because the frozen `devbase` cells were measured without
it and this is one factor at a time; Legacy cap 50, Wall10s unbounded, ten seconds from the
bare request, one eight-worker process at a time. Documents: `guided-exponentfull-*.json.gz`
(p = 1, nine seeds) and `guided-exponent075-*.json.gz` (p = 0.75, seeds 18–20). The frozen
control cells are `devbase-*` under `/var/lib/t3/tmp/astra/dev` (the same documents as in the
proxy-margin experiment).

`analyse.py devbase guided-exponentfull` (p = 1, nine seeds), verbatim:

```
== profile legacy | devbase vs guided-exponentfull | 9 seeds, 9/9 cells ==
  devbase      median  164.954  mean  165.105  best  163.550  worst  167.157  iters/cell   2695  explore bites  99.8
  guided-exponentfull median  164.002  mean  164.685  best  162.132  worst  169.039  iters/cell   2708  explore bites 103.9
  paired median gain (A-B) +0.846 mm | B wins 6/9 | worst paired -4.484 | invalid publications 0
  per seed: 18:-1.12 19:+0.85 20:-1.12 21:+3.16 22:+0.65 23:+1.26 24:+2.38 25:-4.48 26:+2.22
== profile wall10s | devbase vs guided-exponentfull | 9 seeds, 9/9 cells ==
  devbase      median  159.861  mean  160.243  best  159.003  worst  165.237  iters/cell   1345  explore bites   4.9
  guided-exponentfull median  159.659  mean  160.147  best  154.848  worst  164.407  iters/cell   1904  explore bites   4.9
  paired median gain (A-B) +0.114 mm | B wins 5/9 | worst paired -5.404 | invalid publications 0
  per seed: 18:-0.36 19:-4.15 20:+0.11 21:+10.39 22:+0.70 23:-1.43 24:-5.40 25:+0.30 26:+0.71
```

`analyse.py devbase guided-exponent075` (p = 0.75, seeds 18–20), verbatim:

```
== profile legacy | devbase vs guided-exponent075 | 3 seeds, 3/3 cells ==
  devbase      median  164.954  mean  164.898  best  164.262  worst  165.478  iters/cell   2779  explore bites  98.7
  guided-exponent075 median  160.698  mean  161.297  best  159.192  worst  164.002  iters/cell   4497  explore bites 125.3
  paired median gain (A-B) +4.256 mm | B wins 3/3 | worst paired +1.476 | invalid publications 0
  per seed: 18:+5.07 19:+1.48 20:+4.26
== profile wall10s | devbase vs guided-exponent075 | 3 seeds, 3/3 cells ==
  devbase      median  159.773  mean  159.630  best  159.256  worst  159.861  iters/cell   1135  explore bites   5.0
  guided-exponent075 median  159.047  mean  159.160  best  154.391  worst  164.043  iters/cell   4036  explore bites   5.0
  paired median gain (A-B) +0.726 mm | B wins 2/3 | worst paired -4.181 | invalid publications 0
  per seed: 18:+4.87 19:-4.18 20:+0.73
```

The quick A/B at p = 1 on seeds 18–20 alone read −0.414 mm (1/3) on Legacy and −0.645 mm
(0/3) on Wall10s; the nine-seed measure above supersedes it.

Work, from the documents (`relocateEconomics.sampleEvaluations`, explore bites, master
iterations; A = devbase, B = treatment):

| profile | treatment | cells | evaluations per explore bite A → B | explore bites per cell | master iterations per cell | evaluations per cell | give-ups A → B |
|---|---|---|---|---|---|---|---|
| Legacy | p = 1 | 9 | 437 633 → 385 328 (−12 %) | 99.8 → 103.9 | 2695 → 2708 | 43.7 M → 40.0 M | 1535 → 1307 |
| Legacy | p = 0.75 | 3 | 444 323 → 279 493 (−37 %) | 98.7 → 125.3 | 2779 → 4497 | 43.8 M → 35.0 M | 588 → 336 |
| Wall10s | p = 1 | 9 | 9 166 933 → 7 761 497 (−15 %) | 4.9 → 4.9 | 1345 → 1904 | 44.8 M → 37.9 M | 737 → 780 |
| Wall10s | p = 0.75 | 3 | 8 884 701 → 6 604 385 (−26 %) | 5.0 → 5.0 | 1135 → 4036 | 44.4 M → 33.0 M | 169 → 104 |

(Give-ups are the no-margin exact-call refusals "outside the 4 µm band or no sheet slack"; with
margin 8, which both arms of the registered screen carry, they are zero in every cell measured
so far.)

## Against the falsifiers registered with the mechanism

Registered before the run (workflow args, verbatim in the commit's provenance): any invalid
publication; the default path not bit-identical; sample evaluations per explore bite not falling
by at least 30 % on Legacy seeds 18–26; a paired-median depth regression on either profile;
and, recorded for the screen, any per-seed repetition-median regression greater than 1.000 mm.

| falsifier | p = 1 (nine seeds) | p = 0.75 (three seeds) |
|---|---|---|
| invalid publication | none (0 / 0) | none (0 / 0) |
| default path bit-identical | ALL_IDENTICAL | same binary |
| evaluations per explore bite −30 % on Legacy | **fails**: −12 % | passes: −37 % |
| paired-median depth regression | none: +0.846 / +0.114 | none: +4.256 / +0.726 |
| per-seed regression > 1.000 mm (single repetition here, not a median) | Legacy 18, 20, 25 (−1.12, −1.12, −4.48); Wall10s 19, 23, 24 (−4.15, −1.43, −5.40) | Wall10s 19 (−4.18) |

So p = 1, Astra's named value, does not deliver the work reduction the mechanism promised on
the live path: the trickle is shorter (the median gain is positive on both profiles) but the
evaluations per bite fall by an eighth, not a third, and single-repetition regressions of 4–5
mm appear on three seeds per profile. p = 0.75 on three seeds delivers the promised work
reduction and a Legacy gain of 4.3 mm with every seed improving, taking Legacy from 164.95 to
160.70, which is where the Wall10s profile sits; on Wall10s it wins two of three with one
4 mm loss. These are single repetitions on consumed seeds: a development finding, not a
result. Review 5b Q9 is explicit that a fractional exponent found this way does not substitute
its winner into the registered 108-cell screen ("Selecting the best of 1, 0.75 and 0.5 on seed
20 would still be fitting to seed 20") and asks for "a separate selection/validation plan"
before further treatment results. The registered screen (control p = 2 against p = 1, margin
8 and the profile caps in both arms, nine seeds, three repetitions, rotated order) runs as
registered; its result is appended below when it lands, and the p = 0.75 finding goes to
Astra as a development finding with the question of how to validate it prospectively.
