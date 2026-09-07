#!/usr/bin/env python3
"""persists-10-rollbacks.py -- after each rollback (restore to the last new minimum), is the next sweep's end
state near the restored state or already far above it? From persists-census.json (rollback_detail).
Usage: python3 persists-10-rollbacks.py
"""
import json, statistics as st
R = json.load(open("/var/lib/t3/tmp/astra/deep/analysis/persists-census.json"))
for arm in "AB":
    for pub in (False, True):
        d = [x for r in R if r["arm"] == arm and r["published"] == pub for x in r["rollback_detail"] if x["max_after_um"] is not None]
        if not d: continue
        ratio = [x["max_after_um"] / x["max_to_um"] for x in d]
        print(f"arm {arm} {'published' if pub else 'failed'}: rollbacks {len(d)}; at-iteration med {st.median(x['at'] for x in d):.0f}; restored max um med {st.median(x['max_to_um'] for x in d):.0f}; next sweep max um med {st.median(x['max_after_um'] for x in d):.0f}; next/restored ratio med {st.median(ratio):.2f}; next > 2x restored: {sum(1 for q in ratio if q > 2)}/{len(d)}; next > 5 mm: {sum(1 for x in d if x['max_after_um'] > 5000)}/{len(d)}; max at the rollback sweep med {st.median(x['max_at_um'] for x in d):.0f} um")
