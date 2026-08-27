# The T-row repair — immutable specification

**Status: signed by two of three, after one amendment.** Sol returned
`CONFIRM` on sha256 `06054148...`; Grok returned a single `AMEND` on §4, which is
folded in below and recorded there with its reasoning. The amendment was made
before any measurement was taken and repairs a clause that would have failed a
complete success; it is not a clause moved to rescue a miss. **Two of three. Sol (Codex, `model_reasoning_effort="max"`)
and Grok (`--reasoning-effort xhigh`) converged after one cross-exchange, on
the same mechanism, having entered it with two different ones. ox-alpha could
not vote: the provider returns `UnknownError` on a two-word prompt via both
`opencode-go/ox-alpha-free` and `opencode/ox-alpha-free` while another model on
the same client answers immediately, so the outage is provider-side and
verified rather than assumed.**

Reviews: [`sol-review-21-bite22-diagnosis.md`](sol-review-21-bite22-diagnosis.md)
and [`grok-review-16-bite22-diagnosis.md`](grok-review-16-bite22-diagnosis.md),
both with their round-2 cross-exchange appended verbatim.
Evidence: [`experiments/overlap-ics/bite22-microscope/`](experiments/overlap-ics/bite22-microscope/).

This file is frozen at its committed sha256. Nothing below may be amended to
rescue a miss.

---

## §0 What is being tested, and against which bar

The deterministic-30s round passes the 30-second gate at median
`162.94241 mm`, 7/9 at or below `168.484 mm`, and permanently retired the
ten-second quality gate on a 2/9 miss at median `179.07170 mm`.

The microscope establishes that the ten-second median is decided at explore
bite 22, that a frozen seed reaches the publication band thousands of times
without ever calling the exact authority, and that the refusal is
`proxy_depth > target_depth_mm` in essentially every case — by a hair to four
micrometres, two to three on the frozen tail — on layouts that are 0.175 to
0.180 mm better than the incumbent, a full bite of real progress.

**This specification is aimed at the 30-second tail, not at reopening the
ten-second gate.** Both reviewers refuse the reopening on the same arithmetic:
about 62 further publications are needed from the bite-21 parent to reach
`168.484 mm`; seeds 0 and 6 already clear bite 22 at ten seconds and finish at
`170.44` and `169.35`; a repair first offered after 1,100–1,600 iterations can
succeed causally and still leave no clock for the cascade. The two seeds this
mechanism exists for are **7 and 8** — the two that hold Round 4 at 7/9 instead
of 9/9, and the two with thousands of refusals on the lip.

The ten-second column is run and reported against its old unsoftened numbers.
It carries no verdict and does not reopen the retired gate.

## §1 The mechanism, in one sentence

When `max_g <= 4 um` and `0 < proxy_depth - T <= 4 um`, clone the state and run
the **existing** deterministic frozen-θ, `<= 4n`-row, `<= 16 um`-per-piece
Gauss–Seidel micro-repair with the **locked-strip top injected as a repair
row**, rechecking that row and every exact pair and boundary row after each
correction; publish only if `raw_depth <= T`, Exclusive `r = 2.500` and the
untouched contract validator all pass.

It is **not** a deletion of `publish.rs:364` and **not** partial-bite
acceptance. The exact scan already checks the physical sheet rather than the
internal target, and the final target check stays strict. The nominal 0.1 %
bite and the exact 5.0/5.0 contract are unchanged.

## §2 Frozen — no clause below may move to rescue a miss

`mod.rs:2002`'s `None => break` (both reviewers ruled it faithful to
Algorithm 12); the 80/20 explore/compress split; `200 / 3 / 100 / 5 / 0.98`;
`EXPLORE_SHRINK_STEP = 0.001`; `COMPRESS_SHRINK_RANGE`; the sample counts;
`workers = 8`; the relocate operator; GLS; the constructor; the 4 um band; the
16 um displacement cap; the `4n` row cap; `KernelMode::Exclusive` at
`r = 2.500`; `validate_placements_against_contract`; the pool-retry tracker
rebase (closed); the conflict-cluster budget (closed); minimum-conflict binary
close (closed).

**The post-cut far-side entry sweep is deferred, not refuted.** Grok withdrew
it after the census showed the lip has thousands of witnesses and his own
400-iteration probe measures only the early entry transient. If Gate 0 fails
because eligible states never *arrive*, that is a separately funded
specification about entry — it is **not** an automatic rescue of this gate and
may not be appended to it.

## §3 Gate 0 — mechanical, and allowed to say NO before any quality battery

**Setup.** mixed-61, seeds `0..=8`, `--orders=1 --workers=8 --edge=5 --pair=5`,
the 5.0/5.0 exact-clearance contract. Reproduce the Round-4 composed
deterministic **ten-second** trajectory. Immediately after bite 22 is
constructed — exact bite-21 parent installed, centre cut applied, target and
weights reset — fork the **complete continuation state**, including pacer
charge and stream ordinals. A pose-only parent restart is invalid: it is a
different RNG and GLS continuation. Run only the remaining explore allocation.

Gate 0 deliberately uses the **ten-second** residual, not the thirty-second
one: at a 30 s residual the control already closes seeds 1, 4 and 5 on its own,
which would make an ordinary "3 of 5" clause vacuous. The ten-second residual
is where control and treatment actually differ on the frozen set.

