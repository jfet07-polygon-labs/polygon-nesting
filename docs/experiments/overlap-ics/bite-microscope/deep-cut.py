#!/usr/bin/env python3
"""deep-cut.py <microscope.json>... [--replays <dir>]

Reads --microscopetarget documents (the depth-triggered bite microscope, GPT-6 Astra review 7
Q18) and prints, per document and retained bite: the trigger and its target, every separation
attempt with its iterations, actual stop, remaining wall allowance, strikes, rollbacks, exact
calls and publication outcome; per attempt the blocking-row census at entry and at the stop
(count, maximum residual, entry rows persisting to the stop against rows created since), the
all-worker evaluations and the useful moves the tournament discarded with the losers'
expenditure. With --replays <dir>, every replay document in that directory whose capsule came
from one of the microscope documents is listed under its bite: objective, horizon, band entry,
evaluations, identity gate and certification. Print only; nothing here scores anything, and a
document carrying `biteMicroscope` is refused by score.py by construction.

Why it exists: docs/experiments/overlap-ics/sparrow-warm-start/README.md shows the two engines
parting on individual deep bites from the same layout, and the aggregate bite record cannot say
why; Astra review 7 Q18 asks for the microscope on the bite the Wall10s cell dies in.
"""
import glob
import json
import os
import sys


def fmt_s(value):
    return "n/a" if value is None else f"{value:.3f} s"


def census(rows):
    if not rows:
        return "0 rows"
    return f"{len(rows)} rows, max residual {max(r[1] for r in rows) * 1000:.1f} um"


def persistence(entry_rows, stop_rows):
    entry_ids = {r[0] for r in entry_rows}
    stop_ids = {r[0] for r in stop_rows}
    persisted = entry_ids & stop_ids
    created = stop_ids - entry_ids
    released = entry_ids - stop_ids
    return persisted, created, released


def load(path):
    with open(path) as handle:
        return json.load(handle)


def print_document(path, doc):
    m = doc.get("biteMicroscope")
    if m is None:
        print(f"== {os.path.basename(path)}: no biteMicroscope block")
        return
    trigger = m.get("trigger", {})
    exposure = m["exposure"]
    arm = f"p={doc.get('guidedExponent', 2)}"
    print(
        f"== {os.path.basename(path)}  seed {doc.get('seed')}  profile {doc.get('scheduleProfile')}  "
        f"{arm}  margin {doc.get('proxyMarginUm', 0)} um  final depth {doc['outcome']['depthMm']:.3f} mm  "
        f"flag {m.get('flag')}"
    )
    if trigger.get("kind") == "target":
        second = trigger.get("secondTargetMm")
        print(
            f"   trigger: first explore cut targeting <= {trigger['targetMm']} mm"
            + (f", then the next cut targeting <= {second} mm" if second is not None else ", then the next explore cut")
        )
    else:
        print(f"   trigger: {trigger}")
    print(
        f"   exposure: {exposure['status']}; {exposure.get('reason', '')}; explore bites seen "
        f"{exposure['exploreBitesSeen']}; deepest target {exposure.get('deepestTargetMm')} mm; last published "
        f"{exposure.get('lastPublishedDepthMm')} mm; retained {exposure['retainedBites']}"
    )
    for bite in m["bites"]:
        print(
            f"\n-- bite {bite['ordinal']} [{bite['retainedAs']}] {bite['parentDepthMm']:.3f} -> "
            f"{bite['targetDepthMm']:.3f} mm: {bite['masterIterations']} master iterations, "
            f"{len(bite['separations'])} attempt(s), published {bite['published']}, cut moved "
            f"{len(bite['cutMoved'])}, initial positive {len(bite['initialPositive'])}"
        )
        for call in bite["separations"]:
            sweeps = call["sweeps"]
            entry = call.get("entryBlocking", [])
            last = sweeps[-1]["blocking"] if sweeps else entry
            persisted, created, released = persistence(entry, last)
            wall_in = call.get("wallAtEntry", {})
            wall_out = call.get("wallAtStop", {})
            print(
                f"   attempt {call['attempt']} (capsule {call.get('capsule')}): {call['iterations']} iterations, "
                f"stop {call['stop']}, wall left at entry {fmt_s(wall_in.get('leftS'))} / at stop "
                f"{fmt_s(wall_out.get('leftS'))}, strikes {call.get('strikes')}, rollbacks {len(call['rollbacks'])}, "
                f"band entries {call['bandEntries']}, exact calls {call['exactCheckpointCalls']}, "
                f"min raw {call.get('minRaw')}; {call.get('publication')}"
            )
            print(
                f"      blocking at entry: {census(entry)}; at stop (last sweep): {census(last)}; "
                f"entry rows persisting {len(persisted)}, released {len(released)}, created since entry {len(created)}"
            )
            restored = call.get("stopBlocking", [])
            if sweeps and restored != last:
                print(f"      state handed back (restored to iteration {call.get('restoredToIteration')}): {census(restored)}")
            if sweeps:
                maxs = [s["maxAfterMm"] * 1000 for s in sweeps]
                print(
                    f"      end-of-sweep max violation (um): first {maxs[0]:.1f}, min {min(maxs):.1f}, last {maxs[-1]:.1f}; "
                    f"first iteration <= 50 um: {next((s['iteration'] for s in sweeps if s['maxAfterMm'] * 1000 <= 50), None)}, "
                    f"<= 20 um: {next((s['iteration'] for s in sweeps if s['maxAfterMm'] * 1000 <= 20), None)}, "
                    f"<= 4 um: {next((s['iteration'] for s in sweeps if s['maxAfterMm'] * 1000 <= 4), None)}"
                )
            winner_ev = sum(s["evaluationsWinner"] for s in sweeps)
            useful_winner = sum(
                s["workers"][s["winner"]]["usefulMoves"] for s in sweeps if s.get("workers")
            )
            useful_all = sum(w["usefulMoves"] for s in sweeps for w in s.get("workers", []))
            moved_all = sum(w["moved"] for s in sweeps for w in s.get("workers", []))
            relocates_all = sum(w["relocates"] for s in sweeps for w in s.get("workers", []))
            container_all = sum(w["containerCommits"] for s in sweeps for w in s.get("workers", []))
            print(
                f"      evaluations all workers {call.get('evaluationsAllWorkers', sum(s['evaluationsAllWorkers'] for s in sweeps))} "
                f"(winner {winner_ev}); relocates all workers {relocates_all}, moved {moved_all}, container commits {container_all}; "
                f"useful moves all workers {useful_all}, retained (winner's) {useful_winner}, DISCARDED {call.get('usefulMovesDiscarded')} "
                f"with losers' expenditure {call.get('discardedExpenditure')} evaluations"
            )
            # The most persistent blocking rows across the attempt.
            counts = {}
            for s in sweeps:
                for rid, res in s["blocking"]:
                    counts.setdefault(rid, [0, 0.0])
                    counts[rid][0] += 1
                    counts[rid][1] = max(counts[rid][1], res)
            top = sorted(counts.items(), key=lambda kv: (-kv[1][0], kv[0]))[:6]
            if top:
                print(
                    "      most persistent rows (rowId: sweeps present, max residual um): "
                    + ", ".join(f"{rid}: {c}, {r * 1000:.1f}" for rid, (c, r) in top)
                )
        for reset in bite.get("resets", []):
            print(
                f"   reset after attempt {reset['afterAttempt']}: pool {reset['poolSize']} rank {reset['rank']} entry raw "
                f"{reset['entryRawPhi']:.3e}, disruption fired {reset['disruptionFired']} swapped {reset.get('swapped')}, "
                f"capsule {reset['capsule']}"
            )
        if bite.get("publication"):
            pub = bite["publication"]
            print(
                f"   publication: attempt {pub['attempt']} iteration {pub['iteration']} target {pub['targetDepthMm']:.3f} -> "
                f"published {pub['publishedRawDepthMm']:.3f} mm, repair rows {pub['repairRows']}, max displacement "
                f"{pub['repairMaxDisplacementMm']:.4f} mm, pose deltas {len(pub['installedPoseDeltas'])}"
            )


