# Independent next-generation nesting challenge

Work in the public repository `https://github.com/jfet07-polygon-labs/polygon-nesting` from branch `engine/general-polygon-search`. Fetch the branch and start your own branch from the exact checkpoint commit named by the person giving you this prompt. Do not assume access to another machine's local paths or temporary files.

Your task is to independently improve the opt-in next-generation polygon nesting engine, concentrating first on Mixed-61 while preserving exact validity, concave-polygon support, determinism under the declared replay contract, and all protected legacy behavior. You may make disruptive architectural changes inside the experimental engine. Do not route existing production profiles to it or weaken publication validation.

## Current evidence

- The protected current Mixed-61 endpoint is independently valid at `168.361 mm` strip depth.
- A portable upstream Sparrow calibration on the same converted 61-piece geometry, continuous rotations, 5 mm part separation, 5 mm sheet inset, seed 0, eight workers, and a three-second global budget is independently valid at `154.44858 mm`.
- The Sparrow input, raw solution, converter, independent validator, commands, provenance, and hashes are committed under `docs/experiments/sparrow-mixed61/`.
- A historical same-machine ten-second run reached `152.49449 mm`, but its raw output is not part of the checkpoint. Treat that value as a replication target, not acceptance evidence.
- Thus `154-155 mm` is a demonstrated reachable band and approximately `150 mm` is an aspirational next boundary. These are strip depths, not square millimetres.
- The current engine has learned useful geometry and validation machinery, a strong exact-valid constructor, concave `PolygonSet` support, source-rebuilt publication validation, continuous rotations, exact overlap primitives, deterministic quotas, and a persistent exact-valid vacancy experiment. Preserve and reuse valuable work rather than rewriting blindly.
- The failure ledger in `docs/next-generation-engine-plan.md` is mandatory reading. It records rejected scalar tweaks, constructor/order portfolios, terminal repairs, pair shadows, exact projection, frontier-only reconstruction, persistent-population variants, and why they failed. Do not repeat a closed mechanism without identifying a materially different causal variable.
- The latest screen shows that permanently reserving two of eight beam slots for area and count elites is harmful: it regressed the best partial state from 11 inactive pieces / `59571041296` grid-area units to 13 / `64577591268`. Incumbent carryover was not tested and is not closed; it was deferred because the prerequisite fixed-slot policy failed.

## The path I intend to pursue in parallel

My next architecture will keep the strong structured-pole generator and exact-valid partial-state machinery, but remove the permanent beam-slot tax. I intend to:

1. Keep a bounded topology archive outside the active beam, keyed by exact-valid active geometry/contact structure and progress history.
2. Admit archive entries only when they add a genuinely different, promising basin; do not let them consume an active slot every generation.
3. Detect deterministic stagnation and revive one archived topology through a dedicated lane or epoch, with the ordinary control lane preserved for causal comparison.
4. Apply bounded large-neighborhood remove/reinsert around blockers and conflict neighborhoods, rather than another single-pair terminal nudge.
5. Explore two coupled populations if evidence supports it: exact-valid partial layouts and temporarily infeasible complete layouts, with explicit transitions and exact publication gates between them.
6. Add deterministic multicore islands only after the serial lifecycle produces strong placements; threads should expand useful diversity, not conceal a weak heuristic.

This is context, not a required design. You are explicitly free to disagree. Choose one of these approaches:

- independently implement and improve the same hypothesis;
- propose and test a materially different architecture because you believe this path is wrong;
- combine selected parts with a stronger alternative.

If you diverge, explain which evidenced assumption you reject and design a fair control that can falsify your alternative. Do not optimize for agreement with this prompt.

## Success contract

First establish the exact checkpoint baseline on your machine. Then use paired, same-build, same-machine controls. Report exact-valid strip depth, placed count, solver and wall time, peak RSS, seeds, worker count, target/toolchain, request hash, source revision, executable hash, and independent validation. A provisional six-second engine limit is a diagnostic guard, not absolute product truth: a repeatable seven-second result can be valuable if it gives a material quality gain, but the trade-off must be explicit.

The preferred outcome is a strict, independently valid Mixed-61 improvement toward or below `154.44858 mm` without regressing protected validity. Intermediate improvements are useful only when they teach a new causal fact. Do not elevate a golden result from one lucky run. Use repeated fixed-seed controls first, then multiple seeds when the mechanism survives.

You must commit and push everything needed for another machine to validate your claims: source, tests, fixtures, converters, raw result JSON, independent validator output, benchmark commands, provenance, hashes, timing/RSS evidence, and a concise decision ledger. Do not cite files that exist only in your local temporary directory. Keep diagnostic behavior opt-in until the evidence justifies promotion.

Before asking for merge, perform a skeptical review of correctness, validity, reproducibility, overfitting, memory/runtime accounting, and whether the claimed mechanism is actually isolated. Return the branch name, exact commit SHA, commands, evidence paths, result table, rejected attempts, and your recommended next step.
