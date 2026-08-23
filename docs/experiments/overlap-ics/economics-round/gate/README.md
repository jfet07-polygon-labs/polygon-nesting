# Wave 4 — the evidence wave, and where it stopped

**The verdict is a stop, and the stop is the spec's own.** Funded change 3
ends with a pre-committed reject rule:

> B/E/R/D from timing-only microbenchmarks on all three fixtures, conservative
> rounding; **REJECT the currency if wall-prediction error >10 % on any
> transfer fixture.**

It fired. On three independent runs on a quiet box, on both the five-term
currency `U1` and the degenerate `U0` the shipped pacer actually spends, on all
six ordered fixture pairs, and — the reading that matters most, because it
survives every objection to the thinnest fixture — on the two heavy fixtures
alone. **`CURRENCY_ACCEPTED: false`, exit 1, every time.**

§0's gate is a *10-second calibrated-work* plan. A calibrated-work plan is
denominated in this currency. So there is no denomination in which to spend
the gate's budget, and **wave 4 produced no §0 number at all** — not a
depth, not a quorum, not a p95. The six PASS clauses are unanswered, and
`../README.md` §0 carries the gate text verbatim above an empty table rather
than beneath a number that was measured in a currency the spec had already
refused.

| | |
|---|---|
| base | `6e9c2e5` · waves 1–3 at `c6de729` / `883b297` / `21ccc2e` |
| FAST union | **FAILURES=0, exit 0** — 8 numbered stages, 16 green checks, 0 failed; first-bite canary `CANARY_PASS: true` |
| §0 verbatim | `SECTION0_VERBATIM: true`, 13 lines, 779 bytes, exit 0 |
| currency reject check | **REJECTED, exit 1 × 3 runs** |
| persistent executor | not built (wave 1's census: 5.082 % vs a 10.000 % bar) → nothing to promote |
| §0 two-arm gate | **NOT RUN** |

---

## 1. The reject check, and why one run would not have been enough

`meter/currency.py` measures one reading. `gate/rejectgate.py` reduces three of
them, because the rule is a **maximum over six ordered pairs** and a maximum is
exactly the statistic a lucky run flatters. Three questions, three answers:

**Does it reject on every run?** Yes, for both currencies, and not narrowly:
**all six ordered pairs are over the bar in all six currency-runs.** The
worst-pair column below is the maximum the rule is written against; the pair it
belongs to is named beside it, because the *first* pair to fail is
mixed-61 → shapes-17 in every case and quoting a triangle-20 magnitude under a
shapes-17 label would be the easiest mistake in this table to make.

| currency | run 1 | run 2 | run 3 | worst pair | pairs over the bar |
|---|---:|---:|---:|---|---:|
| `U1-weighted-vector` | 236.90 % | 280.95 % | 300.52 % | mixed-61 → triangle-20 | 6 of 6 |
| `U0-sample-evaluations` | 213.40 % | 256.02 % | 277.80 % | shapes-17 → triangle-20 | 6 of 6 |

**Does it still reject with triangle-20 removed?** Yes — and this is the row
the stop rests on. triangle-20's cell is five master batches and five to six
*milliseconds* of search, and a reader is entitled to say a ratio built on six
milliseconds is scheduler noise rather than a statement about a currency. That
objection deserves a number, not a paragraph. So the same clause is re-applied
to mixed-61 and shapes-17 alone — the pair the spec's transfer story is
actually about, both with walls in the hundreds of milliseconds to seconds:

| pair | `U1` run 1 / 2 / 3 | `U0` run 1 / 2 / 3 |
|---|---:|---:|
| mixed-61 → shapes-17 | 11.70 % · 11.84 % · 13.12 % | 14.76 % · 15.45 % · 13.63 % |
| shapes-17 → mixed-61 | 10.48 % · 10.59 % · 11.60 % | 17.31 % · 18.28 % · 15.77 % |

**Twelve readings, two currencies, two directions, three runs — every one over
the 10.000 % bar.** There is no direction, no currency and no run in which the
heavy pair transfers.

**Is the design matrix stable, so that only the response moved?** Yes, and this
is the precondition that makes the three runs repetitions rather than three
different experiments. The cells are `--mode=fixed`, so the five counters are a
deterministic function of request and seed. They are bit-identical across all
three runs — and across **wave 2b's committed cells too**, measured in a
different worktree by a differently built profiling binary:

| fixture | sampleEvaluations | masterBatches | exactCalls | repairRows | disruptionMoves |
|---|---:|---:|---:|---:|---:|
| mixed-61 | 6,977,140 | 514 | 50 | 20 | 8 |
| shapes-17 | 1,418,260 | 840 | 0 | 0 | 4 |
| triangle-20 | 45,364 | 5 | 34 | 0 | 0 |

So four independent readings of this rule exist — wave 2b's one and wave 4's
three — over identical counters, and all four reject. Only the seconds moved:
mixed-61 2.5647 → 2.5641 / 2.5683 / 2.5671 s, shapes-17 0.5812 → 0.6114 /
0.6175 / 0.6041 s.

### What the check also says, which is not a clause

`coefficientSpread` decides nothing and is reported because a reader about to
be told "the currency is rejected" should be able to see which of its terms the
cells were too thin to price at all:

| term | run 1 | run 2 | run 3 | max/min |
|---|---:|---:|---:|---:|
| `B` master batch | 630 | 657 | 628 | 1.05× |
| `D` disruption move | 1,052 | 1,008 | 1,162 | 1.15× |
| `E` publication call | 340 | 346 | 293 | 1.18× |
| **`R` repair row** | **19** | **85** | **131** | **6.89×** |

`R` is not a price. The three cells together contain **20 repair rows**, all of
them on mixed-61 and none on either other fixture, so `R` is fitted to a
residual with almost no support and moves by a factor of seven between
repetitions of its own calibration. `B`, `E` and `D` are stable to within a
fifth. This is a fact about the cells, not a defect in the harness, and it is
*not* the reason the currency is rejected — the rejection survives at `U0`,
where all four coefficients are zero.

### The shape of the failure, in one sentence

`U` has **no per-bite term**. triangle-20's 34 bites publish in zero or one
master iteration each, so nearly all of its search is per-bite work — the cut,
the pose install, the commit, the row rebuild — which `U` prices at zero;
and mixed-61 and shapes-17 differ enough in bites-per-batch that even the heavy
pair misses by 10–18 %. Adding a per-bite term is **a different currency**, and
therefore a different proposal: the spec froze `U`'s five terms and the
no-second-guess discipline that covers the strike quanta covers this too. Wave
4 does not retune, does not add a term, and does not choose different cells.

---

## 2. What was NOT run, and why that is the finding rather than an omission

Everything below §0's first line. In order:

- **the nine 10 s calibrated-work cells, both arms, two processes each** — the
  six PASS clauses;
- **the 5 × 9 p95 wall repetitions** — clause (5);
- **the 30 s clauses on both arms**, the 60 s report, the 3/10/30 curves;
- **the interleaved AB/BA old-wall-arm control cells**;
- **the attribution clause**, and therefore any decision about promoting the
  impatient policy.

The two-arm strike experiment is *built, wired and reachable* — wave 3's
`armgate.py` proved the control arm bit-identical to the round's base binary on
four cells, and a Rust vector proves the treatment changes a trajectory at a
quantum sized to fire. **It is not measured.** Attribution is undecided, so by
the spec's own promotion clause the impatient policy is **not promoted** and
the control's frozen literals `200/3/100/5/0.98` remain the member — which is
the same outcome a failed attribution would have produced, reached by a
different route and with less evidence behind it.

### The strongest argument against this stop, and why it does not carry

It deserves to be written down rather than left for someone else to think of.

> *The reject rule is about **transfer**. The gate calibrates on mixed-61 and
> spends on mixed-61, so no transfer occurs. Quality is deterministic in
> work-space whatever the exchange rate is. The gate would have produced a real
> quality verdict; refusing to run it discards nine seeds of evidence over a
> clause that does not bind them.*

Three answers, in increasing order of weight.

1. The clause does not say "reject the currency for cross-fixture use". It says
   **REJECT the currency**, and a rejected currency is not available to
   denominate anything.
2. §0 clause (5) — *quiet-box p95 ≤10.000 s over 5×9* — is precisely the clause
   that binds calibrated **work** to **seconds**. That binding is the only
   thing the currency is for, and the reject rule is the only thing that tests
   it. With the currency rejected I could still measure a p95, but I could not
   call the plan I spent a *ten-second* plan, so clause (5) would be a number
   without a claim attached.
3. ox-alpha pre-named this exact failure, ranked fourth, before any of it was
   built: *"currency weights mispriced >10 % on a transfer fixture — **depth
   stays deterministic while the wall promise silently dies** (caught only by
   the reject rule; **do not skip it**)."* The counter-argument's first sentence
   is the defect's own description. Running the gate anyway is what the defect
   looks like from the inside, and Sol review 19 §5 names the alternative:
   *"the calibration fails rather than silently inventing another exchange
   rate."*

**What would unblock it**, stated so nobody has to guess: a currency that
passes its own reject rule. On this evidence that needs a per-bite term, which
is a change to `U`'s five frozen terms — a **different proposal**, requiring
its own signatures. It is not a retune and wave 4 has no licence to attempt it.

### The failure license was not invoked

§0: *"one named line-level defect with red/green vector → one identical rerun;
a valid miss closes this funding."* No line-level defect was found. The reject
rule fired on a genuine measurement, three times, over a bit-identical design
matrix — a rerun is exactly what was already done and it changed nothing. This
is a **valid miss** on funded change 3, and by that sentence it closes that
funding rather than licensing another attempt at it.

---

## 3. The persistent executor: still nothing to promote

Wave 1's census rendered funded change 2's gate before anything downstream read
it: prep+dispatch is **5.082 %** of hard-state wall at the largest reading over
every seed that reached shelf density and both of its processes — the reading
most favourable to building — against a **10.000 %** bar.
`BUILD_PERSISTENT_EXECUTOR: false`, `"DO NOT BUILD"`.

So the promotion battery this wave was to run — ≥1,024-batch bit-identity,
≥1.15× shelf p50, ≥1.10× geomean over the three fixtures, ≤5 % any-fixture
regression, ≤10 % RSS — **has no second arm to run against**. There is no
persistent executor in the tree. The **ephemeral executor stays**, and per the
spec the 5/9 clause does **not** drop to 4/9 (Grok's refusal, unanimous; and
ox-alpha's narrow mechanically-triggered 4/9 contingency was offered for vote
and voted down).

The half of the identity clause that *does* have an executor to run on is in
FAST and green: **K = 1,741 master batches, two processes, bit-identical**, on
a cell with 2 strikes, 1 disruption and 2 failed separations.

---

## 4. The boundary HEAVY, and the tautology it turned up

Wave 4 stopped, but three waves of engine work sit between `6e9c2e5` and this
commit and a round that stops still owes the tree an answer about what it did
on the way. `gate/heavy.sh` is that answer, and it is **FAILURES=0, exit 0**
over eleven steps.

**The four pinned engine gates, on both builds.** `base` is the gate binary's
own feature set with `overlap-ics` absent (`32302ec6…`); `meas` has it compiled
and unarmed (`5aef5acd…`). `BASE_ALL_PASS: true`, `MEAS_ALL_PASS: true`, and
`WHOLE_DOCUMENT_IDENTITY: true` — all four gates identical as **whole
documents**, not merely on their four pinned scalars. g1 206.869 mm /
`8a773738…`, g2 159.09233022733062, g3 159.07876040364795, g4
164.0375677990678, each with `exactValid` and `contractValid` true.

**The five release suites**, `EXITS jagua=0 combo=0 example=0 icsstacked=0
icsalone=0`, `SUITES_PASS: true`: **1,293 / 1,357 / 20 / 1,429 / 1,239 passed,
0 failed** across 216 targets, with no rerun of the campaign's known flaky
eviction test needed on any of them.

**Two-binary determinism — and the reason there are now two documents.** The
required cell is same-source, same-feature-set, two target directories. It
passes on all five cells including this round's member, and it is
**tautological**: both builds are `2c5da1ac…`, the same bytes, so it re-proves
single-binary determinism. `determinism.py` now emits `binaryASha256`,
`binaryBSha256` and `binariesDiffer` — additive fields, no verdict depends on
them — so `evidence/determinism-two-binary.json` says
**`binariesDiffer: false`** out loud instead of letting `TWO_BINARY_IDENTICAL:
true` be read as a cross-build claim.

Then the real comparison, in `evidence/determinism-cross-featureset.json`:
`2c5da1ac…` (`overlap-ics`) against `a100542f…`
(`jagua-experimental,overlap-ics`), **genuinely different executables**,
`binariesDiffer: true`, and **all five cells — S0, S1, C175, triangle-20 and
`cutclose` — bit-identical**. That is more than the converged spec claims
(which fixes the feature set), so it is supplementary rather than required; but
it is the evidence the required cell was silently failing to provide.

Evidence: `evidence/gates.json`, `evidence/suites.json`,
`evidence/determinism-two-binary.json`,
`evidence/determinism-cross-featureset.json`, `evidence/binaries.txt`, the five
`evidence/suite-*.log`.

**One trap worth recording for whoever runs this next.** `run-suites.sh` builds
into the shared `target/`, so by the time it finishes,
`target/release/examples/overlap_ics_benchmark` has been rebuilt under
*another* feature set — it read `babe6efc…` here, not the canonical
`2c5da1ac…`. Any driver that takes `lib.BIN`'s default after a suite run is
therefore measuring a binary nobody named. Rebuild the canonical example before
trusting that path; the two determinism documents above were both produced
after doing so, and the sha fields are what make it checkable.

---

## 5. The files

| file | what |
|---|---|
| `section0.py` | re-extracts §0 from the spec by its heading and requires `../README.md`'s quoted copy to be byte-equal. Exit is the verdict. |
| `rejectgate.py` | funded change 3's reject rule over N `currency.json` runs, plus the triangle-20-excluded reading and the coefficient spread. Exit is the verdict. |
| `heavy.sh` | the boundary tier: four pinned gates × two builds, five suites, two-binary determinism. |
| `evidence/rejectgate.json` | the three runs reduced, and `SECTION0_GATE_LICENSED: false`. |
| `evidence/section0.json` | the verbatim check's document. |
| `evidence/currency-run{1,2,3}.json` | the three reject-check runs the reduction reads, committed so `rejectgate.json` names bytes that exist. |
| `evidence/determinism-two-binary.json` | the required cell — same feature set, `binariesDiffer: false`. |
| `evidence/determinism-cross-featureset.json` | the supplementary cell — two genuinely different executables, all five cells identical. |

The three per-run `currency.json` documents are the reduction's inputs and are
named by path inside `evidence/rejectgate.json`; the cells themselves are
`--mode=fixed` and reproduce from the recorded binary shas.

---

## 6. Honest caveats

- **The reject rule is a statement about seconds, and seconds are a statement
  about a box.** One machine, x86_64, 16 cores. Each run was launched only
  after `/proc/loadavg`'s one-minute figure fell below 1.00; the *driver's own*
  reading one moment later was **1.02, 0.86 and 0.92**, and it is in each
  document rather than rounded away. Run 1 is both the one that read above the
  bar and the one with the **smallest** worst error of the three, so a warm box
  did not manufacture this rejection. A different box could in principle move
  the heavy pair's 10.48 % under the bar. It would have to move it by more
  than the spread of three runs on this one, and it would not touch
  triangle-20's 200–300 %.
- **`R` is unidentified by these cells** (6.89× across repetitions, 20 repair
  rows in total). Naming that is not a licence to change the cells; it is the
  next round's finding.
- **The gate text was frozen before the number arrived**, and `section0.py`
  keeps it frozen — but the ordering is only as trustworthy as the commit
  history, and this README and §0 land in the same commit as the reject
  check's evidence. What can be checked is that the copy is byte-identical to
  the spec's, and it is.
- **triangle-20's cell is genuinely thin** (5 master batches, ~6 ms). It is
  reported at full weight because the spec says "all three fixtures", and the
  stop is then shown not to depend on it.
- **The 3 s and 60 s wall curves and the AB/BA control are not run**, so this
  round contributes nothing to the campaign's session-drift record. The last
  such reading is the pivot re-run's.
- The scheduling-order perturbation vector the FAST union names is still not
  built. It belongs to the refused executor branch, and forcing completion
  order needs test-only concurrency inside `tournament`.
- The FAST union's "eight-worker hard-shelf throughput" lives in wave 1's
  census under the `ics-profile` build, not in the FAST tier; the tier's
  `throughput` cell is still the single-worker canary ox-alpha asked to retire
  as the *only* throughput signal.
- **The two-binary determinism cell was comparing a binary with itself, and
  nothing said so.** Building the same source and the same feature set into a
  second `CARGO_TARGET_DIR` produces a byte-identical executable on this
  toolchain — `2c5da1ac…` on both sides — so `TWO_BINARY_IDENTICAL: true` was
  re-proving single-binary determinism and no more. That is a false-green of
  exactly the shape the audit's F9 names, and it was silent because the
  document recorded the two *paths* and not the two *hashes*. See §4 for what
  was done about it.
