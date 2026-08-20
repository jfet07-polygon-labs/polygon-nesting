# Grok review 1 — independent pre-merge review of the v5 round and the 10s plan

> Verbatim reply from the local Grok CLI (v1.0.5, default model, reasoning effort xhigh — the maximum available), read-only, at HEAD 578f2e0, 2026-08-20.
> Independence protocol: formed its verdicts before opening docs/sol-review-6-premerge-v5.md; the comparison section is its own.
> Package: /var/lib/t3/tmp/grok-review1-prompt.md.


| Branch | Verdetto |
|---|---|
| `wf48/coordinator-v5-budget-debit` `66060f1` | **MERGE CON CORREZIONI** (bloccanti) |
| `sol-review-5-current-pose-overlay` `f527bea` | **MERGE CON CORREZIONI**, solo seam flag-off |
| `sol5/se2-rigidity-certificate` `b7a3891`+`ac9b890` | **MERGE CON CORREZIONI**: `b7a3891` sì; `ac9b890` solo come diagnostico etichettato, non come risposta a «posso ridurre la profondità di δ» |

Nessun segno di manomissione. I tre round hanno scritto README più onesti della media di questo repo, poi hanno commesso errori di misura che quei README non chiudono.

---

## 1. Verdetti per branch

### 1.1 `wf48/coordinator-v5-budget-debit` (`66060f1`) — MERGE CON CORREZIONI

Il buco è reale. Su HEAD, `schedule_self_cost_units` alza `ClassStats::cost_max` e il ranking, ma il commento dice ancora «price, never a spend»: `BudgetMeter::work_units` avanza solo sul contatore globale (`portfolio.rs:1461-1463` HEAD). Sotto un budget **work**, una classe il cui self-meter legge 2–11× il globale può comprare più di sé di quanto il cap nominale permetta. Il patch aggiunge `self_metered_debit` e, dopo l’azione, `debit_self_metered(metered_cost, units)` che somma `max(0, self − global)` (`66060f1` `portfolio.rs:1490-1493`, `3152-3155`). Sotto wall è un no-op, come dichiarato.

**F1 — La batteria non ha mai eseguito il codice che pretendi di avere misurato.**  
`evidence/battery-fixed-sched.json`:

```json
"label": "v4", "budget": "work=120000000", "v3": false
```

`spec` = `work=120000000,cells=13:15:17:19,v3=0`. Il parse è `settings.coordinator_v3 = value != "0"` (`general_request_benchmark.rs:1774`). Con `v3=0` si entra nello schedule a fasi v2 (`portfolio.rs:2009-2014`) e **non** in `v3_loop`, unico sito del debit. Archivi: solo `mode20/22/23`. `schedule: null`. Uscite `keysExhausted` / `noResidue` / `patience` a ~28–32M di 120M. Profondità 174.208 / 176.056 / 179.006 = v2 da richiesta nuda, non v4. Lo stesso `v3=0` è nei due script warm-start. Compilare `compression-schedule` non arma m34 se il coordinatore v3 è spento.

**Falso positivo da chiudere:** «paired baseline-vs-fixed `v4:work:120000000:0`» e «nessun numero si è mosso perché m34 è inerte». Nessun numero si è mosso perché **non avete eseguito v3 né m34**. I gate 20/22 bit-identici e i 26 test `portfolio::` non toccano `debit_self_metered` (zero test del nuovo metodo).

**F2 — Il debit vincola già sui work-budget v4 pinnati, che questa round non ha rieseguito.**  
Controfattuale (ordine azioni *vecchio*, quindi non è la curva nuova) su `docs/experiments/coordinator-v4/evidence/work-mixed61.json`, extra = `actualCost − meteredCost` sulle azioni `schedule`:

| cella | `portfolio.workUnits` | extra m34 | speso onesto |
|---|---:|---:|---:|
| 40M s0 | 39,309,265 | 2.50M | **41.81M** |
| 40M s1 | 38,518,915 | 2.67M | **41.19M** |
| 40M s2 | 37,102,965 | 14.23M | **51.33M** |
| 120M s0 | 113,570,496 | 8.79M | **122.36M** |
| 120M s1 | 115,804,466 | 5.81M | **121.61M** |
| 120M s2 | 111,276,458 | 15.24M | **126.52M** |

