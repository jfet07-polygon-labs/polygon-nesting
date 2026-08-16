import json, subprocess, sys, os
# Layout recombination: hybrid of two independent near-degenerate layouts,
# legalized by the warm-started legacy separator, then descent-refined.
# args: layoutA layoutB cut direction tag   (direction: AB = A left of cut, B right; BA = reverse)
ROOT='/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a'
B=f'{ROOT}/target/release/examples/general_request_benchmark'
F=f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
ARGS="1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 0 0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0".split()
REQ='ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3'
A=json.load(open(sys.argv[1]))['placements']; Bp=json.load(open(sys.argv[2]))['placements']
cut=float(sys.argv[3]); direction=sys.argv[4]; tag=sys.argv[5]
D=f'/var/lib/t3/tmp/pv31-{tag}'; os.makedirs(D, exist_ok=True)
left, right = (A, Bp) if direction=='AB' else (Bp, A)
rmap={p['pieceId']:p for p in right}
hybrid=[p if p['translateShortAxis']<cut else rmap[p['pieceId']] for p in left]
nfrom_left=sum(1 for p in hybrid if p in left)
json.dump({"schemaVersion":1,"description":f"crossover {tag} cut {cut} {direction}","requestSha256":REQ,
  "expectedPlacementFingerprint":"crossover","reportedDepthMm":200.0,"independentDepthMm":200.0,
  "provenance":{"producedBy":f"crossover {tag}"},"placements":hybrid}, open(f'{D}/hybrid.json','w'), indent=1)
print(f'{tag}: hybrid built, {nfrom_left}/61 from left parent', flush=True)
r=subprocess.run([B,F]+ARGS+['0',sys.argv[1],'200.0',f'{D}/hybrid.json'],capture_output=True,text=True)
try:
    bt=json.loads(r.stdout)['relaxedDiagnostics']['coupledDynamicSeparator']['boundaryProjectionTreatment']
except Exception:
    print(f'{tag}: separator crash {r.stderr[-160:]}', flush=True); sys.exit(1)
open(f'{D}/sep.json','w').write(r.stdout)
d0=bt['finalDepthMm']
json.dump({"schemaVersion":1,"description":f"crossover {tag} legalized","requestSha256":REQ,
  "expectedPlacementFingerprint":"crossover","reportedDepthMm":d0,"independentDepthMm":d0,
  "provenance":{"producedBy":f"crossover {tag} separator-legalized"},"placements":bt['finalPlacements']},
  open(f'{D}/legalized.json','w'), indent=1)
print(f'{tag}: LEGALIZED DEPTH: {d0:.3f}', flush=True)
