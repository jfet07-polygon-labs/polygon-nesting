# Sol review 15 — the autopsy of Gate 0 (2026-08-22)

Adversarial refutation of Gate 0's STOP (merged 9b3754d), per the owner's
no-failure-without-autopsy rule. Codex resumed instance, max effort,
read-only. Verbatim.

---

Verdetto: il **STOP come verdetto sulla famiglia overlap-ICS non regge**. Va mantenuto lo stop operativo prima di schedule/parallelismo, ma Gate 0 deve essere rieseguito. Triangle-20 e C175 sono contaminati da difetti reali; S1 è vera evidenza contro il move set mono-pezzo corrente, non ancora contro il paradigma.

## A. Classificazione delle celle fatali

| Cella | Verdetto |
|---|---|
| triangle-20 | **IMPLEMENTATION DEFECT**; il fallimento corrente non è utilizzabile |
| C175 | **IMPLEMENTATION + METHOD DEFECT**; non è il test precommittato eseguito fedelmente |
| S1 | **PARADIGM EVIDENCE limitata**: vero fixed point del descent mono-pezzo, ma escape jump e tie-break non sono conformi alla specifica |
| S0 / numeric / throughput | Passano e restano validi |

### 1. Triangle-20: confermo il bug, ma non la spiegazione “one-line e completa”

Il top del target usa davvero il clearance sbagliato:

- Il contratto fisico richiede `sheet_edge + sag` sui quattro bordi reali ([general_polygon.rs:200](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:200), [general_polygon.rs:530](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:530)).
- La profondità pubblicata è invece `max_y + sheet_edge`, senza sag ([state.rs:365](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/state.rs:365), [publish.rs:191](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:191)).
- Φ passa un unico `edge_clearance_mm() = edge+sag` a tutti e quattro i lati, incluso il top virtuale del target ([energy.rs:90](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:90), [broad_phase.rs:26](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/broad_phase.rs:26)).

Quindi il top deve usare `sheet_edge`; left/right/bottom, essendo sheet edge reali, devono mantenere `sheet_edge+sag`.

Ma l’aritmetica non attribuisce tutto al phantom: il top può spiegare al massimo `0.11027 mm`, mentre `max_g=0.11765 mm`. Il verifier stesso dimostra quindi almeno `7.38 µm` su un bordo fisico ([gate0-verification README:169](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/gate0-verification/README.md:169)). Tenere fisso lo stato finale e dire “il verdetto non cambia” è però scorretto: eliminare il top fantasma cambia forze, pesi e intera traiettoria.

Inoltre il bug non è one-line:

- `compressed()` ancora il pavimento a `sheet_edge`, ma il bottom fisico richiede `edge+sag` ([homotopy.rs:115](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/homotopy.rs:115)).
- `lower_scale_mm()` usa `2·edge`; per triangle-20 il bound corretto è circa `60 + 5.25 + 5.0 = 70.25`, non 70.0 ([homotopy.rs:72](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/homotopy.rs:72)).
- Jump, uniform throw, corpus e `sheet_slack` ripetono la simmetria errata ([descent.rs:294](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/descent.rs:294), [overlap_ics_benchmark.rs:901](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/examples/overlap_ics_benchmark.rs:901), [publish.rs:591](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:591)).

Nota minore: il README dice che triangle-20 libera il contratto pair “5.0”; il JSON e il codice dicono correttamente `5.5 = 5.0 + 2·0.25` ([state.rs:97](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/state.rs:97)).

### 2. Jump: confermo che è stato neuterato, e trovo una seconda violazione

Il default `false` è contrario alla specifica precommittata ([descent.rs:40](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/descent.rs:40), [sol-review-14:524](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/sol-review-14-the-overlap-engine-design.md:524)).

Peggio: il “bounded local sweep” non è uno sweep. Per ogni candidato vengono eseguite quattro proposte sul solo pezzo rilocato ([descent.rs:338](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/descent.rs:338)). Nessun vicino si assesta. Inoltre:

- il commit resta subordinato a `improved_guided` ([descent.rs:366](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/descent.rs:366));
- l’allowance viene consumata prima di sapere se il jump ha mosso qualcosa ([descent.rs:260](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/descent.rs:260)).

L’A/B `always` contro `guided` non chiude la questione: confronta una rilocazione strip-wide applicata anche a un residuo di 12 µm, senza il local sweep specificato. Il pessimo `2.55 mm` su S1 è la conseguenza attesa della scala sbagliata.

La semantica corretta è a due scale:

- `max_g > 0.100 mm = 25·EPSILON_GRID`: jump globale, miglior candidato commesso incondizionatamente;
- `max_g ≤ 0.100 mm`: non un jump globale. Usare i medesimi 16 candidati in una palla SE(2) locale con raggio traslazionale `ρ=2·max_g` e raggio angolare equivalente `ρ/R`;
- da ogni candidato: un vero sweep completo su tutti i pezzi;
- una sola adozione totale, poi normale descent per il resto dell’epoca.

Sopprimere semplicemente il jump sotto 0.1 mm lascerebbe S1 nel fixed point già certificato e non costituirebbe una refutazione.

### 3. S1: non c’è il bug di bookkeeping ipotizzato

Le ipotesi (c) e (d) sono smentite dal codice:

- `incident_guided` include tutte le coppie e tutte le quattro righe di bordo ([energy.rs:225](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:225)).
- `guided_update` considera esplicitamente le edge rows ([energy.rs:310](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:310)).
- Ogni pezzo viene comunque visitato nello sweep, ordinato per pressione ([energy.rs:367](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:367)).

La scala minima è 0.25 µm, quindi non manca banalmente un rung da 12 µm ([descent.rs:92](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/descent.rs:92)).