Tutte e sei sforano il cap nominale. Seed 2 a 40M ha 8 slice m34: col debit il run si sarebbe fermato molto prima. I 162.161/163.927/164.004 @ 120M sono leggermente *sovra-comprati* in valuta onesta. Non è «il debit non si vede». È «non l’abbiamo messo nella condizione in cui si vede».

**F3 — Ordinamento rispetto a pubblicazione / archivio / report.**  
`execute_v3_action` chiama `run_operator` (archive + `try_publish`) e solo *dopo* restituisce il self-cost; il caller debita dopo (`v3_loop` ~3102-3155 sul branch). Quindi, per l’azione corrente:

- `birthWorkUnits`, `OperatorCallReport.workUnits`, `ScheduledActionReport.workUnits`, `ClassStats.workUnits` restano il delta globale;
- `actual_cost` / `cost_max` usano `max(global, self)`;
- `spent_fraction` / `remaining_to` vedono l’extra solo dal giro *successivo*.

Le curve anytime in work-units dell’azione che pubblica sono quindi in valuta diversa da quella che chiude il budget. Per m34 non c’è `legalize_residue` intra-azione, quindi il search decision successivo è coerente; la telemetria no. Wall: irrilevante (F4).

**F4 — A che budget il debit vincola, e a quale no.**  
Vincola **solo** `PortfolioBudget::Work`. Non intercetta i 2/27 overrun wall di v4 (una slice stimata 1.95 s costata 5.12 s). Non muove la curva 10 s da richiesta nuda. Il sito in cui è load-bearing è esattamente 40M/120M v4, non misurato.

**F5 — Aritmetica.**  
`(operator_self_units as f64 - global_meter_delta).max(0.0) as u64` (`1491-1492`). Sopra 2⁵³ perde interi; il troncamento vs `saturating_sub` su `u64` è l’API giusta. Impatto pratico a 120M: nullo. Test: assenti.

**Correzioni prima del merge**

1. Rerun appaiato `v3=1` (shipping: `sched/barren/divq` default-on) a **40M e 120M**, mixed-61, 3 seed, baseline `f32c629` vs fix. Riportare profondità, `exitCause`, n° azioni m34, speso globale vs onesto. Chiudere il falso positivo nel README e nel capitolo ledger.
2. Test di unità: 30/50 → speso 50; global ≥ self → extra 0; due azioni consecutive; wall no-op; saturazione.
3. O transazione dispatch → charge → debit → publish/archive, o campi espliciti `globalUnits` / `selfUnits` / `debitedUnits` e timestamp post-hoc. Non lasciare `workUnits` del report in valuta diversa da `spent`.
4. Extra in `u64` (`saturating_sub`), non via `f64`.
5. Non promettere impatto 10 s wall.

Rischio del merge isolato: basso sui gate (mode 20/22, `self_metered_units = None`). Moderato sulla curva work v4: la qualità a 120M può peggiorare. È igiene contabile necessaria, non un lever di produzione.

---

### 1.2 `sol-review-5-current-pose-overlay` (`f527bea`) — MERGE CON CORREZIONI, flag off

Lo seam è quello chiesto: `StructuredGrid` invariato, overlay solo per risolvere pose parent off-grid, pressione `StructuredTrianglePoles`. Flag default `false`. I sei siti che prima passavano `uses_directional_pressure()` ora passano `continuous_rotation_keys()` (`general_relaxed.rs` ~11146, 13552, 13608, 14516, 14532, e `scan_fixed_neighbors` a 14930 che riceve quel bool). A flag off il predicato è identico (`RollbackTriangle && DirectionalPenetration`, `10870-10873`). I caller di `initialize_complete_state` fuori da mode 34 passano `false`.

