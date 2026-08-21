# Sol review 9 — the m34cap provenance break, race selection defects, the corrected board

> Verbatim final reply from the resumed codex session (max effort, read-only) at HEAD 8e7f82e, 2026-08-22.

Verdetto complessivo: il refactor `ScheduleSliceRun` può restare, default-off, ma non promuoverei né race, né witness adoption, né `m34cap/replan` sulla base di questo pacchetto. Ci sono due errori semantici e una rottura seria della provenance dell’evidenza.

## P0 — `m34cap` non interrompe il slice committato

- `m34cap` calcola solo `batch_work_units` prima della chiamata a m34 ([portfolio.rs:6092](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:6092)).
- Quando `advance()` incontra il limite, registra un checkpoint e lascia `finished=false` ([general_relaxed.rs:7059](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:7059)).
- Ma il chiamante lo richiama immediatamente in `while !slice.finished`, fino alla fine del monolite ([general_relaxed.rs:6861](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:6861)). Il coordinatore non riottiene mai il controllo al checkpoint.
- Anche il test richiede esplicitamente che l’ultimo checkpoint sia `finished` e coincida con la fine completa del slice ([general_relaxed.rs:21402](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:21402)): testa segmentazione, non interruzione.

Quindi il codice a HEAD può cambiare il report dei checkpoint, non wall, work, traiettoria o profondità. Un replay read-only sull’artefatto locale corrente lo conferma: `m34cap=0/1`, work 30M, seed 1 → stessa profondità `171.3619986855876`, stessi `28,636,653` work, stesse 8 azioni; cambia soltanto il checkpoint report.

L’evidenza committata invece attribuisce a `m34cap` il salto `162.846 → 165.935` ([cap-30s.json:44](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/replan/evidence/cap-30s.json:44), [cap-30s.json:102](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/replan/evidence/cap-30s.json:102)). Inoltre il driver committato non sa generare `capoff/capon`: interpreta ogni valore diverso da `off` come `planfirst=<value>` ([trancheq.py:44](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/replan/drivers/trancheq.py:44)).

Gli artefatti presenti corrispondono agli SHA documentati, quindi non vedo una sostituzione postuma del binario. Vedo però una rottura netta fra sorgente, driver e binario misurato: probabilmente build da worktree non committato o driver modificato durante la raccolta. La claim `m34cap` va ritirata e rifatta da clean HEAD.

## P0 — Basin race: la diagnosi del costo è vera, quella della selezione è incompleta

Il rapporto 71.500× dimostra che la valuta legacy è cieca a m20; non spiega da solo lo 0/21.

- Il ranker non assegna lo stesso rank ai pareggi. Ordina con il numero dell’arm e poi usa la posizione nell’array ([portfolio.rs:4927](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:4927)). Quindi una `stability=1.0` identica per tutti assegna comunque rank 0/1/2: il criterio “a varianza zero” vota artificialmente per slot 0.
- `confirmations_attempted==0` vale `stability=1.0` ([portfolio.rs:5397](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:5397)). Poiché “higher is better”, non è neutro: è il massimo possibile.
- Gli arm non sono isolati. Ogni audition archivia e prova subito a pubblicare ([portfolio.rs:3246](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:3246)); la documentazione stessa ammette che un loser triangle-20 pubblica e resta incumbent dopo l’eliminazione ([basin-race/README.md:481](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/basin-race/README.md:481)).
- Il winner non viene adottato come incumbent. Vengono soltanto rimossi alcuni challenger; slot 0 non viene mai ritirato ([portfolio.rs:5192](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:5192)). Questo è ancora un ticket per la queue, non una decisione.
- L’audition di slot 0 non è provatamente “l’azione che la queue avrebbe comunque eseguito”: la queue ricalcola tutte le classi, e il slice dell’audition non viene ripreso con cache/stato di schedule.
- Se scade il deadline nel mezzo del `for`, alcuni arm ricevono il round e altri no, ma vengono comunque giudicati insieme ([portfolio.rs:5148](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:5148)).