**Arms.** `Control` (today's `attempt()`), `TRepair`, and `ComputeIgnore` —
which performs the same repair on a detached clone, discards the result, and
keeps its counters out of the production work vector.

**Instrument validity, checked before any clause.** The contemporaneous control
must reproduce the partition: closes bite 22 on `{0, 2, 3, 6}`, does not close
it on `{1, 4, 5, 7, 8}`. If the partition does not reproduce, the run is an
invalid instrument and not a mechanism result.

**PASS requires all seven:**

1. **Per-bite census integrity.** At bite 22 every band entry partitions
   exactly into digest-repeat, above-target, non-improving, or exact-called.
   Counts are deltas for that bite. Whole-run totals are not a clause.
2. **Unique install.** Every fresh eligible digest — band-valid, improving, and
   rejected *only* because `0 < proxy_depth - T <= 4 um` — invokes the T-repair
   exactly once. Repeated identical digests are logged and skipped. Zero
   invocations on a seed whose control census records `aboveTarget >= 1` at
   bite 22 is a wiring failure and an `AUTOFAIL`.
3. **Tail-relevant conversion.** `TRepair` publishes bite 22 for **both** seeds
   `{7, 8}` **and** at least one of `{1, 4, 5}`. At least 3 of 5, with the two
   persistent thirty-second tail seeds mandatory.
4. **Causal witness.** Every counted conversion begins at a state `Control`
   would reject solely at `publish.rs:364`, and processes at least one
   synthetic T-row. A publication that went through today's `proxy_depth <= T`
   path does not count.
5. **No reverse.** All of `{0, 2, 3, 6}` retain their bite-22 closure under the
   identical residual cap, and none finishes worse than its control.
6. **Authority and caps.** Every resulting publication has `raw_depth <= T`,
   frozen θ, `<= 16 um` cumulative displacement per piece, `<= 4n` corrections,
   Exclusive `r = 2.500`, untouched contract validity, and independent
   revalidation. Zero invalid publications.
7. **Isolation, cost, determinism.** `ComputeIgnore` is bit-identical to
   `Control` in poses, publications, fingerprints, base work vector and pacer
   state after stripping shadow diagnostics, and keeps at least 95 % of
   `Control`'s paired base-trajectory rate. `Control` and `TRepair` are each
   two-process bit-identical. The default build is unchanged and the four
   pinned gates plus the FAST and soundness batteries stay green.

**Pre-declared FAIL, and then closed.** Either seed 7 or seed 8 does not
convert; fewer than 3 of 5 convert; an eligible digest bypasses the repair; any
reverse; any authority or cap violation; isolation divergence; more than 5 %
cost; any nondeterminism. Reading: the depth lip is not legalizable with the
existing frozen-θ repair. No quality battery runs. **"Too few unique eligible
states" is also a valid miss** — it is not permission to append the far-side
sweep, and not permission to widen the 4 um window.

## §4 The quality battery — only after Gate 0 passes

A from-request, contemporaneously paired, deterministic **thirty-second**
battery. PASS requires all of:

- median `<= 163.00461 mm`;
- at least 7 of 9 at or below `168.484 mm`;
- paired median gain `>= 1.000 mm` over the Round-4 frozen-member control (the
  standing pairing), **not** over T-repair-off;
- **seeds 7 and 8 individually at or below `168.484 mm`** — a causal tail
  tightening, not a softening;
- zero invalid publications, every publication independently revalidated, all
  plan and charge identities green.

The first three and the last are the standing Round-4 clauses, unchanged. The
fourth is added because it is the clause this mechanism claims to move.

**Amendment, Grok's ballot, folded in before any measurement was taken.** The
first draft of this section demanded the 1 mm paired median gain *over
T-repair-off*, and that clause was unsound as a test of this mechanism. Seeds 7
and 8 are the eighth and ninth of nine. Converting both to `168.484 mm` — the
movement §0 and §4 themselves name as the claim — changes the sorted list only
at positions 8 and 9 and leaves the median at the current fifth value,
`162.94241 mm`, for a paired median gain of **0.000 mm**. The gate would have
failed an honest and complete success. The standing 1 mm is composed against
the frozen member and already reads `1.23247 mm` at this median; asking it again
of the repair against itself-off tests a median promise that neither the tail
clause nor the ten-second-residual Gate 0 makes. This is a soundness repair made
before any number existed, not a clause moved to rescue a miss.

The 3-, 10- and 60-second columns are run and reported. They carry no verdict.

**If the thirty-second gate misses, the T-row repair closes.** It does not
reopen the entry sweep, the patience constants, the tracker rebase, or the
retired ten-second chase.

## §5 Numbers this specification was written against

From the microscope, so a later reader can see what was known when it was
signed:

- 30 s, seed 7: **5,344** above-target refusals carrying **4,300 distinct pose
  digests** (1.2x repeat ratio) — the opportunities are real, not one state
  revisited;
- 30 s, seed 7: **1.4** pieces within 1 um of the layout's deepest point and
  2.9 within 4 um, over 154 samples — the front is one or two pieces, against
  `4n` rows and a 16 um cap;
- the lip on the frozen tail sits at **2–3 um**, with seed 7 putting 3 of 5,174
  refusals below 1.5 um: a stable overhang, not noise;
- 30 s, seed 7: **5,235** band entries, **5,212** refused above target
  (99.6 %), and still 21 explore bites.

The last of these is the whole reason to fund it, and the third is the reason
it may still fail: an overhang that reproducible may be one the repair cannot
legally push. Gate 0 answers that, and nothing else does.
