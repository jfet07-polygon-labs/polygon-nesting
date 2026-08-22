#!/usr/bin/env python3
"""Two processes, whole document: the campaign's hard gate for anything measured.

    determinism.py BINARY REQUEST POSES ALLOWANCES ARC_TOLERANCE BISECT_TOP

Runs the shadow instrument in two separate processes and requires the two
documents to be byte-identical, `executableSha256` included. The instrument has
no threading, no clock and no randomness, so this is a check that nothing in the
Clipper path or the bisection has picked one up - which is exactly the class of
defect a single run cannot see.
"""

import hashlib
import json
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
OUT = os.path.join(
    ROOT, "docs", "experiments", "gate-a-sparrow-import", "evidence", "determinism.json"
)


def main():
    binary, request, poses, allowances, arc, bisect = sys.argv[1:7]
    runs = []
    for index in range(2):
        path = f"/var/lib/t3/tmp/gatea-det-{index}.json"
        with open(path, "w") as handle:
            proc = subprocess.run(
                [binary, request, poses, "5", "5", allowances, arc, bisect],
                stdout=handle,
                stderr=subprocess.PIPE,
                check=False,
            )
        digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
        runs.append(
            {
                "process": index,
                "exit": proc.returncode,
                "sha256": digest,
                "bytes": os.path.getsize(path),
                "stderr": (proc.stderr or b"").decode()[-400:],
            }
        )
    identical = runs[0]["sha256"] == runs[1]["sha256"]
    document = {
        "experiment": "gate-a-sparrow-import",
        "step": "determinism, two processes, whole document",
        "binary": binary,
        "binarySha256": hashlib.sha256(open(binary, "rb").read()).hexdigest(),
        "arguments": {
            "request": os.path.relpath(request, ROOT),
            "poses": os.path.relpath(poses, ROOT),
            "sheetEdgeClearanceMm": 5,
            "pairClearanceMm": 5,
            "allowances": allowances,
            "arcToleranceGridUnits": arc,
            "bisectTop": bisect,
        },
        "runs": runs,
        "identical": identical,
        "exitsZero": all(run["exit"] == 0 for run in runs),
    }
    with open(OUT, "w") as handle:
        json.dump(document, handle, indent=2)
        handle.write("\n")
    print(json.dumps({k: v for k, v in document.items() if k != "runs"}, indent=1))
    for run in runs:
        print(run["process"], run["exit"], run["sha256"])
    return 0 if identical and document["exitsZero"] else 1


if __name__ == "__main__":
    sys.exit(main())