**F6 — Bit-identità flag-off sui sei siti: tenuta, con un caveat di costo.**  
Nessun cambio semantico a `current_pose_overlay = false`. I gate `jagua-experimental` (feature compilata *out*) non provano il path con `compression-schedule` compilato e flag false oltre al JSON gate. I sei siti hot ora costruiscono/passano `GeneralRelaxedSettings` a un helper: non ho evidenza committata di regressione eval/s flag-off.

**F7 — Il test di regressione non prova il lookup.**  
`compression_schedule_without_overlay_snaps_continuous_parent_rotation` (~17828): due quadrati lontani, `rotation_deg: 13.37`, `work_cap_queries: Some(1)`, `rollback_after_steps: 0`. Asserisce `current_pose_overlay_entries == 1` e `parent_proxy_feasible`. `overlay_entries` è `overlay.len()` al *build* (~5543), non un hit di `.get()`. Con il bug originale (seed continuo, key ricanonizzata) il parent resta feasible, le entry overlay restano non lette, e il 2.5° è in catalogo → niente `missing_orientation`. Il test passerebbe. Manca: geometria asimmetrica, `parent_entry_loss` diverso, oppure un hit counter.

**F8 — Costo di setup del catalogo.**  
`catalog_with_current_pose_overlay` clona l’intero `orientations: BTreeMap` (`OrientedSurrogate` è `Clone`: `PolygonSet`, `Vec<Triangle>`, `CellIndex`, poles) per inserire poche key. Per arm, non O(pezzi off-grid). Inaccettabile sul path 10 s se mai si promuove. Serve overlay layered (`base.get().or_else(overlay.get)`), non deep clone.

**F9 — Campagna: `rollback=32`, `past=1` — non è l’m34 del coordinatore.**  
`campaign.py`: `POLYGON_NESTING_COMPRESSION_SCHEDULE=past=1,rollback=32,work=3341379`. Produzione v3 (`execute_v3_action` HEAD ~3397-3414 e `CompressionScheduleSettings::default`): **`rollback_after_steps = 0`**, **`continue_past_bound = false`**, niente work cap, nove rung. Il port ha già misurato rollback=32 = mediana appaiata **−10.962 mm** su 12/12. Gli arm A/B sono appaiati, quindi l’ingresso è informativo; 12/15 vs 9/15 pubblicazioni e +1.85 mm di drop **non si trasferiscono** alla classe in coda.

**F10 — `+9` coppie su 14/15: committato, reale, non spiegato.**  
`evidence/ab-campaign.json`: median Δ collision pairs **+9**, 14 increased / 0 reduced / 1 unchanged (port-seed1, 0 overlay entries). Boundary violations median **−2**. Entry loss median **−1420** (unità proxy, non mm). `parentProxyFeasible` false/false su 15/15. Alcuni parent *peggiorano* in profondità pubblicata (port-seed8 178.754→178.826, port-seed9 174.552→174.605).

**F11 — Composabilità 0/15: negativo reale; la spiegazione «−0.3 mm» è falsa.**  
`parent_proxy_feasible` è `score.feasible()` **dopo** `tight_strip_depth` del parent e **prima** del loop di step (`drive_compression_schedule` ~5576-5595). Non è «chiedevamo 0.3 mm di compressione». I 26–38 colliding pairs sono il parent *alla propria profondità*, lo stesso fatto del port §7.1. L’overlay toglie lo snap e **aumenta** le coppie proxy: non può flippare un threshold a zero da 26–46. Non rifarei gli stessi 15 per «avvicinarmi al bordo»; serve uno sweep di parent già vicini a feasible.

**F12 — `currentPoseOverlayEntries` conta key, non pezzi.**  
`build_current_pose_overlay` skippa se `(geometry_class, angle_key, mirrored)` è già in grid o overlay. Istanze duplicate collassano. Su `true-155.422` esce 49, che coincide col 49/61 noto, quindi lì non ha collassato; la metrica resta sbagliata in generale.

**Falsi positivi da chiudere**

- «12/15 vs 9/15, drop 5.984 vs 4.136» come evidenza per l’m34 di produzione.
- «il test pìna la risoluzione overlay».
- «infeasible perché chiediamo −0.3 mm».

**Correzioni**

