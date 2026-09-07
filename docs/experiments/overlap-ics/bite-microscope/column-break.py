#!/usr/bin/env python3
"""column-break.py <replay.json>...: the replay's committed-geometry readings, verbatim from the document.

Per document: the probe, the exponent, the band entry and the evaluations to band, then the
`replay.column` summary (the spec's entry-graph column) and the `replay.columnLongestLived`
summary (the same analysis with the formation iteration chosen by residence), so the deadlines
GPT-6 Astra review 5b Q6 registered (column break 37 at p = 2 to reproduce; 20 / 16 / 12 at
p = 1 / 0.75 / 0.5; band entry 43 / 27 / 23 / 19) can be scored from the documents. It prints;
it does not score.
"""
import json
import sys


def decode(row_id, count):
    pairs = count * (count - 1) // 2
    if row_id >= pairs:
        rest = row_id - pairs
        return "edge(%d,%s)" % (rest // 4, "LRBT"[rest % 4])
    for first in range(count - 1):
        start = first * count - first * (first + 1) // 2
        end = start + (count - first - 1)
        if row_id < end:
            return "pair(%d,%d)" % (first, first + 1 + (row_id - start))
    return "row(%d)" % row_id


def column_lines(label, column, count):
    lines = []
    release = column.get("breakRelease")
    lines.append(
        "  %s: status=%s formedAt=%s residence=%s paths=%d%s rows=[%s] coreMembers=%s"
        % (
            label,
            column["status"],
            column["formedAtIteration"],
            column.get("residenceIterations"),
            len(column["paths"]),
            "+" if column.get("pathsTruncated") else "",
            ", ".join(decode(r, count) for r in column["rows"]),
            column["coreMembers"],
        )
    )
    if release is not None:
        relocate = release.get("relocate")
        moved = (
            "piece %d dx %.3f dy %.3f dtheta %.1f"
            % (relocate["piece"], relocate["dxMm"], relocate["dyMm"], relocate["dthetaDeg"])
            if relocate
            else "relocate n/a"
        )
        lines.append(
            "    breakIteration=%s release=%s at %d by %s; maxColumnRowWeightAtBreak=%.4e; rowWeightsAtBreak=%s"
            % (
                column["breakIteration"],
                decode(release["rowId"], count),
                release["iteration"],
                moved,
                release["maxColumnRowWeightAtBreak"],
                ", ".join("%s: %.4e" % (decode(r, count), w) for r, w in release["rowWeightsAtBreak"]),
            )
        )
    else:
        lines.append("    breakIteration=%s (no permanent release disconnected the original rows)" % column["breakIteration"])
    lines.append(
        "    firstDisconnectedAt=%s reformedAfterFirstDisconnection=%s reformedAfterBreak=%s bandEntryIteration=%s lastIteration=%s"
        % (
            column["firstDisconnectedAtIteration"],
            column["reformedAfterFirstDisconnection"],
            column["reformedAfterBreak"],
            column["bandEntryIteration"],
            column["lastIteration"],
        )
    )
    lines.append(
        "    temporaryReleases=[%s] permanentReleases=[%s]"
        % (
            ", ".join(
                "%s %d->%d" % (decode(t["rowId"], count), t["releasedAt"], t["reformedAt"])
                for t in column["temporaryReleases"]
            ),
            ", ".join("%s at %d" % (decode(p["rowId"], count), p["iteration"]) for p in column["permanentReleases"]),
        )
    )
    return lines


for path in sys.argv[1:]:
    doc = json.load(open(path))
    r = doc["replay"]
    name = path.rsplit("/", 1)[-1]
    # The row-id scheme needs the piece count (`replay.capsule.pieces`; 61 for
    # the documents written before the field existed).
    count = r["capsule"].get("pieces", 61)
    params = r["params"]
    print(
        "%s: probe=%s exponent=%s bite=%s proxyMarginUm=%s maxIterations=%s resolvedSeed=%s fork=%s certify=%s"
        % (
            name,
            params["probe"],
            params["exponent"],
            r["bite"],
            r["capsule"]["proxyMarginUm"],
            r["maxIterations"],
            doc.get("resolvedSeed"),
            params.get("fork"),
            params.get("certify"),
        )
    )
    print(
        "  stop=%s iterations=%d bandEnteredAtIteration=%s evaluationsToBand=%s evaluationsTotal=%s identity=%s/%s PASS"
        % (
            r["stop"],
            len(r["iterations"]),
            r["bandEnteredAtIteration"],
            r["evaluationsToBand"],
            r["evaluationsTotal"],
            r["identityPass"],
            len(r["identity"]),
        )
    )
    for line in column_lines("column (entry graph)", r["column"], count):
        print(line)
    if "columnLongestLived" in r:
        for line in column_lines("column (longest-lived)", r["columnLongestLived"], count):
            print(line)
    fork = r.get("fork")
    if fork:
        wm = fork["wouldMove"]
        print(
            "  fork: sweep=%d relocates=%d wouldMove p=2: %d, p=1: %d, p=0.75: %d, p=0.5: %d (decidingExponent=%s)"
            % (fork["sweep"], len(fork["relocates"]), wm["2"], wm["1"], wm["0.75"], wm["0.5"], fork["decidingExponent"])
        )
    certification = r.get("certification")
    if certification:
        print("  certification: %s" % json.dumps(certification, sort_keys=True))
