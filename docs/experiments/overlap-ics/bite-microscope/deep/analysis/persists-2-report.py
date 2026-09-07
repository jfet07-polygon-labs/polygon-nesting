#!/usr/bin/env python3
"""persists-2-report.py -- tables from analysis/persists-census.json (written by persists-1-census.py).
Usage: python3 persists-2-report.py
"""
import json, statistics as st
R = json.load(open("/var/lib/t3/tmp/astra/deep/analysis/persists-census.json"))
def f(v, d=1): return "-" if v is None else (f"{v:.{d}f}" if isinstance(v, float) else str(v))
def med(vals): 
    vals = [v for v in vals if v is not None]
    return f"{st.median(vals):.1f}" if vals else "-"
def rng(vals):
    vals = [v for v in vals if v is not None]
    return f"{min(vals):.0f}-{max(vals):.0f}" if vals else "-"
def grp(r): return (r["arm"], r["bite"], r["published"])

print("== attempts per arm / bite / outcome (attempts per bite)")
from collections import Counter
c = Counter((r["arm"], r["bite"], r["published"]) for r in R)
for k in sorted(c): print("  ", k, c[k])
print("  attempts per (arm,seed,bite) >1:", [(r["arm"], r["seed"], r["bite"], r["attempt"]) for r in R if r["attempt"] > 0])

print("\n== PER ATTEMPT: entry / stop (handed back) / last-sweep census and persistence")
hdr = "arm seed bite pub iters stop rb restoredTo | entry n(pair/L/R/B/T) max | stop n(pair/L/R/B/T) max | persist/released/created(stop) | last n max | persist/released/created(last)"
print(hdr)
def kinds(c): k = c["kinds"]; return f"{c['n']}({k['pair']}/{k['L']}/{k['R']}/{k['B']}/{k['T']})"
for r in sorted(R, key=lambda r: (r["arm"], r["bite"], not r["published"], str(r["seed"]))):
    print(f"{r['arm']} {str(r['seed'])[:6]:>6} b{r['bite']} {'P' if r['published'] else 'F'} {r['iterations']:>4} {r['stop']:<9} {len(r['rollbacks'])} {f(r['restoredTo']):>4} | "
          f"{kinds(r['entry']):>18} {r['entry']['max_um']:7.1f} | {kinds(r['stop_census']):>18} {r['stop_census']['max_um']:8.1f} | "
          f"{r['persist_stop']:>2}/{r['released_stop']:>2}/{r['created_stop']:>3} | {r['last']['n']:>3} {r['last']['max_um']:8.1f} | {r['persist_last']:>2}/{r['released_last']:>2}/{r['created_last']:>3}")

print("\n== GROUP SUMMARY (median [range]) by arm/bite/outcome")
groups = sorted(set(grp(r) for r in R))
def gs(sel, key, fn=lambda r: r):
    vals = [fn(r) for r in sel]
    return f"{med(vals)} [{rng(vals)}]"
