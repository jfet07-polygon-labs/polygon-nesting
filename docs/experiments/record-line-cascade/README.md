# The record line: a sub-grid clamp, and 3.6 mm off the record

The standing record on the true 5.0/5.0 exact-clearance contract was
**159.07876040364792 mm**, and its parent was a certified fixpoint of 40 probe
arms. The from-scratch line — the same contract, the same `''` 0.0005 CLI tail,
and the standing rule that it may never import a record-line placement — stood
at 159.668 mm after the compression-schedule port.

This round moved the from-scratch line to **155.42229074464285 mm** and, in doing so,
took the absolute record with it. Every state on the way is pinned, and every
pin replays bit-exactly through modes 27 and 30 on the pristine default-feature
binary.

The lever is one knob and one sentence of physics. The compression schedule
stepped its frontier by exactly one canonical grid unit, and its first
invariant said why: 1 µm is the finest depth change a **layout** can express,
because `snap_mm` rounds every translation onto that lattice. That is true of a
pose and false of the clamp. `strip_depth_mm` is a proxy-tier scalar that
`boundary_penalty` reads as a continuous number, so a sub-grid step is not a
finer move — it is a smaller increment of pressure per step, and because
`confirm_every` counts *steps*, a quarter step asks the exact tier four times as
often per micron of descent. At a fixed 20M-unit budget on the port's own
from-scratch state, `step=1` published 159.102 and `step=0.25` published
**158.668**.

## Provenance

| item | value |
|---|---|
| worktree | `/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_6f601cb2-a5f-2` |
| base commit | `5d6ce0c` (coordinator v3 + compression-schedule port merged) |
| request | `tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`, sha256 `ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3` |
| contract | true 5.0/5.0 exact clearance, search-offset allowance **`0.0005`**, empty warm-start slot — the record lineage's tail, **not** the 0.002 gate band's |
| measure | `rawSourceDepthMm` of a publication that is `exactValid` **and** `contractValid` |
| gate binary (`jagua-experimental`, feature off) | sha256 `a3b3e520f0ef967d83a9d90eb93dfb7590e9c3f98912ad49f0ee5fc32543088f` |
| schedule binary (`jagua-experimental,compression-schedule`) | sha256 `6833f12301602b4ec91fd29bc71ebba3199997bee75971e441276c7682d9b279` |
| box | x86_64, 16 cores, engine pinned at 8 threads, **shared with another measurement agent** |

Nothing here is comparable to the coordinator band's 160.985 / 169.251 /
174.208 numbers: those are measured at allowance `0.002` and this whole
document is at `0.0005`.

## 1. The result

The whole line, every state pinned and every step an `exactValid` **and**
`contractValid` publication measured by `rawSourceDepthMm`:

| pin | declared raw (mm) | via | Δ from previous |
|---|---:|---|---:|
| `pinned-fs-parent-164.0376.json` (prior round) | 164.0375677990678 | — | — |
| `pinned-fs-parent-159.668.json` | 159.668 | mode 34, `past=1,work=33413789` | −4.370 |
| **`pinned-fs-158.668.json`** | **158.668** | mode 34, **`step=0.25`**, `work=20000000` | −1.000 |
| `pinned-fs-157.484.json` | 157.484 | mode 22 ×2, then mode 34 `step=0.25` | −1.184 |
| `pinned-fs-156.9188.json` | 156.91883051080544 | mode 22 / flatten→mode 33 alternation | −0.565 |
| `pinned-fs-156.418.json` | 156.418 | mode 34, `step=0.25` | −0.501 |
| `pinned-fs-156.0914.json` | 156.09136504906917 | mode 34 `step=0.1`, then 19 cheap-tier rounds | −0.327 |
| `pinned-fs-155.4633.json` | 155.46327292304915 | **mode 26 ladder, drop 1.0, seed 0** | −0.628 |
| `pinned-fs-155.4563.json` | 155.45627292304914 | mode 22 seed 0 | −0.007 |
| **`pinned-fs-155.4223.json`** | **155.42229074464285** | mode 22 / mode 33 / mode 31 grind, 266 arms | −0.034 |

**155.42229074464285 mm is 3.656 mm below the standing record**
(159.07876040364792) and 0.422 mm above the 155 mm goal. Its fixture sha256 is
`9b28ad598a0af789d848f1ea07cd81e8cbcbd0492dd77ce47c043d7cbd73159d` and its
placement fingerprint is
`c62879b403f9e71efd545cf70258e13747a90ca51ea86d72034bbaa7db98aa76`.

