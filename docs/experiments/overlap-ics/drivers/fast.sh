#!/usr/bin/env bash
# The FAST tier, exactly per Sol review 14 Round 2 §3. Minutes, every iteration.
#
#     bash docs/experiments/overlap-ics/drivers/fast.sh
#
# Do NOT pipe this into `tee` or `tail`: you would read the pipe's status
# instead of the script's. Every exit status below is captured on its own line
# immediately after the command, and nothing here uses a pipeline to decide
# anything.
#
# Stages, in the order the spec lists them:
#
#   1. compile-only default-build isolation  (`--no-default-features --lib`)
#   2. dependency hygiene                    (no `jagua-rs` in the tree, and no
#                                             `jagua`/`Xoshiro`/`rand::` in the
#                                             member's own source)
#   3. one release feature combo             (`--features overlap-ics`)
#   4. the module's unit vectors + the three pinned vector suites
#   5. the 1,000-state deterministic contact corpus
#   6. the two-process fixed-work smoke, S0 canary and S1 locked strip
#   7. the `CutCloseRelocate` additions (`cutclose.py`): the first-bite canary,
#      the four driver-level tripwires, the K=8 two-process bite sequence and
#      the eight-worker merge across two processes
#
# **Stage 7's canary is a stop, not a report.** Grok review 12 Round 2 §6.3.4:
# "FAIL here is a member fail; do not run the 9-seed wall." `wall.py` reads
# `CANARY_PASS` out of stage 7's own document and refuses to start without it,
# so the two cannot drift apart by anyone forgetting.
#
# Stages 4 and 7 overlap on purpose and neither subsumes the other. Stage 4
# proves the *functions* - `relocate` commits a container pose beyond the old
# `ladder_top`, a refused publication does not advance `W`, `split_and_close`
# touches only `ty` on the far side, a repaired publication becomes the next
# bite's exact parent, the tournament is a function of its key. Stage 7 proves
# the *shipped binary*, in two processes, with eight OS threads really running.
#
# Stage 6 was red for two rounds and that was the finding, not a broken script:
# S1's *mechanism* clause (republish inside the locked strip) failed at
# max_g = 12.6 um against a 4 um attempt band, while its *invariant* clauses -
# no invalid publication, repair <= 16 um, giveback <= 0.050 mm, two-process
# bit-identity - all held. `INVARIANTS_PASS` in the smoke document is the field
# that separates the two.
#
# **It is green as of the Gate-0 re-run**: S1 republishes at 150.16536 inside
# the locked 150.16547 with zero repair rows, so this script exits 0. See
# docs/experiments/overlap-ics/gate0-rerun/README.md §6. If it goes red again,
# read `INVARIANTS_PASS` before `SMOKE_PASS`.

set -u

# ROOT resolves to the repository containing this script (sol-review-17: a
# stale hard-coded worktree default let the strongest tripwires validate the
# wrong tree). ICS_ROOT still overrides explicitly.
#
# It is **exported**, because stages 5 and 6 are Python and `lib.py` resolves
# `BIN` and every request path from its own `ICS_ROOT`. Repairing this script's
# path alone left those two stages reading whichever worktree `lib.py`'s
# constant named - which on this box still exists, so nothing would have said
# so. `lib.py` now derives its own default from its own location as well; the
# export is the belt to that suspenders, and it also pins the two to the same
# tree when only one of them is overridden.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${ICS_ROOT:-$(cd "$SCRIPT_DIR" && git rev-parse --show-toplevel)}"
export ICS_ROOT="$ROOT"
OUT="${ICS_OUT:-/var/lib/t3/tmp/overlapics/fast}"
LOG="$OUT/log"
mkdir -p "$LOG"
cd "$ROOT"
FAILURES=0

note() {
  echo "[fast] $1"
}

record() {
  local name="$1"
  local status="$2"
  if [ "$status" -eq 0 ]; then
    note "$name EXIT=$status"
  else
    note "$name EXIT=$status  <-- FAILED"
    FAILURES=$((FAILURES + 1))
  fi
}

# ---------------------------------------------------------------- 1. default --
# Catches accidental unconditional imports, module visibility changes and
# feature leakage. It does not prove semantic identity; the heavy gates do that.
cargo check -p polygon-nesting-core --no-default-features --lib > "$LOG/default-check.log" 2>&1
record "default-build compile check" $?

# --------------------------------------------------------------- 2. hygiene --
# The Chinese wall, in the dependency graph. `overlap-ics` implies
# `round-envelope-kernel` and `fast-contract-validator` and nothing else, so
# `jagua-rs` must not appear at any depth of the resolved feature tree.
cargo tree -p polygon-nesting-core --features overlap-ics -e features > "$LOG/tree.log" 2>&1
record "cargo tree --features overlap-ics" $?
grep -q "jagua-rs" "$LOG/tree.log"
GREP_STATUS=$?
if [ "$GREP_STATUS" -eq 1 ]; then
  note "dependency hygiene: jagua-rs ABSENT EXIT=0"
