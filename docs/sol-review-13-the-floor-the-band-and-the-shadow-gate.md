# Sol review 13 — the floor, the band, and the shadow gate (2026-08-22)

Independent post-round review of the round-envelope kernel + matched gate
(commits e523c91..2e63d4a on b67f350). Run on this machine: Codex
(`codex exec resume --last`, model_reasoning_effort=max, sandbox read-only),
same resumed instance as reviews 3-12. Reply verbatim below.

---

Verdict: **DO-NOT-PROMOTE stands.** The integer kernel is a credible exact oracle for already-canonicalized rings, and the P0 observation is real. But the package overstates three things: “zero false accepts” without qualification, “the two authorities never disagreed,” and “legality solved.” None is true at full scope.

## 1. Method and false positives

### Kernel battery

- The narrow phase itself looks sound. Point–segment distance is an exact `i128` comparison, containment is checked separately—including holes—and boundary admissibility is four integer comparisons: [round_envelope.rs:282](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/round_envelope.rs:282), [round_envelope.rs:539](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/round_envelope.rs:539), [round_envelope.rs:402](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/round_envelope.rs:402).

- The `√2 µm` derivation is correct conditional on the canonicalization model: each endpoint moves at most `√2/2 µm`, and the corresponding segment moves no farther because it is an endpoint convex combination. `PolygonRing::new` preserves vertex correspondence and only reverses/rotates ordering after snapping: [general_polygon.rs:74](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/geometry/general_polygon.rs:74).

- The three inside-band rows are therefore not kernel errors relative to the canonical rings. They are, however, unresolved disagreements relative to `source ⊕ disc(r)`. Report the result as:

  `0 violations outside the representation band; 3 source/canonical disagreements inside it`

  rather than unqualified “zero false accepts.” All three remain above the public 5.0 mm contract, so they are not publication false accepts: [kernel README:282](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/round-envelope-kernel/README.md:282).

- Population 2 does not literally discharge the requested inherited-proposal population. It is openly constructed, and its rows are correlated walks selected near a threshold—not 194 independent production proposals: [kernel README:267](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/round-envelope-kernel/README.md:267). It is a good arithmetic/flip test, but promotion still lacks a live shadow over actual rejected search states.

- “No `f64`, therefore cross-platform bit-identical” is too broad. The predicate contains no `f64`, but its canonical input is produced by `sin_cos`, translation, and grid rounding: [general_fast.rs:3712](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:3712), [general_polygon.rs:348](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/geometry/general_polygon.rs:348). Exactness holds given identical canonical rings; end-to-end cross-platform identity has not been proved. Same-x86 determinism is proved.

- `critical_two_r_micron` returns the largest accepted integer threshold—the floor of the true rational distance—not the exact distance “full stop”: [round_envelope.rs:596](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/round_envelope.rs:596). The contrary wording at [round_envelope_gate.rs:17](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/round_envelope_gate.rs:17) should be corrected.

- Minor proof-test defect: `certifies()` permits `two_r <= 2*MAX_RADIUS`, while the overflow test evaluates its RHS with only `MAX_RADIUS`: [round_envelope.rs:535](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/round_envelope.rs:535), [round_envelope.rs:789](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/round_envelope.rs:789). Recomputing with the factor two still leaves ample `i128` headroom, so this is a test/claim mismatch, not an overflow bug.

### P0 miter permissiveness

The core finding is sound:

1. The exact canonical round predicate refuses at `2r`.
2. An ideal miter/square-limited offset contains the round Minkowski offset.
3. The implemented canonical miter polygons have zero intersection/fitting violation.

Therefore the implemented Clipper offset pipeline is permissive relative to the ideal declared envelope.

Two causal claims need correction:

- Production `JoinType::Miter` does **not** call `do_round()`. It dispatches to `do_miter` or `do_square`: [offset.rs:883](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/clipper/offset.rs:883). Both emit grid-rounded vertices: [offset.rs:687](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/clipper/offset.rs:687), [offset.rs:750](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/clipper/offset.rs:750). The evidence attributes the leak to the implemented offset’s floating/quantized output pipeline, not uniquely to `do_round`.