`pinned-fs-parent-164.0376.json` is the from-scratch parent the descent campaign
left, and `pinned-fs-parent-159.668.json` is the compression-schedule port's own
159.668 state — which that round published as a **run report only**, so this
round regenerated it from the same parent, seed, spec and `%.17g` bound, pinned
it with the `lib.pin` pattern, and replay-verified it before building anything
on it (`evidence/replay-159.668.json`: modes 27 and 30 reproduce the fingerprint
at 0 ULPs).

Every pin in the table is in
`docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/record-line-cascade/`.

## 2. The sub-grid step

`step=` is a new key on `POLYGON_NESTING_COMPRESSION_SCHEDULE`, in canonical
grid units, default `1`. It lives inside the existing off-by-default
`compression-schedule` feature, so no default path can reach it.

Measured on `pinned-fs-parent-159.668.json`, seed 5, `past=1`, at a fixed 20M
units of the schedule's own conservative currency
(`evidence/stepsweep-from-scratch-159.668.json`):

| step (grid units) | step (mm) | published | steps taken | confirmations accepted |
|---:|---:|---:|---:|---:|
| 4 | 0.004 | none | 2,001 | 0 |
| 2 | 0.002 | none | 2,402 | 0 |
| 1 | 0.001 | 159.102 | 4,017 | 64 |
| 0.5 | 0.0005 | none | 3,759 | 0 |
| **0.25** | **0.00025** | **158.668** | 6,550 | **190** |
| 0.1 | 0.0001 | 159.661 | 4,569 | 7 |

Two things are worth saying honestly about this table. The first is that the
relationship is **not monotone** — 0.5 published nothing and 0.25 published a
millimetre — so this is not a smooth "finer is better" law, it is a search whose
outcome depends on where the confirmations happen to land. The second is that
the *direction* is nevertheless real and mechanical: the coarse steps take the
frontier away from the floor faster than the sweeps can repair it, and 0 of
their thousands of steps ever reach an accepted confirmation, while the fine
steps keep the frontier inside repair range and accept 190.

## 3. The 159.079 record parent is still a fixpoint, and now for a better reason

The brief's third item: the record parent was a fixpoint for the schedule *at
one step size and one slice budget*, which is not a certificate. Probed at six
step sizes across two seeds and two budgets
(`evidence/stepsweep-record-parent-159.079.json`):

| arms | steps probed | budgets | seeds | published below 159.07876040364792 |
|---:|---|---|---|---:|
| 12 | 1, 0.5, 0.25, 0.1, 0.05 grid units, and 0.25 at `sweeps=18` | 20M, 60M | 5, 0 | **0** |

Every one of the twelve returned the parent's own depth to the digit. The
fixpoint survives the knob that broke the from-scratch line open, which is the
strongest statement this round can make about it — and it is *why* the record
fell on the other lineage instead.

## 4. Why the schedule is inert on almost everything: the entry snap

This is the round's structural finding, and it explains both the wins and every
barren stretch.

`initialize_complete_state` maps every warm-start rotation through
`canonical_angle`, which snaps it onto the structured surrogate's
`SURROGATE_ANGLE_STEP_DEG = 2.5` grid. The compression-schedule round measured
that snap's cost as a median 0.448 mm of entry loss. What this round measures is
the *consequence*: the snapped layout's collisions are rotation-induced, and the
schedule's repair is translation-only, so it never recovers.

| parent | produced by | poses off the 2.5° grid | proxy-feasible on arrival | colliding pairs | entry loss | schedule's result |
|---|---|---:|---|---:|---:|---|
| `pinned-fs-parent-159.668` | mode 34 | 0 | **yes** | 0 | 0.019 mm | descends 1.000 mm |
| `pinned-fs-156.9188` | mode 22 / mode 33 | 28 | no | 28 | 0.647 mm | **nothing, at any budget** |

The negative is nailed down three ways, all in `evidence/`:

* **Budget is not the answer.** `stepsweep-bigbudget-156.919.json`: 200M units
  (10x), 40 sweeps per step, `confirm=1`, `repair=sweeps`, `step=0.02` — six
  arms, **0 below**. The frontier reaches 154.9 mm and the floor never leaves
  the parent.
