# Selection rule between p = 1 and p = 0.75 (frozen before the 54 additional development cells)

Registered 2026-09-07 10:20 UTC, before any p = 0.75 cell of the development screen ran. The
rule is GPT-6 Astra's (review 6, Q12, `docs/astra-review-6-the-live-exponent.md`), copied here
so that the cells are scored against a text that predates them.

**The added arm.** `exp1-C`: `--proxymargin=8 --guidedexponent=0.75`, the same frozen live
executable and configuration as the existing arms A (`--proxymargin=8`) and B (`--proxymargin=8
--guidedexponent=1`): `/var/lib/t3/tmp/astra/frozen-f856d19`, sha256 82c6d5b3ccc9…, Legacy cap
50, Wall10s unbounded, seeds 18–26, three repetitions, one fresh eight-worker process per cell
under the bench lock, ten seconds from the bare request. The arm is measured later than A and
B and shares their control cells; this is acknowledged as a development-selection convenience,
not a contemporaneous three-arm experiment. p = 0.5 stays outside the selection.

**Eligibility.** An exponent is eligible only if it passes, on both profiles, the declared
development requirements against the margin-8 control (per-seed repetition medians): positive
paired median depth gain; no per-seed-median regression greater than 1.000 mm (unrounded);
zero invalid publications; and the engineering requirement in Astra's corrected accounting:
lower total evaluations per cell, lower exploration evaluations and lower exploration elapsed
time per published explore bite (failed and deadline-truncated bites charged in the numerators
and counted as opportunities), no reduction in published explore bites per cell or in their
publication fraction.

**Selection.**
- Exactly one eligible exponent: select it.
- Neither eligible: do not proceed to validation.
- Both eligible: with `H_{h,s} = D_{1,h,s} − D_{0.75,h,s}` (per-seed repetition medians of the
  final valid depth; positive means p = 0.75 is deeper), select **p = 0.75 only if all three
  hold**: Legacy median H > 1.000 mm; Wall10s median H ≥ 0; no seed on either profile with
  H < −1.000 mm. Otherwise select p = 1.

One p is selected for both profiles. The 1.000 mm hurdle is a declared practical preference,
not a statistical threshold.

**Two archives.** The as-run archive (the 101 cells kept plus the seven originals in
`screen/perturbed-originals/`) is primary; the selection must agree when computed on the
replacement archive (the 108 cells in `screen/`). If they disagree, retain p = 1 provided it is
eligible under both; otherwise select neither.

**Contamination.** Defined from the independently logged load samples
(`/var/lib/t3/tmp/astra/screen/load-exp1.log`, one sample every 5 s): a C cell whose ten-second
window contains a 1-minute load average above 10.0 is re-run whole after the arm completes,
its original kept beside it; no cell is excluded or re-run for any other reason, and never
because its work count looks low. Builds by other agents run under the bench lock and cannot
overlap a cell.

**What is not decided here.** The prospective specification `ICS-guided-exponent-v1` (seeds
27–35) is written after the selection and names the selected p; nothing in this rule is
changed after the C cells are read.

**Applied, 2026-09-07 10:06 UTC, before any C cell was scored.** The arm ran 09:54:13–10:03:16
UTC with the sampler live throughout (112 samples in the window). Three C cells contain a
sample above 10.0 (legacy-r1-s18: 11.42; legacy-r1-s19: 10.32; wall10s-r0-s26: 11.78); they are
kept in `screen/exp1-C-contaminated-originals/` and re-run whole with the same script and
binary. No other cell is touched.