1. Test che fallisce se il lookup ricanonizza (geometria asimmetrica; oppure `overlayLookupHits`; oppure score/bounds ≠ grid e = surrogate continuo).
2. Catalogo layered, zero clone della griglia.
3. Rerun A/B con `rollback=0`, `continue_past_bound=0`, settings v4; tenere i numeri d’ingresso, etichettare i 12/15 come non-shipping.
4. Spezzare `offGridPieceCount` / `overlayEntryCount`.
5. Non promuovere nel coordinatore. Non abilitare sul 10 s.

---

### 1.3 `sol5/se2-rigidity-certificate` (`b7a3891`+`ac9b890`) — MERGE CON CORREZIONI

**`b7a3891` è il pezzo buono.** Tag collision `step=` vs `work=` in `certify_full.py`; «certified fixpoint» → «finite negative on the declared battery» nei due README toccati; 16 righe non 12 su `m26sweep-155.452`; ogni m34 della cert battery era `parentProxyFeasible: false`. Il record 155.422 non si tocca.

Residui **non** corretti da `b7a3891` (HEAD, quindi anche sul branch rispetto al ledger): `docs/next-generation-engine-plan.md:5129` «certified fixpoint of 36 further probe arms»; `orientation-floor/drivers/table.py:17`; `update_readme.py`, `fillreadme.py`; `certify_full.py` lascia `"fixpoint": true` nel JSON. Cherry-pick dopo un grep del claim, non prima.

**`ac9b890` — la formulazione non risponde alla domanda.**

Il programma è `max_{x ∈ Box} min_i (a_i·x − rhs_i)` (`solve_se2_program` ~3207). È slack uniforme su **tutte** le righe (pair + quattro lati × due gate × ogni pezzo). Ridurre la profondità di δ è

```text
max δ
  pair e bordi non-depth:  a_i·x ≥ rhs_i
  bordo far/depth:         a_i·x ≥ rhs_i + δ
```

Il min-slack è sufficiente e **non necessario**: un moto che chiude slack di una coppia (restando feasible) e ritira il pezzo più profondo è un δ-depth legale che questo LP rifiuta.

**F13 — Coefficienti rotazionali assenti sulle righe che *sono* la profondità.**  
`se2_row_theta(Axis) = (0,0)` (~3141-3145). Le Axis sono esatte in traslazione («outer bounds of a translated outline», commento `GlobalRow::Axis`). Rotare un pezzo per diminuire l’estremo long-axis **non entra** nel vincolo di profondità. Il README dice «sottostima, quindi conservativo». È falso per *questo* programma: θ può aprire i pair senza pagare il moto dei vertici estremi → **sovrastima** lo slack uniforme. Per un obiettivo δ-depth, l’assenza è il termine che serviva. Due errori, direzione opposta, stessa omissione.

**F14 — θ = 0 anche sui contatti attivi e sugli overlap.**  
Touch: `measure_approach` azzera `witness` se la normale non è normalizzabile (~1874-1888). Envelope-overlap: bisezione, `theta: (0,0)` (~2750-2759). Il test «blocked» sulla catena di quadrati toccanti ha θ = 0 per costruzione: **non esiste un test in cui dθ ≠ 0 cambia il verdetto**.

**F15 — Bound numerici.**  
Calibrazione post-hoc del `sheet_long_axis_mm` (+0.276570 mm sul 155.264, +0.150 sul 171.238) perché materiale pubblicato e collision miter non condividono lo stesso estremo. Poi si certifica rispetto al bound *allentato*. Non è un certificato della profondità pubblicata.  
`gap_mm = (dual − primal).max(0.0)` (~3341): il codice ammette un bracket invertito e lo nasconde. In `se2-battery.json` succede (tre celle, scarto ~2 ulp a trust 0.006). Non è un certificato in aritmetica FP.  
Trust-box: `|dx|,|dy|≤trust` **e** rotazione che muove il vertice più lontano di altri `trust` mm (`theta_cap = trust/reach`). Il guard `2*trust` di `build_global_rows` è il reach euclideo vecchio, insufficiente. Coppie oggi legali che nel box collidono non hanno riga.