* **Seed is not the answer.** `stepsweep-seeds-156.105.json`: 16 arms over
  seeds 0-7 at two sweep counts, **0 below**. The schedule's productivity is a
  property of the *state*, not of the salt.
* **Walking around it costs more than it pays.** `regrid-156.919.json` rounds
  every pose onto the 2.5° grid first (28 of 61 move, +0.391 mm), legalizes the
  result with mode 30 / mode 31 (both reach 157.188), and hands that to the
  schedule, which now *does* ratchet — 37 accepted confirmations down to
  157.017. The round trip loses 0.391 mm and wins 0.171 mm, and 157.017 is
  0.098 mm **worse** than the 156.919 it started from. The mechanism is
  confirmed and the trade is negative at this depth.

So mode 34 is an operator with a precondition, and the precondition is "this
state came out of mode 34". That is why the cascade's schedule tier is gated to
every Nth round rather than run every round.

## 5. The cascade, and the ordering mistake it exposed

`drivers/cascade.py` runs every instrument on the incumbent and restarts from
the new incumbent the moment any arm publishes an `exactValid` **and**
`contractValid` layout whose raw is **strictly** below — strict `<`, no decimal
epsilon, because ~35 ULPs of slack at this magnitude hides real improvements.

Three cascades contribute arms — `fsline`, `fsline2` and `fsline4`, **907 arms**
in total, each restarting from its new incumbent (`evidence/cascade-*.json`,
`evidence/cascade-*.log`). A fourth, `fsline3`, was launched with mode 26 hoisted
to the *front* of the round and stopped after 6 barren mode-26 arms without
adopting anything — that ordering starved the cheap tiers exactly as the
cheap-first ordering had starved mode 26, and its log is kept
(`evidence/cascade-fsline3.log`) because the mistake is the finding:

| tier | mode | arms | publications | adoptions |
|---|---|---:|---:|---:|
| A | 22, salted waves over 8 seeds × up to 3 bound slacks | 678 | 678 | 20 |
| B | frontier flatten {0.0005 … 0.01} → 33 | 147 | 111 | 23 |
| C | 31, tiny-step ratchet | 4 | 4 | 4 |
| E | 34, six schedule specs × 2 seeds | 78 | 78 | 3 |
| F | 26, short ladders | 0 in-cascade | — | 0 |
| D / G | nudges, mode-23 crossover | 0 reached | — | — |

The one mode-26 adoption on this line — the 0.628 mm one — came from **outside**
the cascade, out of the certification battery, for the reason below. Tiers D and
G were never reached at all: no round ever got past tier F without adopting
first, so the crossover instrument the brief asked for had to be run outside the
cascade against the declared-battery incumbent (§8: 24 arms, 13 publications, 0 below).

The ordering finding is worth more than any single arm. The first cascade ran
the cheap tiers first — mode 22 at 3 s an arm, the flatten grid at 2 s — and
they kept publishing 0.001-0.002 mm and restarting the round, so **mode 26 was
never reached in 555 arms**. When the certification battery finally ran mode 26
against that incumbent, **six of six arms came back below it**, the best by
0.628 mm. An adopt-and-restart cascade ordered by arm cost is a cascade that
starves its most productive instrument whenever the cheap tiers are merely
*non-zero*; the fix here was to hoist mode 26 above mode 34 and above the nudge
tiers, and to run it as a concurrent sweep (`drivers/armsweep.py`) when it is
the tier that is paying.

Mode 26's own yield is basin-shaped rather than steady: on the 156.091
incumbent it published 6 of 6 (best 155.463); on the 155.452 incumbent, **16**
arms over four drops and four seeds published, and **0** were below the
incumbent (`evidence/m26sweep-155.452.json` has 16 rows, not the 12 originally
claimed here — the negative is if anything stronger than stated).

## 6. Regression

The four pinned gates, on the pre-change default-feature binary (`base`), the
post-change default-feature binary (`after`) and the schedule binary with the
feature compiled in and unarmed (`sched`):

| gate | pinned | `base` | `after` | `sched` |
|---|---|---|---|---|
| g1 mode 20 | 206.869 / `8a7737381238fa4d` | hit | hit | hit |
| g2 mode 22 | 159.09233022733062 / `fa01012af1d559ae` | hit | hit | hit |
| g3 mode 22 | 159.07876040364795 / `e28fba007f8031d4` | hit | hit | hit |
| g4 mode 22 | 164.0375677990678 / `49f094d7e59a9008` | hit | hit | hit |