else
  note "dependency hygiene: jagua-rs PRESENT (grep exit $GREP_STATUS)  <-- FAILED"
  FAILURES=$((FAILURES + 1))
fi

# The same wall one level down, in the member's own source. Grok review 12 §6.3.7
# and Sol review 17 Round 2 §2: `jagua`, `Xoshiro` and `rand::` must be absent
# from `search/overlap_ics/`. The dependency tree above proves nothing about a
# path written by hand, and `CutCloseRelocate` is the first round whose sampler
# had a tempting off-the-shelf answer.
#
# Line comments are stripped before the grep, because this tree's module docs
# name all three by name - "they draw from Xoshiro, we draw from counter_hash" is
# the provenance the no-copying ruling asks for, and a check that punished the
# citation would be pushing the evidence out of the source. Stripping can only
# hide a match inside a string literal, and there is no such literal here.
: > "$LOG/rng-hygiene.log"
for source in "$ROOT"/crates/polygon-nesting-core/src/search/overlap_ics/*.rs; do
  sed 's://.*::' "$source" | grep -n -E 'Xoshiro|rand::|jagua' | sed "s:^:$source\::" \
    >> "$LOG/rng-hygiene.log"
done
if [ -s "$LOG/rng-hygiene.log" ]; then
  note "source hygiene: jagua/Xoshiro/rand:: PRESENT in search/overlap_ics/  <-- FAILED"
  FAILURES=$((FAILURES + 1))
else
  note "source hygiene: jagua/Xoshiro/rand:: ABSENT from search/overlap_ics/ EXIT=0"
fi

# ------------------------------------------------------------------ 3/4. tests --
cargo test -p polygon-nesting-core --release --features overlap-ics --lib search::overlap_ics:: > "$LOG/unit.log" 2>&1
record "module unit vectors" $?

cargo test -p polygon-nesting-core --release --features overlap-ics --test validation_vectors sat_penetration_matches_ts_oracle > "$LOG/sat-oracle.log" 2>&1
record "validation_vectors::sat_penetration_matches_ts_oracle" $?

cargo test -p polygon-nesting-core --release --features overlap-ics --test canonical_grid_vectors > "$LOG/grid-vectors.log" 2>&1
record "canonical_grid_vectors" $?

cargo test -p polygon-nesting-core --release --features overlap-ics --test collision_builder_vectors > "$LOG/collision-vectors.log" 2>&1
record "collision_builder_vectors" $?

# -------------------------------------------------------- 5. the 1,000 corpus --
cargo build -p polygon-nesting-core --release --features overlap-ics --example overlap_ics_benchmark > "$LOG/build-example.log" 2>&1
record "release example build" $?

ICS_OUT="$OUT" python3 "$ROOT/docs/experiments/overlap-ics/drivers/corpus_gate.py" 1000 > "$LOG/corpus.log" 2>&1
record "1,000-state contact corpus" $?

# ------------------------------------------------------------------ 6. smoke --
ICS_OUT="$OUT" python3 "$ROOT/docs/experiments/overlap-ics/drivers/smoke.py" 200000 > "$LOG/smoke.log" 2>&1
record "two-process fixed-work smoke (S0 canary, S1 locked strip)" $?

# ------------------------------------------------- 7. the CutCloseRelocate --
# One process per stage, every stage fixed-work, no clock inside any
# trajectory. The canary is reported on its own line **before** the aggregate,
# because it is the one member of this script whose failure has a different
# consequence from every other: it cancels the 9-seed wall rather than merely
# failing the tier.
ICS_OUT="$OUT" python3 "$ROOT/docs/experiments/overlap-ics/drivers/cutclose.py" \
  > "$LOG/cutclose.log" 2>&1
CUTCLOSE_STATUS=$?
if python3 -c "
import json,sys
d=json.load(open('$OUT/cutclose-fast.json'))
sys.exit(0 if d.get('CANARY_PASS') else 1)
" 2>/dev/null; then
  note "first-bite canary (mixed-61 seed 0, 0.1% bite, 8 workers) EXIT=0"
else
  note "first-bite canary FAILED  <-- DO NOT RUN THE 9-SEED WALL"
fi
record "cutclose FAST additions (canary, tripwires, K=8 bites, 8-worker merge)" \
  "$CUTCLOSE_STATUS"

note "logs in $LOG"
note "FAILURES=$FAILURES"
exit "$FAILURES"
