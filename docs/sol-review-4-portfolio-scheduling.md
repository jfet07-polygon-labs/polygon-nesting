# Sol review 4 — portfolio scheduling and the 165-in-10 envelope

> Verbatim final reply from the resumed codex session (gpt-5.6-sol, max effort, read-only) at merged HEAD b522373, 2026-08-18.
> Package sent: /var/lib/t3/tmp/sol-round4-prompt.md (anytime curve 3/10/30s, inner-certificate 6.47x, coordinator v2 verdicts, five questions).

Ho verificato il worktree a `b522373`, discendente di `4c35855`; nessuna modifica.

Verdetto secco: per il prossimo esperimento invertirei `(a)` e `(b)`. Per l’architettura finale servono entrambi, ma non porterei m26 prima di aver dimostrato che produce frontiere riparabili nella fascia attuale 174–179. E non lo porterei come pressione continua dentro ogni mossa.

## 1. Spesa per rompere la saturazione

1. **Coda di azioni crossover + feeder di basin — M**

   È la prima spesa perché la saturazione a 11 s è soprattutto esaurimento dello schedule corrente, non un fixpoint dell’insieme degli operatori.

   Oggi:

   - si considerano tre stati e al massimo tre crossover;
   - con una frontiera invariata sono esattamente le tre coppie non ordinate;
   - esiste un solo taglio, `0.5`;
   - mode 23 è direzionale — sinistra da A, destra da B — ma la chiave considera solo una direzione;
   - il costruttore viene eseguito dopo il crossover, quindi un basin generato in `diversify` non può diventare suo genitore nello stesso run.

   Evidenza: [portfolio.rs:1396](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:1396), [portfolio.rs:1525](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:1525), [portfolio.rs:1692](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:1692), [general_relaxed.rs:5463](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:5463).

   Costruirei una coda deterministica di azioni `(operatore, genitori, parametro, quantum)`, che dopo ogni nuovo stato possa accodare:

   - m22 breve;
   - crossover diretto `A→B` e reciproco `B→A`;
   - tagli derivati dai gap/interfacce dove i genitori differiscono, non una griglia di costanti;
   - un ticket m20 a singolo restart quando non rimangono coppie complementari;
   - compressione se lo stato entra nella fascia dove il clamp produce un residuo utile.

   Il ticket m20 va valutato su credito differito: `m20 → m22 breve → crossover → m22`, non sulla pubblicazione immediata. L’attuale patience misura la cosa sbagliata per un feeder: la genealogia storica dimostra che il valore del costruttore può emergere parecchi discendenti dopo.

   Non aumenterei semplicemente `crossover_attempts`: il quarto tentativo è un no-op se i primi tre non cambiano la top-3.

2. **m26 resumable a un rung, con legalizer incorporato — L**

   Necessario per 165, ma solo dopo un census offline sui basin 174/176/179.

   Preserverei esattamente la caratteristica che ha reso m26 produttivo:

   - incumbent esatto-valid separato;
   - frontiera compressa potenzialmente infeasible;
   - hard clamp;
   - avanzamento della frontiera anche quando la pubblicazione fallisce;
   - global legalizer sul residuo.

   Sono già le due catene separate del mode 26 corrente: [general_relaxed.rs:4859](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:4859) e hard clamp a [general_relaxed.rs:4890](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:4890).

   “Continuo” deve significare resumable, un quantum/rung per evento. Non metterei il clamp dentro ogni mossa ordinaria: consumerebbe diversità, trasformerebbe tutto il search in una sola frontiera infeasible e perderebbe proprio l’asimmetria incumbent/frontier.

   Inoltre la fase oggi chiamata `compression` non testa questo meccanismo: esegue m22 per un ciclo con target `depth + 0.8`; se è valido, m31 viene saltato. [portfolio.rs:1449](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:1449).

   Gate prima del port L: su tutti gli stati dell’archivio saturo dei tre seed, un solo rung legacy deve produrre almeno frontiere riparabili o pubblicazioni con efficienza migliore del prossimo crossover. Se nella fascia 174–179 genera soltanto residui enormi, il port non chiude 165.

