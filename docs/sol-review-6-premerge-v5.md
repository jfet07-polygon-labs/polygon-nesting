# Sol review 6 — pre-merge verdicts on the v5 round

> Verbatim final reply from the resumed codex session (gpt-5.6-sol, max effort, read-only) at HEAD f32c629, 2026-08-20.
> Package: /var/lib/t3/tmp/sol-round6-prompt.md (evidence-led pre-merge review of wf48/coordinator-v5-budget-debit, sol-review-5-current-pose-overlay, sol5/se2-rigidity-certificate).

## Verdetto

| Branch | Verdetto |
|---|---|
| `wf48/coordinator-v5-budget-debit` / `66060f1` | **MERGE CON CORREZIONI** |
| `sol-review-5-current-pose-overlay` / `f527bea` | **MERGE CON CORREZIONI**, solo seam sperimentale off-by-default |
| `sol5/se2-rigidity-certificate` / `b7a3891 + ac9b890` | **RIFIUTA il branch composto**. Salvare `b7a3891`, completandolo; riscrivere `ac9b890` |

Non ho trovato segni di manomissione o comportamento occulto nei diff. Le incoerenze sono visibili e sembrano errori da implementazione/evidenza affrettata: `v3=false`, `rollback=32`, claim documentali incompleti, formulazione matematica sbagliata.

## 1. Budget debit: MERGE CON CORREZIONI

Il bug è reale e l’idea `max(global_delta, self_units)` è corretta. Ma la batteria dichiarata non ha esercitato il codice.

### Finding critici

1. **La misura “nessun numero si muove” è mal costruita.**

   In `66060f1:docs/experiments/coordinator-v5-budget-debit/evidence/battery-fixed-sched.json` l’unico arm ha:

   ```json
   "label": "v4",
   "v3": false,
   "extra": ""
   ```

   Il driver traduce letteralmente quel booleano in `v3=0`. Compilare con `compression-schedule` non esegue né v3 né m34. Le profondità 174.208/176.056/179.006 non dicono nulla sul debit.

2. **Il debit vincola già entro 40M.**

   Ricalcolandolo sulla traccia v4 pinnata in [work-mixed61.json](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/coordinator-v4/evidence/work-mixed61.json):

   - 40M: il totale corretto sarebbe circa **41.19M, 41.81M e 51.33M**
   - 120M: **121.61M, 122.36M e 126.52M**

   Questo è un controfattuale sull’ordine vecchio, non la nuova curva: appena il debit entra, cambiano affordability e azioni successive. Serve rerun vero con `v3=1,sched=1,barren=1,divq=1`, almeno 40M e 120M.

3. **Non avrebbe intercettato i 2/27 wall overrun.**

   È esplicitamente no-op in wall mode. Gli overrun di una singola azione indivisibile richiedono preflight/p95 pricing, quanta più corti o batch deadline-aware.

4. **Il debit avviene troppo tardi per la telemetria anytime.**

   `run_operator` archivia e pubblica prima di restituire il self-cost; `try_publish` legge immediatamente il meter in [portfolio.rs](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:1651). Il caller applica il debit solo dopo `execute_v3_action`.

   Quindi la pubblicazione prodotta dall’azione corrente, `birthWorkUnits` dell’archivio e `OperatorCallReport.workUnits` non includono il suo debit. Le pubblicazioni successive includono quello precedente: curva temporalmente incoerente.

### Correzioni richieste

- Trasformare l’esecuzione in una transazione: dispatch → determinazione charge → debit → archive/publication/report. In alternativa, riportare esplicitamente `globalUnits`, `selfUnits`, `debitedUnits` e correggere tutti i timestamp post hoc.
- Usare `u64` e `operator_self_units.saturating_sub(global_delta)`; evitare il passaggio via `f64`.
- Far rendere al metodo l’extra applicato.
- Test specifici:
  - global 30/self 50 → speso 50;
  - global ≥ self → nessun extra;
  - due azioni consecutive;
  - saturazione;
  - wall no-op;
  - publication/archive/action report includono l’azione corrente.
- Rerun v4 autentico a 40M/120M e curve wall 3/10/30s.

Rischio del merge isolato: basso sui percorsi protetti, moderato sull’esperimento v4. È una correzione contabile necessaria, ma può legittimamente peggiorare la qualità a fixed-work e non migliora il wall time.

## 2. Current-pose overlay: MERGE CON CORREZIONI

