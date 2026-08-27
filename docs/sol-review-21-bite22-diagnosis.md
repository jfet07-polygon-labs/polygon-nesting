# Sol review 21 — the bite-22 diagnosis

Codex (Sol) at `model_reasoning_effort="max"`, read-only sandbox, resumed on
the campaign session. Asked to refute or confirm the bite-22 diagnosis, to
decide whether `mod.rs:2002`'s `None => break` is a defect or faithful to
Algorithm 12, to name one mechanism for a new pre-committed specification, and
to name the gate that would kill his own proposal.

Answered in Italian, verbatim below. He confirms the barrier, **corrects four
overclaims in the brief** — the nine bite-21 parents are not one layout, seed
1 is at 19 bites not 21, the escape-time span is about 30.5x rather than three
orders of magnitude, and clearing bite 22 is necessary but not sufficient —
and finds independently, before the census existed, that seed 7 enters the
4 µm band 5,885 times with zero exact checkpoint calls.

---

1. **Confermo il “bite-22 barrier”; respingo la formulazione assoluta.**

La semantica del contatore è autentica: `explore_bites` aumenta solo dopo una pubblicazione riuscita; il bite fallito viene registrato e non contato ([mod.rs:1970](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1970)). Quindi `exploreBites=21` significa davvero “21 bite pubblicati, ordinal 22 aperto e non pubblicato”, non 21 tentativi.

I record confermano il collo:

- seed 3 chiude il 22º bite in 137 batch/2.278M eval ([raw cell:1092](/var/lib/t3/tmp/overlapics/deterministic-30s-round/gate10/gate10-composed-seed3-r0.json:1092));
- seed 7 a 30 s resta sullo stesso bite dopo 7,262 batch, 23 strike, 7 disruption e 60.38M eval ([raw cell:1091](/var/lib/t3/tmp/overlapics/deterministic-30s-round/curve30/curve30-composed-seed7-r0.json:1091));
- a 60 s seed 7/8 lo chiudono infine dopo 63.63M/69.56M eval.

Ma correggo quattro affermazioni:

- Non è “un layout”. I nove parent pubblicati al bite 21 hanno nove fingerprint diversi; perfino seed 7 e 8, con la stessa profondità, hanno fingerprint `cca8…` e `32c9…` ([seed 7:18474](/var/lib/t3/tmp/overlapics/deterministic-30s-round/gate10/gate10-composed-seed7-r0.json:18474), [seed 8:18539](/var/lib/t3/tmp/overlapics/deterministic-30s-round/gate10/gate10-composed-seed8-r0.json:18539)). È una barriera comune per ordinale/profondità su basin già divergenti.
- La tabella stessa smentisce “tutti arrivano a 21 entro 0.61 s”: seed 1 è a 19.
- Non sono tre ordini di grandezza misurati. Il lavoro del bite 22 va da 2.278M a 69.56M eval, circa **30.5×**. Il `<0.1 s` non deriva dai dati.
- Il **mediano 179** è spiegato dai cinque bite censurati, ma il fallimento 5/9 non è soltanto quello: seed 0 e 6 superano il bite 22 e restano comunque a 170.44/169.35. Inoltre seed 3 accumula 13 disruption dopo il 22º bite. Non tutto il seguito è “flusso libero”.

La frase onesta è: **il plateau pubblicato a 179 è principalmente una barriera di reachability al 22º target, comune ma non geometricamente identica; non è un asintoto di qualità.** È però ancora un fenomeno di basin, e più lavoro ne aumenta la probabilità di fuga. Non lo chiamerei né “nessun plateau” né “nessun deficit temporale”.

Il falsificatore sarebbe stato: `exploreBites` conta anche i fallimenti, il record ordinal 22 manca, oppure il budget viene speso altrove mentre il contatore resta 21. Codice e record escludono tutte e tre le letture.

