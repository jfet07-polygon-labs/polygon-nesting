# Grok review 4 — race diagnosis audited, surgery judgments, the board corrected

> Verbatim reply from the local Grok CLI (xhigh, read-only) at HEAD 8e7f82e, 2026-08-22.


**Diagnosi: completa sul meccanismo, un filo larga sul yield.**

- Landslide strutturale, non sfortuna: `stability` è 1.000 su 45/45 bracci (`docs/experiments/basin-race/README.md:393-397`). Causa nel codice: tentativo 0 ⇒ 1.0 “neutro”, e su questo carico tentativo>0 accetta sempre (`portfolio.rs:5400-5404`). Zero varianza. Lo dicono (`README.md:747-751`).
- `infeasibility` misura “già riparato”: incumbent pubblicato exact-valid vs draw fresco (`README.md:398-407`; letto all’*entry* del batch, `portfolio.rs:5405-5406`). Incumbent strettamente sotto ogni challenger nelle 15 celle con avversario. Criterio del tipo giusto, confronto sbagliato (un braccio non è peer).
- 0/21 mosse (18@10s + 3@30s keep-2) e quindi **tutto il delta è costo**. Gate equal-work fallito: mixed-61 +2.366 / +2.934 mm sulle due celle equal-work (`README.md:339-341, 472-474`). 30s: una sola riga equal-work, +1.879 (`README.md:498-513`). Caveat onesto sulla sottigliezza.
- 71.500× è la causa nominata e regge: m20 92.7 u/s vs m34 6.628.431, 70.8% wall / 0.0123% work (`README.md:443-460`; affordability quasi inerte, `portfolio.rs:5079-5083`). Stesso meter di `work_units_now` (query + 5× exact pairs, `portfolio.rs:2782-2787`). Un share ceiling in quella valuta **non può** boundare i draw.
- Controllo archive: race senza challenger costa 0 (`README.md:796-800`). Fase ≠ tassa; i challenger sì.
- Leak triangle-20 nominato: `run_operator` pubblica il draw eliminato, 70.7273 a 17 cifre, 15.5–15.8M work per 0.014–0.044 mm (`README.md:481-487`). Non è la race che vince.

**Yield: overclaim lieve.** «yield also favours the incumbent in most cells» (`README.md:408-410`) è falso sulle celle che portano il verdetto: mixed-61 s1 slot2 0.7157 > 0.635; s2 slot2 1.1193 > 0.357 (`README.md:380-385`). Vince comunque l’infeasibility. Il rank-sum con tie-break verso slot 0 (`portfolio.rs:4927-4965, 4981-4987`) è corretto *dato* il set di bracci; il set è il bug.

**71.500× da una fase di una cella** (mixed-61 s0, `README.md:427-429`). Il rapporto è strutturale (classi diverse, stesso meter) e sopravvive a uno slowdown uniforme; non è ripetuto su shapes-17/triangle-20. Non invalida la tesi; non è una distribuzione.

**Fix: giusti, tutti e tre.**

- **B / trigger:** `StallDetector` confronta la loss *consegnata*, uguaglianza = stall, NaN fail-armed (`general_relaxed.rs:4334-4338`). Un solo tipo, serial + fan-out (`7216-7239`, `7343-7357`). Test che asserisce il *disaccordo* con la regola vecchia (`22202-22234`). Dopo un NaN, `handed_loss` resta NaN ⇒ ogni sweep successivo arma (`!(x < NaN)`). Fail-closed accettabile; va detto.
- **Disarm:** catena proposal → winner-che-muove → commit della stessa pose (`README.md:150-176`); 68.48% conversion, vecchio contatore 1.602×. Control: 0 rungs, 3841 `rotationAcceptedMoves` (`README.md:129-137`). `pose_owner` non viene pulito da NFP/second pass (`general_relaxed.rs` commento sul campo). Test di produzione, non copia (`22115`; `portfolio.rs:8629`).
- **C:** `publish == off` 12/12 to the digit, 7 witness / 0.173 mm mai sopravvissuti (`README.md:561-569`). Wire: `confirmed_state` + `state`, floor intatto (`general_relaxed.rs:7545-7558`). Mapping refuse-not-guess (`19251-19258`, test `22269`). `adopt` spec-keyed, default off.