All four are `exactValid` and `contractValid` in every run. Whole-document
comparison with the wall-clock and build-identity fields removed
(`evidence/docdiff-base-after.json`, `evidence/docdiff-base-sched.json`):

| comparison | fields compared (g1/g2/g3/g4) | differences |
|---|---|---|
| `after` vs `base` | 3,262 / 3,243 / 3,243 / 3,243 | **0** |
| `sched` vs `base` | 3,262 / 3,243 / 3,243 / 3,243 | **0** |

Release suite: **1,244 passed, 0 failed, 2 ignored** with `jagua-experimental`,
and **1,258 passed, 0 failed, 2 ignored** with
`jagua-experimental,compression-schedule` — the 12 the port added plus this
round's two, which pin the sub-grid walk's step count and the degradation of a
non-positive step to the canonical unit.

## 7. Certification

`drivers/certify_full.py` on `pinned-fs-155.4223.json`, declared raw
`155.42229074464285`, in 548 s (`evidence/cert-final.json`):

| half | what | result |
|---|---|---|
| replay | modes 27, 30 and 22 seeds 0-3, on the **default-feature** binary that contains no mode 34 at all | 6 of 6 `exactValid` **and** `contractValid`, all six reproducing fingerprint `c62879b4…` at **0 ULPs** from the declared raw |
| fixpoint | 30 search arms (plus the 6 replay arms above, for `probeArms: 36`): mode 31 × 4 tiny steps, the flatten grid × 6 deltas × 2 slacks → mode 33, mode 26 × 3 drops × 2 seeds, and **mode 34 × 2 seeds over three distinct step sizes (0.25, 1, 0.1) plus a repeat of `step=0.25` at a 60M-unit budget — not four step sizes** | **0 below the incumbent** |

`replayPass: true`, `belowIncumbent: 0`, `fixpoint: true` in the raw JSON. The
right English label for that is a **finite negative on the declared
battery** — not a "certified fixpoint" — for two reasons. First, `probeArms:
36` folds the 6 replay arms into the search count; only 30 arms actually probe
for a better neighbor. Second, and more important: every one of the eight
mode-34 arms entered with `parentProxyFeasible: false` (35 colliding pairs, 9
boundary violations), because the incumbent is not itself a mode-34 product —
so the mode-34 half of this battery mostly measured the schedule's regrid
recovery off an infeasible entry, not a local schedule search around 155.422
the way §4 shows mode 34 behaves on a state it produced itself. `0 below` is
still real and still the whole of what "0 below" claims; it is a finite
battery run once, not evidence of a fixpoint in the mathematical sense. The
mode-34 arms are new to the battery this round, and they are there because
§3's lesson applies to this incumbent too: a claim probed at one step size is a
claim about one step size.

A second bug in the same battery is worth recording honestly: the mode-34 loop
built its per-arm tag from `step=` alone, so the `work=20000000,step=0.25` and
`work=60000000,step=0.25` arms wrote to the same `{tag}.json` raw artifact and
the second overwrote the first. All eight arms still contributed their own row
to the summary above — each row comes from the run's own in-process return
value, not a re-read of the overwritten file — so `belowIncumbent: 0` is
unaffected, but only six of the eight raw per-arm artifacts survive on disk in
`/var/lib/t3/tmp/recordline/cert-final-runs/`. `SCHED_SPECS` runs
`work=20000000,step=0.25` before `work=60000000,step=0.25` for each seed, so
the later write wins: the two `work=20000000,step=0.25` raw files (one per
seed) are gone, overwritten by the `work=60000000,step=0.25` run at the same
path, while `step=1` and `step=0.1` never shared a tag with anything and are
intact. `drivers/certify_full.py`'s tag now encodes `work=` as well as
`step=`, so this cannot recur; the existing evidence is left as it was
produced, not re-run.

An earlier incumbent's certification is kept as well
(`evidence/cert-156.0914.json`) precisely because it **failed** the fixpoint
half — 20 of 36 arms below, six of them mode 26 — which is what redirected the
whole round and is the evidence for §5.

## 8. Honest limits

* **The from-scratch line and the record line are now the same line.** The
  record fell to the from-scratch lineage, which is the good case for the
  "unaided" claim — 164.0375677990678 -> 155.42229074464285 with no record-line
  placement imported at any step — but it also means this round produced **no**
  new descent on the 159.079 lineage itself. That parent is where it was.
