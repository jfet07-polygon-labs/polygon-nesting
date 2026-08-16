import json, subprocess, os
ROOT='/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a'
B=f'{ROOT}/target/release/examples/general_request_benchmark'
F=f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
FIX=__import__('sys').argv[1]
SEED=__import__("sys").argv[4] if len(__import__("sys").argv)>4 else "0"
ARGS=f"1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 {SEED} 0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0".split()
REQ='1bb905674beeaaa0f2cd4e50488daf25ac06c9279a7cd77db00cd8b730ba0161'
D='/var/lib/t3/tmp/pv22-alt-'+__import__('sys').argv[3]; os.makedirs(D, exist_ok=True)
def wf(path, placements, ind, tag):
    json.dump({"schemaVersion":1,"description":f"alternation {tag}","requestSha256":REQ,
      "expectedPlacementFingerprint":"alternation","reportedDepthMm":ind,"independentDepthMm":ind,
      "provenance":{"producedBy":f"alternation {tag}"},"placements":placements}, open(path,'w'), indent=1)
def sep_arm(fixture, tag):
    # legacy separator warm-started from fixture
    r=subprocess.run([B,F]+ARGS+['0',FIX,'200.0',fixture],capture_output=True,text=True)
    d=json.loads(r.stdout)
    bt=d['relaxedDiagnostics']['coupledDynamicSeparator']['boundaryProjectionTreatment']
    open(f'{D}/{tag}.json','w').write(r.stdout)
    return bt['finalDepthMm'], bt['finalPlacements']
def descent_arm(fixture, best, tag):
    # m11/m17 salted chain (short: 6 stalls)
    salt=0; stalls=0; hop=0; cur=fixture; b=best
    while hop<20 and stalls<6:
        target=round(b,3)+0.8+salt*0.001
        mode='17' if stalls>0 else '11'
        r=subprocess.run([B,F]+ARGS+[mode,cur,f'{target:.3f}'],capture_output=True,text=True)
        try: p=json.loads(r.stdout)['relaxedDiagnostics']['coupledDynamicSeparator']['persistentVacancyPopulation']
        except Exception: print(f'{tag} crash',flush=True); break
        if not p.get('exactValid'): salt+=1; stalls+=1; continue
        ind=p['independentDepthMm']
        if ind < b-1e-9:
            stalls=0; b=ind
            nf=f'{D}/{tag}-hop{hop:03d}.json'; wf(nf,p['finalPlacements'],ind,f'{tag} hop{hop}')
            cur=nf; hop+=1
        else: salt+=1; stalls+=1
    return b, cur
import sys
best=float(sys.argv[2]); fixture=sys.argv[1]
for cycle in range(6):
    sd, sp = sep_arm(fixture, f'sep{cycle}')
    print(f'cycle {cycle}: separator -> {sd:.3f}', flush=True)
    if sd < best-1e-9:
        best=sd
        nf=f'{D}/sep{cycle}-parent.json'; wf(nf, sp, sd, f'sep{cycle}')
        fixture=nf
    db, df = descent_arm(fixture, best, f'des{cycle}')
    print(f'cycle {cycle}: descent -> {db:.3f}', flush=True)
    if db < best-1e-9:
        best=db; fixture=df
    else:
        if sd >= best-1e-9 and db >= best-1e-9:
            print(f'cycle {cycle}: fixpoint at {best:.3f}', flush=True); break
print(f'ALTERNATION BEST: {best:.3f}', flush=True)
