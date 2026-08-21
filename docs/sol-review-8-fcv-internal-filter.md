# Sol review 8 — FCV as internal filter, rotation policy nulls, calibration conditions

> Verbatim final reply from the resumed codex session (max effort, read-only) at HEAD a131a72, 2026-08-21.

Verdetto netto:

- **FCV:** sì come filtro interno opt-in di v3, **no** come autorità globale di pubblicazione. La geometria del broad phase mi convince; la prova numerica dichiarata chiusa no.
- **Sparse rotation:** resta off. La tassa è sostanzialmente sparita, ma trigger, attribuzione e composizione non misurano ancora la cosa che dichiarano.
- **Work-budget calibrato:** direzione giusta, purché non usi un probe wall live per decidere il risultato e mode 34 diventi interrompibile.

## 1. FCV — findings

### P0 — `orient2d` non è provato esatto sul dominio dichiarato

La guardia evita l’overflow, non l’underflow. La prova assume che ogni operazione finita abbia errore relativo ≤u e che `robust::orient2d` conservi sempre il segno ([general_polygon.rs:967](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:967), [general_polygon.rs:986](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:986)). Ma `robust 1.2.0` dice esplicitamente che l’adaptive path **non garantisce correttezza in underflow** ([robust/lib.rs:98](/var/lib/t3/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/robust-1.2.0/src/lib.rs:98)).

Manca un lower bound su edge/determinanti trasformati. Quindi “385 binadi dall’overflow” non chiude il predicato. È load-bearing perché FCV deve riprodurre il verdetto floating della reference, non soltanto la geometria reale.

Correzione richiesta prima di chiamare la prova chiusa:

- rescaling esatto power-of-two delle differenze prima di `orient2d`, oppure
- fallback dyadic/exact quando i prodotti entrano nella fascia subnormal.

### P0 — il bound `16.5·C·u` non è ancora una derivazione

Il passo “nessun overflow ⇒ ogni operazione ha errore relativo ≤u” è falso per risultati subnormal ([general_polygon.rs:969](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:969)). Inoltre:

- `dx = fl(E-S)` contribuisce prima di `fl(p*dx)`; il `2.1·C·u` non deriva dalle righe scritte.
- Il bound `2u` di `hypot` non è un contratto portabile qui: nel repo è verificato empiricamente su 200k input, non provato ([general_relaxed.rs:2731](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:2731)).
- Il test controlla che `32 ≥ 16.5`, cioè ricopia la conclusione; non verifica il calcolo ([general_polygon.rs:1935](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:1935)).

Lo shipped `1e-12` offre enorme slack pratico e non vedo un controesempio operativo. Ma `max(32u)` non è ancora una garanzia strutturale. Farei un bound conservativo completo, incluso termine assoluto per underflow; anche 1024u resterebbe sotto lo shipped e non cambierebbe il hot path.

### P1 — il ceiling da `interior_sample` ha un passaggio non valido

È corretto che:

- la sorgente sia bounded;
- la traslazione sia solo finite-checked ([general_polygon.rs:328](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:328));
- `validate_sheet` non possa essere usato ([general_polygon.rs:421](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:421));
- ogni regione debba produrre un interior sample ([general_polygon.rs:408](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:408)).

Non segue però che la distanza fra due coordinate **già arrotondate dopo la trasformazione** sia bounded dal diametro reale della regione, come usato in [README:504](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/fast-contract-validator/README.md:504). Due valori reali vicinissimi possono cadere sui lati opposti di una soglia di rounding.

Credo che il lemma sia riparabile usando la forma `fl(translation + q)`, con `translation` rappresentabile e `q` bounded dalla sorgente trasformata: se due output sono distinti, l’ulp della traslazione non può essere arbitrariamente maggiore del range di `q`. Ma questa è una prova diversa e deve includere entrambi gli assi e le intersezioni dello scan. Il test attuale ricalcola soltanto la formula contestata ([general_polygon.rs:1955](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:1955)).

Importante: la guardia `2^112` resta fail-closed ([general_polygon.rs:857](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:857)). Quindi questo buco minaccia il claim “irraggiungibile”, non introduce da solo uno skip insicuro.

### Parti che reggono

Non trovo un buco in:

- outward rounding delle slab e soglia rialzata;
- contenimento: intervalli annidati non possono produrre gap positivo ([general_polygon.rs:1091](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:1091));
- `NaN`/coordinate fuori dominio: danno `None` e exact fallback;
- esclusione del caso (b): il validator consuma solo il threshold verdict; `measure_approach` è un’implementazione separata ([FCV README:73](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/fast-contract-validator/README.md:73));
- witness a ±1.3e308: è un buon regression test del vecchio falso lemma e del fail-closed ([general_polygon.rs:1988](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:1988)). Non è una prova di reachability perché fabbrica direttamente set degeneri interni.

Il corpus release è forte evidenza empirica: copre davvero certificati, rifiuti e near-threshold ([README:650](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/fast-contract-validator/README.md:650)). Gli mancano però proprio i due domini contestati: traslazioni vicine al ceiling e determinant/subnormal adversariali.

### Promozione

§13 **non basta per un default globale**, e onestamente lo ammette già: spec key, secondo box e seed set sono ancora aperti ([README:911](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/fast-contract-validator/README.md:911)).

Inoltre oggi la feature cambia `validate_publication` per tutti i caller ([general_polygon.rs:77](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/validation/general_polygon.rs:77)); anche `try_publish` finisce nel validator filtrato ([portfolio.rs:2118](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:2118)). Per “default dentro v3” serve:

1. modalità runtime esplicita, non solo feature globale;
2. FCV ammesso sulle conferme interne m34;
3. `validate_publication_exact_reference` chiamato **per nome** sull’adozione/final output;
4. v3-off e legacy sempre exact-reference.

Con questa separazione approvo FCV come acceleratore interno anche prima della chiusura formale: un errore può cambiare la ricerca, non pubblicare geometria invalida.

`pconfirm` va trattato separatamente. Il miglior factorial ha wall mediano **10.508 s**, non 10 ([factorial-10s.json:967](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/fast-contract-validator/evidence/factorial-10s.json:967)). Quindi 168.756 non è ancora un punto rigoroso dell’envelope ≤10 s.

## 2. Sparse rotation

### P0 — il disarm adattivo legge il contatore sbagliato

`rotation_accepted_moves` conta qualunque accepted move che cambi rotazione/mirror, non una proposta sparse ([general_relaxed.rs:13172](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:13172)). Il portfolio lo usa invece come attribuzione dell’operatore ([portfolio.rs:2329](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:2329)).

La prova materiale è nel controllo: zero rungs ma **11.523 `rotationAcceptedMoves`** ([armgate.json:24044](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/sparse-rotation/evidence/armgate.json:24044)). Quindi “disarm mai necessario” non è interpretabile.

Serve un contatore operator-specific: sparse proposal → winner → committed move, associato all’episode ID.

### P1 — il trigger B non implementa il testo

Il documento dice “disarma se lo sweep abbassa la loss ricevuta”. Il codice confronta invece contro il **minimo storico**, perché esegue `stall_loss = min(stall_loss, now)` ([general_relaxed.rs:6750](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:6750)). Sequenza `10→8→9→8.5`: l’ultimo sweep migliora, ma resta armed.

Inoltre:

- l’arming avviene dopo lo sweep; un episodio aperto sull’ultimo sweep non verrà mai esercitato;
- 10.370 episode contro 8.264 armed sweep lo rende quantitativamente visibile ([README:188](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/sparse-rotation/README.md:188));
- vengono armate solo collision pairs, quindi uno stall boundary-only non entra.

Il 5.22× è densità di **local score improvements**, non accepted/committed/publication yield ([general_relaxed.rs:14319](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:14319)). Lo chiamerei così.

### P1 — equivarianza reale, fidelity discreta non dimostrata

Offset miter e trasformazioni O(2) commutano in geometria esatta. Non commutano con lo snapping intermedio: Clipper può persino cambiare ramo miter/square vicino alle soglie ([offset.rs:879](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/clipper/offset.rs:879)).

Il test controlla AABB e area su una L, tre angoli e mirror ([general_relaxed.rs:21077](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:21077)). Non può escludere un corner locale mancante. Le “36 celle” sono 12 parent deterministici ripetuti tre volte; il 27/36 è effettivamente 9/12 indipendenti ([armgate.json:24256](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/sparse-rotation/evidence/armgate.json:24256)).

Non è un rischio di pubblicazione, perché il final exact gate resta. È un rischio di traiettoria. Prima di promuovere il lever costruirei un corpus con acute/reflex, angoli vicini alle due soglie Clipper, mirror, holes/multi-region e confronto locale support/Hausdorff o collision-verdict near-contact.

