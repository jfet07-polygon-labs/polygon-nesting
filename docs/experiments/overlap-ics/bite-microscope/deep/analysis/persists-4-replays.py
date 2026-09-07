#!/usr/bin/env python3
"""persists-4-replays.py -- what persists in the 100 replays (live horizon, both objectives).
Per replay: objective vs captured, iterations run / horizon, evaluations total (and the live attempt's),
band entry, certification; entry rows (replay.entryBlocking) persisting to the last iteration / released /
created; blocking count first/min/last; maxAfter first/min/last; the replay's own column analysis
(status, formedAt, firstDisconnected, break, residence, reformedAfterBreak, permanent / temporary releases)
and persistentRows / watchedRows status. Identity-passing replays reproduce the live attempt bit for bit
(so their column analysis describes the live trajectory).
Usage: python3 persists-4-replays.py
"""
import glob, json, os, statistics as st
DEEP = "/var/lib/t3/tmp/astra/deep"
rows = []
for path in sorted(glob.glob(os.path.join(DEEP, "replays", "rp-*.json"))):
    d = json.load(open(path)); r = d["replay"]; cap = r["capsule"]
    its = r["iterations"]
    eids = {x for x, _ in r["entryBlocking"]}
    last = {x for x, _ in its[-1]["blocking"]} if its else set()
    col = r.get("column") or {}; cll = r.get("columnLongestLived") or {}
    wr = r.get("watchedRows") or []
    rows.append({"file": os.path.basename(path), "src": os.path.basename(cap["path"]), "arm": "A" if cap["capturedExponent"] == 2 else "B", "seed": cap["seed"], "bite": cap["bite"],
                 "objective": r["probe"], "own": r["identityPass"] == len(r["identity"]) and r["identityPass"] > 0, "horizon": r["params"]["horizon"]["iterations"], "ran": len(its),
                 "stop": r["stop"], "evals": r["evaluationsTotal"], "live_evals": r["control"]["evaluationsTotal"], "band": r["bandEnteredAtIteration"], "live_band": r["control"]["bandEnteredAtIteration"],
                 "cert": r["certification"]["published"], "entry_n": len(eids), "persist": len(eids & last), "released": len(eids - last), "created": len(last - eids),
                 "nblk_first": len(its[0]["blocking"]), "nblk_min": min(len(i["blocking"]) for i in its), "nblk_last": len(last),
                 "max_first": its[0]["maxAfterMm"] * 1000, "max_min": min(i["maxAfterMm"] for i in its) * 1000, "max_last": its[-1]["maxAfterMm"] * 1000,
                 "rollbacks": len(r.get("rollbacks") or []),
                 "col_status": col.get("status"), "col_rows": len(col.get("rows", [])), "col_core": len(col.get("coreMembers", [])), "col_formed": col.get("formedAtIteration"), "col_first_disc": col.get("firstDisconnectedAtIteration"),
                 "col_break": col.get("breakIteration"), "col_residence": col.get("residenceIterations"), "col_reformed": col.get("reformedAfterBreak"), "col_perm": len(col.get("permanentReleases", [])), "col_temp": len(col.get("temporaryReleases", [])),
                 "col_orig_connected_frac": (sum(col["originalRowsConnected"]) / len(col["originalRowsConnected"])) if col.get("originalRowsConnected") else None,
                 "cll_residence": cll.get("residenceIterations"), "cll_formed": cll.get("formedAtIteration"), "cll_rows": len(cll.get("rows", [])), "cll_core": len(cll.get("coreMembers", [])), "cll_reformed": cll.get("reformedAfterBreak"),
                 "watched_blocking_at_end": sum(1 for w in wr if w["status"] == "blocking-at-end"), "watched_n": len(wr), "watched_sweeps": [w["blockingIterations"] for w in wr]})
