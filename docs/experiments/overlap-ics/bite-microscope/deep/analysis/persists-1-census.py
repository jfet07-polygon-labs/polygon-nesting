#!/usr/bin/env python3
"""persists-1-census.py  -- WHAT PERSISTS in the retained attempts of the depth microscope.

Reads the 42 deep-<A|B>-wall10s-s<seed>.json documents (read-only) and, for every retained
attempt, computes from biteMicroscope.bites[].separations[]:
  * the blocking rows at entry (entryBlocking), at the stop (stopBlocking = the state handed
    back; for a published attempt it is the last sweep) and at the live end (last sweep),
    by kind (pair / boundary side) with residual distributions;
  * entry rows persisting / released / created (entry vs stop, entry vs last sweep);
  * the blocking graph (vertices: 61 pieces + L,R,B,T strip edges; edges: blocking rows):
    component count, largest component (pieces), whether a component joins B and T or L and R,
    the piece overlap of the largest component at the stop with the entry's;
  * residence of rows over the sweeps (sweeps present), the most persistent rows and whether
    they are entry rows; the first release iteration of entry rows;
  * re-formation (row present, absent, present again), separating episodes that start on the
    sweep right after a rollback (a restoration) from the others;
  * transfer: created rows whose pieces were endpoints of released entry rows; piece-level
    persistence;
  * trajectory of maxAfterMm, blocking count and rawAfter: first / min (iteration) / last,
    segments between rollbacks, and the values around every rollback.
Writes analysis/persists-census.json and prints per-attempt and per-group tables.
Usage: python3 persists-1-census.py
"""
import glob, json, os, statistics as st
DEEP = "/var/lib/t3/tmp/astra/deep"
OUT = os.path.join(DEEP, "analysis", "persists-census.json")
N = 61
PAIRS = N * (N - 1) // 2  # 1830
SIDES = "LRBT"
TREAT_SUCC = {4872519857840070441, 5671471283886933426, 11151532166486038253, 17316774662183274765, 18390115156762500293}
CTRL_SUCC = {10636268072709740349}

