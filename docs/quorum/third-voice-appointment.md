# Prospective appointment of the third reviewer

**Committed and pushed before any candidate is invoked.** That is the whole
point of the document: the deciding voice is named by a rule fixed in advance,
so nobody - least of all the proposer, who wants a particular answer - can be
accused of having chosen the voter after seeing the vote.

Sol, round 4: *"Ox-alpha - or a prospectively appointed independent replacement
if that provider remains unavailable - must cast the deciding vote against the
already frozen specification and evidence. The owner must not silently invent
either majority or unanimity after seeing the result."*

Grok, round 4: *"A third voice can vote. It cannot rewrite the adoption rule
after seeing the data."*

The owner was asked and chose this route over an owner overrule.

## Why a replacement at all

`ox-alpha` is the campaign's third seat. It has been unavailable for the whole
of this quorum and was retried at the close of every round; the last attempt, at
the close of round 4, returned `UnknownError / Unexpected server error`
(`ref err_6fcbf28f`), as did the attempts at rounds 1 and 3 (`err_10fa293a`).
The two-model quorum is already a recorded deviation. A 1-1 split writes
nothing, so without a third seat the question is simply closed by default rather
than decided.

## Selection rule, fixed here

1. **Independence.** The candidate must come from a model family **not already
   represented in the quorum**. Sol is OpenAI/Codex and Grok is xAI, so
   `opencode-go/gpt-*` and `opencode-go/grok-*` are excluded by rule, not by
   preference.
2. **Order of invocation**, fixed now and not to be reordered:

   1. `opencode-go/kimi-k3`
   2. `opencode-go/deepseek-v4-pro`
   3. `opencode-go/qwen3.8-max`
   4. `opencode-go/glm-5.3`
   5. `opencode-go/minimax-m3`

3. **First complete ballot wins the seat.** A ballot is *complete* iff it
   returns a decision on every one of the four questions in
   `third-voice-brief.md` - `Q1` through `Q4` - each identifiable as an answer
   to that question. Anything else - a provider error, a refusal to answer, a
   response that omits a question, a response that answers a different question -
   is incomplete, and the next candidate in the order is invoked.
4. **No shopping.** Once a candidate returns a complete ballot, **no later
   candidate is invoked**, whatever the ballot says. Earlier candidates are not
   retried. The transcript of every invocation, including the failures, is
   recorded verbatim.

## What the third voice may decide, and what it may not

It votes on one question: **does `EXPLORE_SHRINK_STEP` move to `0.032` as the
ten-second wall profile of the ICS engine?**

It may **not**:

- re-score the signed round, whose clauses were fixed before its first cell and
  whose evaluation has already been audited and corrected once;
- re-scope, amend or re-run either specification;
- rewrite the adoption rule after seeing the data (Grok's constraint, which the
  proposer accepts as binding on the appointment);
- propose a third value of the step. Only `0.032` is on the ballot, because only
  `0.032` has a passed specification behind it.

## Resolution

- **Third voice with Sol** (write `0.032` as the ten-second profile): 2-1, and
  it is written - as the ten-second wall profile only, never as a thirty-second
  default where `0.016` is the better datum, with `--shrinkstep` retained so
  every pre-ratification replay reproduces.
- **Third voice with Grok** (refuse): 2-1, nothing is written,
  `EXPLORE_SHRINK_STEP` stays `0.001`, and `--shrinkstep=0.032 --itercap=0`
  remains the documented ten-second CLI recipe.
- **Third voice declines to break the tie**: that is a complete ballot only if it
  answers Q1 with an explicit abstention *and* answers Q2-Q4. An abstention
  leaves 1-1, and nothing is written.

Already written and outside this appointment: `Pacer::Wall::iteration_cap`
defaults to `50`, ratified unanimously in round 3.

---

# Amendment, committed before the re-invocation: the first two were never asked

The first two invocations both returned `RC=124` - killed by `timeout` at 900 s
with zero bytes on stdout and stderr. Under the rule above that is an incomplete
ballot and the seat passes on. **It is not.** It was a defect in the proposer's
harness, and recording it as a candidate's failure would have been a silent
manipulation of an order this document exists to fix.

A positive control on `opencode/hy3-free` - a model deliberately **not** in the
candidate order, so the order could not be contaminated by the test - reproduced
the hang on a one-line prompt, which no model could plausibly take 240 s to
refuse. Closing stdin fixes it:

| invocation | result |
| --- | --- |
| `opencode run --pure -m M --agent plan "..."` | `RC=124`, 0 bytes, 240 s |
| `opencode run --pure -m M --agent plan "..." < /dev/null` | `RC=0`, `PLUMBING_OK`, seconds |
| `opencode run --pure -m M "..." < /dev/null` | `RC=0`, `PLUMBING_OK`, seconds |

`opencode run` blocks on an open stdin. The candidate invocations inherited a
pipe and waited on it until the timeout. The prompt never reached the model.

## Ruling

An invocation that never delivered the brief is **not a ballot**. The rule's
list of incompletenesses - a provider error, a refusal, an omitted question, a
different question answered - all presuppose that the model was asked. None was.

Therefore the order **restarts from the top** with the corrected invocation:
`opencode run --pure -m <model> --agent plan "<brief>" < /dev/null`, the same
900-second ceiling applied uniformly to every candidate. `opencode-go/kimi-k3`
is invoked again, and the clause "earlier candidates are not retried" is not
engaged, because it governs a candidate that *answered*, and neither did.

The failed transcripts are kept at `evidence/third-voice/` exactly as they came
back - two empty files and their exit codes - because the record of a proposer's
harness bug belongs in the record as much as a ballot does.

This amendment is committed and pushed **before** the corrected invocation runs.

---