C’è inoltre un dettaglio più diagnostico: seed 7 entra **5,885 volte** nella banda da 4 µm ma effettua zero checkpoint esatti. Il top residual usa coerentemente `T - depth_top_inset` ([broad_phase.rs:46](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/broad_phase.rs:46)); l’entry band permette quindi fino a 4 µm sopra `T`, mentre `publish::attempt` rifiuta rigidamente qualsiasi `proxy_depth > T` prima della riparazione ([publish.rs:360](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:360)). Non è un clearance mismatch: è un labbro terminale misurato.

2. **`None => break` è fedele; non è il bug.**

Con budget residuo, un fallimento non raggiunge quel `break`: viene messo nel pool, si seleziona un rank, si reinstalla, si disrupta e si richiama `separate` allo stesso `W` ([mod.rs:1781](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1781), [mod.rs:1861](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:1861)). Nel percorso Calibrated, `attempts_per_bite=0` significa esplicitamente illimitato ([mod.rs:3550](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:3550)).

Il `None` esterno arriva normalmente solo dopo deadline/esaurimento della fase. Sparrow fa la stessa cosa: finché `!term.kill()` resta allo stesso width, poola, ripristina e disrupta; quando il terminatore scatta, esce dall’explore ([explore.rs:32](/var/lib/t3/tmp/sparrow-bench/src/optimizer/explore.rs:32), [explore.rs:51](/var/lib/t3/tmp/sparrow-bench/src/optimizer/explore.rs:51)). Il nostro inner loop ha semplicemente fuso quel ciclo allo stesso bite. Non esiste un “next bite” lecito senza una nuova soluzione legalizzata.

La differenza vera è intenzionale: Sparrow avanza su proxy loss zero; noi solo su pubblicazione dual-valid. Non rende difettosa la riga 2002.

3. **Il meccanismo che testerei: target-aware depth-band repair.**

Oggi la banda dice “abbastanza vicino da tentare la micro-riparazione”, ma il controllo `proxy_depth > T` la tronca prima che la riparazione possa vedere il vincolo top. Non basta cancellare quella guardia: lo scan esatto usa il top fisico del foglio, quindi potrebbe risultare già admissible e arrivare comunque al rifiuto finale `published_depth > T` ([publish.rs:446](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:446), [publish.rs:526](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:526)).

La nuova operazione sarebbe:

- quando `max_g ≤ 4 µm` e `0 < proxy_depth-T ≤ 4 µm`, clonare lo stato;
- aggiungere il locked-strip top come riga di riparazione;
- applicare la riparazione deterministica frozen-θ già esistente, alternando top e coppie generate;
- mantenere invariati banda, cap di 16 µm, target, kernel Exclusive e validatore contrattuale;
- pubblicare solo se il risultato finale ha `raw_depth ≤ T` ed è dual-valid.

È un nuovo meccanismo terminale, non una tolleranza ampliata, non un retune e non riapre il tracker-rebase. Attacca direttamente le migliaia di stati quasi pubblicabili osservati. Non finanzierei ancora persistent lanes: prima va falsificato questo seam molto più stretto e causalmente visibile.

4. **Gate pre-quality che lo uccide.**

Userei i nove parent esatti post-bite-21 di Round 4 e lo stesso cap di lavoro residuo del braccio composed a 10 s.

Il Gate 0 passa solo se:

- il controllo riproduce bit-for-bit le quattro chiusure note `{0,2,3,6}`;
- in Shadow, la nuova riparazione chiude esattamente il bite 22 in almeno **3 dei 5** seed bloccati `{1,4,5,7,8}`;
- ogni nuovo successo nasce da uno stato che il guard attuale avrebbe rifiutato esclusivamente per `proxy_depth>T`;
- nessuna delle quattro chiusure di controllo regredisce;
- profondità finale `≤T`, displacement ≤16 µm, zero pubblicazioni invalide, identità a due processi;
- il braccio ComputeIgnore conserva la traiettoria e almeno il 95% del rate appaiato del controllo.

**Pre-declared FAIL:** meno di tre conversioni, una reverse, nessun witness causalmente depth-gated, violazione dei cap/autorità, nondeterminismo o costo oltre il 5%. In quel caso il labbro da 4 µm non è legalizzabile con l’infrastruttura esistente: niente batteria 3/10/30 e il gate 10 s resta ritirato. Un PASS licenzia soltanto una nuova batteria end-to-end col vecchio bar non ammorbidito; non costituisce già un risultato di qualità.
---

