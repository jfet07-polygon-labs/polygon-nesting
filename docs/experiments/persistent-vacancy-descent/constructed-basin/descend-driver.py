import json, subprocess, os, sys
ROOT='/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a'
B=f'{ROOT}/target/release/examples/general_request_benchmark'
F=f'{ROOT}/tests/fixtures/mixed-61/mixed61-request.json'
ARGS="1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 0 0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0".split()
REQ='dfd2ceecf02efe3475e3344dfefbfb2a2a5bd8a673008b449f5689507c933ba1'
SRC=sys.argv[1]; D=sys.argv[2]; os.makedirs(D, exist_ok=True)
src=json.load(open(SRC))
pv=src['relaxedDiagnostics']['coupledDynamicSeparator']['persistentVacancyPopulation']
def wf(path, placements, fp, ind, hop):
    json.dump({"schemaVersion":1,"description":f"mode20 constructed basin hop {hop}","requestSha256":REQ,
      "expectedPlacementFingerprint":fp,"reportedDepthMm":ind,"independentDepthMm":ind,
      "provenance":{"producedBy":f"mode20 descend hop {hop}"},"placements":placements}, open(path,'w'), indent=1)
fixture=f'{D}/parent-start.json'
best=pv['independentDepthMm']
wf(fixture, pv['finalPlacements'], pv['finalPlacementFingerprint'], best, -1)
start=best; salt=0; hop=0; stalls=0; seen=set()
while hop < 60 and stalls < 10:
    target = round(best,3) + 0.8 + salt*0.001
    mode = '17' if stalls > 0 else '11'
    r=subprocess.run([B,F]+ARGS+[mode,fixture,f'{target:.3f}'],capture_output=True,text=True)
    try:
        p=json.loads(r.stdout)['relaxedDiagnostics']['coupledDynamicSeparator']['persistentVacancyPopulation']
    except Exception:
        print(f'hop {hop}: crash {r.stderr[-160:]}', flush=True); break
    if not p.get('exactValid'):
        print(f'hop {hop}: m{mode} fail {(p.get("failureReason") or "?")[:40]}', flush=True); salt+=1; stalls+=1; continue
    ind=p['independentDepthMm']; fp=p['finalPlacementFingerprint']
    fresh = fp not in seen
    improved = ind < best - 1e-9
    print(f'hop {hop}: m{mode} t={target:.3f} -> {ind:.3f} best={best:.3f} fresh={fresh}', flush=True)
    if improved and fresh:
        seen.add(fp); stalls=0
        open(f'{D}/hop{hop:03d}.json','w').write(r.stdout)
        nf=f'{D}/parent-hop{hop:03d}.json'; wf(nf, p['finalPlacements'], fp, ind, hop)
        fixture=nf; best=ind; hop+=1
    else:
        salt+=1; stalls+=1
print(f'CONSTRUCTED START: {start:.3f} BEST: {best:.3f} YIELD: {start-best:.3f}', flush=True)
