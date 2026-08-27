# Constructed-basin constructor family

## Status

**Closed as a constructor family.** Bottom-left construction was implemented,
measured across salts, descended through the full post-construction pipeline,
extended with contact walking, and continued with a larger stall budget. The
generic hypothesis “try a bottom-left constructor” must not be reopened.

The evidence was durable but poorly indexed: the raw records and the narrative
in `docs/next-generation-engine-plan.md` existed, but this directory had no
summary of the family-level verdict. This file is that missing index.

## Evidence chain

| experiment | constructor/start | best descended endpoint | verdict |
|---|---:|---:|---|
| drop-settle | 229.121 mm | 201.626 mm | useful increment, superseded |
| exact bottom-left drop-slide-drop | **203.208 mm** | **191.572 mm** | real improvement, high-variance outlier |
| eight-salt bottom-left screen | salts 1–7: 215.300–218.237 mm | — | 203.208 is not the typical basin |
| profile constructor, 32-basin harvest | typical band about 213–218 mm | tail saturates at **189–191 mm** | further salt brute force is dry |
| translation-only contact walk | 202–209 mm screen band | record chain reaches **187.463 mm** | retained within the family |
| tripled-stall continuation | from 187.463 mm | **184.759 mm** | one final gain, then dry |

Primary artifacts:

- [`bottom-left/chain.log`](bottom-left/chain.log) records the exact
  203.208→191.572 chain.
- [`salt-screen/screen.log`](salt-screen/screen.log) records the eight-salt
  distribution and the 203.208 outlier.
- [`salt-harvest-2/thirty-two-basin-table.json`](salt-harvest-2/thirty-two-basin-table.json)
  records the saturated 32-basin harvest.
- [`contact-walk/salt0-record/chain.log`](contact-walk/salt0-record/chain.log)
  and [`contact-walk/salt0-record/deep-continuation.log`](contact-walk/salt0-record/deep-continuation.log)
  record the 184.759 terminal line.

The independent constructor-order oracle supplies the same family verdict from
another direction. Four exact-valid constructor starts at 184.728, 190.945,
233.339, and 258.103 mm reached 168.361, 180.331, 195.814, and 204.996 mm after
the identical relaxation/separation pipeline. The winning order was already
selected by production; no losing constructor basin crossed the declared
165 mm gate. See `docs/next-generation-engine-plan.md`, the paragraph beginning
“A reviewed cold-process constructor-basin oracle”.

## What is closed

- bottom-left/drop-slide-drop under another name;
- order-only constructor variants and salt harvesting;
- checkpoint settling inside construction;
- deeper continuation of the same descent ladder;
- context-blind contact-pose enumeration in the same greedy insertion
  lifecycle.

The closure is causal, not merely numerical: construction improved, the
downstream descent extracted a repeatable 5–27 mm, and then every sampled basin
entered the same operator-locked terminal class. More seeds or more patience do
not create the missing transition.

## What this does not close

It does not reject a new nonterminal lifecycle, a persistent population of
partial/infeasible states, or a context-aware multi-piece transition with a
different state objective. Such a mechanism needs its own pre-committed spec
and may not cite this constructor family as positive evidence.