# Round 2 — cross-exchange

Sol was sent the publication-gate census together with Grok's complete round-1
answer, and asked to converge, to name the one mechanism in final form, to
merge the two gates, and to say honestly which bar the mechanism is aimed at.
Verbatim below. His three factual corrections to the census prose were checked
against the committed cells and are all correct; they are folded into the
microscope README §3, and the methodological one — that an above-target state
is never memoized, so entries are not opportunities — is what §7's distinct-
digest count was built to answer.

1. **Converge: I keep the terminal T-row repair; the entry sweep waits.**

I disagree with:

> “run one sequential Gauss–Seidel relocate pass over the cut-moved pieces”

as the first funded mechanism. Its 400-iteration probe measures the early entry transient; the refusal that actually closes the bite occurs later. At 30 s, seeds 7/8 reach the band 5,235/3,060 times and are rejected above target 5,212/3,026 times, so entry is demonstrably not their binding problem at that clock. [README.md:166](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/bite22-microscope/README.md:166>)

The census prose nevertheless needs three corrections before becoming specification evidence:

- The digest guard does **not** fire only 0–3 times. The nine raw cells total 302 repeats; seed 0 alone has 150 and seed 6 has 120. [g-seed0.json:1601](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/bite22-microscope/evidence/g-seed0.json:1601>)
- Not every above-target state improves: seed 2 has 1,038 above-target entries and 1,034 improvements. [g-seed2.json:3121](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/bite22-microscope/evidence/g-seed2.json:3121>)
- The overhang is not globally 1.5–4.0 µm: seed 8 reaches 0.712 µm, seed 4 0.141 µm, and seed 0 effectively zero. The important bound is `0 < excess ≤ 4 µm`; the typical frozen-tail mass is 2–3 µm. [g-seed8.json:521](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/bite22-microscope/evidence/g-seed8.json:521>)

More importantly, above-target states are not memoized: `last_attempt_pose_digest` is written only after `publish::attempt` returns `Some`; the target guard returns `None`, so identical proud states may be counted thousands of times. [mod.rs:594](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:594>) Thus these are thousands of **entries**, not proven-distinct opportunities.

None of that reverses the choice. Seeds 4/7/8 have zero whole-run digest repeats in the initial census and abundant target refusals; the 30-second tail is overwhelmingly lip-bound. It does mean the new gate must count per-bite **unique pose digests**, not quote whole-run entry totals.

The far-side sweep is not refuted. It is simply unsupported as Gate 1 and could pass its proposed Φ test without publishing anything. It requires a separately funded specification if the terminal repair later fails specifically because eligible states never arrive; it is not an automatic rescue.

2. **One mechanism**

When `max_g ≤ 4 µm` and `0 < proxy_depth − T ≤ 4 µm`, clone the state and run the existing deterministic frozen-θ, ≤4n-row, ≤16 µm-per-piece Gauss–Seidel micro-repair with the locked-strip top injected as a repair row, rechecking that row and all exact pair/boundary rows after every correction; publish only if `raw_depth ≤ T`, Exclusive `r=2.500` and the untouched contract validator all pass.

This is not deletion of the guard and not partial-bite acceptance. The existing exact scan checks the physical sheet, not the internal target [publish.rs:319](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:319>), while the final target check remains strict [publish.rs:526](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:526>). The nominal 0.1% bite and exact contract therefore remain unchanged.

3. **Gate, final form**

Setup:

- Mixed-61, seeds `0..=8`, `orders=1`, eight workers, 5.0/5.0 contract.
- Reproduce the Round-4 composed deterministic 10-second trajectory.
- Immediately after constructing bite 22—exact bite-21 parent installed, centre cut applied, target/weights reset—fork the **complete continuation state**, including pacer charge and stream ordinals. A pose-only parent restart is invalid because it creates a different RNG/GLS continuation.
- Run only the remaining explore allocation.
- Arms: `Control`, `TRepair`, and `ComputeIgnore`. `ComputeIgnore` performs the same repair on a detached clone, discards it, and keeps repair counters separate from the production work vector.