**F16 — «Nessuno ≥ 0.422» non è nemmeno conclusivo nel programma sbagliato.**  
A trust 1.0 mm, parent A SE(2) = `[0.3347, 0.5024]`, parent B = `[0.3140, 0.4617]`. Il lower sta sotto 0.422; l’upper **non esclude** 0.422. A trust 0.422 i numeri sono più piccoli (~0.24) ma la linearizzazione a 1 mm è comunque la meno fidata. I 40/40 `positive` sono relativi al bound calibrato, slack uniforme, θ assente sulle Axis.

**F17 — Niente candidato m33.**  
Il solver non conserva il miglior `x = (dx, dy, dθ)`. Un lower «costruttivo» che non si può applicare, snappare e rivalidare esatto non è un witness. `Approach.witness` è sul path produzione **senza** `#[cfg]` (solo `GlobalRow::Pair.theta` è feature-gated): il default build non è byte-identico a livello di struct, anche se `global_legalize` non legge il campo.

**Cosa manca per un risultato azionabile**

1. Obiettivo δ-depth, non min-slack uniforme.
2. θ sulle Axis (vertice che realizza l’AABB, `n · J(p − c)`).
3. Witness tenuto al tocco; coppie raggiungibili nel box, non solo overlap attuali.
4. Due bound riportati: profondità materiale pubblicata **e** envelope, senza calibrazione silenziosa.
5. Vettore `x` restituito → applicato → snap → validazione esatta. Solo allora è un candidato m33.
6. Un test in cui θ cambia il verdetto.

Finché manca (1)–(5), `ac9b890` è un diagnostico del front lineare incompleto. Non apre e non chiude i 0.422 mm. Non giustifica m33 in coda. Feature off: merge del codice solo con README che ritira il claim «nessuno ≥ 0.422 come risposta alla profondità».

---

## 2. Piano 10 s (171–176 → 150.165)

### (a) Il collo vero

Non è eval/s del substrato. m22 è a parità con Sparrow (3.775M vs 3.742M, stesso box, 8 worker). Il gap 10 s è **qualità per secondo di wall del compressore**, non del candidato loop.

Fatti misurati, tutti su file:

- v4 @ 10 s wall, mixed-61, 9 round: **173.575 / 171.362 / 176.162** vs Sparrow **150.16547 @ 10 s** sullo stesso x86 (`docs/experiments/sparrow-mixed61/README.md`). Gap ~21 mm.
- La classe che ha comprato i mm è m34: 9 azioni / 9 pubblicazioni a 10 s, ladder 0 azioni (prezzata fuori). Ablation 120M: `sched=1` da solo riproduce 163.927/162.161/164.004; `barren` e `divq` inerti su mixed-61.
- m34 è **una lane**; mode 26 ne usa otto (`compression-schedule` README §6.3). I 7 worker idle non sono un’ipotesi.
- Il prezzo work di m34 **non** si trasferisce al wall: actual/estimate **0.97–1.01** in work, **2.54–2.59×** mixed-61 wall, **2.94–3.07×** shapes-17, **5.1×** triangle-20. Prima azione = 1.5–5 s di un budget da 10 s. Un overrun v4 a 30 s: stima 1.95 s, costo 5.12 s.
- Prior 1.104 mm è un numero mixed-61. Pubblicazioni m34: **0/29** shapes-17, **0/37** triangle-20.
- Ogni parent 171–179 arriva **già** proxy-infeasible (26–38 coppie) *alla propria profondità*. Una frazione della slice è regrid, non compressione.
- 40M work (ancora ~10 s-class in valuta globale) dà 165.8–171.4; 120M dà 162. Il 10 s wall sta **dietro** il 40M work. Chiudere wall-vs-work è ~4–6 mm, non 21. I 150 non stanno in un reschedule.

Crossover seam (legalizer 8-lane/40-sweep): a 10 s v4 il crossover è già crollato da 17 azioni a 1 perché schedule+compression hanno spostato l’incumbent. Non è il collo mixed-61 a 10 s. Lo è su shapes-17 (churn, già tagliato dalla patience).