Conclusione: tenere il race off è giusto, ma “0/21 perché i criteri sono una landslide” non è dimostrato. Prima di riaprirlo servirebbero dense ranks, arm senza pubblicazione globale, uguale maturazione, e adozione esplicita del winner. Sotto il mandato 10 s lo taglierei dal board.

### I tre fix

- `StallDetector`: la regola “loss consegnata” è implementata correttamente ([general_relaxed.rs:4290](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:4290)). Nota: `common_loss` è raw, non weighted ([general_relaxed.rs:3388](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:3388)); la spiegazione che attribuisce direttamente l’oscillazione all’aggiornamento dei pesi è troppo forte.
- Attribuzione operator-specific: corretta nel seriale, non nel fan-out. Il report somma i contatori di tutti i worker, inclusi quelli scartati ([general_relaxed.rs:7698](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:7698)), e il disarm legge quel totale ([portfolio.rs:3275](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:3275)). Un proposal produttivo su un worker perdente può mantenere armato l’operatore sulla traiettoria vincente.
- Inoltre un episodio può essere aperto dall’ultimo sweep e non ricevere mai una proposta prima del disarm ([general_relaxed.rs:7282](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:7282)). `episodes>0 && committed==0` non prova quindi che il meccanismo abbia avuto un’opportunità. Il bit deve leggere almeno `proposals>0`, e solo sulla lane adottata.

## P0 — Il child frontier witness non conserva l’invariante “confirmed”

Il certificate valida soltanto il contratto sul foglio originale ([se2_certificate.rs:1794](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_micro_legalization/se2_certificate.rs:1794)). Non valida l’envelope di ricerca; la differenza è esplicitata qui: [general_fast.rs:3456](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_fast.rs:3456).

Nonostante ciò, l’adozione:

1. assegna subito il child a `confirmed_state`;
2. lo sposta sul clamp frontier;
3. soltanto dopo costruisce surrogate e tracker, con `?` fallibile.

Vedi [general_relaxed.rs:7572](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:7572). Il percorso normale scrive `confirmed_state` solo quando il frontier stesso ha passato il composite validator ([general_relaxed.rs:7181](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:7181)).

Conseguenze:

- rollback può ripristinare uno snapshot contract-valid ma proxy/envelope-infeasible;
- un errore di surrogate/score abortisce tutto l’operatore dopo aver trovato una pubblicazione valida;
- `se2_witness_ms` non viene accumulato sull’errore;
- `exact_valid=true` di mode 34 può descrivere un risultato che non ha passato il composite gate interno.

Il “2/12 descendant” non salva il meccanismo. Il driver definisce descendant semplicemente come `final(adopt) < final(publish)` ([witnessab.py:150](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/basin-race/drivers/witnessab.py:150)); non dimostra che una conferma successiva alla witness abbia pubblicato. Inoltre gli arm non hanno uguale lavoro effettivo: per seed 1 sono `10,150,405` contro `10,433,031` unità ([witnessab-12parents.json:132](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/basin-race/evidence/witnessab-12parents.json:132), [witnessab-12parents.json:190](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/basin-race/evidence/witnessab-12parents.json:190)).

Verdetto: 2/12 refuta soltanto “la traiettoria non cambia mai”; non giustifica witness/m33 in produzione. Terrei m33 come strumento record indipendente e witness come diagnostica off. Per riprovare: costruzione transazionale in temporanei, composite/proxy gate al floor, work addebitato, e contatore esplicito “post-adoption confirmation published”.

## P1 — Replan e digest