**2/12 giustifica tenere witness/m33? Sì sulla regola di Sol, no sullo shipping.**

Sol: «se resta 0/12 descendant publications, taglio» (`sol-review-8-fcv-internal-filter.md:154`). 2/12 non è 0/12; la regola non scatta. Non spedire `adopt`: 2W/3L/7T, mediana 0.000, media +0.0453; stall 807→2308 e 254→2468 (`README.md:577-613`). Il 2/12 confuta lo *strumento*, non paga l’adozione.

Limiti da non dimenticare: 12 parent, mixed-61, un raggio; composizione *in-slice* non A/B esterno (`README.md:540-550`); `score_state` dell’adopt non entra nel cap (`README.md:551-559`); test di mapping fuori dalle suite nominate (`README.md:705-712`). Tenere il certificato e la chiave; non promuovere.

---

## 2. Replan / m34 resumibile (`b3308ee`)

**FNV basta come strumento del gate di concatenazione? Sì per il walk; no da solo per i contatori innestati.**

Due strumenti, giustificati: il documento aggregato non vede una divergenza che riconverge (`replan/README.md:117-135`; `compression_schedule.rs:1326-1346`). FNV-1a su ogni riga: step, clamp, sweeps, query, coppie/boundary prima/dopo, tre esiti conferma, raw depth; `to_bits`; discriminante `None` vs `Some(0)` (`compression_schedule.rs:995-1026`, test `1544-1575`). 21 celle, 1741 confini, entrambi gli strumenti (`README.md:575-580`; `concat-400k.json:5-6` `extraB=m34batch=400000`).

Cosa l’FNV **non** hash-a: `se2_witness_*`, sparse counters, rng, pesi, `StallDetector`. Gli effetti di traiettoria finiscono nelle righe successive (coppie/sweep/query). I bug *solo-contatore* (adoptions resettate per batch) li prende il documento: `equiv.py` **non** droppa `se2WitnessAdoptions` (`equiv.py:38-49`; vs `planbattery.py:44-55` che droppa `scheduleSlice` intero). `se2WitnessMs` è volatile (`VOLATILE_SUFFIXES = ('Seconds','Ms')`, `planbattery.py:55`) ⇒ il giudizio «ms non accumulati su errore» **non è misurato dal gate**.

Buco di riga: `run_se2_witness` è **dopo** aver riempito `collision_pairs_after` e **prima** del `rows.push` (`general_relaxed.rs:7123-7209`). Un’adozione all’ultimo step del slice non lascia traccia FNV; la prende fingerprint/`se2WitnessAdoptions` del documento, e solo se l’operatore è armato.

**Quiet-box: non invalida l’architettura work-plan; invalida il canone di shipping.**

`calibrated-plan` §8.2: «one plan, one depth, one document per seed», 60/60, p95 8.282s (`calibrated-plan/README.md:429-439`). Stesso `plan=10000` sotto carico: 2/3/1 depth per seed, moda 85–100% (`replan/README.md:649-657`; `next-generation-engine-plan.md:6907-6913`). Non è una ritrattazione del *meccanismo* (probe → ladder → work cap). È la proprietà «un secondo processo ottiene lo stesso numero» che è condizionale al box, e quel round non l’ha testata (`replan/README.md:983-989`). Replan peggiora l’asse (4/2/3): due letture di clock (`README.md:659-662`). Non è il fix della robustezza al carico.

**Cosa serve per un piano load-robust**

