# Grok review 11 — the verdict stands (2026-08-23)

Final confirmation review of the pivot round's C175 verdict (merged 791a434),
under Grok 10's own pre-committed reading. `grok --reasoning-effort xhigh`,
full tree. Verbatim.

---

## 1. Confirm — nothing reopens it

**Grok 10 §C, and pivot §0.2, applied as written:** if C175 is 0/3 with the pivot fixed, the jump real and installed on trajectories that stall above 0.1 mm, and the clearances right, that is the family separator failing; stop before `homotopy.rs`. All three antecedents hold. Both escape clauses are closed.

| Antecedent | Evidence |
|---|---|
| Pivot fixed | `compose_proposal` about the transformed centroid; red log on `1f5cd5b`, green on this tree |
| Jump real and installed | 3/3 strip, `jumpCommitted: 1`, fired at `max_g` 2.169 / 1.973 / 1.945 mm (all ≫ 0.1) |
| Clearances right | mixed-61 `physicalEdgeClearanceMm = depthTopInsetMm = 5.0`; `entryDepthSlackMm = 0.0` on 3/3 |
| 0/3 children | `strictChildren: 0`, `exactCheckpoints: []`, `publications: 0` |
| Φ does not explode as under `always` | named bombs were raw Φ 925 / 3359; with-jump terminals are 327 / 30.4 / 42.5; jump-free arm is 35.91 / 36.02 / 36.15 |

`GATE0_PASS: false`, `fatalFailures: ["C175"]`. The other five fatal cells hold. Jump-free A/B vs the un-fixed binary: `max_g` −28.7 / −44.9 / −33.6 %. Four batteries, 2,879,688 proposals, `exactCheckpointAttempts = 0`.

**The three notes you flagged do not reopen it.**

- **2.223 s on seed 0.** The 2 s clause was converted to a 240,000-proposal quota before this battery. Seed 0 spent 239,974 proposals, same as the other two. The cell is 0/3 on the proposal clause with or without the clock. Grok 10’s “≤ 2 solver seconds” is a pass requirement, not a reason to discard a fail. Recorded, not causal.
- **Census-arming correction.** The 32-slot sample is the first stalled sweep (ordinals ~7k of 240k on seed 0, guided/raw = 1.13 vs 184.8 at the end). That is a **documentation** defect in the previous round’s §2.1, not a trajectory defect. The census is read-only: no scoping of it can move 0/3, 1.6 mm, or zero publication attempts. It is not a second pivot.
- **Ascent population 11–14/32.** The pivot’s fingerprint was first-order ascent from a coordinate/torque mismatch; that halved and the green vectors closed it. Residue is the subgradient of a max (`v_ij` and each boundary residual are maxima). No remaining line where gradient and coordinate disagree. Caveat 5 is honest and still not a named defect.

**One precision on the summary, not a reopen.** The 7-bottom + 14-top, L/R clear shape is the **jump-free attractor** (`probes.json` / `pivot-ab.json`), Φ 35.9–36.2, `max_g` 1.597–1.620 mm. The with-jump cell terminals are not that attractor: seed 0 is 9+14 with 2 right and `max_g` 8.30 mm (the strip smash). The cell fails on both arms. The attractor is the descent; the cell as specified includes a jump that installs and makes it worse.

Unconditional strip commit is frozen, catastrophic (guided Φ ×21–207), and irrelevant to the verdict: `--jumps=0` is also 0/3 at 1.6 mm. Diagnostics moving the wrong way on `max_g` is the same smash, not a controlled experiment on S2/C168/T, and none of them is fatal.

---

## 2. What was actually falsified

**The Round-1 solver member, on the 10 % shock, on mixed-61.** Not “ICS as a literature family,” and not “Φ / publication / S0.”

Falsified, precisely: **one-piece strict-decrease PGS + GLS weights + one strip/ball jump**, with a correct SE(2) gradient, correct clearances, entry exactly at \(T\), and the one licensed topology move installed. That member cannot produce a dual-valid child from constructor shocked by \(0.10(D_0-L)\) (6.714 mm, \(T=176.262\)) in 240k proposals.

Not falsified: the measure (S0 bit-for-bit, 1k/10k all zeros on the scored band), the publication path (never called), triangle-20, or S1.

**The proposed sentence is directionally right and one shock too coarse.** Two different shocks were measured:

| Shock family | Scale | Result |
|---|---|---|
| Per-piece SE(2) from the S0 pin | 0.5 mm / 2° (S1) | Φ → 0, publishes 150.16374, **jump never licensed** |
| Same, one rung up | 2.0 mm / 10° (S2) | fails, `max_g` 10.5 mm (guided-commit) / 21.3 mm (unconditional) |
| Affine Y-compression (C175, the homotopy’s actual start) | 6.71 mm; entry `max_g` 5.57 mm | corrected descent → attractor Φ 36, `max_g` **1.60 mm**, 399× the 4 µm band |

So: this member’s **per-piece** basin on mixed-61 includes 0.5 mm and excludes 2 mm. The homotopy’s first bite is a **6.7 mm affine Y-shock** whose corrected descent has a **1.6 mm Y-frustration attractor**. Those are not the same “O(1 mm) vs O(5 mm)” number. The 1.6 mm is residual after recovering ~4 mm of a 6.7 mm shock, not a basin radius.