- FNV-1a è adeguato come checksum regressivo, non come certificato. Il payload contiene clamp, conteggi e loss aggregate ([compression_schedule.rs:995](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/compression_schedule.rs:995)); non contiene placement fingerprint, identità delle coppie, pesi, stato RNG, lane vincente o witness child. Due cammini differenti possono avere lo stesso payload senza alcuna collisione FNV. La frase “same digest walked the same walk” ([replan/README.md:126](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/replan/README.md:126)) è falsa.
- Aggiungerei, solo nei gate, fingerprint completi di state/tracker/RNG/worker scelto, oppure confronto diretto lock-step. Esistono già fingerprint geometrici e del tracker riutilizzabili vicino a [general_relaxed.rs:10082](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:10082).
- Il caveat quiet-box invalida il canone in forma assoluta. `install_plan` legge il wall e sceglie un rung ([portfolio.rs:2388](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:2388)); `replan` aggiunge una seconda lettura ([portfolio.rs:2500](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:2500)). Sotto carico, “una depth per seed” non è una proprietà dell’algoritmo.
- Nel merge c’è anche un’interazione non dichiarata: il piano viene installato, poi gira il race, poi la queue ([portfolio.rs:3570](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:3570)); il replan definisce `queue_seconds` come tutto il tempo dopo phase 0 ([portfolio.rs:2508](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:2508)). Con race armato include quindi i draw m20 quasi gratuiti in work e costosissimi in wall. “Neither reads the other” non è vero.

## Chirurgia di merge

- `se2_witness_adoptions` carried: corretto come contatore cumulativo. La motivazione “episodio aperto in un batch e risposto nel successivo” è inesatta: `run_se2_witness` è sincrono; basta dire che un report contiene più batch.
- `StallDetector` per-step local: corretto.
- `run_se2_witness` fallibile con timing perso: non corretto. L’adozione opzionale deve essere transazionale e un suo errore deve diventare `adoption_failed`, non fallire un slice che possiede già un incumbent esatto.
- I gate post-merge descritti dall’agente non sono presenti come artefatto identificato da HEAD `8e7f82e`. I file binari committati identificano separatamente `ship-meas` pre-merge ([binaries.txt:1](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/replan/evidence/binaries.txt:1)) e i binari race ([binaries.json:17](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/basin-race/evidence/binaries.json:17)). Il dichiarato 9/9 armato con quattro adozioni non è auditabile dal repository.

Non vedo codice estraneo o un segno tecnico di manomissione intenzionale. Vedo però una failure di provenance abbastanza seria da richiedere clean rebuild + evidence rigenerata prima di qualsiasi promozione.

## Board corretto, in ordine

1. **P0 — Rendere reale l’interruzione m34.** `advance_one_batch() -> Checkpoint|Finished`, controllo restituito al coordinatore, incumbent completo al checkpoint, possibilità di stop e successivo resume. Gate: fermarsi al checkpoint K, restituire exact-valid; riprendere e ottenere monolite bit-identico.

2. **P1 — Valuta scheduler V2, parallela e versionata.** Non muovere i numeri legacy. Conservare `legacyWorkUnits` e aggiungere `schedulerUnitsV2`. Per m20 contare lavoro Clipper deterministico — edge/vertex input, output, offset e conferme — non moltiplicare per 71.500 una misura singola. Per fan-out mantenere separati:
   - CPU work = somma lane;
   - critical-path estimate = seriale + massimo lane.
   
   Coefficienti calibrati offline e validati su richieste held-out; mai wall osservato come debit della singola run.

3. **P2 — Densità della prima slice.** È la spesa qualitativa più promettente, ma solo dopo V2. Oggi il self-meter addebita tutte le 1.830 coppie per conferma pur dichiarando che il narrow phase ne vede circa 99, sovrastima ~18× ([compression_schedule.rs:591](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/compression_schedule.rs:591)). Un gate “equal current work” penalizzerebbe artificialmente `step=0.25`. Misurare time-to-depth, mm/wall, floor moves/conferma e p95 slice, oltre a mm/query.

4. **P3 — Piano load-robust.** Non `min-of-k`: sceglie il probe fortunato/più veloce, sovrastima throughput e aumenta gli overrun. I contatori deterministici predicono costo geometrico, non carico CPU. Servono due contratti distinti:
   - fixed work da profilo hardware/request → stessa depth, wall variabile;
   - hard wall stop ai checkpoint → wall limitato, depth variabile.
   
   Per garantire entrambi servono core riservati/admission control. Nessuna formula può dare “una depth per seed sotto carico arbitrario” e contemporaneamente un hard 10 s.

Taglierei ora: basin race dalla produzione, witness/m33 dall’anytime 10 s, `replan` come meccanismo di qualità, e FNV come preteso certificato. Terrei default-off il refactor resumibile e gli strumenti record.