for g in groups:
    sel = [r for r in R if grp(r) == g]
    print(f"\n-- arm {g[0]} bite {g[1]} {'published' if g[2] else 'failed'}: n={len(sel)}")
    print("  iterations              ", gs(sel, None, lambda r: r["iterations"]))
    print("  entry rows              ", gs(sel, None, lambda r: r["entry"]["n"]), " pair", gs(sel, None, lambda r: r["entry"]["kinds"]["pair"]),
          " boundary L/R/B/T", *(gs(sel, None, lambda r, s=s: r["entry"]["kinds"][s]) for s in "LRBT"))
    print("  entry residual um: pair med", gs(sel, None, lambda r: r["entry"]["residual_um"]["pair"]["med"] if r["entry"]["residual_um"]["pair"] else None),
          " boundary med", gs(sel, None, lambda r: r["entry"]["residual_um"]["boundary"]["med"] if r["entry"]["residual_um"]["boundary"] else None),
          " max", gs(sel, None, lambda r: r["entry"]["max_um"]))
    print("  stop rows (handed back) ", gs(sel, None, lambda r: r["stop_census"]["n"]), " pair", gs(sel, None, lambda r: r["stop_census"]["kinds"]["pair"]),
          " boundary L/R/B/T", *(gs(sel, None, lambda r, s=s: r["stop_census"]["kinds"][s]) for s in "LRBT"), " max um", gs(sel, None, lambda r: r["stop_census"]["max_um"]))
    print("  stop residual um: pair med", gs(sel, None, lambda r: r["stop_census"]["residual_um"]["pair"]["med"] if r["stop_census"]["residual_um"]["pair"] else None),
          " boundary med", gs(sel, None, lambda r: r["stop_census"]["residual_um"]["boundary"]["med"] if r["stop_census"]["residual_um"]["boundary"] else None))
    print("  last-sweep rows         ", gs(sel, None, lambda r: r["last"]["n"]), " max um", gs(sel, None, lambda r: r["last"]["max_um"]))
    print("  entry->stop persist     ", gs(sel, None, lambda r: r["persist_stop"]), " released", gs(sel, None, lambda r: r["released_stop"]),
          " created", gs(sel, None, lambda r: r["created_stop"]), " persist pair/boundary", gs(sel, None, lambda r: r["persist_stop_pair"]), gs(sel, None, lambda r: r["persist_stop_boundary"]))
    print("  entry->last persist     ", gs(sel, None, lambda r: r["persist_last"]), " released", gs(sel, None, lambda r: r["released_last"]), " created", gs(sel, None, lambda r: r["created_last"]))
    print("  persisting rows' residual at stop (med um)", gs(sel, None, lambda r: r["persist_stop_residual_um"]["med"] if r["persist_stop_residual_um"] else None),
          " created rows' residual at stop (med um)", gs(sel, None, lambda r: r["created_stop_residual_um"]["med"] if r["created_stop_residual_um"] else None))
    print("  entry rows: first release iter med", gs(sel, None, lambda r: r["entry_first_release_iter"]["med"] if r["entry_first_release_iter"] else None),
          " q3", gs(sel, None, lambda r: r["entry_first_release_iter"]["q3"] if r["entry_first_release_iter"] else None),
          " never released", gs(sel, None, lambda r: r["entry_never_released"]), " released by iter 10", gs(sel, None, lambda r: r["entry_released_by_10"]),
          " by 50", gs(sel, None, lambda r: r["entry_released_by_50"]))
    print("  residence sweeps: entry rows med", gs(sel, None, lambda r: r["entry_residence_sweeps"]["med"]), " max", gs(sel, None, lambda r: r["entry_residence_sweeps"]["max"]),
          "; created rows med", gs(sel, None, lambda r: r["created_residence_sweeps"]["med"] if r["created_residence_sweeps"] else None), " max", gs(sel, None, lambda r: r["created_residence_sweeps"]["max"] if r["created_residence_sweeps"] else None))
    print("  rows ever blocking      ", gs(sel, None, lambda r: r["n_rows_ever"]), " of which created", gs(sel, None, lambda r: r["n_created_ever"]))
    print("  top10 persistent rows that are entry rows", gs(sel, None, lambda r: r["top10_entry_count"]),
          " top1 sweeps / iterations", gs(sel, None, lambda r: r["top10"][0]["sweeps"] / r["iterations"] if r["top10"] else None))
    print("  re-formation: rows w/ any re-formation", gs(sel, None, lambda r: r["reformed_rows_total"]), " organic (not at a rollback restore)", gs(sel, None, lambda r: r["reformed_rows_organic"]),
          " events total", gs(sel, None, lambda r: r["reform_events_total"]), " organic", gs(sel, None, lambda r: r["reform_events_organic"]),
          " entry rows re-formed organically", gs(sel, None, lambda r: r["entry_reformed_organic"]))
    print("  transfer: released rows' pieces", gs(sel, None, lambda r: r["released_pieces"]), " created@stop touching them", gs(sel, None, lambda r: r["created_stop_touching_released_pieces"]),
          " of created", gs(sel, None, lambda r: r["created_stop"]),
          "; pieces in entry blocking", gs(sel, None, lambda r: r["entry_pieces"]), " still blocking at stop", gs(sel, None, lambda r: r["pieces_persist_stop"]), " at last", gs(sel, None, lambda r: r["pieces_persist_last"]))
    print("  graph entry: components", gs(sel, None, lambda r: r["graph_entry"]["n_components"]), " largest pieces", gs(sel, None, lambda r: r["graph_entry"]["largest_pieces"]),
          " B-T spanning n=", sum(1 for r in sel if r["graph_entry"]["any_bt"]), " L-R spanning n=", sum(1 for r in sel if r["graph_entry"]["any_lr"]),
          " largest touches edges", sorted(Counter("".join(r["graph_entry"]["largest_edges"]) for r in sel).items()))
    print("  graph stop : components", gs(sel, None, lambda r: r["graph_stop"]["n_components"]), " largest pieces", gs(sel, None, lambda r: r["graph_stop"]["largest_pieces"]),
          " B-T spanning n=", sum(1 for r in sel if r["graph_stop"]["any_bt"]), " L-R spanning n=", sum(1 for r in sel if r["graph_stop"]["any_lr"]),
          " largest touches edges", sorted(Counter("".join(r["graph_stop"]["largest_edges"]) for r in sel).items()),
          " shared pieces with entry's largest", gs(sel, None, lambda r: r["largest_shared_pieces_stop"]))
    print("  graph last : components", gs(sel, None, lambda r: r["graph_last"]["n_components"]), " largest pieces", gs(sel, None, lambda r: r["graph_last"]["largest_pieces"]),
          " B-T spanning n=", sum(1 for r in sel if r["graph_last"]["any_bt"]), " L-R spanning n=", sum(1 for r in sel if r["graph_last"]["any_lr"]),
          " shared pieces with entry's largest", gs(sel, None, lambda r: r["largest_shared_pieces_last"]))
    print("  maxAfter um: first", gs(sel, None, lambda r: r["max_um"]["first"]), " min", gs(sel, None, lambda r: r["max_um"]["min"]), " min iter", gs(sel, None, lambda r: r["max_um"]["min_iter"]),
          " last", gs(sel, None, lambda r: r["max_um"]["last"]), " last-quarter median", gs(sel, None, lambda r: r["max_um"]["last_q_med"]))
    print("  after the min: frac sweeps >2x min", gs(sel, None, lambda r: r["after_min_frac_gt2x"]), " >5x min", gs(sel, None, lambda r: r["after_min_frac_gt5x"]),
          " first sweep >5 mm after min", gs(sel, None, lambda r: r["first_gt5mm_after_min_iter"]), " n with such", sum(1 for r in sel if r["first_gt5mm_after_min_iter"] is not None))
    print("  blocking count: first", gs(sel, None, lambda r: r["nblk"]["first"]), " min", gs(sel, None, lambda r: r["nblk"]["min"]), " min iter", gs(sel, None, lambda r: r["nblk"]["min_iter"]),
          " last", gs(sel, None, lambda r: r["nblk"]["last"]), " last-quarter median", gs(sel, None, lambda r: r["nblk"]["last_q_med"]))
    print("  rawAfter: first", gs(sel, None, lambda r: r["raw"]["first"]), " min", gs(sel, None, lambda r: r["raw"]["min"]), " min iter", gs(sel, None, lambda r: r["raw"]["min_iter"]), " last", gs(sel, None, lambda r: r["raw"]["last"]))
    print("  rollbacks per attempt", gs(sel, None, lambda r: len(r["rollbacks"])), " restoredTo", gs(sel, None, lambda r: r["restoredTo"]))

