# The two measurements the quorum ordered, and what they did to its votes

Round 2 of `docs/quorum/schedule-defaults-ballot.md` ended with each reviewer
naming one measurement that would most change their vote. Both were run as
specified. Both disagreed with the vote that ordered them.

## Sol's pre-committed factorial, held-out seeds 9-17

Seeds 0-8 were the discovery population for every sweep of the previous night;
the "third repetition" was more of them, not a holdout. These nine seeds chose
nothing. Five repetitions, ratio `0.80` - the real default, not the swept
`0.95` - one fresh process per cell, 45 cells per arm.

| cap | step | median | mean | best | worst |
| ---: | ---: | ---: | ---: | ---: | ---: |
| none | 0.001 **(the real defaults)** | 169.221 | 171.791 | 162.799 | 179.082 |
| **50** | 0.001 | **165.287** | **165.445** | 163.053 | **168.621** |
| 200 | 0.001 | 166.961 | 167.443 | 161.376 | 176.151 |
| none | 0.032 | 159.480 | **158.941** | **154.108** | **160.304** |
| 50 | 0.032 | 164.245 | 163.131 | 159.000 | 170.550 |
| 200 | 0.032 | 159.297 | 158.966 | 154.582 | 160.305 |

Decomposed and paired:

- **at step 0.032 the cap is worth `+0.008 mm` and wins 5 of 9.** Once the bite
  is coarse the cap does nothing.
- **at cap 200 the step is worth `+9.060 mm` and wins 9 of 9.**
- **cap 200 alone regresses on 3 of 9 held-out seeds** (`-2.160`, `-2.337`,
  `-4.697`).

Grok's pre-committed concession clause - *"if `(0.001, 200)` wins median and
worst at 10 s by >= 2 mm versus `(0.001, 50)`, I abandon 50"* - resolves against
the prediction that framed it: `200` is **worse**, paired median `-1.942 mm`,
4/9, worst `176.151` against `168.621`. Grok's other prediction holds:
`(0.032, 50)` loses badly, 163.131 mean and 170.550 worst, on the retries a
50-iteration separation cannot finish.

**The quorum had approved the change that does nothing on top of the step and
whose stand-alone form hurts a third of the held-out seeds.**

## The head-to-head against the engine this repository actually ships

`general_request_benchmark`, bare request, no pinned parent. Its work
parameters were calibrated to the wall: `relaxed-epochs 36, sweeps 160,
refinement 8` gives a mean of **10.56 s**. Same held-out seeds, three
repetitions.

| arm | median | mean | best | worst | paired vs shipped | wins |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| **shipped `general_relaxed`** | 165.904 | 166.713 | 162.847 | 170.666 | - | - |
| ICS at its real defaults | 169.221 | 171.791 | 162.799 | 179.082 | **-3.081** | 3/9 |
| ICS cap 50, step 0.001 *(approved)* | 165.287 | 165.445 | 163.053 | 168.621 | **+1.101** | 5/9 |
| ICS cap 200, step 0.032 *(refused)* | 159.297 | 158.966 | 154.582 | 160.305 | **+6.893** | **9/9** |

**The ICS engine at its own faithful defaults is 3 mm worse than the engine this
repository already ships.** The change the quorum approved brings it to a coin
flip. The change it refused is the only one that wins, and it wins on every
held-out seed.

Two facts found on the way that are worth keeping:

- the shipped path **has no deadline**. It is work-parameterised, and its wall
  is an outcome: the calibrated configuration's mean is 10.56 s and its max
  12.97 s. At the pinned driver's own settings it finishes in 2.4 s at 179.9 mm
  and **more epochs change nothing at all** - 24 and 192 produce byte-identical
  results. `relaxed-sweeps` is the knob that binds.
- `relaxed-failed-attempts-per-depth` at 1, 8 and 32 gives byte-identical depth.
  The shipped path's retry knob is inert.

## Grok's falsifier on the shipped path: both predictions confirmed

Sweeping the shipped engine's own `initial_shrink_ratio`, nine seeds, everything
else at the pinned tail:

| shrink | median depth | wall |
| ---: | ---: | ---: |
| 0.001 | **181.601** | 1.50 s |
| 0.005 | 179.821 | 1.96 s |
| 0.02 | 179.887 | 2.63 s |
| 0.032 | 180.488 | 2.78 s |

Grok predicted *"`0.001` loses and `0.02 ~ 0.032`"*. Both halves hold, on an
engine that shares none of ICS's operators. The economics generalise; the
prescription is one number wide.

## What this round is not allowed to do

It is exploratory evidence, and by the quorum's own round-3 ruling it **cannot
approve anything**. Sol: *"i seed 9-17 ora sono evidenza esplorativa consumata:
non possono approvare il cambiamento."* Grok: *"a later specification, committed
before the next cell, can lift a freeze. This table cannot."*

The specification that can is `docs/quorum/ics-schedule-round-spec.md`,
committed before its first cell ran.