- A reported critical value of `2r−1 µm` proves the actual canonical distance lies in `[2r−1, 2r)`. It proves a positive shortfall of at most one micrometre, not exactly one micrometre.

For shipped integrity: **public contract safety is intact**. The material validator still runs on both branches, and every P0 row exceeds 5.0 mm: [general_fast.rs:3854](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:3854), [kernel README:233](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/round-envelope-kernel/README.md:233).

That is not quite the end:

- The internal 0.002 mm allowance is not enforced to its ideal continuous meaning.
- Any soundness proof relying on implemented `offset_miter ⊇ P⊕disc` needs its discretization slack audited.
- The constructor disk certificate survives this particular finding: it already budgets 1.916 µm, erodes 5 µm, and requires 20 µm penetration: [constructor certificate:43](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/constructor-inner-certificate/README.md:43).

### Gate mechanics

- Linear interpolation is not a reliable observation of an anytime curve: best-so-far depth is a staircase, not a linear response. The `8/12` wins are model-generated. The measured-only reading is `0/12` at three budgets, and equal work is exactly zero everywhere: [gate README:226](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/round-envelope-gate/README.md:226), [gate README:665](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/round-envelope-gate/README.md:665). This cannot create a false promotion; it makes the rejection stronger.

- The example-only environment door is acceptable experimental plumbing: strict parsing, refusal when unsupported, explicit output tagging, and one-request process semantics are all present: [general_request_benchmark.rs:317](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/examples/general_request_benchmark.rs:317).

- The underlying process-global atomic is not promotable in a concurrent library. Two simultaneous coordinator calls can overwrite and restore one another’s authority; memory ordering does not solve ownership: [round_envelope.rs:725](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/round_envelope.rs:725), [portfolio.rs:5013](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:5013). Production needs a request-scoped validation policy.

- Per-layout union is a coherent set union: “all round rows pass” OR “all miter rows pass.” It is not a defect. But it is not a certified round surface; `exclusive` is the round oracle, while `union` retains every miter permissiveness.

- There is also a semantic confound: when round refuses and miter admits, Union returns the **round metrics**, not the accepting authority’s metrics: [general_fast.rs:3877](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:3877), [general_fast.rs:4024](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:4024). The constructor depth subsequently drives coordinator clamping. Thus the from-request anytime A/B changes acceptance, validation cost, and an internal metric basis simultaneously. Its −2.135 mm median cannot be attributed to released legality.

I found no raw-number contradiction between the JSON and README. The defects are interpretation and scope, not evidence tampering.

## 2. What 48/48 actually proves

My reduction of `matched.json` for the 48 miter cells is:

- 44,710 confirmations attempted
- 44,710 accepted
- 0 refused
- 149,762 skipped as proxy-infeasible

The fields are captured at [matched.json:43](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/round-envelope-gate/evidence/matched.json:43), and the skip occurs before exact confirmation at [general_relaxed.rs:7489](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:7489).

So I accept this narrower conclusion:

> On these 12 parents, in mode 34, no round-valid/miter-invalid state reached the confirmation call. Any exploitable difference must occur before that call.

I do **not** accept:

- “The two authorities never disagreed.” Union does not record the two component verdicts. Kernel-refuse/miter-accept disagreements are invisible—and already known from the exclusive-parent failures.
- “The residual is 100% reachability” globally. It may instead be absence of useful near-join topology in these parents, the fixed operator trajectory, or the shipping radius.
- “Legality is solved.” Sparrow is round-valid only at `r=2.500`; the shipping `r=2.502` round kernel itself refuses two pairs: [gate README:478](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/round-envelope-gate/README.md:478).
- “Economics are solved.” Confirmation is 0.5216×, but a whole slice is only 0.9313×: [gate README:257](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/round-envelope-gate/README.md:257). The proposed round search representation has not been priced.