### (b) Quattro azioni, in ordine

**1. Prezzo wall di m34 (prima azione inclusa), non tre livelli di prior.**  
Meccanismo: prior wall da p95/worst della stessa richiesta, o da un modello `exact_share × confirmations`; ratchet già esiste ma arriva un’azione troppo tardi.  
Rischio: sovrastima → m34 non compra a 10 s su mixed-61 (perdita dei 9/9). Tenere un floor «almeno 1 slice se eligible».  
Misura: anytime **3/10/30 s**, 3 seed × 3 round, mixed-61 **e** shapes-17 **e** triangle-20, appaiato a v4, stessa macchina, `v3=1`. Metriche: profondità, n° slice m34, wall della prima slice, overrun, mm/s. Successo mixed-61: non perdere i 9/9 e tagliare gli overrun; successo altri corpora: 0 slice sterili da 5 s.

**2. Parallelismo *interno* di mode 34 (le 8 lane che m26 già usa), non batch di arm indipendenti.**  
Meccanismo: lo `strip_depth_mm` è già uno scalare letto da tutte le lane; `run_independent_lanes` esiste. Un clock, otto worker, stessa conferma/floor.  
Rischio: semantica di conferma/rollback tra lane; determinismo.  
Misura: stesso protocollo wall 3/10/30, più queries/s e lane occupancy. Ipotesi: il 10 s si avvicina alla qualità 40M-work (~166), non a 162 e non a 150.

**3. Entry feasible *senza* overlay: legalize di sola traslazione del parent alla propria profondità prima dello step 0.**  
Meccanismo: `global_legalize` / mode 27-31 già esistenti; pubblicare solo se exact+contract. Obiettivo: `parentProxyFeasible=true` all’ingresso, così la slice non paga il regrid. L’overlay ha *alzato* le coppie proxy: non è questo lever.  
Rischio: 4.8 ms × conferme; fallimento → skip m34 (meglio di 2.5 s di regrid).  
Misura: frazione `parentProxyFeasible` pre/post, Δraw della prima slice, same anytime triad.

**4. Prior m34 *per richiesta* (zero dove ha pubblicato 0), un bit, non uno state-machine.**  
Meccanismo: shapes-17/triangle-20 prior Δ=0 o costo ∞ dopo N=0 su quella richiesta; mixed-61 invariato. Audition rara se si vuole falsificabilità.  
Rischio: prior assorbente — mitigato dall’audition, e qui abbiamo già 0/29 e 0/37.  
Misura: shapes-17 10/30 s non deve regressare in profondità e deve restituire wall; mixed-61 10 s invariato.

Dopo (1)+(2) il tetto onesto è ~166 @ 10 s, non 150. I 12 mm restanti (162→150) e i 5 mm sotto il record 155.264 sono un operatore di qualità diverso (rotazione continua *nel search*, non un certificato lineare a 155 mm). Non entra in un piano da 4 azioni sulla curva 10 s da richiesta nuda.

### (c) Cosa non farei

- **Prior a tre livelli state-conditioned.** Su mixed-61 a 10 s m34 pubblica già 9/9. La complessità (feasible / wall model / posterior) ricrea prior assorbenti. Il problema 10 s non è «comprare m34 sulle richieste sbagliate» come collo *misto-61*.
- **Wall batch di schedule arm indipendenti.** m34 è già sotto-parallelizzato *dentro* l’arm (1 vs 8). N arm seriali da 1 lane su 8 core = oversubscription o idle a seconda del pool. Determinismo del reducer a barriera su un coordinatore che oggi è una coda. Prima occupare le 8 lane di *un* arm.
- **m33 witness-driven.** A 10 s da nudo non si è mai vicini a 155. Il certificato SE(2) attuale non produce un vettore applicabile. mm/s su parent da richiesta nuda: non misurato, non promettibile. Strumento da record-line (ore), non da curva 10 s.
- **CurrentPoseOverlay promosso.** 0/15 flip; +9 coppie; clone del catalogo; campagna a rollback=32. Flag-off come laboratorio della snap, basta.

