import json, subprocess, sys, os
# Forced compression: affine-squeeze all anchors along the depth (long) axis by factor f,
# creating deliberate distributed overlaps, then legalize with the warm-started separator.
# args: fixture factor tag
ROOT='/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a'
B=f'{ROOT}/target/release/examples/general_request_benchmark'
F=f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
ARGS="1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 0 0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0".split()
REQ='ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3'
SRC=sys.argv[1]; f=float(sys.argv[2]); tag=sys.argv[3]
D=f'/var/lib/t3/tmp/pv34-{tag}'; os.makedirs(D, exist_ok=True)
pl=json.load(open(SRC))['placements']
tmin=min(p['translateLongAxis'] for p in pl)
sq=[dict(p, translateLongAxis=tmin+(p['translateLongAxis']-tmin)*f) for p in pl]
json.dump({"schemaVersion":1,"description":f"squeeze {tag} f={f}","requestSha256":REQ,
  "expectedPlacementFingerprint":"squeeze","reportedDepthMm":200.0,"independentDepthMm":200.0,
  "provenance":{"producedBy":f"squeeze {tag}"},"placements":sq}, open(f'{D}/squeezed.json','w'), indent=1)
r=subprocess.run([B,F]+ARGS+['0',SRC,'200.0',f'{D}/squeezed.json'],capture_output=True,text=True)
try:
    bt=json.loads(r.stdout)['relaxedDiagnostics']['coupledDynamicSeparator']['boundaryProjectionTreatment']
except Exception:
    print(f'{tag} f={f}: separator crash {r.stderr[-120:]}', flush=True); sys.exit(1)
open(f'{D}/sep.json','w').write(r.stdout)
d0=bt['finalDepthMm']
json.dump({"schemaVersion":1,"description":f"squeeze {tag} legalized","requestSha256":REQ,
  "expectedPlacementFingerprint":"squeeze","reportedDepthMm":d0,"independentDepthMm":d0,
  "provenance":{"producedBy":f"squeeze {tag} separator-legalized"},"placements":bt['finalPlacements']},
  open(f'{D}/legalized.json','w'), indent=1)
print(f'{tag} f={f}: LEGALIZED DEPTH: {d0:.3f}', flush=True)
