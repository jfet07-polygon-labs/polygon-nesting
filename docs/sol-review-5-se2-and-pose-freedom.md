# Sol review 5 — SE(2) rigidity, pose freedom, and production economics

> Verbatim final reply from the resumed codex session (gpt-5.6-sol, max effort, read-only) at HEAD 8cebcaa+orientation-floor, 2026-08-18.
> Package: /var/lib/t3/tmp/sol-round5-prompt.md (the A/B/C refutation, v3/v4, mode-34, record 155.422, five questions).

Sì: B è chiuso. La mia previsione era sbagliata; lo toglierei sia dalla linea record sia dal portfolio Mixed-61.

Il verdetto breve è: per superare 155 punterei su m33 guidato dai vincoli reali, non su altro grind; per arrivare verso 150 inizierei subito a rimuovere la barriera StructuredGrid/CurrentAssignment. Prima, però, c’è una correzione importante al pacchetto di certificazione.

## 0. La certificazione sovrastima la propria copertura

Il risultato `155.42229074464285` è reale, exact-valid, contract-valid e replay-stabile. Questo non cambia.

Non è invece supportata alla lettera la frase “fixpoint di 36 arm, m34 a quattro step”:

- `probeArms=36` include i 6 replay; gli arm dedicati alla ricerca del fixpoint sono 30. Il breakdown scritto nella tabella somma infatti a 30.
- I quattro spec m34 sono tre step distinti: `0.25`, `1`, `0.1`, più `0.25` a budget 60M. Non quattro step. Lo mostra direttamente [`SCHED_SPECS`](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/record-line-cascade/drivers/certify_full.py:36), mentre il README dichiara quattro step size [qui](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/record-line-cascade/README.md:233).
- I due spec `step=0.25` generano lo stesso tag/file: i raw da 20M sono stati sovrascritti dai 60M. Restano otto righe nel sommario, ma solo sei artefatti m34 completi.
- Nei sei raw superstiti, m34 entra sempre con `parentProxyFeasible=false`, 35 coppie collidenti e 9 boundary violations. Quindi misura soprattutto il recupero dal regrid, non un vero schedule locale attorno a 155.422. Un esempio è [questo raw](/var/lib/t3/tmp/recordline/cert-final-runs/cert-final-m34-step=0.1-s5.json).
- Piccolo errore separato: [`m26sweep-155.452.json`](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/record-line-cascade/evidence/m26sweep-155.452.json) contiene 16 righe, non 12. Il negativo è quindi semmai più forte.

Rinominerei la conclusione: “negativo finito sulla batteria dichiarata”, non “fixpoint certificato”. E conserverei integralmente i diagnostics m34 nel JSON finale.

## 1. Ordine degli strumenti per ≤155

| Rank | Strumento | Verdetto |
|---:|---|---|
| 0 | Certificato locale di rigidità SE(2) | Costruire prima; M |
| 1 | **(a) m33 orientazionale guidato dai witness** | Scommessa principale; M |
| 2 | **(c) perturbazione feasible → m34**, riformulata | Buona fabbrica di basin; M |
| 3 | **(e) crossover fra 155.456/155.422** | Screen piccolo e finito; S |
| 4 | **(b) step più fini** | Solo esperimento fattoriale su parent m34-native; S |
| CUT | **(d) flatten generico → m29/m33** | Già saturo; m29 dominato |

### 0. Prima il certificato di rigidità

Estenderei come diagnostica il programma globale translation-only già presente in [`GlobalRow`](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_micro_legalization.rs:2089), aggiungendo una variabile angolare per pezzo.

Per un witness `p`, centro `c` e normale di contatto `n`, il coefficiente rotazionale non è `r`: è

```text
aθ = n · J(p − c)
```

Per una coppia entrano entrambi i coefficienti. Il programma massimizza la contrazione uniforme del fronte sotto un trust radius.

Il risultato decide la spesa:

- Translation LP bloccato, SE(2) positivo per almeno 0.422 mm: costruire il candidato m33 indicato.
- Anche SE(2) bloccato con duale/self-stress: servono mirror/ejection/topology; niente ricerca di micron.
- SE(2) positivo ma candidato esatto fallisce: il problema è nella linearizzazione o nel generatore, non nello schedule.

Gli exact closest-approach witness sono già calcolati; oggi `Approach` conserva solo direzione e distanza [qui](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_micro_legalization.rs:814). Va conservata anche la coppia di punti.

### 1. m33 derivato: sì, ma non `δθ=δx/r` letterale

Userei `δθ=δx/|n·Jr|`, con segno scelto per aprire i contatti attivi. Il solo raggio:

- ignora la direzione del contatto;
- può suggerire una rotazione che non apre affatto il vincolo;
- non gestisce pezzi incastrati da più normali incompatibili.