- Non un secondo clock (replan). Il secondo clock *aggiunge* uno straddle.
- Probe **min-of-k** (rung conservativo, più bias, più accordo) **oppure** rate da contatori deterministici / probe offline persistito (Sol 8 §3.1–3.2, `sol-review-8-fcv-internal-filter.md:123-125`). Target: 1 depth/seed **anche sotto carico**, non solo moda 85%.
- Il 30s overrun (41.15→37.14, `replan/README.md:780-792`) appartiene ai modi work-denominati; `m34cap` è stop in work (p50 32.64→25.91, 3.089 mm su un seed, `README.md:805-821`). Stop *wall* tra checkpoint = Sol §3.3, ancora non costruito (`README.md:1016-1018`).

Altro che tiene: 2.808 mm = floor cost di `calibrated-plan` §9 a tre decimali (`README.md:462-466`); `PLAN_FIRST_TRANCHE=1.0` perché 0.6 sposta l’overrun nella mediana (`README.md:436-482`); replan a 3s peggiore (triangle-20 4.96 vs 2.26, `README.md:749-755`).

---

## 3. Chirurgia di merge (`8e7f82e`)

Cinque giudizi (commit message + codice). Quattro giusti; il quinto è giusto e morto per i report.

| # | Giudizio | Verdetto |
|---|---|---|
| 1 | `StallDetector` per-step local, serial + fan-out | Giusto. Aperto in `repair_serial`/`repair_fanned_out` (`7216-7239`, `7343-7357`). Non attraversa né step né batch (`6936-6939`). Se fosse un campo, il primo sweep del batch successivo armierebbe con una loss pre-rollback. |
| 2 | `se2_witness_adoptions` carried (episodio a *floor*) | Giusto (`6997-7003`, `6856`). Compagno load-bearing non elencato: `se2_witness_last_floor` (`7004-7005`, `7505-7513`). Senza di esso il secondo batch richiama il certificato. |
| 3 | Child frontier = campi di `ScheduleSliceRun` | Giusto (`7567-7571`: `confirmed_state`/`state`/`score` + surrogates/pesi). Adozione all’ultimo step di un batch = frontier del successivo. |
| 4 | `run_se2_witness` fallibile; ms non accumulati su errore | Fallibilità giusta (`7491-7493`, `7581-7583` `?` su `ensure_state_surrogates`/`score_state`). Ms: `+=` è *dopo* i `?` (`7592`) — stesso ordine del parent race (`920867b` locals). Su errore lo slice abortisce: il report non esiste. Giudizio morto per i gate (`se2WitnessMs` comunque stripped). Half-mutation: state già child, pesi non puliti, score vecchio, poi `Err`. |
| 5 | Race prima della coda; `tranches` argomento di `run_v3_schedule`; nessuno legge l’altro | Strutturalmente vero (`portfolio.rs:3606-3623`, `5432-5448`). |

**Gate prescritti non raggiungono l’innesto.** Concat committed: `extraB=m34batch=…` only (`concat-400k.json:5-6`; analoghi 25k/100k/120M). Unit test `concatenated_batches_reproduce_the_monolithic_slice`: `sparse_rotation` default false, `se2_witness` None (`general_relaxed.rs:21345-21357`, `652-654`). 9/9 unarmed. `se2WitnessAdoptions` sempre 0 ⇒ carried vs locale è invisibile.

**Gate extra: dichiarati, non in ledger.** Nessun `concat-*se2*`/`sparserot`, nessun test che armi concat+adopt. `witnessab` ha 7 adozioni su 5 parent, max 2 per parent (`witnessab-12parents.json`; somma 7) — «4 adozioni reali» non compare in evidenza committata. I 5 giudizi stanno nel messaggio di merge, non in `next-generation-engine-plan.md` (il merge aggiunge solo il capitolo replan).

**Siti non coperti dai gate extra (anche se esistessero solo-concat-armati):**