3. **m33 triggered tail — S/M**

   Solo dopo un residuo strutturale preciso:

   - bound già soddisfatto;
   - legalizer traslazionale fermo;
   - uno o pochi componenti;
   - componente contenente un pezzo depth-setting;
   - deficit compatibile con una rotazione locale.

   Budget massimo circa il 5% del residuo post-m0, un solo tentativo fine-first. Niente sweep m33 generalizzato.

   Correzione importante: ±0.02° ha rotto il primo fixpoint, ma non è il rung produttivo del record corrente. Le adozioni finali sono a `0.0032°` e `0.008°`, con zero accettazioni a `0.02°` o più grossolane. [next-generation-engine-plan.md:1748](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:1748), [next-generation-engine-plan.md:1791](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:1791).

   Per DXF arbitrari quei rung vanno derivati dal raggio del pezzo e dalla griglia di posa, non spediti come angoli assoluti.

4. **Terzo/quarto crossover cieco — tagliato**

   Il terzo già esiste. Il quarto ha senso solo dopo aver ampliato lo spazio delle azioni: direzione, taglio e genitori. Spendere altri 2–3 secondi sul prossimo pair depth-ranked a metà foglio non è una strategia.

## 2. Un envelope 165-in-10 credibile

La decomposizione più onesta è questa; sono fasce storiche, non guadagni additivi già dimostrati sul parent 174.208:

| Meccanismo | Millimetri da produrre | Evidenza |
|---|---:|---|
| Basin/topologia/crossover | circa 6.4 mm, da 174.208 alla fascia 167.849 | il crossover è l’unico meccanismo che ha attraversato il plateau |
| Clamp + repair | circa 2.5 mm, 167.849 → 165.368 | compress-repair storico |
| Coda finale | circa 0.37 mm | non attribuibile credibilmente a m33 |

Il dato favorevole a m26 è 167.846 → 166.855 al primo ciclo e poi 165.368 con la cascata. [next-generation-engine-plan.md:1046](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:1046). Il dato avverso è che il tier globale guadagna 4.892 mm sulla linea record, ma soltanto 0.836 mm sulla linea from-scratch profonda. [next-generation-engine-plan.md:1290](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:1290), [next-generation-engine-plan.md:1310](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:1310). Non assegnerei quindi “9 mm a m26”.

Economia richiesta:

- 10 s valgono circa 37.8M candidate evaluation.
- Tolti circa 2–2.3 s di fase protetta, restano circa 29–30M.
- Servono almeno 4–6 transizioni topologiche e 2–3 quanta clamp/repair: grossolanamente 3–5M evaluation per evento produttivo.
- Crossover/legalizzazione deve scendere sotto circa 1 s.
- Un rung m26 deve stare a 0.5–0.8 s, non 12–95 s: il gap residuo è ancora 15–190×, secondo il rung.
- Il ticket m20 a un restart dovrebbe stare intorno a 0.5 s se scala col full sweep, ma va misurato: non assumerei linearità.

Il collo nascosto è il legalizer di mode 23: richiama il percorso protetto 8-lane, 40-sweep, 10/10 sample, 5 refinement. [general_relaxed.rs:5697](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:5697). Generare più genitori senza proxy-ranking dei crossover e senza accorciare la seam legalization crea soltanto una coda di chiamate da 2–3 s.

Schedule-obiettivo post-m0:

- 50–55%: azioni topologiche, incluso al massimo un ticket m20;
- 30–35%: m26 resumable + global legalization;
- ≤5%: m33 strutturalmente triggered;
- resto: exact publication, drain e margine deadline.

m28/m29/m32 e m30/m31 standalone restano strumenti record. m31 appartiene internamente al clamp/repair, non come fase autonoma.

## 3. Residuo lane relaxed del m20

Il prossimo 2× è soprattutto meno lavoro per basin utile, non altri nanosecondi per evaluation.

Attenzione ai numeri: `moveSweep` 5.22 s e `scorePlacement` 4.75 s non sono due bucket wall disgiunti. Il totale leaf è 7.57 s aggregato mentre la wall è 4.06 s: ci sono nesting e aggregazione fra worker. Non si può applicare Amdahl sommando 9.97 s. [next-generation-engine-plan.md:3867](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:3867).

Misurerei per restart/rank/worker:

- candidate generate → proxy-pass → score → finalist → child retained;
- evaluation per figlio retained;
- max wall del worker critico, non somma CPU;
- duplicati di posa prima dello score;
- cache misses, branch misses e istruzioni per `scorePlacement`;
- valore differito del basin dopo crossover.

Le ottimizzazioni residue certificate — containment per 66,919 righe e inner convex cover — sono legittime, ma non valgono un 2× end-to-end. [next-generation-engine-plan.md:3933](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:3933).