**S1 passing is the other half of the same picture.** Last round S1 needed a ball jump at 12 µm. This round it never stalls. The one-piece field, once the pivot is right, **does** close a 0.5 mm mixed-61 basin. C175 is not “PGS cannot nest mixed-61.” It is “PGS cannot legalize this millimetre-scale distributed compression.”

**Could a different member cross 1.6 mm? Unknowable without building one. The evidence is not silent about which operator could even name the gap.**

The attractor is 7 through the sheet bottom (~1.60 mm) and 14 through the strip top (~1.10–1.20 mm), **different pieces**, L/R clear, `piecesSqueezedOnOppositeSides: 0`, plus 23–25 pair rows at ~1.3 mm. That is a packing 1.6 mm too tall with internal overlaps. A global translation cannot satisfy both sheet sets. A one-piece move cannot name “these 21 pieces occupy 1.6 mm less Y.”

Among the operators both reviews excluded as **new**:

| Operator | Does the attractor diagnose it? |
|---|---|
| **Chain / component Y-compaction** | **Yes — the only one that can name the missing direction.** |
| Accept-equal + tie | No. S1 no longer needs it. 1.6 mm is not a tied 12 µm active set. A zero-energy neighbour still cannot move (`0+ε` is not `< 0`). |
| SOR overshoot | No. Might rattle a GS deadlock; it does not create a 21-piece Y-compaction. |
| Two-endpoint PGS | No. The pivot already aligned torque and coordinate. Frozen-θ still kills S1; rotation is necessary and now pointed the right way, and C175 still dies on Y. |

Two measurements argue **against** treating chain as likely rather than merely nameable:

1. The spec’s own topology operator — relocate the highest-pressure piece, real n-piece settle, unconditional commit — **moves away from legality on 3/3**. Cluster relocate is a bigger jump of the same kind.
2. C168 (diagnostic, but it is the 10 s number: constructor squeezed 14.5 mm onto 168.484) is the **same shape** at larger scale: 10 bottom + 16 top, L/R 0, opposite-side squeezes 0, `max_g` **11.67 mm**. A chain that closed C175’s 1.6 mm has no evidence it would close that.

So: the honest sentence is not “chain would work.” It is “chain is the only excluded operator that addresses the attractor; whether it travels 1.6 mm, let alone 11.7 mm, is a new experiment.”

---

## 3. Rank for the owner

**Evidence vs recommendation, separated.**

| | Evidence shows | I recommend |
|---|---|---|
| **(a) STOP the overlap-ICS 10 s line** | **Licensed.** Grok 10 §C and pivot §0.2 both named this outcome. Clean machine, five of six fatal cells green, separator red, no new pivot-grade defect. | **This. Single action: stop. Do not build `homotopy.rs`. Do not retarget C175. Park Φ, S0, dual-valid publication, and the corpus as artifacts.** |
| **(b) One new-solver-member round** | **Not licensed as a mechanical continuation.** Licensed only as a **new spec**, which both reviews already said a chain round would be. Residual shape picks the operator: Y-connected-component compaction, not the other three. Gate if the owner funds it: same C175 0/3 at 240k, jump-free A/B must beat the 1.6 mm attractor to a publication attempt; shock frozen. Cost: a full Gate 0 battery, days, not the hours before the owner is up. C168 remains a second untested gap 7× larger. | Do not start it tonight. If funded later, that is the operator and that is the gate. I would not bet the month on it. |
| **(c) Reduce the shock (2–3 % / epoch)** | **Forbidden, and not a same-spec reading.** Grok 10 Finding 4: do not shrink the shock. Pivot §0.4 froze `D0 − 0.10(D0−L)`. The 10 % is a frozen Round-1 knob, not an accident. | **Rescuing the corpse. Do not.** |

**(c) is the category error, spelled out.** The spec contracts the remaining gap by 10 % **per success**, from a **legal** child. C175 never produced one. Sparrow-class small bites (`W ← W−1 mm` after publish, Grok 9 R1) are legal-to-legal shrinks. “The separator tested one 10 % bite, so try 2–3 %” rewrites \(T_0\) and walks back toward the retained-infeasible experiment (compress a feasible parent, overlap drops, **zero** feasible states) that is why this family was not supposed to be “mode-0 plus `global_legalize` with a bigger cap.” The homotopy’s **failure** path already is a smaller bite: \(T \leftarrow (T+D^\ast)/2\) ≈ 3.36 mm. That path is `homotopy.rs`, which this verdict says not to build, and 3.36 mm is still outside the measured 0.5 mm per-piece basin.

**What STOP does and does not mean.** It kills **this** specified engine as a 10-second program: do not schedule, do not parallelize, do not choreograph around a separator that failed on a clean machine. It does not prove that no Imamichi/Egeblad member with a richer neighborhood can nest mixed-61. That is exactly option (b), and it is a new decision, not a retrofit of Gate 0.

**Single recommended next action:** **(a).** Stop the overlap-ICS line as specified. Write the Gate 0 kill as C175 0/3 after a real jump and a correct pivot. Spend the owner’s morning on a different 10 s bet, not on a shock-knob or a chain that still has to invent a way from 1.6 mm to 168.484.
