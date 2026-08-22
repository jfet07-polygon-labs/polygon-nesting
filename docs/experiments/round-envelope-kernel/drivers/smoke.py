#!/usr/bin/env python3
"""The spec key's own gate: an unarmed binary must refuse `rek`, an armed one
must honour it, and neither may change what it does without being asked.

    smoke.py OFF_BINARY ON_BINARY [OUT.json]

Six cells, on a tiny work budget because none of them is a measurement:

1. the **unarmed** binary given `rek=1` must exit non-zero with
   `unknown portfolio spec key "rek"`. This is the condition Grok review 7 and
   the `fcv` precedent both name: a binary that cannot honour a key must refuse
   it rather than run the other arm under its label.
2. the armed binary given `rek=yes` must be refused too. A mode key that fell
   back to a boolean would silently pick an arm.
3. the **armed** binary given no `rek` at all must produce the same document as
   the unarmed binary. The feature compiled is not the feature armed.
4. the armed binary given `rek=0` must produce that same document again.
5. `rek=1` — the union — must **run**, because the union cannot lose a layout
   the miter authority admits and the constructor's own `validate_result` is
   one of those layouts.
6. `rek=2` — the exclusive kernel — is reported as it comes. On this request it
   aborts, and that abort is the round's central finding rather than a defect
   in this driver: the short-side-first constructor places pieces at *contact*,
   Clipper re-quantizes its offset output to the 1 µm grid, and the exclusive
   kernel is one grid step stricter there than the geometry the constructor
   itself reasons in.

Published depths are reported next to the verdicts, not asserted: this driver
is not a quality gate and the search-side comparison is the *next* round's
assignment.
"""

import hashlib
import json
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
REQUEST = f"{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json"
ARGS = (
    "1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 5 "
    "0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0"
).split()
VOLATILE = {
    "elapsedMs", "elapsedSeconds", "engineElapsedSeconds", "wallMs", "durationMs",
    "timestamp", "totalMs", "ms", "processWallSeconds", "phaseProfile", "phases",
    "profile", "leafSeconds", "engineVersion", "buildIdentity", "binaryPath",
    "peakResidentBytes", "allocatedBytes", "medianElapsedMs", "minElapsedMs",
    "maxElapsedMs", "firstQuartileElapsedMs", "thirdQuartileElapsedMs",
    "executableSha256", "relevantSourceTreeSha256", "engineWorktreeStatus",
    "engineCommit", "engineWorktreeDirty", "milliseconds", "leafMilliseconds",
    "leafSharePercent",
    # The coordinator's own wall-clock stamps. Measured, not assumed: two runs
    # of the SAME binary on the SAME spec differ in exactly these three and in
    # nothing else.
    "birthSeconds", "publishedSeconds", "occupancyOverTime",
}


def strip(node):
    if isinstance(node, dict):
        return {k: strip(v) for k, v in sorted(node.items()) if k not in VOLATILE}
    if isinstance(node, list):
        return [strip(v) for v in node]
    if isinstance(node, float):
        return repr(node)
    return node


def run(binary, spec, tag):
    argv = [binary, REQUEST] + ARGS + ["0", "", "", "", "0.002"]
    if spec is not None:
        argv.append(spec)
    path = f"/var/lib/t3/tmp/rek-smoke-{tag}.json"
    with open(path, "w") as handle:
        proc = subprocess.run(
            argv, stdout=handle, stderr=subprocess.PIPE, check=False
        )
    try:
        document = json.load(open(path))
    except (json.JSONDecodeError, OSError):
        document = None
    digest = (
        hashlib.sha256(
            json.dumps(strip(document), sort_keys=True).encode()
        ).hexdigest()
        if document is not None
        else None
    )
    depths = []
    if document is not None:
        stack = [document]
        while stack:
            node = stack.pop()
            if isinstance(node, dict):
                for key, value in node.items():
                    if key == "independentDepthMm" and isinstance(value, (int, float)):
                        depths.append(value)
                    stack.append(value)
            elif isinstance(node, list):
                stack.extend(node)
    return {
        "tag": tag,
        "spec": spec,
        "exit": proc.returncode,
        "stderr": (proc.stderr or b"").decode()[-300:].strip(),
        "docDigest": digest,
        "independentDepthsMm": sorted(set(depths)),
    }


def main():
    off_binary, on_binary = sys.argv[1:3]
    out = sys.argv[3] if len(sys.argv) > 3 else None
    budget = "work=8000000,slots=4,cells=13:15:17:19,v3=1"
    cells = [
        run(off_binary, f"{budget},rek=1", "off-rek1"),
        run(off_binary, budget, "off-plain"),
        run(on_binary, f"{budget},rek=yes", "on-rekbad"),
        run(on_binary, budget, "on-plain"),
        run(on_binary, f"{budget},rek=0", "on-rek0"),
        run(on_binary, f"{budget},rek=1", "on-union"),
        run(on_binary, f"{budget},rek=2", "on-exclusive"),
    ]
    by_tag = {cell["tag"]: cell for cell in cells}
    checks = {
        "unarmedBinaryRefusesTheKey": by_tag["off-rek1"]["exit"] != 0
        and "rek" in by_tag["off-rek1"]["stderr"],
        "armedBinaryRefusesAnUnknownMode": by_tag["on-rekbad"]["exit"] != 0
        and "rek takes" in by_tag["on-rekbad"]["stderr"],
        "armedBinaryWithoutTheKeyEqualsUnarmed": by_tag["on-plain"]["docDigest"]
        == by_tag["off-plain"]["docDigest"]
        and by_tag["on-plain"]["docDigest"] is not None,
        "armedBinaryWithRekZeroEqualsUnarmed": by_tag["on-rek0"]["docDigest"]
        == by_tag["off-plain"]["docDigest"],
        "unionRunSucceeds": by_tag["on-union"]["exit"] == 0,
        "unionRunDiffersFromUnarmed": by_tag["on-union"]["docDigest"]
        != by_tag["off-plain"]["docDigest"],
    }
    document = {
        "experiment": "round-envelope-kernel",
        "step": "the spec key's own gate",
        "offBinary": off_binary,
        "onBinary": on_binary,
        "budget": budget,
        "cells": cells,
        "checks": checks,
        "allChecksPass": all(checks.values()),
        "exclusiveModeObserved": {
            "exit": by_tag["on-exclusive"]["exit"],
            "stderr": by_tag["on-exclusive"]["stderr"],
            "note": "reported, not asserted: see this file's header",
        },
    }
    if out:
        with open(out, "w") as handle:
            json.dump(document, handle, indent=2)
            handle.write("\n")
    print(json.dumps(document, indent=1))
    return 0 if document["allChecksPass"] else 1


if __name__ == "__main__":
    sys.exit(main())