The current proposal geometry remains miter: constructor collision polygons are offset at [general_fast.rs:416](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:416), and relaxed surrogates transform then offset at [general_relaxed.rs:18142](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:18142). Moreover, the existing kernel seam still fixes the shape representation to `OrientedSurrogate`: [kernel/mod.rs:65](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/kernel/mod.rs:65).

## 3. What I recommend, ranked

### 1. Keep it experimental and stop the promotion

Keep the feature-off code; do not make Union default. Rename the surfaces accurately:

- `Exclusive`: canonical round correctness oracle.
- `Union`: backward-compatible hybrid experiment.

Correct the `do_round`, exact-critical-distance, cross-platform, and zero-false-accept wording. Replace the global arming before any production use.

This is the only action directly licensed by the precommitted gate.

### 2. If the owner buys one final cheap falsifier: shadow the skipped states

Before round-native search surgery, shadow-classify the 149,762 `proxy_feasible=false` states without altering the trajectory:

- round-exclusive verdict at `r=2.502` and `r=2.500`;
- untouched contract verdict;
- raw source depth;
- miter verdict;
- parent and first depth-setting step.

Kill the entire round-search direction if:

- fewer than 8/12 parents expose any contract-valid, round-valid, miter-proxy-infeasible state with at least 1 mm immediate gain;
- median best gain is below 1 mm;
- all useful states exist only at `r=2.500` while the product retains `r=2.502`;
- projected extra confirmation cost consumes the gain.

This is S effort, roughly one or two days, and directly measures the unobserved population instead of rerunning another scheduler.

### 3. Resolve allowance as a product policy—not with per-row union

If 5.0 mm is the sole immutable requirement, the coherent policy is `round-exclusive, r=2.500`, with the constructor/search rebuilt to match it. Removing the 0.002 allowance is a product decision, not an optimization refactor.

I would reject per-row union. It admits Sparrow at `r=2.502` precisely by selecting whichever authority is permissive on each row; it corresponds to no single envelope and does not enforce a 2.502 mm disc. It may remain contract-safe, but only as an explicitly specified third product authority.

### 4. Round-native proposal geometry—only after the shadow gate passes

Start with mode 34 only. A plausible proxy is a permissive material-distance representation—segment capsules/disc-expanded cells with a cheap spatial index—followed by this exact round kernel. Do not port the whole engine first.

Sizing:

- mode-34-only representation/query prototype: L, approximately 1–2 engineer-weeks;
- constructor, catalogue, NFP/contact generation, scoring/tracker, and all operators: XL, multiple PRs and several additional weeks.

Kill it unless:

- the proxy never rejects a round-valid shadow state;
- equal-work quality survives;
- equal operator wall gives ≥8/12 wins and ≥1 mm median;
- from-request 10-second testing produces a stable improvement, not the current ±10 mm basin lottery.

For the 150 program, I would require about **5 mm robust median improvement at 10 seconds**, plus transfer to at least two non-mixed instances, before authorizing the full migration. A 0.1–1 mm effect does not pay an 18.3 mm gap.

### 5. Do not reopen contact-block as an engineering track

The new authority does not kill it logically a priori: its proposal population differs from mode 34, and some of its miter-refused scales might become round-valid. But Union changes only the terminal validator; its contact model still uses zero-margin miter `EnvelopePair` rows.

The existing economics are crushing: 0.0438 mm versus m34’s 1.1044 mm, 1/12 wins, and a 0.003 efficiency ratio: [contact-block README:188](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/contact-block/README.md:188). Even the optimistic retracted contract-only arm reached only 0.506 mm: [contact-block README:85](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/contact-block/README.md:85).

A zero-code-cost 12-parent rerun under Union is defensible as a diagnostic. Kill immediately unless it beats m34 on the original ≥8/12 and ≥1 mm gate. Do not redesign the operator.

Bottom line: exact round legality at `r=2.500` is demonstrated; shipping legality at `r=2.502`, useful proposal reachability, and candidate-loop economics are not. Until the skipped-state shadow proves a millimetre-class opportunity, the honest decision is to stop this route to 150@10s.
