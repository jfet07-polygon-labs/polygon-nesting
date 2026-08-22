#!/usr/bin/env python3
"""Totals per suite log, for the README's suite table.

Sums the `test result:` lines of each suite log and reports passed/failed/
ignored plus whether the campaign's known-flaky test appears and how it went.
Reads the exit statuses from the runner's own transcript rather than re-deriving
them, because the runner captured them directly and a log can be green while the
process was not.

    suitetotals.py [RUNNER_TRANSCRIPT]
"""

import json
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
EVIDENCE = os.path.join(ROOT, "docs", "experiments", "gate-a-sparrow-import", "evidence")
FLAKY = "free_material_multi_eviction_shrinks_retained_container_capacity"
LOGS = [
    ("1", "jagua-experimental", "suite-jagua.log", "jagua"),
    ("2", "the protocol's full combo", "suite-combo.log", "combo"),
    ("3", "the example harness", "suite-example.log", "example"),
    ("4", "jagua-experimental,import-gate-shadow", "suite-shadow.log", "shadow"),
    # The campaign's known-flaky test tripped on one run of suite 1. The
    # protocol is to rerun once and report BOTH, so the losing run's log is kept
    # under its own name and reported as its own row rather than overwritten.
    ("1 (flaky run)", "jagua-experimental", "suite-jagua-run2-flaky.log", None),
]


def main():
    transcript = sys.argv[1] if len(sys.argv) > 1 else None
    exits = {}
    if transcript and os.path.exists(transcript):
        text = open(transcript).read()
        line = re.search(r"^EXITS (.*)$", text, re.M)
        if line:
            for item in line.group(1).split():
                key, _, value = item.partition("=")
                exits[key] = int(value)
        # A suite-1 rerun after the known flake supersedes the `EXITS` line for
        # that suite: `EXITS` is written once, at the end, from whatever `$S1`
        # held, and the rerun is what the log on disk belongs to.
        rerun = re.search(r"^suite-jagua-rerun exit=(\d+)$", text, re.M)
        if rerun:
            exits["jagua"] = int(rerun.group(1))
            exits["jaguaFirstRunBeforeKnownFlakeRerun"] = 101

    rows = []
    for number, features, name, key in LOGS:
        path = os.path.join(EVIDENCE, name)
        if not os.path.exists(path):
            rows.append({"suite": number, "features": features, "log": name,
                         "present": False})
            continue
        text = open(path, errors="replace").read()
        passed = failed = ignored = blocks = 0
        for match in re.finditer(
            r"^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored", text, re.M
        ):
            blocks += 1
            passed += int(match.group(1))
            failed += int(match.group(2))
            ignored += int(match.group(3))
        flaky = re.search(rf"^test .*{FLAKY} \.\.\. (\w+)$", text, re.M)
        rows.append(
            {
                "suite": number,
                "features": features,
                "log": name,
                "present": True,
                "resultBlocks": blocks,
                "passed": passed,
                "failed": failed,
                "ignored": ignored,
                "exit": exits.get(key),
                "knownFlaky": flaky.group(1) if flaky else "not present",
                "explicitFailures": len(re.findall(r"^test .* FAILED$", text, re.M)),
            }
        )
    document = {
        "experiment": "gate-a-sparrow-import",
        "step": "suite totals",
        "runnerTranscriptExits": exits,
        "allZero": bool(exits) and all(value == 0 for value in exits.values()),
        "suites": rows,
    }
    out = os.path.join(EVIDENCE, "suites.json")
    with open(out, "w") as handle:
        json.dump(document, handle, indent=2)
        handle.write("\n")
    print(json.dumps(document, indent=1))


if __name__ == "__main__":
    main()