print("\n== ROLLBACK DETAIL (max um at the rollback sweep -> restored state -> next sweep; blocking count at -> next)")
for r in sorted(R, key=lambda r: (r["arm"], r["bite"], str(r["seed"]))):
    for d in r["rollback_detail"]:
        print(f"  {r['arm']} {str(r['seed'])[:6]:>6} b{r['bite']} {'P' if r['published'] else 'F'} rb at {d['at']:>4} -> {d['to']:>4}: max {f(d['max_at_um'])} -> {f(d['max_to_um'])} -> {f(d['max_after_um'])}; nblk {d['nblk_at']} -> {d['nblk_after']}")

print("\n== SEGMENTS between rollbacks (from-to: max first / min@iter / last, um)")
for r in sorted(R, key=lambda r: (r["arm"], r["bite"], str(r["seed"]))):
    print(f"  {r['arm']} {str(r['seed'])[:6]:>6} b{r['bite']} {'P' if r['published'] else 'F'}: " + "; ".join(f"{s['from']}-{s['to']}: {s['max_first']:.0f}/{s['max_min']:.0f}@{s['max_min_iter']}/{s['max_last']:.0f}" for s in r["segments"]))

print("\n== TOP-6 most persistent rows per attempt (rid kind sweeps first-last entry? maxum)")
for r in sorted(R, key=lambda r: (r["arm"], r["bite"], str(r["seed"]))):
    items = []
    for t in r["top10"][:6]:
        k = t["kind"]; kk = f"{k[1]}-{k[2]}" if k[0] == "pair" else f"{k[1]}{k[2]}"
        items.append(f"{t['rid']}({kk}) {t['sweeps']} [{t['first']}-{t['last']}]{'E' if t['entry'] else 'c'} {t['max_um']:.0f}")
    print(f"  {r['arm']} {str(r['seed'])[:6]:>6} b{r['bite']} {'P' if r['published'] else 'F'} it={r['iterations']}: " + "; ".join(items))
