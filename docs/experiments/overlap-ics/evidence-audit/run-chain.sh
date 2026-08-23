#!/usr/bin/env bash
# `chain.py` over every full cell document in one or more directories.
#
#     bash run-chain.sh <audit-dir> <out.json> <cells-dir> [<cells-dir> ...]
set -u
AUDIT="$1"
OUT="$2"
shift 2
FILES=()
for dir in "$@"; do
  while IFS= read -r file; do FILES+=("$file"); done < <(find "$dir" -name 'wall-*.json' -o -name 'replay-*.json' | sort)
done
python3 "$AUDIT/chain.py" "${FILES[@]}" "--out=$OUT"
exit $?
