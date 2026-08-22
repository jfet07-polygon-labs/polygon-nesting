# Kimi review 1 — the never-run m26 band audition, and the target below the asymptote

> Verbatim final answer (local Kimi CLI, K3, thinking enabled, first consultation, blinded to Sol/Grok reviews until its comparison section) at HEAD 5ccab1c, 2026-08-22.

# Parere indipendente (scritto prima di leggere `shipped-surface.md` e le review; confronto in fondo)

  ## 0. Il numero che decide la domanda

  Mettendo in fila solo evidenza committata:

  | punto | valore | fonte |
  |---|---:|---|
  | 10s wall (best arm, irriproducibile) | 168.484 | `docs/experiments/calibrated-plan/README.md:479-484` |
  | 10s plan (riproducibile, post-debit) | 174.280 | `docs/experiments/consolidation/README.md:271-275` |
  | 120M work units (~30s+) | 163.927 | `docs/experiments/consolidation/README.md:213-218` |
  | record di campagna (ore, ~7.204 arm) | 155.264 | `docs/experiments/record-line-cascade/README.md` + orientation-floor |
  | Sparrow, stesso box, 10s | 150.165 | `docs/experiments/sparrow-mixed61/README.md:66-74` |

  Il tasso marginale è misurato e collassa: +5.96mm per 3× lavoro a 40→120M, poi ~0.16mm/round al record. **Il risultato di Sparrow a 10 secondi sta 5.1mm sotto il miglior risultato che questa macchina ha mai dimostrato a qualsiasi budget.** Questo ribalta la domanda: il problema a 10s non è throughput (substrato pari, `plan:2031`), non scheduling, non l'exact tier (che a 10s non rifiuta nulla e pesa 1.55% del leaf — `docs/experiments/mode26-rung-anatomy/README.md` §1.6). La risorsa scarsa a 10s è il **salto di bacino**: lo spread di seed a pari lavoro è 8.6mm (`calibrated-plan/README.md:481`), cioè l'endpoint è dominato dalla fortuna della traiettoria greedy, non dal lavoro speso.

  ## 1. La direzione: l'audition mai eseguita del ladder m26 corto dalla banda 171–179

  **Meccanismo.** Mode 26 non è repair: ricostruisce la pipeline costruttiva mode-0 sotto un clamp di profondità (`run_ladder_compression`, `general_relaxed.rs:6062`) — una ri-soluzione costruttiva a target, non un ratchet. È l'unico meccanismo con un guadagno multi-millimetro misurato esattamente nella banda dove vive il run a 10s: arm C "m26 short ladder → m31" ha prodotto **−4.9571 e −4.3170mm su seed 0/1** da archivio saturo a ~174mm, "the largest single gain this coordinator has ever made" (`docs/next-generation-engine-plan.md:4362-4372`). Quella cella non è mai stata convertita in un gate matched-arm: il negativo noto di m26 è a 155 (0/16, `record-line-cascade/README.md:203-207`) e l'anatomia è a 159–164 — **sulla banda 171–179 non esiste né un positivo gated né un negativo gated**.

  **Perché evade le cause di morte nominate, una per una:**

  - *Multi-basin race 0/21*: arm C non è una race. È una sonda sequenziale post-drain con adozione solo su miglioramento stretto exact-valid; niente criteria contest, niente challenger fresco a 192–210 contro incumbent disceso (`work-currency/README.md:476-510`). Le quattro cause P0 della race sono tutte nel meccanismo di selezione, che qui non esiste.
  - *Pricing/currency*: i draw invisibili erano mode 20 (0 su 4 contatori, `work-currency/README.md:101-135`); il ladder vive interamente nel meter globale (1.13B candidate queries nell'anatomia). Il gate è a **work budget**, dove i documenti sono deterministici 9/9 (`consolidation/README.md:515-523`).
  - *Densità di conferma*: non aggiunge pressione exact; una conferma completa costa 0.49ms (`mode26-rung-anatomy` §1.3) e l'exact è 1.55% del leaf.
  - *"Scheduler migliore"*: non riordina la coda; aggiunge una classe di azione con kill gate. Il fallimento "le classi che pubblicano zero" non si applica: arm C ha pubblicato 2/3.
  - *Rotazione / contact-block / witness*: meccanismo ortogonale, nessuna di quelle autopsie si applica.

  **La causa di morte nota che prezzo dentro il gate**: l'anatomia mostra 85.4% degli arm abortiti su un disaccordo ULP del rollback tracker e 75.5% del wall buttato (`mode26-rung-anatomy` §1.4–1.5), 32.25M query/rung. Quindi il confronto deve essere **mm pubblicati per unità di lavoro del coordinatore**, non per arm.

  **Gate falsificabile (≤1 round, harness esistente)**: i 12 parent from-request 171.614–179.620 di `contact-block/drivers/parents12.json`, il controllo verbatim di `contact-block/drivers/matched.py` (slice m34 seriale `past=1,work=W` dallo stesso parent). Braccio: un rung m26 corto (drop 1.0) → m31, cappato allo stesso W (~15–35M unità, dove m34 compra ~1.5–2.5mm). **Kill**: se il braccio ladder non batte la mediana del controllo di ≥1mm o non scende sotto il controllo su ≥8/12 parent, m26 è tagliato dalla banda 10s con evidenza — e si chiude per sempre anche la spesa rimasta aperta. Costo: un pomeriggio di batteria, zero chirurgia al motore.

  **Aspettativa onesta**: pass/no-pass è ~50/50 (2/3 positivo su 3 seed in una cella, contro 0/171 e 0/16 altrove). E anche se passa, a 10s serve il porting del rung (4.7–13.8s → slice 0.5–1.0s, il design di `mode26-rung-anatomy` §3); senza, l'EV a 10s è marginale. Lo dico prima, non dopo.

  ## 2. La risposta onesta alla domanda vera

  Se il requisito è **avvicinare** (qualche mm, deterministico): la direzione sopra è l'unica con evidenza in-band non ritirata, e va spesa una volta sola.

  Se il requisito è **150.165 @10s**: su questo box/contratto/definizione di budget non esiste. Non è prudenza retorica: è l'aritmetica del §0 — il target è sotto l'asintoto dimostrato a budget infinito. Cosa riapre il gioco, in ordine di evidenza:

  1. **Definizione del budget / persistenza.** Il protocollo ha già lo slot warm-start (`record-line-cascade/README.md:35` lo dichiara vuoto per regola della campagna, non del motore). Popolarlo da un archivio per-geometria mette la linea certificata 155.264 — replay bit-exact su binario default (`record-line-cascade` §7) — a 10s domani, con zero ricerca. È l'unico cambiamento che *dimostrabilmente* muove il numero di 13+mm. E resta 5.1mm sopra Sparrow: nemmeno questo pareggia.
  2. **Rappresentazione.** L'autorità esatta a griglia canonica con contatto a margine 0.0 è il certificato di morte di contact-block ("`EnvelopePair` rows carry `contract_mm = 0.0`", `contact-block/README.md:250-257`; la variante a margine ε>0 è nominata e non tentata, `:407-413`) e il reticolo angolare 2.5° (`general_relaxed.rs:75`) è la tassa d'ingresso irrecuperabile di `record-line-cascade` §4. Un'autorità continua a margine reale riapre l'intera famiglia di metodi continui — oggi chiusa per costruzione, non per inferiorità misurata. Rischio di ricerca vero; il contratto di determinismo va riscritto.
  3. **Non hardware**: eval/s già pari da mesi; lanes=8 è più lento. L'unico trucco adiacente (best-of-N processi sullo spread di seed) vale 1–3mm, non 18.

  Nota a margine, da verificare: il lower bound 131.98 citato come invariante appartiene al **vecchio** contratto 5.5/5.25 (`depth-lower-bound-evidence.json:9-11`); dopo la ricalibrazione del contratto (`next-generation-engine-plan.md:730-757`) la costruzione stessa dà 124.887. Andrebbe ri-pinnato prima di usarlo in argomenti.

  ## Confronto con i documenti riservati (letti dopo aver fissato il parere)

  - **Grok-5**: concordo e indurisco. Il suo "150@10s richiede un'idea fuori dallo spazio operatori" e il tetto onesto ~166 sono la stessa tesi del mio §0; aggiungo che il target è sotto l'asintoto a *qualsiasi* budget, non solo a 10s. **Divergenza sostanziale**: Grok-5 archivia arm C ("ladder forzata post phase-0") perché il suo contesto era archivio saturo dopo ~16s di v2 e ~21M unità (`grok-review-5:29`) — ma quello è un argomento di *precondizione della cella misurata*, non un negativo matched-arm dalla banda; la batteria su parents12 è esattamente ciò che distingue "precondizione vera" da "mai provato".
  - **Sol-10**: la sua "nuova azione" (contact-block SE(2)) è stata costruita ed è fallita su entrambe le clausole — HEAD `5ccab1c`. La mia direzione è la sua **spesa #2, mai eseguita** perché l'owner scelse prima la #3; ci arrivo indipendentemente (barrier-crossing come risorsa scarsa, §0) e gli correggo il prezzo dentro il gate (l'85% di abort ULP, che Sol non cita). Sulla sua spesa #1 (hard-wall governor): concordo con l'esclusione dell'owner — a 10s vale 0.000mm (`shipped-surface.md` §3.2).
  - **shipped-surface.md**: verificato — m26 non è sulla retired board; la direzione non viola il vincolo del brief.
  - Onestà di percorso: prima di leggere i riservati la mia bozza convergeva col lifecycle non-monotono di `plan:299`/mode-16 (sonde rotazionali dentro separazione overlap-mediata). L'ho scartata come direzione perché l'effetto misurato è micron (168.277→168.275) contro i −4.3/−5.0mm di arm C nella banda giusta; resta la linea di ricerca corretta se l'audition m26 passa il gate e si fa il porting.