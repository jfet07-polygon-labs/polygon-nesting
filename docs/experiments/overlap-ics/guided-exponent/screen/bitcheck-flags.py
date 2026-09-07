#!/usr/bin/env python3
"""bitcheck-flags.py <new-binary> <old-binary> [extra flags...] - the SAME configured path (e.g. --proxymargin=8 --guidedexponent=1)
must be bit-identical between two binaries on three fixed-work cells (seeds 0, 3, 7; budget 40000; itercap 50). This is the
treatment-identity check GPT-6 Astra review 6 Q12 asks for ("default p = 2 identity alone does not establish treatment identity")."""
import json,subprocess,sys,hashlib
NEW=sys.argv[1]; OLD=sys.argv[2]; EXTRA=sys.argv[3:]
R='/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
DROP=('Ns','Seconds','PerSecond','Sha256','sha256','Millis','elapsed','Rate','wallSeconds')
SKIP={'wall','executableSha256','buildFeatures','measured','instrument','satSeparatedCalls','satDiscardedCalls',
      'convexCellGapQueries','cellPairBoxTests','adaptiveStepCeiling','adaptiveStepFloor','compressStartStep','scheduleProfile'}
def strip(o):
    if isinstance(o,dict): return {k:strip(v) for k,v in o.items() if not (k.endswith(DROP) or k in SKIP)}
    if isinstance(o,list): return [strip(v) for v in o]
    if isinstance(o,float): return repr(o)
    return o
ok=True
print('flags:', ' '.join(EXTRA) or '(none)')
for seed in (0,3,7):
    hs=[]
    for b in (OLD,NEW):
        out=subprocess.run([b,'--cell=cutclose','--request='+R,'--edge=5','--pair=5','--budget=40000','--orders=1','--workers=8',
                            '--arm=control','--itercap=50','--exploreratio=0.95']+EXTRA+['--seed=%d'%seed],capture_output=True,text=True,
                           cwd='/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a')
        if out.returncode!=0: print('seed',seed,b,'RC',out.returncode,out.stderr[-300:]); ok=False; break
        d=json.loads(out.stdout); hs.append((hashlib.sha256(json.dumps(strip(d),sort_keys=True).encode()).hexdigest()[:16],d.get('finalPoseDigest','')[:16],d['outcome']['depthMm'],d.get('guidedExponent',2.0),d.get('proxyMarginUm',0)))
    if len(hs)==2:
        same=hs[0]==hs[1]; ok&=same
        print('seed %d %s  depth %.6f  poses %s  (guidedExponent %s, proxyMarginUm %s)'%(seed,'IDENTICAL' if same else 'DIFFERENT',hs[1][2],'same' if hs[0][1]==hs[1][1] else 'DIFFER',hs[1][3],hs[1][4]))
print('ALL_IDENTICAL' if ok else 'MISMATCH')
