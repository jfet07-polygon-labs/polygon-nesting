# Sol review 18 — the strike predicate (2026-08-23)

Implementation review of CutCloseRelocate round 1 (merged 2aaf681), mandated
by the owner. Same verdict as Grok 13, independently: (A) — mod.rs:768
resets since_improvement on every new minimum; the frozen spec requires 2%;
the existing test is a false green (checks literals, never the transition).
Verbatim.

---

Verdict: **A — line-level defect found.** The 0/9 measurement remains a valid gate FAIL, but the declaration “the member closes” does not survive review. The frozen failure license grants exactly one repair and one unchanged rerun.

## P0 — strike semantics are wrong

The implementation resets `since_improvement` on **every** new raw-Φ minimum:

[overlap_ics/mod.rs:768](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:768)

```rust
if totals.raw < min_raw {
    min_raw = totals.raw;
    snapshot.clone_from(&self.state);
    since_improvement = 0;
}
```

The frozen spec says a reset requires a **≥2%** improvement. Sparrow implements exactly that: a sub-2% new minimum updates the snapshot but does not reset the counter. [separator.rs:102](/var/lib/t3/tmp/sparrow-bench/src/optimizer/separator.rs:102)

The existing test is a false green: it checks only `200/3`, `100/5`, and the literal `0.98`; it never exercises the transition rule. [tests.rs:2644](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/tests.rs:2644)

This is materially aligned with the cliff:

- Bite 22 accumulates 797–1,754 iterations in the stuck 10-second cells but generally 0–1 strikes.
- A trickle of microscopic new minima can therefore keep one `separate()` alive until the phase deadline.
- The pool/disrupt path only runs after `separate()` returns, so the operator intended to cross the shelf is silently starved. The committed evidence itself notes that most bite-22 runs terminate on deadline before disruption. [README.md:605](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/cutclose-round1/README.md:605)

Minimal correction:

```rust
if totals.raw < min_raw {
    let substantial =
        totals.raw < STRIKE_IMPROVEMENT_RATIO * min_raw;
    min_raw = totals.raw;
    snapshot.clone_from(&self.state);
    if substantial {
        since_improvement = 0;
    }
} else {
    since_improvement += 1;
}
```

A sub-2% new minimum must neither reset nor increment, matching the source.

Red/green vector: feed the state machine repeated blocks of nine non-minima followed by a 0.01%-better minimum. Current HEAD never reaches 200 because it resets every block; the corrected implementation reaches a strike after 200 non-minimum observations. Also assert that a single >2% improvement resets the counter. The test must call the same helper used by `separate()`, not duplicate the rule.

## 1. The top-clearance suspicion is refuted

There is no new clearance split.

- Physical sheet edges use `edge+sag`; the depth top uses sag-less `edge`. [state.rs:169](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/state.rs:169)
- The strip row is evaluated at `T-depth_top_inset`. [broad_phase.rs:46](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/broad_phase.rs:46)
- Proxy depth is the same transformed-ring maximum plus the same edge inset. [state.rs:491](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/state.rs:491)

When the target strip binds:

\[
\max g_{\text{top}}=\max(0,\text{proxyDepth}-T).
\]

Therefore `max_g ≤ 0.004` and `proxyDepth > T` are perfectly compatible whenever the overshoot is between 0 and 4 µm. That is the mechanical explanation for the 53 events. They are not evidence of mismatched insets.

There is, however, a diagnostics overclaim: `exact_attempts` increments before `publish::attempt`, while the publisher rejects over-target poses before incrementing `exactCheckpoints`. [mod.rs:776](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:776), [publish.rs:264](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:264)

Those 53 are “publication-preflight entries,” not exact validations. The statement that the funnel proves 100% exact-attempt conversion is consequently too strong. Rename or split the counters later; this does not itself license a trajectory rerun.

## 2. What bite 22 is

The currently demonstrated operational binder is **failure-transition starvation**, not exact publication and not raw evaluation speed.

