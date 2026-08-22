#!/usr/bin/env python3
"""Stamps the binary manifest into every JSON evidence file in this round.

The campaign rule is that every evidence file carries the sha256 of the binary
that produced it. `round_envelope_battery` hashes itself into its own output; the
Python-produced documents cannot, so this injects the same manifest into all of
them - including the ones whose numbers came out of Python alone, because "no
binary was involved" is itself a fact a reader needs stated rather than inferred
from an absent field.

    stamp.py EVIDENCE_DIR [MORE_JSON ...]
"""

import json
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
DEFAULT = os.path.join(
    ROOT, "docs", "experiments", "round-envelope-kernel", "evidence"
)


def manifest(evidence_dir):
    path = os.path.join(evidence_dir, "binaries.txt")
    entries = {}
    if os.path.exists(path):
        for line in open(path):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            digest, _, name = line.partition("  ")
            entries[os.path.basename(name.strip())] = digest
    return {
        "binaries": entries,
        "binariesManifest": os.path.relpath(path, ROOT),
        "commit": subprocess.run(
            ["git", "-C", ROOT, "rev-parse", "HEAD"],
            capture_output=True, text=True, check=False,
        ).stdout.strip(),
        "rustc": subprocess.run(
            ["rustc", "--version"], capture_output=True, text=True, check=False
        ).stdout.strip(),
        "python": sys.version.split()[0],
    }


def main():
    evidence_dir = sys.argv[1] if len(sys.argv) > 1 else DEFAULT
    extra = sys.argv[2:]
    block = manifest(evidence_dir)
    targets = [
        os.path.join(evidence_dir, name)
        for name in sorted(os.listdir(evidence_dir))
        if name.endswith(".json")
    ] + extra
    for target in targets:
        try:
            document = json.load(open(target))
        except (json.JSONDecodeError, OSError) as error:
            print(f"skip {target}: {error}")
            continue
        if not isinstance(document, dict):
            print(f"skip {target}: not an object")
            continue
        document["provenanceStamp"] = block
        with open(target, "w") as handle:
            json.dump(document, handle, indent=2)
            handle.write("\n")
        print(f"stamped {os.path.relpath(target, ROOT)}")


if __name__ == "__main__":
    main()