### Flag-off

Non vedo un percorso protetto che cambi semanticamente con il flag falso. `continuous_rotation_keys()` calcola il vecchio predicato directional e aggiunge `|| current_pose_overlay`; a `false` restituisce il valore precedente. I siti di derivazione delle key risultano coperti.

Quello che i gate non provano è l’assenza di regressione prestazionale: ora quei siti hot chiamano un helper passando `GeneralRelaxedSettings`. Renderei il booleano lane-local/inlined e misurerei il flag-off con la feature compilata.

### Problemi che impediscono il merge così com’è

1. **Il regression test non prova che l’overlay venga consultato.**

   Usa quadrati simmetrici lontani, per cui grid e continuous risultano entrambi feasible e identici. Sarebbe passato anche col bug originale che inseriva la key continua ma continuava a cercare quella canonica.

   Serve geometria asimmetrica e almeno uno fra:

   - contatore `overlayLookupHits > 0`;
   - bounds/score che differiscono dalla grid e coincidono col surrogate costruito direttamente;
   - test separati per tutti i percorsi di lookup e per entrambe le varianti di scan.

2. **Il catalogo viene clonato profondamente.**

   `catalog.orientations.clone()` clona poligoni, triangoli, assi, poles e indici per tutte le rotazioni solo per aggiungere poche entry. È esattamente il tipo di costo setup che non possiamo introdurre nel path 10s.

   Usare ownership/`Arc::get_mut` prima della condivisione oppure catalogo layered `base + overlay`.

3. **La campagna usa `rollback=32`, negativo già certificato.**

   È esplicito in `drivers/campaign.py:96` e nel README: `past=1,rollback=32`. Gli arm sono appaiati, quindi la misura d’ingresso resta utile; il claim downstream 12/15 contro 9/15 non è trasferibile alla configurazione che spedisce. Rerun con `rollback=0` e settings reali v4.

4. **`currentPoseOverlayEntries` non conta i pezzi.**

   Conta le key uniche `(geometry_class, angle, mirror)`. Istante duplicate possono collassare. Separare `offGridPieceCount` da `overlayEntryCount`.

5. **Manca la suite completa sul feature combo.**

   Il log suite committato è per `jagua-experimental`; richiedo l’intera suite con `jagua-experimental,compression-schedule`, non solo i gate/example.

### Interpretazione dei numeri

Il `+9` coppie su 14/15 non invalida la fedeltà alla posa esatta, ma non è neppure “il prezzo atteso” finché non è classificato. Dice che il surrogate continuo è più conservativo, o più inaccurato, proprio alle rotazioni dei parent. Per ogni coppia nuova misurerei:

- collisione esatta material/envelope;
- risultato proxy grid;
- risultato proxy continuous;
- margine dal confine.

`0/15 parentProxyFeasible flip` è un negativo reale sulla lineage rilevante. Inoltre `parentProxyFeasible` è misurato prima della compressione: la spiegazione “chiedevamo −0.3mm” nel README è falsa. Non rifarei semplicemente gli stessi 15; costruirei una sweep causale intorno al confine proxy-feasible.

Quindi: merge del seam dopo le correzioni, sempre off, senza abilitarlo nel coordinator. La promozione richiede campagna `rollback=0`, wall/RSS e classificazione delle coppie.

## 3. SE(2): RIFIUTA il branch composto

`b7a3891` contiene correzioni documentali buone, ma non le ha applicate “ovunque”. Restano almeno:

- “certified fixpoint … four step sizes” in [next-generation-engine-plan.md](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:5129);
- “certified fixpoint of 36 arms” in [table.py](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/orientation-floor/drivers/table.py:16);
- `certify_full.py` conserva descrizione vecchia e campo `"fixpoint": true`.

Cherry-pickerei `b7a3891` dopo aver chiuso questi residui.

### Il solver non è il problema principale

Per il programma finito che implementa, il ragionamento è sostanzialmente corretto:

- ogni punto visitato dà un lower bound;
- ogni distribuzione duale normalizzata dà un upper bound per dualità debole;
- la convergenza del subgradiente influenza il gap, non la validità algebrica dei bound.

Ma sono bound in aritmetica reale, non “certificati esatti” floating-point. L’evidenza contiene già un caso `lower > upper` di pochi ulp e il codice lo nasconde con `.max(0.0)`. Servono rounding outward/scoped tolerance e un’asserzione sul bracket, non il clamp silenzioso.