1. **Race ∧ replan** condividono meter e clock. Race 8–9s a 70.8% m20 prima della coda (`3611-3623`). `queue_rate = (work_now−probe_work)/(seconds_now−probe_seconds)` (`replan/README.md:174-175`) include quella fase ⇒ rate depresso ⇒ tranche sotto-compra. Nessuno legge l’altro; il coupling è il `BudgetMeter`. Default entrambi off. Concat non lo tocca.
2. **Rollback dopo adopt nello stesso step** (`7195-7207`): se `due_for_rollback`, lo stato torna al child *a floor*, non al clamp frontier. Intra-step, uguale al parent race; concat lo vede solo se cambia le righe dopo.
3. **Riga FNV pre-witness** (`7123-7136` vs `7195`): adozione last-step-of-slice senza riga successiva. Documento sì, FNV no.
4. **Fan-out StallDetector** è locale al worker (`7343-7357`), reseed rng da `(seed, step, worker)` (`7327-7335`). Corretto; un hoist futuro sul struct sarebbe un cambio di operatore.

---

## 4. Board in volo

Correzione di (a): **due problemi, non uno.** Sol §3.4 è «meter costoso ~17% / 1.882 mm» (`replan/README.md:1025-1028`; `sol-review-8:126`). La race ha mostrato che lo stesso meter è **cieco** a m20 (71.500×). Un debit cheap delle *stesse* unità non rende m20/m22/m34 confrontabili. Vincolo: non muovere i numeri work pinnati (`work_units_now`, `portfolio.rs:2782-2787`). Via: valuta parallela, oppure debit per-classe generalizzando `schedule_self_cost_units` (oggi solo m34, `portfolio.rs:2846-2850`, `6236-6240`).

Correzione di (b): il canone da proteggere è «1 depth/seed **sotto carico**», non il quiet-box 60/60. Replan non è il mezzo. min-of-k **o** rate da contatori/probe persistito.

Correzione di (c): 0.26/0.28 ms è grok-3 pconfirm (`grok-review-3-fcv-promotion.md:153`); FCV serial è **0.861 ms** (`fast-contract-validator/README.md:19`). Il costo di `step=0.25` è 4× sweep di repair (~540 ms), non la conferma. Gate: equal-work sullo stesso parent, depth-per-query nel *self-meter* m34 (stessa classe: il 71.500× non entra). Record-line: −1.000 mm a 20M da parent 159.668 (`record-line-cascade/README.md:54`).

**Ordine**

1. **(a) valuta per-classe / self-meter generalizzato** — causa nominata di due fallimenti (race equal-work; m20 sotto-prezzato, che è anche grok-3 #3 «parent costruttore»). Sblocca retry race, secondo bacino, parent m20. Prima, perché ogni spend cross-classe è illeggibile.
2. **(b) piano load-robust** — il canone di shipping è condizionale; senza questo ogni claim wall/plan è un numero di box. Parallelo ad (a) (non condivide il meter).
3. **(c) densità di conferma della prima slice** — unica leva 10s *stessa classe*. Può partire in parallelo ad (a). Non aspettare la valuta.

**Pezzo mancante: stop wall sui checkpoint + slice sospesa tra azioni.**

(a)(b)(c) non coprono l’interruptibilità. Sol §3.3 chiede stop wall tra checkpoint e incumbent exact-valid (`sol-review-8:125`). `m34cap` è work-stop (`replan/README.md:1016-1018`). L’overrun 30s «belongs to the work-denominated modes» (`README.md:796-798`). I batch sono *dentro* una call (`README.md:1020-1024`); senza portare uno slice sospeso alla coda, m34 non può cedere a un’altra classe. (c) senza questo: più conferme per µm, lo slice resta atomico verso il coordinatore e può ancora mangiare i 10s. Grok-3 #2 (secondo bacino in 10s) è morto come race finché mancano (a)+criteri peer; vive come *interleaving* se lo slice può fermarsi e restituire.

Non in board come spend: retry race (criteri da rifare: drop slot 0 *oppure* infeasibility tra peer, stability solo dove le conferme falliscono — e solo dopo (a)); `adopt` on; piegare `se2-rigidity-certificate` nel combo (igiene del test `22269`).