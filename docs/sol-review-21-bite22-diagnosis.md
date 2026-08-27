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