The contemporaneous control must reproduce the partition:

- closes bite 22: `{0,2,3,6}`;
- does not close bite 22: `{1,4,5,7,8}`.

If that partition does not reproduce, the instrument is invalid—not a mechanism result.

PASS requires all clauses:

1. **Per-bite census integrity.** At bite 22, every band entry partitions exactly into digest-repeat, above-target, non-improving, or exact-called. Counts are deltas for that bite, not whole-run totals.

2. **Unique install.** Every fresh eligible digest—band-valid, improving, and rejected only because `0 < proxy_depth−T≤4 µm`—invokes T-repair exactly once. Repeated identical digests are logged and skipped.

3. **Tail-relevant conversion.** T-repair publishes bite 22 for both seeds `{7,8}` and at least one of `{1,4,5}`: at least 3/5 conversions, with the two persistent 30-second tail seeds mandatory.

4. **Causal witness.** Every counted conversion begins at a state that Control would reject solely at [publish.rs:364](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:364>), and processes at least one synthetic T-row. A normal `proxy_depth≤T` publication does not count.

5. **No reverse.** `{0,2,3,6}` all retain their bite-22 closure under the identical residual cap and none finishes worse than its control.

6. **Authority and caps.** Every resulting publication has `raw_depth≤T`, frozen θ, ≤16 µm cumulative displacement per piece, ≤4n corrections, Exclusive `r=2.500`, untouched contract validity, and independent revalidation. The frozen limits remain those already declared in [publish.rs:54](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:54>).

7. **Isolation, cost, determinism.** `ComputeIgnore` is bit-identical to Control in poses, publications, fingerprints, base work and pacer state after stripping shadow diagnostics. Its paired base-trajectory rate is at least 95% of Control. Control and T-repair are each two-process bit-identical; default-build and existing FAST/soundness gates remain green.

Pre-declared failure: either 7 or 8 does not convert; fewer than 3/5 convert; an eligible digest bypasses the repair; any reverse, authority/cap violation, isolation divergence, >5% cost, or nondeterminism occurs. Then this mechanism is closed and no quality battery runs. “Too few unique eligible states” is also a valid miss—not permission to append the far-side sweep.

4. **Aim it at 30-second quality, not a 10-second reopening.**

The 10-second gate remains retired. Clearing bite 22 still leaves roughly 62 successful bites to reach 168.484; seeds 0 and 6 already clear it and finish at 170.44/169.35. A repair first offered after 1,100–1,600 iterations can succeed causally yet leave too little clock for that cascade. Reopening 10 seconds would therefore test a promise this mechanism does not make.

Gate 0 deliberately uses the 10-second residual because it discriminates the five frozen seeds; at 30 seconds seeds 1/4/5 already escape naturally, making an ordinary 3/5 mechanical clause vacuous.

After Gate 0 passes, run a from-request, contemporaneously paired deterministic 30-second battery. PASS requires:

- median ≤163.00461 mm;
- ≥7/9 at ≤168.484 mm;
- paired median gain ≥1.000 mm over T-repair-off Control;
- seeds 7 **and** 8 individually ≤168.484 mm;
- zero invalid publications and all plan/charge identities green.

Those preserve the standing 30-second clauses [deterministic-30s-round-spec.md:105](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/deterministic-30s-round-spec.md:105>) and add a causal tail tightening. The 10-second column is run and reported against its old unsoftened numbers, but carries no verdict. If the 30-second gate misses, T-repair closes; it does not reopen the sweep, patience constants, tracker rebase, or the retired 10-second chase.
---

# Ballot on the frozen specification

Sol returned, on sha256 `06054148b294309498823f3a29e4e801478b1e4c5cba2baff85ba8496c5975f7`:

> **CONFIRM.** I read the complete file identified by its sha256, checked it
> against my own round-2 text, the census evidence, and the five resolutions
> above, and I confirm it without reservation or hidden amendment.

