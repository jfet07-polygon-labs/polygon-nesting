#!/usr/bin/env bash
# batch.sh <label> <profile> <seeds...> : warm-start Sparrow from our incumbents, 10 s each, under the lock
label=$1; profile=$2; shift 2
IN=/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/sparrow-mixed61/input.json
for s in "$@"; do
  cell=/var/lib/t3/tmp/astra/dev/$label-$profile-r0-s$s.json
  sol=/var/lib/t3/tmp/astra/warm/ours-$label-$profile-s$s.json
  log=/var/lib/t3/tmp/astra/warm/sparrow-from-$label-$profile-s$s.log
  python3 /var/lib/t3/tmp/astra/warm/to-sparrow-solution.py $cell $IN $sol > /dev/null || { echo "s$s convert FAILED"; continue; }
  python3 - "$sol" <<'PY'
import json,sys
p=sys.argv[1]; s=json.load(open(p))
def area(pts):
    if pts[0]==pts[-1]: pts=pts[:-1]
    return abs(sum(pts[i][0]*pts[(i+1)%len(pts)][1]-pts[(i+1)%len(pts)][0]*pts[i][1] for i in range(len(pts))))/2
tot=sum(area(it['shape']['data']) for it in s['items']); sol=s['solution']
sol['density']=tot/(sol['strip_width']*s['strip_height']); sol['layout']['density']=sol['density']; sol['run_time_sec']=10
for it in s['items']: it.setdefault('min_quality',None)
json.dump(s,open(p,'w'))
PY
  (cd /var/lib/t3/tmp/sparrow-bench && flock /var/lib/t3/tmp/astra/bench.lock timeout 60 ./target/release/sparrow -i $sol -t 10 -s 0 --min-item-separation 5 --workers 8 > $log 2>&1)
  init=$(grep -m1 "initial width" $log | sed 's/.*initial width: \([0-9.]*\).*/\1/')
  expl=$(grep "EXPL\] finished" $log | sed 's/.*width: \([0-9.]*\).*/\1/')
  comp=$(grep "COMPR\] finished" $log | sed 's/.*width: \([0-9.]*\).*/\1/')
  n=$(grep -c "(S)" $log)
  echo "$label $profile s$s: start $init -> explore $expl -> compress ${comp:-n/a}  separates $n"
done