- Weight reset at a new width is correct and matches Sparrow’s tracker rebuild. [mod.rs:912](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:912), [separator.rs:240](/var/lib/t3/tmp/sparrow-bench/src/optimizer/separator.rs:240)
- The exact judge is not the common binder: stuck cells retain positive raw Φ; they generally never reach an exact scan.
- Worker streams are not proven identical: the FAST merge vector was contested 9/9 with four winning ordinals. Wall runs disable fingerprints, so diversity specifically at bite 22 remains unmeasured.
- A structural shelf near 179 mm is plausible, but the committed evidence does not contain active-row/piece censuses sufficient to name its geometric topology.

The implementation does re-enter `separate()` after pool restore and disruption, with fresh local strike counters. [mod.rs:936](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:936) There is no counter leak between attempts. The defect is the opposite: the counter inside one separation is forgiven by every microscopic minimum, so the first separation often lasts until deadline.

One fidelity difference remains declared: Sparrow rebuilds the tracker when restoring a pool solution; ours restores the pooled entry’s saved weights. [explore.rs:78](/var/lib/t3/tmp/sparrow-bench/src/optimizer/explore.rs:78), [mod.rs:1528](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1528). That is frozen and cannot be bundled into this repair. It also cannot explain cells where the pool path never ran.

## 3. The 30-second result

The 30-second column cannot change the 10-second FAIL. It does show that this member can reach a strong basin: five seeds publish 163.69–165.06. [README.md:187](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/cutclose-round1/README.md:187)

But “tight band above the bar ⇒ throughput, not basin” is not satisfied:

- At 10 seconds the range is 169.00–179.08: about **10.08 mm**, plainly not tight.
- At 30 seconds it remains bimodal: three seeds near 179, one at 168.66, five around 164–165.
- That is a heavy-tailed **time-to-escape** distribution, not ordinary leaf throughput. The README simultaneously calls it a basin barrier and “throughput, not basin”; those claims conflict. [README.md:315](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/cutclose-round1/README.md:315)

Thus the 30-second fact grants nothing by itself. The strike defect independently grants the one rerun.

## General fidelity

The rest is substantially faithful:

- 25+50 samples, three finalists, two-stage CD, accept-equal, and unconditional best-sample installation are present. [relocate.rs:696](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/relocate.rs:696)
- GLS multipliers/order are correct. [energy.rs:384](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:384)
- Centre cut and far-side-only Y translation are correct. [homotopy.rs:158](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/homotopy.rs:158)
- Tournament selection and ordinal merge are deterministic.
- `incident_gradient` has one diagnostic/corpus caller and no live optimizer route; acceptable. [corpus.rs:37](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/corpus.rs:37)
- `relocate_eval_budget` defaults inactive and is confined to the locked-work harness; acceptable.

Two non-gating risks should be recorded, not folded into the licensed repair:

1. The implementation spawns eight OS threads and clones eight states on every master iteration, whereas Sparrow uses a persistent Rayon pool. [mod.rs:646](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:646), [separator.rs:52](/var/lib/t3/tmp/sparrow-bench/src/optimizer/separator.rs:52). That wall tax was not isolated by FAST. Measure it later; do not combine its optimization with the strike rerun.

2. The raw 27 cell documents containing per-bite schedules were not committed. `wall.py` reduces them to aggregate fields and drops `outcome.bites`. [wall.py:71](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/drivers/wall.py:71) Consequently the README’s 922/53/strike statements cannot be reconstructed from committed `wall.json`. Commit the raw bite rows—or a lossless per-bite extract—in the rerun.

## Recommended ruling

**A — repair and rerun licensed.**

Do exactly one semantic change: correct the 2% strike-reset transition. Add the red/green state-machine vector, correct the provenance/test claim that strike semantics were already covered, retain the raw bite evidence, then rerun the identical 9-seed 10-second gate and unchanged floor. No publication change, no sampler change, no thread-pool optimization, no weight-policy change, no bite retuning.

If that corrected rerun still produces fewer than 3/9 at ≤168.484 with the floor green, the member closes under §0. The 164-at-30s result remains valuable owner-facing evidence, but not a rescue of the 10-second target.