Non spenderei ora su `pairPressure`, `hypot` o un’altra scorciatoia aritmetica. A 265 ns/eval il substrate m22 è già pari; la metrica che manca è millimetri per milione di evaluation e figli utili per sweep, non eval/s.

## 4. Archivio ed eviction

A 11/16 e zero refusal, l’eviction vale oggi zero millimetri. Prima dell’eviction conta la selezione: il default usa un solo stato per m22 e tre per crossover, ordinati prima per profondità; molti degli undici membri non ricevono mai un’azione. [portfolio.rs:396](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:396).

L’eviction diventa lavoro reale solo quando coesistono:

- capacità piena;
- `RefusedArchiveFullAllDistinct > 0` oppure eviction osservate;
- candidati con azioni non tentate che vengono esclusi.

Prima farei un A/B 16/32/64 a work budget identico.

La policy attuale è fragile:

- la similarità è uguaglianza bit-esatta delle pose; una piccola relax di tutto il layout rende due topologie equivalenti “diverse”;
- viceversa, due parent crossover complementari possono condividere oltre metà delle pose;
- una qualsiasi coppia già dominata/simile può fornire la vittima che permette l’ingresso di un candidato non collegato e peggiore.

Evidenza: [portfolio.rs:315](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:315), [portfolio.rs:332](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:332), [portfolio.rs:484](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:484).

Userei nicchie basate su firma topologica scale-free: ordine relativo dei pezzi, orientazioni/mirror e grafo di contatto/adiacenza. A capacità piena:

- incumbent mai evictable;
- ultimo rappresentante di una nicchia mai evictable;
- stato con azioni produttive non ancora tentate protetto;
- sostituzione dentro la stessa nicchia solo per miglior Pareto `(best descendant, raw depth, costo)`;
- tie deterministico sul fingerprint.

## 5. Cose che state sovrastimando

- **“Joint fixpoint a 11 s” è troppo forte.** È un fixpoint della coda finita top-3/midpoint/una-direzione, non dello spazio degli operatori.
- **Le 9 ripetizioni non sono nove traiettorie di qualità.** Sono tre seed con repliche di rumore wall. Il fatto che seed 0 faccia 174.208 una volta su tre dimostra anche che wall-budget e risultato deterministico per seed non sono contemporaneamente soddisfatti. Per determinismo stretto serve un work plan fisso calibrato per stare sotto il p95/p99 wall.
- **“Substrate parity” vale per m22.** Non prezza automaticamente mode 20, mode 23 o i rung Clipper-heavy; il work budget stesso omette parte dei contatori deep. [next-generation-engine-plan.md:3645](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:3645).
- **Il composito esatto non viene eseguito solo in pubblicazione.** Ogni layout completo viene validato entrando nell’archivio; un candidato adottato viene poi validato nell’adoption e nuovamente per `dual_gate_valid`. [portfolio.rs:1034](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:1034), [portfolio.rs:987](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:987). Caching per fingerprint e validità tri-state eliminerebbero duplicati lasciando intatta l’autorità del gate di pubblicazione.
- **“Da DXF in 10 s” non è ancora la misura fatta.** Il timer del portfolio parte quando riceve già `pieces`; parsing, offset e preprocessing stanno fuori. [portfolio.rs:1205](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:1205). Va dichiarato solver-only oppure misurato end-to-end.
- Tre fixture non sostengono ancora “arbitrary DXF”; su richieste diverse 165 non è neppure una metrica confrontabile. Serve anche un envelope normalizzato rispetto al lower bound/baseline.

La prima misura della prossima sessione dovrebbe essere un **opportunity-and-delayed-credit ledger** sullo stato saturo:

1. tutte le azioni crossover dirette e i tagli ancora non tentati;
2. stati esclusi da top-K e similarità;
3. causa esatta di uscita: fixpoint geometrico, key esaurite, patience, affordability o deadline;
4. credito genealogico fino a un discendente/crossover successivo;
5. `Δraw / M evaluation`, p50 e p95 di costo per azione.

Poi un A/B/C a work identico sui tre archivi saturi:

- A: prossima azione crossover derivata;
- B: un ticket m20 → crossover diretto → m22;
- C: un rung m26 → global legalizer.

Quello decide con dati se il prossimo investimento L è davvero m26. Oggi il candidato più promettente per rompere gli 11 secondi è B; il componente indispensabile per chiudere 165 resta C.