# pair index -> (i, j)
PAIR = {}
for i in range(N):
    for j in range(i + 1, N):
        PAIR[i * N - i * (i + 1) // 2 + (j - i - 1)] = (i, j)
assert len(PAIR) == PAIRS and max(PAIR) == PAIRS - 1

def decode(rid):
    if rid < PAIRS:
        return ("pair",) + PAIR[rid]
    k = rid - PAIRS
    return ("boundary", k // 4, SIDES[k % 4])

def endpoints(rid):
    d = decode(rid)
    return (d[1], d[2]) if d[0] == "pair" else (d[1], "E" + d[2])

def q(vals):
    if not vals:
        return None
    v = sorted(vals)
    def pct(p):
        k = (len(v) - 1) * p
        f = int(k); c = min(f + 1, len(v) - 1)
        return v[f] + (v[c] - v[f]) * (k - f)
    return {"n": len(v), "min": v[0], "q1": pct(.25), "med": pct(.5), "q3": pct(.75), "max": v[-1]}

def census(rows):
    kinds = {"pair": 0, "L": 0, "R": 0, "B": 0, "T": 0}
    res = {"pair": [], "boundary": []}
    for rid, r in rows:
        d = decode(rid)
        if d[0] == "pair":
            kinds["pair"] += 1; res["pair"].append(r * 1000)
        else:
            kinds[d[2]] += 1; res["boundary"].append(r * 1000)
    return {"n": len(rows), "kinds": kinds, "residual_um": {k: q(v) for k, v in res.items()},
            "max_um": max([r * 1000 for _, r in rows], default=0.0)}

def components(rows):
    parent = {}
    def find(x):
        parent.setdefault(x, x)
        while parent[x] != x:
            parent[x] = parent[parent[x]]; x = parent[x]
        return x
    def union(a, b):
        ra, rb = find(a), find(b)
        if ra != rb: parent[ra] = rb
    for rid, _ in rows:
        a, b = endpoints(rid); union(a, b)
    comps = {}
    for v in list(parent):
        comps.setdefault(find(v), set()).add(v)
    out = []
    for c in comps.values():
        pieces = {v for v in c if isinstance(v, int)}
        edges = {v[1] for v in c if isinstance(v, str)}
        out.append({"pieces": sorted(pieces), "edges": sorted(edges),
                    "bt": "B" in edges and "T" in edges, "lr": "L" in edges and "R" in edges})
    out.sort(key=lambda c: -len(c["pieces"]))
    return out

def comp_summary(rows):
    cs = components(rows)
    largest = cs[0] if cs else {"pieces": [], "edges": [], "bt": False, "lr": False}
    return {"n_components": len(cs), "largest_pieces": len(largest["pieces"]), "largest_edges": largest["edges"],
            "largest_bt": largest["bt"], "largest_lr": largest["lr"],
            "any_bt": any(c["bt"] for c in cs), "any_lr": any(c["lr"] for c in cs),
            "sizes": [len(c["pieces"]) for c in cs], "_largest": set(largest["pieces"])}

def series_stats(vals, iters):
    if not vals: return None
    imin = min(range(len(vals)), key=lambda k: vals[k])
    return {"first": vals[0], "min": vals[imin], "min_iter": iters[imin], "last": vals[-1],
            "last_q_med": st.median(vals[-max(1, len(vals) // 4):])}

def analyse(path):
    doc = json.load(open(path))
    m = doc["biteMicroscope"]
    seed = doc["seed"]; arm = "A" if doc.get("guidedExponent", 2) == 2 else "B"
    recs = []
    for bite in m["bites"]:
        for sep in bite["separations"]:
            sweeps = sorted(sep["sweeps"], key=lambda s: s["iteration"])
            iters = [s["iteration"] for s in sweeps]
            entry = sep["entryBlocking"]; stop = sep["stopBlocking"]; last = sweeps[-1]["blocking"]
            eids = {r for r, _ in entry}; sids = {r for r, _ in stop}; lids = {r for r, _ in last}
            rollbacks = sep["rollbacks"]; restore_starts = {a + 1 for a, _ in rollbacks}
            # residence over sweeps
            present = {}
            for s in sweeps:
                for rid, res in s["blocking"]:
                    present.setdefault(rid, []).append((s["iteration"], res))
            # episodes
            ep_info = {}
            for rid, lst in present.items():
                its = [i for i, _ in lst]; its_set = set(its)
                episodes = []; start = None
                for i in iters:
                    if i in its_set and start is None: start = i
                    if i not in its_set and start is not None:
                        episodes.append(start); start = None
                if start is not None: episodes.append(start)
                reform = [e for e in episodes[1:]]
                organic = [e for e in reform if e not in restore_starts]
                ep_info[rid] = {"sweeps": len(its), "first": its[0], "last": its[-1], "episodes": len(episodes),
                                "reform_total": len(reform), "reform_organic": len(organic),
                                "max_um": max(r for _, r in lst) * 1000, "entry": rid in eids}
            # entry rows: first release iteration
            first_release = {}
            for rid in eids:
                its_set = {i for i, _ in present.get(rid, [])}
                rel = next((i for i in iters if i not in its_set), None)
                first_release[rid] = rel  # None = never released within the attempt
            rel_iters = [v for v in first_release.values() if v is not None]
            entry_res = [ep_info[r]["sweeps"] if r in ep_info else 0 for r in eids]
            created_res = [v["sweeps"] for r, v in ep_info.items() if r not in eids]
            top = sorted(ep_info.items(), key=lambda kv: (-kv[1]["sweeps"], kv[0]))[:10]
            # transfer
            released = eids - sids
            rel_pieces = set()
            for rid in released:
                rel_pieces |= {e for e in endpoints(rid) if isinstance(e, int)}
            created = sids - eids
            created_touch = sum(1 for rid in created if any(isinstance(e, int) and e in rel_pieces for e in endpoints(rid)))
            ep_pieces = lambda ids: {e for rid in ids for e in endpoints(rid) if isinstance(e, int)}
            entry_pieces, stop_pieces, last_pieces = ep_pieces(eids), ep_pieces(sids), ep_pieces(lids)
            # graph
            ce, cs_, cl = comp_summary(entry), comp_summary(stop), comp_summary(last)
            shared_stop = len(ce["_largest"] & cs_["_largest"]); shared_last = len(ce["_largest"] & cl["_largest"])
            for c in (ce, cs_, cl): c.pop("_largest")
            # trajectories
            mx = [s["maxAfterMm"] * 1000 for s in sweeps]; nb = [len(s["blocking"]) for s in sweeps]
            raw = [s["rawAfter"] for s in sweeps]
            by_iter = {s["iteration"]: s for s in sweeps}
            rb_detail = []
            for a, t in rollbacks:
                rb_detail.append({"at": a, "to": t,
                                  "max_at_um": by_iter[a]["maxAfterMm"] * 1000 if a in by_iter else None,
                                  "max_to_um": by_iter[t]["maxAfterMm"] * 1000 if t in by_iter else None,
                                  "max_after_um": by_iter[a + 1]["maxAfterMm"] * 1000 if a + 1 in by_iter else None,
                                  "nblk_at": len(by_iter[a]["blocking"]) if a in by_iter else None,
                                  "nblk_after": len(by_iter[a + 1]["blocking"]) if a + 1 in by_iter else None})
            # segments between rollbacks
            bounds = [iters[0]] + [a + 1 for a, _ in rollbacks] + [iters[-1] + 1]
            segs = []
            for b0, b1 in zip(bounds, bounds[1:]):
                idx = [k for k, i in enumerate(iters) if b0 <= i < b1]
                if idx:
                    segs.append({"from": iters[idx[0]], "to": iters[idx[-1]], "max_first": mx[idx[0]], "max_min": min(mx[k] for k in idx),
                                 "max_min_iter": iters[min(idx, key=lambda k: mx[k])], "max_last": mx[idx[-1]]})
            ms = series_stats(mx, iters)
            # after the minimum: how often above 2x and 5x the minimum, and the first sweep after the min above 5 mm
            kmin = iters.index(ms["min_iter"]); after = mx[kmin + 1:]
            blow = next((iters[kmin + 1 + k] for k, v in enumerate(after) if v > 5000), None)
            rec = {"arm": arm, "p": 2 if arm == "A" else 1, "seed": seed, "bite": bite["ordinal"], "published": bite["published"],
                   "attempt": sep["attempt"], "capsule": sep["capsule"], "iterations": sep["iterations"], "stop": sep["stop"],
                   "rollbacks": rollbacks, "restoredTo": sep.get("restoredToIteration"), "strikes": sep.get("strikes"),
                   "minRaw": sep.get("minRaw"), "cutMoved": len(bite["cutMoved"]), "initialPositive": len(bite["initialPositive"]),
                   "entry": census(entry), "stop_census": census(stop), "last": census(last),
                   "stop_is_last": sorted(stop) == sorted(last),
                   "persist_stop": len(eids & sids), "released_stop": len(eids - sids), "created_stop": len(sids - eids),
                   "persist_last": len(eids & lids), "released_last": len(eids - lids), "created_last": len(lids - eids),
                   "persist_stop_pair": sum(1 for r in eids & sids if r < PAIRS), "persist_stop_boundary": sum(1 for r in eids & sids if r >= PAIRS),
                   "persist_stop_residual_um": q([res * 1000 for rid, res in stop if rid in eids]),
                   "created_stop_residual_um": q([res * 1000 for rid, res in stop if rid not in eids]),
                   "graph_entry": ce, "graph_stop": cs_, "graph_last": cl,
                   "largest_shared_pieces_stop": shared_stop, "largest_shared_pieces_last": shared_last,
                   "entry_pieces": len(entry_pieces), "stop_pieces": len(stop_pieces), "pieces_persist_stop": len(entry_pieces & stop_pieces),
                   "pieces_persist_last": len(entry_pieces & last_pieces),
                   "created_stop_touching_released_pieces": created_touch, "released_pieces": len(rel_pieces),
                   "entry_first_release_iter": q(rel_iters), "entry_never_released": sum(1 for v in first_release.values() if v is None),
                   "entry_released_by_10": sum(1 for v in rel_iters if v <= 10), "entry_released_by_50": sum(1 for v in rel_iters if v <= 50),
                   "entry_residence_sweeps": q(entry_res), "created_residence_sweeps": q(created_res),
                   "n_rows_ever": len(ep_info), "n_created_ever": sum(1 for r in ep_info if r not in eids),
                   "top10": [{"rid": rid, "kind": decode(rid), **v} for rid, v in top],
                   "top10_entry_count": sum(1 for _, v in top if v["entry"]),
                   "reformed_rows_total": sum(1 for v in ep_info.values() if v["reform_total"] > 0),
                   "reformed_rows_organic": sum(1 for v in ep_info.values() if v["reform_organic"] > 0),
                   "reform_events_total": sum(v["reform_total"] for v in ep_info.values()),
                   "reform_events_organic": sum(v["reform_organic"] for v in ep_info.values()),
                   "entry_reformed_organic": sum(1 for r in eids if r in ep_info and ep_info[r]["reform_organic"] > 0),
                   "entry_rows_released_then_reformed_any": sum(1 for r in eids if r in ep_info and ep_info[r]["reform_total"] > 0),
                   "max_um": ms, "nblk": series_stats(nb, iters), "raw": series_stats(raw, iters),
                   "after_min_frac_gt2x": (sum(1 for v in after if v > 2 * ms["min"]) / len(after)) if after else None,
                   "after_min_frac_gt5x": (sum(1 for v in after if v > 5 * ms["min"]) / len(after)) if after else None,
                   "first_gt5mm_after_min_iter": blow, "segments": segs, "rollback_detail": rb_detail,
                   "max_series_um": [round(v, 1) for v in mx], "nblk_series": nb}
            recs.append(rec)
    return recs

def main():
    recs = []
    for path in sorted(glob.glob(os.path.join(DEEP, "deep-*-wall10s-s*.json"))):
        recs.extend(analyse(path))
    json.dump(recs, open(OUT, "w"), indent=1)
    print(f"{len(recs)} attempts written to {OUT}")

if __name__ == "__main__":
    main()