Il ladder corrente è già molto ampio e arriva a `0.00128°` [qui](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_persistent_vacancy.rs:320). La sua giustificazione usa però un raggio esemplificativo di 100 mm: non è generale su DXF arbitrari. La scala deve essere per pezzo e per witness.

Inoltre, la certificazione finale ha già accettato varianti orientazionali (`0.0032`, `0.02`, mirror) senza scendere. Quindi il valore nuovo non è “più angoli”, ma:

- componenti selezionati dal duale;
- direzione e ampiezza derivate dai contatti;
- massimo K varianti per componente;
- budget separato, dopo lo stream legacy.

Il diff complessivo `155.456 → 155.422` contiene una rotazione `+0.008°` e un mirror flip con rilocazione di circa `130×50 mm`: l’ultima discesa non è stata puramente micrometrica. È un forte voto per la libertà di posa.

### 2. Perturb→m34: sì, ma solo preservando la precondizione

La forma diretta è sbagliata:

- m34 valida esattamente il parent all’ingresso [qui](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:5383);
- poi StructuredGrid ricostruisce le pose sulla griglia [qui](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:5454).

Il parent finale ha 49 rotazioni su 61 fuori dalla griglia 2.5°; l’ultimo pin totalmente grid-native è `156.418`. Quindi farei:

```text
parent m34-native
→ perturbazione traslazionale exact-valid nel nullspace del contact graph
→ verifica parentProxyFeasible
→ m34
```

Niente nudge casuale e niente perturbazione infeasible. Il gate è binario: se l’ingresso non è proxy-feasible, non contare l’arm come test dello schedule.

### 3. Crossover near-tie: screen ammesso, prior basso

Il test esistente include `155.463`, ma non `155.456`, e usa solo tagli `0.35/0.5/0.65`; il migliore resta 2.7 mm dietro [evidenza](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/record-line-cascade/README.md:277).

Farei un solo screen con:

- `155.456 ↔ 155.422`;
- band d’interfaccia derivate, già implementate nel coordinator;
- priorità alle band attorno ai due pezzi che cambiano orientamento/mirror;
- entrambe le direzioni;
- stop dopo l’enumerazione completa, se nessun figlio batte l’incumbent.

“Near-tie in depth” non significa “complementare in topology”, quindi niente campagna larga.

### 4. Step 0.125/0.0625: non è ancora una curva ben definita

Variando `step` a sweeps e `confirmEvery` fissi state cambiando simultaneamente:

```text
compressione per sweep = step / sweeps
repair work per mm      = sweeps / step
spazio tra conferme     = step × confirmEvery
fase delle conferme
```

Il successo di `0.25` può essere risonanza di cadence, non superiorità dello step.

Farei un fattoriale piccolo che mantenga costanti repair/mm e spacing fisico delle conferme, poi vari separatamente la fase. Solo su `156.418` o altro parent m34-native. Non farei schedule lunghi sul 155.422: lì state pagando il regrid.

### CUT: flatten→m29/m33 generico

Il flatten→m33 ha già 147 arm nella cascade e altri 12 nella batteria finale. Continua a produrre micron, non 0.422 mm. Inoltre m33 aggiunge i candidati orientazionali dietro tutto ciò che m29 può raggiungere, a struttura invariata [qui](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_persistent_vacancy.rs:2138). Taglierei m29. Un flatten guidato dai witness diventa il punto (a), non un’altra classe.

## 2. Prior e prezzi di produzione

Un prior per sola richiesta è già confutato dai vostri dati: sulla stessa Mixed-61 m34 funziona sui propri output e fallisce sugli output m22/m33; m26 fa 6/6 a 156.091 e 0/16 a 155.452.

Serve:

```text
P(pubblicazione | geometria richiesta, stato parent, configurazione operatore)
```

Separerei tre livelli.

1. Eligibility deterministica:

   - `parentProxyFeasible`;
   - coppie/boundary violations all’ingresso;
   - numero e chord displacement delle pose off-grid;
   - headroom normalizzato rispetto all’area bound;
   - rigidità del contact graph;
   - densità del fronte attivo.

   Per m34, parent proxy-infeasible per snap rotazionale deve valere prior zero/ineligible.

2. Prezzo wall modellato:

```text
T̂ = Tcatalog
   + Qproxy / Rproxy_single_lane
   + Naccepted × Tvalidate_accepted
   + Nrejected × Tvalidate_rejected
```

`Qproxy` deriva da step, sweeps e query/sweep misurate in una prima audition produttiva. La conferma accettata costa circa 4.8 ms su Mixed-61, non il narrow-counter da 0.49 ms; l’anatomia completa è [qui](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/compression-schedule/README.md:293).

