#!/usr/bin/env python3
"""Read a --bitemicroscope=1 document and print what the two ranked causes predict."""
import json,sys,statistics as st,collections
d=json.load(open(sys.argv[1])); m=d['biteMicroscope']; band=0.004
print(f"== {sys.argv[1].split('/')[-1]}  margin {d.get('proxyMarginUm',0)} um  depth {d['outcome']['depthMm']:.3f}  exposure: {m['exposure']['status']}")
for b in m['bites']:
    print(f"\n-- bite {b['ordinal']} [{b['retainedAs']}] {b['parentDepthMm']:.3f} -> {b['targetDepthMm']:.3f}: {b['masterIterations']} iterations, published {b['published']}, cutMoved {len(b['cutMoved'])}, initialPositive {len(b['initialPositive'])}")
    for sp in b['separations']:
        sw=sp['sweeps']; n=len(sw)
        maxs=[s['maxAfterMm']*1000 for s in sw]
        zone=collections.Counter('<=4' if x<=4 else '4-20' if x<=20 else '20-50' if x<=50 else '50-200' if x<=200 else '>200' for x in maxs)
        print(f"   stop {sp['stop']}; iterations {n}; end-of-sweep max violation (um) by zone: {dict(zone)}; first iteration with max<=50um: {next((s['iteration'] for s in sw if s['maxAfterMm']*1000<=50),None)}, <=20um: {next((s['iteration'] for s in sw if s['maxAfterMm']*1000<=20),None)}, <=4um: {next((s['iteration'] for s in sw if s['maxAfterMm']*1000<=band*1000),None)}")
        ev_all=sum(s['evaluationsAllWorkers'] for s in sw); ev_win=sum(s['evaluationsWinner'] for s in sw)
        print(f"   evaluations all workers {ev_all}, winner {ev_win}; relocates per sweep median {st.median(len(s['relocates']) for s in sw)}; order size median {st.median(len(s['order']) for s in sw)}")
        # blocking rows persistence
        pers=collections.Counter(); 
        for s in sw:
            for rid,res in s['blocking']: pers[rid]+=1
        top=pers.most_common(5); print(f"   distinct blocking rows {len(pers)}; most persistent (rowId, iterations present): {top}")
        # fine-CD exits
        rel=[r for s in sw for r in s['relocates']]; nz=[r for r in rel if r['fineCd']['exitMaxMm']>0]
        unmoved=[r for r in nz if r['dxMm']==0 and r['dyMm']==0 and r['dthetaDeg']==0]
        moved=[r for r in nz if not (r['dxMm']==0 and r['dyMm']==0 and r['dthetaDeg']==0)]
        ex=[r['fineCd']['exitMaxMm']*1000 for r in nz]
        print(f"   relocates {len(rel)}; nonzero fine-CD exits {len(nz)} ({100*len(nz)/max(len(rel),1):.0f}%): exit max median {st.median(ex):.1f} um, <=8um {sum(1 for x in ex if x<=8)}, 8-20 {sum(1 for x in ex if 8<x<=20)}, 20-50 {sum(1 for x in ex if 20<x<=50)}, >50 {sum(1 for x in ex if x>50)}")
        print(f"     of which UNMOVED (local minimum of the weighted incident objective, any +- step worse): {len(unmoved)} (exit max median {st.median([r['fineCd']['exitMaxMm']*1000 for r in unmoved]) if unmoved else 0:.1f} um); MOVED then stopped by limits: {len(moved)} (exit max median {st.median([r['fineCd']['exitMaxMm']*1000 for r in moved]) if moved else 0:.1f} um)")
        ratios=[max(r['fineCd']['finalStepXMm'],r['fineCd']['finalStepYMm'])*1000 for r in moved]
        print(f"     moved exits: final translation step median {st.median(ratios) if ratios else 0:.1f} um (limit median {st.median([r['fineCd']['translationLimitMm']*1000 for r in moved]) if moved else 0:.0f} um), candidate pairs median {st.median([r['fineCd']['candidatePairs'] for r in moved]) if moved else 0}")
        # residual worsened by later relocates? rows changed with other endpoint status
        stat=collections.Counter(); births=collections.Counter()
        for s in sw:
            for r in s['relocates']:
                for rc in r.get('rows',[]):
                    rid,vb,va,status=rc[0],rc[1],rc[2],rc[3]
                    stat[status]+=1
                    if vb<=0 and va>0: births[status]+=1
        print(f"   changed rows by other-endpoint status: {dict(stat)}; conflict BIRTHS (row went 0 -> positive) by status: {dict(births)}")