def print_replays(paths, replay_dir):
    names = {os.path.basename(p) for p in paths}
    replays = []
    for path in sorted(glob.glob(os.path.join(replay_dir, "*.json"))):
        try:
            doc = load(path)
        except (OSError, ValueError):
            continue
        replay = doc.get("replay")
        if not replay:
            continue
        source = os.path.basename(replay.get("capsule", {}).get("path", ""))
        if source in names:
            replays.append((source, path, doc, replay))
    if not replays:
        print(f"\n== replays: none in {replay_dir} came from these documents")
        return
    print(f"\n== replays from {replay_dir}")
    for source, path, doc, replay in sorted(replays, key=lambda r: (r[0], r[3]["capsule"]["bite"], r[1])):
        cap = replay["capsule"]
        params = replay.get("params", {})
        cert = replay.get("certification")
        control = replay.get("control", {})
        cert_text = "not requested"
        if cert:
            cert_text = (
                f"attempted {cert['attempted']}, published {cert['published']}, depth {cert.get('depthMm')}, "
                f"refusal {cert.get('refusal')}, exact calls {cert.get('exactCalls')}"
            )
        print(
            f"-- {source} bite {cap['bite']} capsule {cap['index']} ({cap['label']}, attempt {cap['separationAttempt']}) "
            f"-> {os.path.basename(path)}"
        )
        print(
            f"   objective: probe {replay['probe']} (captured p={cap.get('capturedExponent')}, trajectory "
            f"p={params.get('trajectoryExponent')}); horizon {params.get('horizon', {}).get('kind')} = "
            f"{params.get('horizon', {}).get('iterations')} iterations; ran {len(replay['iterations'])}, stop {replay['stop']}"
        )
        print(
            f"   band entry at iteration {replay['bandEnteredAtIteration']} (live attempt: {control.get('bandEnteredAtIteration')}, "
            f"stop {control.get('tracedStop')} after {control.get('tracedIterations')}); evaluations to band "
            f"{replay['evaluationsToBand']} / total {replay['evaluationsTotal']} (live {control.get('evaluationsTotal')}); "
            f"identity {replay['identityPass']}/{len(replay['identity'])} pass, first divergence "
            f"{replay.get('divergesFromTraceAtIteration')}; reconstruction raw {replay['reconstruction']['rawEqual']} "
            f"guided {replay['reconstruction']['guidedEqual']}"
        )
        print(f"   certification: {cert_text}")


def main(argv):
    replay_dir = None
    paths = []
    it = iter(argv)
    for arg in it:
        if arg == "--replays":
            replay_dir = next(it)
        else:
            paths.append(arg)
    if not paths:
        print(__doc__)
        return 2
    for path in paths:
        print_document(path, load(path))
        print()
    if replay_dir:
        print_replays(paths, replay_dir)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