---

## 3. Confronto

Aperto solo qui: `docs/sol-review-6-premerge-v5.md`.

### Convergenze

- Debit: bug reale; batteria `v3: false`; extra già sopra 40M e 120M sulle tracce v4; debit dopo publish; wall no-op; MERGE CON CORREZIONI. I suoi 41.19/41.81/51.33 e 121.61/122.36/126.52 coincidono con il ricalcolo su `work-mixed61.json` (ordine seed 1/0/2).
- Overlay: test che non prova il lookup; clone profondo; campagna `rollback=32`; +9 su 14/15; 0/15 composabilità; MERGE CON CORREZIONI, non promuovere.
- SE(2): obiettivo min-slack ≠ δ-depth; Axis senza θ; tocco/overlap a θ=0; calibrazione 0.15–0.28 mm; nessun vettore `x`; `b7a3891` da salvare; residui «certified fixpoint» nel ledger e in `table.py`.

### Divergenze

**Debit.** Lui chiede curve wall 3/10/30 come correzione del merge. Il debit è no-op in wall: quelle curve non testano il patch. Io le chiedo per il *piano 10 s* (prezzo wall di m34), non per chiudere questo PR. Extra via `u64`: d’accordo, ma non è il blocker; il blocker è `v3=1` a 40M/120M.

**Overlay.** D’accordo sul layered catalog e sul test. Lui vuole la suite intera `jagua-experimental,compression-schedule`; è igiene, non il difetto che invalida i 15 parent. La spiegazione README «−0.3 mm» è falsa: l’ho verificata su `tight_strip_depth` *prima* del loop; lui l’ha detta più netto.

**SE(2) — verdetto.** Lui **RIFIUTA** il branch composto e riscrive `ac9b890`. Io **MERGE CON CORREZIONI**: `b7a3891` dopo grep dei residui; `ac9b890` può restare come diagnostico feature-off *se* il README ritira il claim di profondità. Rifiutare il codice perché risponde alla domanda sbagliata è pulito; tenerlo etichettato è accettabile in un repo che già accumula strumenti di laboratorio. Non dissento sul merito («non apre né chiude i 0.422»). Dissentono sul packaging. Il suo punto «θ=0 sulle Axis *sovrastima* lo slack uniforme» è più preciso del README «conservativo»; la mia review diceva la stessa cosa per δ-depth e non contraddiceva abbastanza il claim conservativo.

**Piano 10 s — divergenza principale.** Il suo ordine è (1) prior a tre livelli, (2) wall batch, (3) m33 witness-driven: esattamente i tre item che l’owner ha messo tra «il team sta considerando» e che io non farei. Su mixed-61 a 10 s m34 pubblica già 9/9: un prior a tre livelli non compra i 21 mm. Il wall batch di arm indipendenti ignora il fatto misurato che **un** arm m34 lascia 7 lane idle; lui stesso avverte l’oversubscription, poi lo mette al secondo posto comunque. m33 terzo richiede un witness che `ac9b890` non emette, e parent che la curva 10 s da nudo non raggiunge.

Lui nota «se un arm usa già 8 worker, il batch sovrascrive»: la misura committata dice che l’arm **non** usa 8 worker. Quella occupazione andava letta in `compression-schedule` §6.3 prima di proporre il batch.

### Cosa aggiunge questa review

- Chiusura esplicita dei falsi positivi (`v3=false`; test overlay che passerebbe il bug originale; 12/15 a rollback=32; «−0.3 mm»; «nessuno ≥0.422» con dual che a trust 1 mm *include* 0.422).
- Tabella extra onesto 40M/120M per seed, non solo «vincola già a 40M».
- Overlay: `entries` = key uniche; parent peggiori (seed 8/9); `Approach.witness` non cfg-gated.
- Piano 10 s costruito sul collo misurato (wall-price m34 × 1-lane × entry infeasible × tetto ~166), non sulla lista strumenti del round v5. Stima onesta: (1)+(2) chiudono wall-vs-work, non Sparrow 150.165.
- Non promuovo overlay, non parto da m33, non batcho arm sterili.