I coefficienti ns sono un profilo della macchina; i conteggi sono deterministici e request-local. Non userei il rapporto col phase-0 aggregato: m34 è una lane, phase-0/m26 ne usano otto.

3. Yield online:

   - prior pooled molto debole;
   - prima audition corta, cap fisso e capace di pubblicare;
   - posterior su `p_publish × gain_normalizzato`, dove il gain è frazione del rung richiesto, non millimetri assoluti;
   - se non ripara l’ingresso o non raggiunge conferme proxy-feasible, stop immediato.

C’è poi un bug semantico nel budget: oggi il self-meter m34 modifica il prezzo e il ranking, ma non il budget effettivamente speso, come dice il codice stesso [qui](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:3438). In work mode aggiungerei un debit virtuale:

```text
spent += max(global_meter_delta, operator_self_units)
```

Altrimenti l’envelope work non è la valuta che il report sostiene.

Infine: pricing corretto evita un’azione costosa, ma non la rende veloce. Per il wall farei un batch deterministico di 2/4/8 schedule arm indipendenti su parent/phase distinti, con cap di lavoro fisso e reducer dopo barriera. La schedule oggi lascia inutilizzati gli altri worker [evidenza](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/compression-schedule/README.md:346). Misurerei scaling e contesa exact a pari wall.

## 3. CurrentAssignment: ora, non dopo 150

La inizierei ora, ma non sostituirei direttamente StructuredGrid col CurrentAssignment esistente.

Costruirei prima `StructuredGrid + CurrentPoseOverlay`:

- griglia e ordine dei candidati invariati;
- pose correnti aggiunte in un lookup separato solo per warm-start/repair;
- stesso `StructuredTrianglePoles` pressure model;
- nessun passaggio implicito a DirectionalPenetration.

Questo isola davvero il costo dello snap. Oggi `CurrentAssignment` cambia insieme catalogo, pair-NFP e pressure model [seam attuale](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_relaxed.rs:15293), quindi il confronto proposto nel v4 resta un confronto fra due motori.

La campagna A/B/C:

- A: StructuredGrid;
- B: Grid + current-pose overlay;
- C: CurrentAssignment + DirectionalPenetration;
- dodici parent originali più `156.9188`, `156.091`, `155.422`;
- pari work, con entry loss, collision pairs, query/s, conferme e pubblicazioni.

Lo `+0.448 mm` non è un premio atteso: potrebbe trasformarsi in zero, in un guadagno maggiore o in una regressione. Il vero premio è la composabilità `m33/m22 → m34`, oggi spezzata. Per arrivare a 150, questa composabilità è probabilmente necessaria.

## 4. Il soffitto prima di 150

Non è principalmente il quantum traslazionale da 0.001 mm. È importante per gli ultimi micron, ma non spiega un gap di 5.422 mm.

In ordine:

1. **Barriera di rappresentazione:** m34 non può consumare fedelmente i migliori stati continui.
2. **Rigidità/topologia del contact graph:** non conta solo quanti binding front esistono, ma il rango del Jacobiano e il suo duale. Un grafo denso può essere floppy; uno più sparso può avere self-stress.
3. **Repair translation-only:** m31 e m34 redistribuiscono traslazioni; il record mostra che mirror/rotazione sono ormai load-bearing.
4. **Basin generation:** m26 e m34 hanno resa discontinua per parent. Cinque millimetri richiederanno probabilmente almeno una nuova topologia, non una coda più fine.
5. **Quantum 0.001:** diventa sospetto solo se il programma continuo trova una direzione utile che scompare dopo snap. Questo si misura col certificato SE(2), non dal numero di micron osservati.

## 5. Cosa state sovrastimando

- La parola “fixpoint”: avete una batteria negativa finita, con copertura m34 descritta male.
- La promessa dei passi più fini: `0.25` è una singola interazione favorevole fra step, sweeps e conferme.
- Il `+0.448 mm`: è danno d’ingresso, non gain recuperabile.
- La possibilità di derivare il yield dalla sola geometria della richiesta: lo stato/basin domina già sulla stessa istanza.
- La complementarità dei near-tie: la vicinanza in depth non è diversità utile.
- L’efficienza mm/work di m34 come prestazione di produzione: una lane e un meter incompleto non fanno un envelope wall.
- La continuità 155→150: gli ultimi 0.034 mm hanno richiesto 266 arm; non è evidenza che altri 5.4 mm siano una continuazione della stessa curva.

Ordine operativo che sceglierei: correggere harness/certificate; costruire il Jacobiano SE(2); m33 witness-driven; self-loop grid-feasible da `156.418`; piccolo crossover `155.456↔155.422`; in parallelo avviare CurrentPoseOverlay e accounting wall/work. Non spenderei un’altra campagna lunga su m29, flatten generico o m34 fine applicato direttamente al final.