# The seat, and the verdict

`opencode-go/kimi-k3`, first in the fixed order, returned a **complete ballot**
on the corrected invocation - all four questions decided. By the rule above it
takes the seat, and **no later candidate was invoked**. The ballot is verbatim
at `evidence/third-voice/1-kimi-k3.ballot.txt`; it opens by checking the brief's
claims against the repository rather than taking them from me, and names the
lines it checked.

| | Q1 | Q3 | confidence |
| --- | --- | --- | ---: |
| Sol | write | (a) contender | 98 |
| Grok | refuse | (b) research-only | 70 |
| **kimi-k3** | **WRITE** | **(c)** the configuration, not the engine | 78 |

**2-1 to write**, on the terms the appointment fixed in advance: `0.032` as the
**ten-second wall profile only**, never as a thirty-second default where
`0.016` is the better datum, with `--shrinkstep` retained so every
pre-ratification replay reproduces.

Its answer to Q2 - the question the whole quorum turned on - is worth quoting,
because it is the argument neither sitting reviewer made:

> The forbidden-rescue rule forbids fitting a literal after seeing results and
> then validating with those same results - it is a rule about evidence status,
> not about a value's ancestry. What distinguishes Sol's route: `0.032` was
> pre-registered in a signed specification before the validating cells ran, then
> passed every clause on seeds that chose nothing ... Grok signed a spec under
> the same protocol, and its failure (0.02, his only signable value) shows the
> protocol has teeth rather than rubber-stamping proposals.

And it corroborates Grok against Grok:

> the sweep finding is corroborated off-fixture: Grok's own shipped-engine
> falsifier predicted and confirmed `0.02 ~ 0.032` on an engine sharing none of
> ICS's operators - the coarse regime is generic, a broad optimum, not the
> knife-edge "whatever reaches 168 in 80 bites" the freeze text was aimed at.

Its Q3 is neither Sol's nor Grok's: **(c)**, the measured configuration is a
feature-gated production contender, the engine at its faithful defaults - which
loses to the shipped path - stays research-only.

Its own strongest counter-argument, which it was asked for and gave: the shelf
reading, the budget-dependence, and that *"writing a per-budget literal now sets
a precedent that future post-selected constants may ride, grandfathered by this
one, instead of the parametric law the schedule-defaults ballot already names as
the correct form."*

## The one thing the ballot does not settle

"The ten-second wall profile" is a sentence, and the engine needs a rule. The
passed specification ran arm D at **10.000 s and nowhere else**. What a wall of
7 s or 11 s should do is not in any ballot, and the proposer will not invent it:
see the open question put to both sitting reviewers in round 5.

---

# Round 5: what was written, and the defect Grok caught before it was built

The two sitting reviewers converged on the *shape* of the write while remaining
split on its merit.

**Sol:** *"a named profile, not a numeric interval. Add `ScheduleProfile::Wall10s`
containing `step=0.032`, cap `None`, ratio `0.80`. ... This implements the 2-1
WRITE as configuration, avoids inventing behavior at 3/7/15 seconds, and avoids
pretending an exact floating-point comparison is a scheduling theory."*

**Grok:** *"caller-named request wall of 10 s, wall mode only, uses the passing
pair `step 0.032` / `cap none`. Every other named wall, and all of fixed-work,
keep `0.001` / cap 50. Bind on the request the caller named, not leftover clock.
Do not mutate the const `EXPLORE_SHRINK_STEP`; the identity pin stays `0.001`. A
global const is a thirty-second default."*

## The defect in my own appointment wording

Grok, asked whether the *appointment* was defective:

> It names the **step** and leaves cap 50. The passed specification is D:
> `0.032` / **none**. Holdout `(0.032, 50)` median **164.245**, worst
> **170.550** - the pair that cannot finish a coarse bite. ... Writing the const
> onto the ratified cap is a third configuration nobody passed.

He is right and it would have been built. The appointment's resolution clause
said "write `0.032` as the ten-second profile" and said nothing about the cap,
and the cap had just been ratified at `50` in a *different* round at a *different
step*. `Wall10s` therefore carries **cap none**, which is what arm D ran, and the
ratified `50` stays where it was ratified: on `Legacy`.

## And the trap in binding on the clock

`Budget::Wall` is *leftover* clock: a ten-second request enters the loop with
about 7.7 s after the constructor. Binding the profile on that number would have
made a "ten-second profile" into a seven-second one without anybody noticing.
`validate_for` takes the **request's** named wall.

## Grok on the outcome

> I accept the 2-1. I named three reviewers as the campaign and the two-model
> quorum as the deviation. A third voice can vote. I do not convert this to 1-1.
> ... The stdin hang was the proposer's harness, positive-controlled off-order,
> amendment committed before re-invocation. Not a defect.
>
> I still will not sign `0.032`. Majority is not my signature.

## What is in the tree

| | step | wall cap | ratio |
| --- | ---: | ---: | ---: |
| `ScheduleProfile::Legacy` (default, everything unnamed) | 0.001 | 50 | 0.80 |
| `ScheduleProfile::Wall10s` (`--profile=wall10s`) | **0.032** | **none** | 0.80 |

`EXPLORE_SHRINK_STEP` is **still `0.001`** and is still the identity pin.
`Wall10s` refuses wall 7 s, 9.999 s, 10.001 s, 15 s, 30 s and fixed-work mode.
`--shrinkstep` and `--itercap` still override both profiles, so every
pre-ratification replay reproduces. Four pinned gates `ALL_PASS`; 841
`overlap-ics` tests, 1,104 workspace tests.

Neither reviewer wants another quality battery. Sol: *"run no further quality
battery before writing the authorized ten-second profile."* Grok: *"then stop on
the step."*
