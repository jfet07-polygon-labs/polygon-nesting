# The skip pile: what the proxy is hiding

Grok review 8 item 2. The round-envelope gate found that
`schedule_confirmationsRefused = 0` on all 108 runs of its twelve-parent matched
gate — the miter confirmation never refused anything, because almost nothing
reached it. The filter is one level up:

```rust
// compression_schedule.rs, due_for_confirmation
if !proxy_feasible { self.confirmations_skipped_infeasible += 1; return false; }
```

`proxy_feasible` is the **relaxed surrogate's** verdict, and the surrogate's
collision geometry is the production **miter** offset. Across the miter ladder
that clause suppressed **149 762** frontiers, cell-for-cell identical on the
union arm.

**The question.** What fraction of those skipped frontiers is *disc-legal*
(the certified round kernel accepts) yet *miter-illegal* — is there a released
region hiding behind the proxy — and if there is, is the released material in the
sub-micron class (~1 µm, the canonical grid's own step) or the join-tax class
(~0.5 mm, what Gate A measured on a miter-refused pair)?

**Nothing here changes shipped behaviour.** The instrument is a new cargo
feature, `skip-pile-dump`, compiled out by default; the four pinned gates are
what proves the default build did not move.

---

## 0. Pre-committed interpretation

> **This section was written and committed before a single frontier was
> scored.** It is here so that the answer cannot be read backwards into the
> question. The commit that carries it carries the instrument and no evidence;
> `git log --follow` on this file is the proof.

The proposal-geometry surgery Grok's review calls **option (b)** — moving the
schedule's proposal geometry off the miter proxy so that disc-legal frontiers
stop being suppressed — is worth funding only if the region it would open is
both **non-empty** and **large enough to matter**. Three outcomes, and what each
one means, fixed in advance:

| outcome | reading | consequence for option (b) |
|---|---|---|
| **~0 released rows** (0, or a handful that are all sub-micron-class) | the released region does not exist behind the proxy on this population: every frontier the proxy suppressed is one the disc refuses too | **killed a priori for these parents.** The proxy is not costing reachability; it is doing the job it was written for, and a surgery on it buys nothing to search |
| **a material fraction released, sub-micron class** | the region exists but the material it frees is at the canonical grid's own step | not funded. A micrometre of clearance is not a millimetre of depth, and the campaign's own currency is millimetres |
| **a material fraction released, 0.5 mm class** | the region exists and is join-tax sized | **sized and reported** — rows, millimetres, per seed — as a live candidate, without being promoted here |

"Material fraction" is fixed at **≥1 % of sampled skipped frontiers**, which on
the sample below is ≥18 rows. Below that, an existence proof; at or above it, a
rate.

**Grok's own prior** is that the pile is mostly *bulk overlap* — frontiers where
the clamp has just been lowered and the repair has not yet run them apart, so
every authority refuses them and the proxy is simply right. That prior is
reported as a number either way: the `contract=False, miter=False, kernel=False`
row of the joint table.

**What would falsify the instrument rather than answer the question.** Any
row where the miter accepts and the disc refuses (`kernel refuses ∧ miter
accepts`) on a layout the contract accepts is a **P0** against the kernel and is
reported as one, not as a data point. The previous round's soundness battery says
there should be none.

---

## 1. The instrument

One cargo feature, `skip-pile-dump`, and three edits behind it:

| file | what |
|---|---|
| `search/compression_schedule.rs` | one `cfg`-gated accessor, `confirmations_skipped_infeasible()` |
| `search/general_relaxed.rs` | one `cfg`-gated block **after** the confirmation branch of a schedule step |
| `search/skip_pile_dump.rs` | the sink: JSONL, deduplicated by the engine's own placement fingerprint, capped |

Four properties, each a mechanism rather than a claim:

* **compiled out by default.** The module does not exist without the feature and
  the call site is behind the same `cfg`. §4.1 is the measurement.
* **disarmed by default when compiled.** The sink opens only when
  `POLYGON_NESTING_SKIP_PILE_DUMP` names a path. A compiled-but-unarmed binary
  reproduces all four gates *as whole documents* — §4.1 again.
* **it reads, it does not decide.** The block runs after
  `due_for_confirmation` has already returned; nothing is fed back, no field is
  published. Its only cost is wall.
* **and wall is not in the trajectory here**, because a mode-34 slice under
  `past=1,rollback=0,work=W,lanes=1,pconfirm=0` is capped in *work units*. That
  is an argument, so §3 replaces it with a measurement: every dumped cell is
  checked against the round-envelope gate's committed `matched.json` on the
  schedule's **step digest**, its skip count, its confirmation counts, its step
  count and its published depth.

The scoring stage is a second example, `skip_pile_score`, which needs
`round-envelope-kernel` and `import-gate-shadow` and **not** `skip-pile-dump`:
the writing binary and the reading binary are different programs.

## 2. The three authorities

Every sampled frontier is asked of three, at two radii:

| verdict | function | what it is |
|---|---|---|
| contract | `import_gate::authority_verdict().contract_only` | the untouched material contract validator |
| miter | `round_envelope_gate::wired_verdicts().miter` | HEAD's authority, through `validate_and_measure_placements` |
| kernel | `round_envelope_gate::wired_verdicts().exclusive` | the certified disc kernel, same wire point |
| union | `round_envelope_gate::wired_verdicts().union` | whichever half admits it |

Both composites run the material contract on the same placements, so
**kernel-accept ∧ miter-refuse is a released layout** and not a weaker
authority's opinion. The per-pair attribution beside it is `census` against
`miter_census` — the soundness battery's own pair, so a magnitude here is
comparable to Gate A's 0.5057 mm and to the 1 µm grid step.

The two radii are **2.502 mm** (allowance 0.002, the expansion the skip actually
happened at) and **2.500 mm** (allowance 0.0, the contract radius itself). The
collision expansion is `total_padding/2 + margin + allowance` and this population
is the exact-clearance 5.0/5.0 contract with a zero safety margin, so those two
numbers are the whole difference.

---

*(Sections 3 onward are written after the runs.)*