### Design C e verdetto null

C trova proposte exact-valid, ma aggiorna solo `published_depth_mm/placements`; non aggiorna `state`, `confirmed_state`, floor o archive ([general_relaxed.rs:7048](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:7048)). Quindi 0/12 finali prova che il one-shot publication viene poi dominato, non che `witness → m34` non componga.

Conclusione corretta:

- per B, la tassa residua non spiega più il null; il null è della policy/trigger corrente;
- il 30 s, 5/6 contro, è un segnale negativo serio per quella policy ([README:378](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/sparse-rotation/README.md:378));
- non è una confutazione della rotazione continua come DOF;
- C è soprattutto un null dello strumento d’integrazione.

Correggerei anche il testo obsoleto che dice che C “non ha mai accettato”: contraddice la tabella 6/7/16 e la ritrattazione successiva.

## 3. Work-budget in volo

Sì, è la mossa giusta con quattro condizioni:

1. Il probe hardware dev’essere offline/persistito e il cap parte della spec. Un probe wall live renderebbe il risultato deterministico solo **condizionalmente al cap scelto**, non per seed.
2. Un probe non stima un p95. Servono distribuzioni ripetute sotto carico deployment e margine esplicito.
3. p95≤10 s non è un hard deadline: serve comunque stop wall fra checkpoint e ritorno dell’ultimo incumbent exact-valid.
4. Non spedirei il meter attuale: il work mode paga circa 17% di profiling e conta solo candidate queries + exact pairs ([portfolio.rs:1888](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:1888)). Serve un debit lane-local economico. Inoltre mode 34 oggi è atomico e senza work cap interno ([portfolio.rs:4276](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:4276)).

## 4. Prossime tre spese

1. **M — Mode 34 resumibile + prior a tre livelli.**

   Spezzare ogni slice in batch deterministici che terminano a un deepest-confirmed checkpoint e conservano frontier/cache. Scheduler su:

   - prior geometrico request-level;
   - costo/yield osservato sulla request;
   - posterior parent×depth-band.

   Obiettivo: massimizzare exact raw-mm per wall previsto, con credito differito per non affamare breaker costosi. È l’unico serbatoio da 5 mm già osservato: da 40M a 120M v4 guadagna 5.964 mm mediani per seed, mentre schedule pubblica 17/19 a 0.581 mm/M ([coordinator-v4 README:405](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/coordinator-v4/README.md:405), [README:422](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/coordinator-v4/README.md:422)).

   Rischio: batching o ricostruzione cache cambia la traiettoria. Gate: N batch concatenati devono riprodurre il monolite a pari lavoro.

2. **M/L — Chiudere il residue row-ownership e abolire il rescore di sweep.**

   La causa è ormai nota: operand order del proxy; `canonical-pair-order` elimina la classe strutturale, ma il dynamic-pole f32 non può ancora esprimere la query index-ordered ([next-generation-engine-plan.md:2161](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:2161), [next-generation-engine-plan.md:3088](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:3088)). Ridisegnare quell’interfaccia, ottenere zero structural+magnitude disagreement, poi ereditare il tracker.

   Rischio: cambia le decisioni marginali, quindi è una nuova production profile, non refactor legacy. Prima farei un profilo HEAD: se rescore+meter non valgono almeno ~15% del leaf corrente, taglio.

3. **M — Multi-basin race con successive halving, non m20 feeder.**

   Il best arm FCV ha spread 165.656–174.280 ([factorial-10s.json:967](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/fast-contract-validator/evidence/factorial-10s.json:967)): il rischio dominante è ancora entrare nel basin sbagliato. Avviare 2–3 salts a cap breve, valutare non solo depth ma primo-batch m34 yield, binding-front stability e proxy infeasibility; continuare top-1/top-2.

   Rischio: l’early leader può essere il late loser e dividere 10 s può affamare tutti. Gate equal-work su molti seed/request. Non riaprirei il ticket m20→crossover già confutato né il crossover profondo, che a 120M pubblicava 0/12.

Fuori top-3 farei solo un A/B economico: **accepted witness → child frontier → un batch m34** contro parent a pari lavoro. Se resta 0/12 descendant publications, taglio witness/m33 dalla produzione. Non spenderei su più iterazioni SE(2), altra micro-ottimizzazione FCV o tuning ulteriore della sparse rotation prima di questi tre punti.