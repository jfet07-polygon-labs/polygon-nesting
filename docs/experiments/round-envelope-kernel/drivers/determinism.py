#!/usr/bin/env python3
"""Two processes, whole document, wall-clock fields stripped by name.

    determinism.py BINARY PLAN

The battery is the first instrument in this campaign whose document carries
*timings* — the economy section — so a byte-for-byte comparison of the raw
stdout would fail for the one reason a determinism check is allowed to ignore.
The stripped set is listed here rather than inherited, because
`gatelib.strip_times` does not know these field names and a silent miss is how a
determinism check passes without checking anything.

Everything else must be byte-identical, `executableSha256` included: the kernel
has no clock, no threading and no randomness, and the census that reads it has
none either.
"""

import hashlib
import json
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
OUT = os.path.join(
    ROOT, "docs", "experiments", "round-envelope-kernel", "evidence", "determinism.json"
)

# Wall-clock, and the ratios and medians derived from wall-clock. Every one of
# these is in the `economy` section and nowhere else; nothing in populations 1-4
# is timed.
VOLATILE = {
    "envelopeHalfMiterMs",
    "envelopeHalfRoundMs",
    "envelopeHalfRatio",
    "envelopeHalfRatioMedian",
    "confirmationMiterMs",
    "confirmationRoundMs",
    "confirmationRatio",
    "confirmationRatioMedianComparable",
    "confirmationUnionMs",
    "confirmationUnionRatio",
    "confirmationUnionRatioMedianComparable",
    # The protocol's own note: `gatelib.strip_times` misses these three, so they
    # are named here even though this instrument does not currently emit them.
    "milliseconds",
    "leafMilliseconds",
    "leafSharePercent",
}


def strip(node):
    if isinstance(node, dict):
        return {k: strip(v) for k, v in sorted(node.items()) if k not in VOLATILE}
    if isinstance(node, list):
        return [strip(v) for v in node]
    if isinstance(node, float):
        return repr(node)
    return node


def digest_of(path):
    document = json.load(open(path))
    stripped = json.dumps(strip(document), sort_keys=True).encode()
    return (
        hashlib.sha256(open(path, "rb").read()).hexdigest(),
        hashlib.sha256(stripped).hexdigest(),
    )


def main():
    binary, plan = sys.argv[1:3]
    runs = []
    for index in range(2):
        path = f"/var/lib/t3/tmp/rek-det-{index}.json"
        with open(path, "w") as handle:
            proc = subprocess.run(
                [binary, plan], stdout=handle, stderr=subprocess.PIPE, check=False
            )
        raw, stripped = digest_of(path)
        runs.append(
            {
                "process": index,
                "exit": proc.returncode,
                "rawSha256": raw,
                "strippedSha256": stripped,
                "bytes": os.path.getsize(path),
                "stderr": (proc.stderr or b"").decode()[-400:],
            }
        )
    identical = runs[0]["strippedSha256"] == runs[1]["strippedSha256"]
    document = {
        "experiment": "round-envelope-kernel",
        "step": "determinism, two processes, whole document less the timings",
        "binary": binary,
        "binarySha256": hashlib.sha256(open(binary, "rb").read()).hexdigest(),
        "plan": os.path.relpath(plan, ROOT),
        "strippedFields": sorted(VOLATILE),
        "runs": runs,
        "identicalAfterStripping": identical,
        "identicalRaw": runs[0]["rawSha256"] == runs[1]["rawSha256"],
        "exitsZero": all(run["exit"] == 0 for run in runs),
    }
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as handle:
        json.dump(document, handle, indent=2)
        handle.write("\n")
    print(json.dumps({k: v for k, v in document.items() if k != "runs"}, indent=1))
    for run in runs:
        print(run["process"], run["exit"], run["rawSha256"], run["strippedSha256"])
    return 0 if identical and document["exitsZero"] else 1


if __name__ == "__main__":
    sys.exit(main())