json.dump(rows, open(os.path.join(DEEP, "analysis", "persists-replays.json"), "w"), indent=1)
def med(v): v = [x for x in v if x is not None]; return f"{st.median(v):.1f}" if v else "-"
def rng(v): v = [x for x in v if x is not None]; return f"[{min(v):.0f}-{max(v):.0f}]" if v else ""
print(f"{len(rows)} replays")
for arm in "AB":
    for bite in (5, 6):
        for own in (True, False):
            for pub in (True, False):
                sel = [x for x in rows if x["arm"] == arm and x["bite"] == bite and x["own"] == own and (x["live_band"] is not None) == pub]
                if not sel: continue
                print(f"\n-- captured arm {arm} bite {bite} live {'published' if pub else 'failed'}, replay objective {'OWN (identity)' if own else 'OTHER'} ({sel[0]['objective']}): n={len(sel)}")
                print(f"  horizon it med {med([x['horizon'] for x in sel])}, ran {med([x['ran'] for x in sel])} {rng([x['ran'] for x in sel])}; evaluations med {med([x['evals'] for x in sel])} (live {med([x['live_evals'] for x in sel])}); band entries {sum(1 for x in sel if x['band'] is not None)}/{len(sel)}; certified {sum(1 for x in sel if x['cert'])}")
                print(f"  entry rows {med([x['entry_n'] for x in sel])}: persisting at end med {med([x['persist'] for x in sel])} {rng([x['persist'] for x in sel])}, released {med([x['released'] for x in sel])}, created {med([x['created'] for x in sel])}")
                print(f"  nblk first/min/last {med([x['nblk_first'] for x in sel])}/{med([x['nblk_min'] for x in sel])}/{med([x['nblk_last'] for x in sel])}; max um first/min/last {med([x['max_first'] for x in sel])}/{med([x['max_min'] for x in sel])}/{med([x['max_last'] for x in sel])}; rollbacks {med([x['rollbacks'] for x in sel])}")
                from collections import Counter
                print(f"  column: status {dict(Counter(x['col_status'] for x in sel))}; rows {med([x['col_rows'] for x in sel])}, core pieces {med([x['col_core'] for x in sel])}, formed {med([x['col_formed'] for x in sel])}, first disconnected {med([x['col_first_disc'] for x in sel])} {rng([x['col_first_disc'] for x in sel])}, break {med([x['col_break'] for x in sel])} {rng([x['col_break'] for x in sel])} (no break {sum(1 for x in sel if x['col_break'] is None)}), residence {med([x['col_residence'] for x in sel])} {rng([x['col_residence'] for x in sel])}, reformedAfterBreak {sum(1 for x in sel if x['col_reformed'])}/{len(sel)}, permanent releases {med([x['col_perm'] for x in sel])}, temporary {med([x['col_temp'] for x in sel])}, original rows connected frac {med([x['col_orig_connected_frac'] for x in sel])}")
                print(f"  longest-lived column: residence {med([x['cll_residence'] for x in sel])} {rng([x['cll_residence'] for x in sel])}, formed {med([x['cll_formed'] for x in sel])}, rows {med([x['cll_rows'] for x in sel])}, core {med([x['cll_core'] for x in sel])}, reformed {sum(1 for x in sel if x['cll_reformed'])}/{len(sel)}")
                print(f"  watched (most persistent) rows blocking at end: {sum(x['watched_blocking_at_end'] for x in sel)}/{sum(x['watched_n'] for x in sel)}")
print("\n== per replay")
for x in sorted(rows, key=lambda x: (x["arm"], x["bite"], str(x["seed"]), x["objective"])):
    print(f"  {x['arm']} {str(x['seed'])[:6]:>6} b{x['bite']} {x['objective']:<10} {'own' if x['own'] else 'oth'} ran {x['ran']:>3}/{x['horizon']:>3} ev {x['evals']:>9} band {str(x['band']):>4} cert {int(bool(x['cert']))} | entry {x['entry_n']} persist {x['persist']:>2} created {x['created']:>2} | nblk {x['nblk_first']}/{x['nblk_min']}/{x['nblk_last']} max {x['max_first']:.0f}/{x['max_min']:.0f}/{x['max_last']:.0f} | col {x['col_status']} rows {x['col_rows']} core {x['col_core']} disc {x['col_first_disc']} break {x['col_break']} res {x['col_residence']} reformed {x['col_reformed']} perm {x['col_perm']} temp {x['col_temp']} | cll res {x['cll_residence']} formed {x['cll_formed']} rows {x['cll_rows']}")