* **The 155 mm goal is not reached.** The gap is **0.422 mm**, and the final
  state is a **finite negative on the declared battery** in §7 (30 search arms
  plus 6 replays, not a certified fixpoint — see §7 for why), so closing it
  needs an instrument this round did not fire rather than more of the same. The
  measured rates say how it would have to close: the two instruments that moved
  more than 0.1 mm in a single arm are mode 34 on a state it produced itself
  (1.000, 0.671, 0.495 mm) and mode 26 in a basin (0.628 mm), and both are
  intermittent. The cheap tiers grind at 0.001-0.002 mm a round and would need
  hundreds of rounds.
* **Not monotone, and not a law.** §2's step table is one parent at one seed.
  `step=0.25` is the value that worked twice here; it is not established as a
  better default, and the compiled-in default is unchanged at `1`.
* **One request.** Nothing here says anything about shapes-17, triangle-20 or
  any other request, and coordinator v2's generality finding applies in full.
* **No wall-clock claim.** The box was shared with another measurement agent
  throughout, and several sweeps were deliberately run concurrently. Every
  quality number is a work-budgeted or seeded arm; the seconds in the logs are
  reported so a reader can see the arms were concurrent, not as measurements.
* **Mode 23 crossover is measured, and it is barren here.** It never ran
  *inside* the cascade — tier G sits behind tiers A-F and no round ever went
  barren above it — so it was run separately against the declared-battery
  incumbent
  (`drivers/crosssweep.py`, `evidence/crossover-155.4223.json`): the incumbent
  against two of its own ancestors and both record co-states, three cut
  fractions, both directions, 24 arms. Thirteen published, **none below**, and
  the best was 158.144 — 2.7 mm above the incumbent. That is one incumbent, one
  seed and three cuts, so it is a negative about this state, not about the
  operator.
* **Mode 26's determinism under load is assumed, not proved here.** The four
  pinned gates (three of them mode 22) reproduce bit-exactly under every load
  this round ran them at, and mode 26's budget is a query count rather than a
  clock, but this round did not run a paired same-arm-under-two-loads check on
  mode 26 specifically.
* **The `regrid` negative is one depth.** The round trip through the 2.5° grid
  loses more than it wins at 156.9 mm. It might not at 170 mm, where the entry
  loss is a smaller fraction of the slack, and that was not measured.

## Files

* `drivers/lib.py`, `drivers/drv.py` — the finer-ladder runner, `ROOT`
  repointed at this worktree, plus a per-arm binary/environment override so the
  schedule binary and its knobs can be driven from the same harness.
* `drivers/gatelib.py`, `drivers/gates.py` — the four pinned gates.
* `drivers/docdiff.py` — the whole-document gate comparison.
* `drivers/sched.py`, `drivers/schedsweep.py` — one mode-34 arm, and a
  concurrent grid of them.
* `drivers/armsweep.py` — a concurrent grid of any one mode's arms.
* `drivers/crosssweep.py` — the mode-23 crossover grid, both directions.
* `drivers/regrid.py` — the 2.5°-grid re-entry probe.
* `drivers/cascade.py` — the cascade.
* `drivers/pinrun.py`, `drivers/replay.py` — pin a run report, and the
  one-ULP replay check.
* `drivers/certify_full.py` — the certification battery, with mode-34 arms
  added at three distinct step sizes (one of them repeated at a second work
  budget); the per-arm tag now encodes the work budget too, so specs that
  share a step size no longer overwrite each other's raw artifact (§7).
* `drivers/collect.py` — copies the evidence out of the scratch tree.
* `evidence/` — every driver's own emitted document, unedited.

Reproduce:

```
cargo build --release --example general_request_benchmark \
    --features jagua-experimental                              # gate binary
cargo build --release --example general_request_benchmark \
    --features jagua-experimental,compression-schedule \
    --target-dir target-sched                                  # schedule binary

python3 drivers/gates.py after target/release/examples/general_request_benchmark
python3 drivers/schedsweep.py fs-stepsweep <pin-159.668> 159.668 0.3 \
    'past=1,work=20000000,step=1;past=1,work=20000000,step=0.25' 5 3
python3 drivers/cascade.py fsline <pin> <raw> 40
python3 drivers/certify_full.py <pin> <raw> cert
python3 drivers/collect.py
```
