# Superseded evidence from the first (uncorrected) round

The files listed here are kept because deleting a measurement because it turned
out to be wrong is how a repository forgets what it learned. None of them may
be cited. The corrected round's documents are in `round6/`.

| file | why it is superseded |
|---|---|
| `battery-fixed-sched.json` | Its only arm is `"v3": false`, which the driver turns into the spec `work=120000000,cells=13:15:17:19,v3=0` (visible in every row's `spec` field). With `v3=0` the coordinator's v3 loop never runs, so the schedule class never runs, so mode 34 never runs, so no self-metered charge is ever reported and `debit_self_metered` is never called. The depths 174.208 / 176.056 / 179.006 are true numbers about runs that did not execute one line of the code under test. This is Sol review 6 §1 finding 1, and it is correct. |
| `battery-fixed-nosched.json` | Same `v3=0` defect, and additionally built without `compression-schedule`, so mode 34 does not exist in the binary at all. |
| `battery-baseline-sched.json`, `battery-baseline-nosched.json` | The paired halves of the two above. Same defect; a paired comparison of two arms that both skip the code proves only that they skip it identically. |
| `warmstart-159.092-baseline-40M.json`, `warmstart-159.092-fixed-40M.json` | The first round already reported this probe as inconclusive: its depth and work total match the *cold-start* seed-1 numbers exactly, so the pinned 159.092 parent was not in fact the run's starting point. That reading stands. |
| `gates-fixed-jagua-experimental.json` | Not wrong - the four gates did pass - but produced by a binary built before the Sol review 6 §1 corrections. Superseded by `round6/gates-fixed.json`, which was produced by a binary built from the committed source, and is paired with `round6/gates-unfixed.json` and a whole-document diff. |
| `portfolio-rs.diff` | The diff of the uncorrected implementation (`f32c629..66060f1`). Superseded by `round6/portfolio-rs.diff`. |
| `portfolio-unit-tests.log` | 26 tests, before the six Sol review 6 §1 asked for existed. Superseded by `round6/suite-jagua-sched.log`. |

The round-1 drivers `drivers/lib-repointed.py`, `drivers/runlib-repointed.py`
and `drivers/warmstart-{baseline,fixed}.sh` are kept for the same reason and
are equally uncitable: their `ROOT` is `/tmp/topo-work-wf48`, a worktree that
no longer exists. The round-2 equivalents are `drivers/gatelib.py` and
`drivers/runlibv6.py`, the same files repointed at this round's worktree
(`gatelib.py` differs from
`docs/experiments/constructor-inner-certificate/drivers/lib.py` on the `ROOT`
line and nothing else, which is the gate contract this round was handed).

What the superseded battery *did* establish, and which the corrected round did
not need to redo: that the fix is inert when the schedule class is off. That is
worth exactly as much as it sounds like, and the first round presented it as
the headline finding rather than as the control.