### Il programma risolve la domanda sbagliata

Ottimizza:

```text
max_x min_i(a_i·x − rhs_i)
```

cioè cerca di aumentare uniformemente lo slack di **tutte** le righe. Ridurre la profondità di 0.422mm non richiede di aprire simultaneamente tutti i pair contact, il bordo sinistro, il fondo e il bordo corto di 0.422mm.

Il programma corretto deve introdurre `δ`:

```text
max δ
pair e bordi non-depth:       a_i·x ≥ rhs_i
bordo far/depth:              a_i·x ≥ rhs_i + δ
```

con separazione esplicita fra profondità materiale pubblicata e strip bound del collision envelope.

### Altri blocker di modello

- Le `Axis` row hanno `theta=0`. La rotazione può quindi alleggerire i pair constraint senza pagare il movimento dei vertici estremi contro il bordo. Questo può **sovrastimare**, non sottostimare, lo spazio rotazionale.
- Il guard band `2*trust` deriva dal vecchio trust euclideo. Qui ogni pezzo ha `|dx|,|dy|≤trust` più una rotazione che può muovere un vertice di un altro `trust`: la reach è insufficiente.
- Le collision-envelope row nascono solo per overlap effettivi. Una coppia legale ma raggiungibile dentro il box può collidere e non avere alcuna row.
- Al touch `measure_approach` scarta il witness; proprio i contatti attivi ottengono coefficiente rotazionale zero.
- Il risultato non conserva il miglior vettore `(dx,dy,dθ)`. Quindi il “constructive lower bound” non è utilizzabile da m33 né validabile esattamente.
- `Approach.witness` è presente anche senza feature: il costo sul percorso produzione non è realmente compilato via.
- I test non contengono un caso end-to-end in cui `dθ≠0` cambia il verdetto, né finite-difference sulle geometrie reali/mirrored/miter.

Il ricalibro `+0.15–0.28mm` è un red flag sulla formulazione dell’envelope, non sul record. State imponendo a materiale raw-depth e miter collision envelope lo stesso `sheet_long_axis_mm`; il motore dispone già di una trasformazione `tight_strip_depth`. Non si può correggere post hoc il bound dopo aver osservato le violazioni e poi chiamarlo certificato della profondità originale.

Infine, “nessuno ≥0.422” non è nemmeno conclusivo nel programma corrente: a trust 1mm i parent 155.264 e 155.422 hanno bracket SE(2) rispettivamente circa **[0.3347, 0.5024]** e **[0.3140, 0.4617]**. Il lower non raggiunge 0.422, ma l’upper non lo esclude.

Conclusione: il risultato dice soltanto che il front lineare incompleto non appare banalmente rigido. Non apre né chiude la caccia ai 0.422mm e non giustifica m33 in produzione.

## Prossima spesa sotto 10 secondi

Il mio ordine resta:

1. **Prior a tre livelli, iniziando dall’admission state-conditioned.**
2. **Wall batch**, dopo aver smesso di batchare azioni sterili.
3. **Witness-driven m33**, come audition bounded e solo dopo aver prodotto un witness reale.

### Cosa farei per primo

Implementerei il livello minimo del prior:

- request prior: pezzi, vertici/triangoli/poles/celle, catalogo orientazioni, aspect ratio e densità;
- state admission: `parentProxyFeasible`, entry collision/boundary count, snap damage, target drop, densità del binding graph;
- audition breve e deterministica: costo wall/self/global, pubblicazione e mm/work;
- almeno una quota di esplorazione per non rendere il prior auto-confermante.

È il passo col miglior impatto immediato: evita di comprare m34 dove shapes-17/triangle-20 hanno già mostrato resa zero e rende onesto il pricing per geometria.

Poi wall batch con quanta 2/4/8, reducer a barriera e tie order stabile. Prima, però, misurerei l’occupazione reale: se un singolo arm usa già tutti gli 8 worker, eseguire più arm completi in parallelo crea oversubscription. In quel caso il batch va portato dentro il kernel o deve partizionare i worker.

m33 viene terzo perché oggi manca proprio il witness. La versione utile del certificato deve massimizzare `δ-depth`, restituire il vettore, applicarlo e passarlo all’esatta validazione. Solo una misura positiva in **mm/secondo** su parent da richiesta nuda gli compra uno slot nella curva 10s. Il record offline può aspettare.