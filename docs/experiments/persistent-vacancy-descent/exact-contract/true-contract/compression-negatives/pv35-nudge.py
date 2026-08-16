import json, subprocess, sys, os
# Binding-stack nudge: shift only the k deepest-anchored pieces inward by delta mm,
# leave the rest untouched, legalize with the warm-started separator.
# args: fixture k delta tag
ROOT='/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a'
B=f'{ROOT}/target/release/examples/general_request_benchmark'
F=f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
ARGS="1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 0 0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0".split()
REQ='ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3'
SRC=sys.argv[1]; k=int(sys.argv[2]); delta=float(sys.argv[3]); tag=sys.argv[4]
D=f'/var/lib/t3/tmp/pv35-{tag}'; os.makedirs(D, exist_ok=True)
pl=json.load(open(SRC))['placements']
deep=sorted(pl, key=lambda p: -p['translateLongAxis'])[:k]
ids={p['pieceId'] for p in deep}
sq=[dict(p, translateLongAxis=p['translateLongAxis']-delta) if p['pieceId'] in ids else dict(p) for p in pl]
json.dump({"schemaVersion":1,"description":f"nudge {tag} k={k} d={delta}","requestSha256":REQ,
  "expectedPlacementFingerprint":"nudge","reportedDepthMm":200.0,"independentDepthMm":200.0,
  "provenance":{"producedBy":f"nudge {tag}"},"placements":sq}, open(f'{D}/nudged.json','w'), indent=1)
r=subprocess.run([B,F]+ARGS+['0',SRC,'200.0',f'{D}/nudged.json'],capture_output=True,text=True)
try:
    bt=json.loads(r.stdout)['relaxedDiagnostics']['coupledDynamicSeparator']['boundaryProjectionTreatment']
except Exception:
    print(f'{tag}: separator crash {r.stderr[-120:]}', flush=True); sys.exit(1)
open(f'{D}/sep.json','w').write(r.stdout)
d0=bt['finalDepthMm']
json.dump({"schemaVersion":1,"description":f"nudge {tag} legalized","requestSha256":REQ,
  "expectedPlacementFingerprint":"nudge","reportedDepthMm":d0,"independentDepthMm":d0,
  "provenance":{"producedBy":f"nudge {tag} separator-legalized"},"placements":bt['finalPlacements']},
  open(f'{D}/legalized.json','w'), indent=1)
print(f'{tag} k={k} d={delta}: LEGALIZED DEPTH: {d0:.3f}', flush=True)