Il meccanismo osservabile è questo: ogni pezzo riceve una sola direzione SE(2) aggregata e ogni rung deve diminuire strettamente l’energia incidente ([descent.rs:148](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/descent.rs:148)). Aumentare i pesi non introduce una direzione nuova; dopo la normalizzazione può lasciare invariata la direzione. Il sistema è quindi in un minimo coordinate-wise della mossa corrente.

Due qualificazioni:

- S1 non ha pair rows “clear”: ne restano 2, massimo `11.72 µm`, oltre alle 2 edge rows ([overlap-ICS README:158](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/README.md:158)).
- `maxGuidedPenalty` include anche righe inattive ([energy.rs:425](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:425)); non prova che la specifica edge row bloccante abbia ricevuto quel peso.

Prima della rerun serve un rejection census: per pezzo/rung, `Δguided`, `Δraw`, `Δmax_g`, nuove righe attivate, direzione translation-only/rotation-only/combinata e penalità delle sole righe attive. Non accenderei ancora chain move o SOR: sarebbero nuovi operatori. Il jump locale scale-matched è già il knob preconcordato più piccolo.

### 4. C175: il target resta, la cella eseguita no

La formula `D−0.10(D−L)` deve restare immutata. Ma il driver:

1. comprime sul target;
2. poi aggiunge una perturbazione ulteriore ±0.25 mm/±1° ([overlap_ics_benchmark.rs:553](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/examples/overlap_ics_benchmark.rs:553)).

Questo spiega perché l’entry è già circa 0.8 mm oltre il target. La cella nominata era affine shock, non affine shock più rotazione casuale post-target. Inoltre il jump era un no-op e i 200k proposal hanno usato solo 1.26–1.44 dei due secondi disponibili ([overlap-ICS README:538](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/README.md:538)).

Rerun corretto:

- perturba eventualmente il parent per differenziare i seed, poi comprimi ciascun parent sul medesimo `T`;
- assert `entry_depth ≤ T + 1e-9`;
- jump macro funzionante;
- quota congelata a **240k proposal**, derivata prima del codice dal più lento throughput misurato (`987,861/8s × 2s`, arrotondata verso il basso);
- stesso pass originale: almeno 1/3 child strict, dual-valid, entro T.

## B. Fix minimi prima della rerun

1. Separare `physical_edge_clearance` e `depth_top_inset` in `Contract`, `boundary_residuals`, energia, corpus, homotopy, jump e repair slack.
2. Correggere floor e lower bound sag-aware.
3. Rendere il jump a due scale, commit reale, vero local sweep, contatori `attempted/committed`; non consumare allowance su no-op.
4. Riallineare C175 al target dopo ogni perturbazione.
5. Aggiungere rejection census e per-side edge census.
6. Correggere un altro difetto certo: a contatto materiale esatto `closest_feature` produce una violazione positiva con normale zero ([contact.rs:187](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/contact.rs:187)). Il ramo SAT deve conservare un asse deterministico anche a depth zero.
7. Test sag-specifico: top virtuale soddisfatto a `max_y=T−edge`, mentre left/right/bottom continuano a richiedere `edge+sag`.

## C. Lettura precommittata della rerun

Nessun band widening, target relaxation, repair enlargement o cambio seed dopo i risultati.

Pass solo se:

- S0 resta bit-identico: Φ bits 0, depth 150.16451, dual-valid, repair 0.
- Numeric: 0 false-feasible fuori banda, 0 containment false-feasible, 0 incremental mismatch.
- Throughput mantiene tutte le soglie originali.
- S1 pubblica dual-valid entro 150.16547 in 200k proposal, repair ≤16 µm.
- Triangle-20 pubblica dual-valid entro 70.742 in 200k proposal, repair ≤16 µm.
- C175: almeno 1/3 entro 240k, entry dentro T, strict child dual-valid.
- Il target non viene mai ampliato.

Se S1 fallisce ancora dopo il jump locale corretto, il verdetto è STOP per il **member mono-pezzo/strict-decrease**. Una chain move sarebbe un nuovo solver round, non una correzione retroattiva. Se C175 o triangle falliscono dopo queste correzioni, diventano finalmente evidenza di paradigma.

## D. Altri difetti latenti

- Il guard “unchanged state” confronta solo la profondità, non lo stato: una posa diversa alla stessa profondità viene saltata ([mod.rs:257](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/mod.rs:257)). Deve usare digest dei bit delle pose.
- Il repair calcola slack da `state.geometry`, che non incorpora le correzioni precedenti ([publish.rs:550](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/publish.rs:550)).
- Φ usa `sag`, il kernel usa `safety`; il dominio ammette `safety > sag` ([general_fast.rs:2406](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:2406)). Sui due fixture coincidono, ma su richieste arbitrarie può creare una tassa sistematica superiore ai 4 µm.
- La specifica dice libm, ma il pose transform usa `std::sin_cos` ([state.rs:54](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/state.rs:54)). È un difetto di claim deterministico, non la causa di Gate 0.

Infine, il 82.71% non è né benigno né un kill pulito. Il corpus conta la correlazione solo quando `propose()` accetta già la mossa ([corpus.rs:616](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/corpus.rs:616)); quindi misura precisione condizionale, non capacità di uscire dai fixed point. Il grazing miss è in parte L2-vs-L1; il 20% containment è una debolezza reale per jump che possono creare containment. Inoltre l’oracolo “indipendente” condivide proprio `boundary_residuals`, quindi non poteva scoprire il bug del top. Il 100% sulla popolazione compressed resta un pass onesto della clausola originaria, ma non salva il solver.
